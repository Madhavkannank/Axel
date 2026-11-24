use crate::ai::AiEngine;
use crate::config::Config;
use crate::engine::{
    analyze_and_build_item, build_graph_snapshot, build_system_stats,
    delete_document_from_index, scan_folder_into_items, DocumentDetail, GraphSnapshot, ScanSummary,
    SystemStats,
};
use crate::extractor::extract_text;
use crate::index::{DocumentIndex, IndexItem};
use crate::watcher::FolderWatcher;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path as AxumPath, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json, Response,
    },
    routing::{get, post},
    Router,
};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::CorsLayer;

const UI_HTML: &str = include_str!("../../ui/index.html");
const UI_CSS: &str = include_str!("../../ui/styles.css");
const UI_JS: &str = include_str!("../../ui/app.js");

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<std::sync::RwLock<Config>>,
    pub index: Arc<Mutex<DocumentIndex>>,
    pub ai: Arc<std::sync::RwLock<Arc<AiEngine>>>,
    pub events_tx: broadcast::Sender<String>,
    pub api_token: Option<String>,
    pub session_token: String,
    pub is_remote: bool,
    pub remote_sessions: Arc<tokio::sync::RwLock<std::collections::HashMap<String, std::time::Instant>>>,
}

impl AppState {
    pub fn new(
        config: Config,
        index: Arc<Mutex<DocumentIndex>>,
        ai: Arc<AiEngine>,
        events_tx: broadcast::Sender<String>,
        api_token: Option<String>,
        session_token: String,
        is_remote: bool,
    ) -> Self {
        Self {
            config: Arc::new(std::sync::RwLock::new(config)),
            index,
            ai: Arc::new(std::sync::RwLock::new(ai)),
            events_tx,
            api_token,
            session_token,
            is_remote,
            remote_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn get_config(&self) -> Config {
        self.config.read().unwrap().clone()
    }

    pub fn get_ai(&self) -> Arc<AiEngine> {
        self.ai.read().unwrap().clone()
    }
}

pub fn generate_secure_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf)
        .expect("FATAL: Failed to acquire cryptographically secure random bytes from OS entropy source");
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn extract_token_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    // 1. Authorization: Bearer <token>
    if let Some(auth_val) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth_val.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
    }

    // 2. X-API-Key: <token>
    if let Some(key_val) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        return Some(key_val.trim().to_string());
    }

    // 3. Cookie: axel_session=<token>
    if let Some(cookie_hdr) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for cookie in cookie_hdr.split(';') {
            let cookie = cookie.trim();
            if let Some(val) = cookie.strip_prefix("axel_session=") {
                return Some(val.trim().to_string());
            }
        }
    }

    None
}

pub fn is_request_authorized(
    headers: &axum::http::HeaderMap,
    expected_api_token: Option<&str>,
    expected_session_token: &str,
) -> bool {
    if let Some(token) = extract_token_from_headers(headers) {
        if token == expected_session_token || expected_api_token.map_or(false, |k| token == k) {
            return true;
        }
    }
    false
}

async fn auth_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = req.uri().path();

    // Static assets, UI and auth session login are accessible without prior token
    if path == "/" || path == "/styles.css" || path == "/app.js" || path == "/api/auth/session" {
        return next.run(req).await;
    }

    // H01 & H02 & A10: Require authorization for all API routes with session lifecycle
    let is_authed = if let Some(token) = extract_token_from_headers(req.headers()) {
        if state.api_token.as_deref().map_or(false, |k| token == k) {
            true
        } else if !state.is_remote && token == state.session_token {
            true
        } else {
            let sessions = state.remote_sessions.read().await;
            if let Some(&expires_at) = sessions.get(&token) {
                std::time::Instant::now() < expires_at
            } else {
                false
            }
        }
    } else {
        false
    };

    if !is_authed {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"error":"Unauthorized: Valid API token or active session required"}"#))
            .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response());
    }

    next.run(req).await
}

pub fn create_router(state: AppState) -> Router {
    let allowed_origins_list = state.get_config().allowed_origins.clone();
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            move |origin: &HeaderValue, _parts: &axum::http::request::Parts| {
                if let Ok(orig_str) = origin.to_str() {
                    if let Ok(url) = reqwest::Url::parse(orig_str) {
                        let scheme = url.scheme();
                        if scheme == "http" || scheme == "https" {
                            if let Some(host) = url.host_str() {
                                if host == "localhost"
                                    || host == "127.0.0.1"
                                    || host == "::1"
                                    || host == "[::1]"
                                {
                                    return true;
                                }
                            }
                        }
                    }
                    if allowed_origins_list.iter().any(|allowed| orig_str == allowed) {
                        return true;
                    }
                }
                false
            },
        ))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::COOKIE,
            axum::http::HeaderName::from_static("x-api-key"),
        ]);

    Router::new()
        .route("/", get(serve_ui))
        .route("/styles.css", get(serve_css))
        .route("/app.js", get(serve_js))
        .route("/api/auth/session", post(create_auth_session))
        .route("/api/auth/logout", post(logout_session))
        .route("/api/graph", get(get_graph))
        .route("/api/documents", get(list_documents))
        .route("/api/documents/:hash", get(get_document_detail).delete(delete_document))
        .route("/api/documents/:hash/raw", get(get_document_raw))
        .route("/api/search", get(search_documents))
        .route("/api/stats", get(get_stats))
        .route("/api/scan", post(trigger_scan))
        .route("/api/upload", post(upload_document))
        .route("/api/events", get(sse_events))
        .route("/api/settings", get(get_settings).post(update_settings))
        .route("/api/settings/test-key", post(test_api_key_endpoint))
        .layer(DefaultBodyLimit::max(110 * 1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(cors)
        .with_state(state)
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

pub async fn run_server(config: Config, open_browser: bool) -> Result<(), Box<dyn std::error::Error>> {
    let port = config.port;
    let host = config.host.clone();
    let is_remote = host != "127.0.0.1" && host != "localhost" && host != "::1";
    let api_token = config.api_token.clone();
    let session_token = generate_secure_token();

    let index = Arc::new(Mutex::new(DocumentIndex::load(&config.index_file)));
    let ai = Arc::new(AiEngine::new(&config));
    let (events_tx, _) = broadcast::channel::<String>(64);

    let state = AppState::new(
        config.clone(),
        index,
        ai,
        events_tx,
        api_token.clone(),
        session_token.clone(),
        is_remote,
    );

    spawn_background_watcher(state.clone());

    let app = create_router(state);

    let addr = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let bound_port = listener.local_addr()?.port();

    let local_url = if host == "0.0.0.0" || host == "::" {
        format!("http://localhost:{bound_port}")
    } else if host.contains(':') && !host.starts_with('[') {
        format!("http://[{host}]:{bound_port}")
    } else {
        format!("http://{host}:{bound_port}")
    };

    println!("\n=======================================================");
    println!("  AXEL Server running at: {local_url}");
    if is_remote {
        println!("  [SECURITY NOTICE] Server is bound to remote host '{host}'");
        if api_token.is_none() {
            println!("  [INFO] Ephemeral remote session token generated (masked):");
            println!("         {}", mask_token(&session_token));
            println!("  Set AXEL_API_KEY environment variable for persistent credentials.");
        }
    } else {
        println!("  Loopback interface active ({host}).");
        println!("  Session token: {}", mask_token(&session_token));
    }
    println!("  Web UI:    {local_url}/");
    println!("  Graph API: {local_url}/api/graph");
    println!("=======================================================\n");

    if open_browser && !is_remote {
        let _ = open::that(&local_url);
    }

    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_background_watcher(state: AppState) {
    let watch_dir = state.get_config().input_folder.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<crate::watcher::WatchEvent>();

    std::thread::spawn(move || {
        let watcher = match FolderWatcher::new(&watch_dir) {
            Ok(w) => w,
            Err(err) => {
                eprintln!("[Watcher] Disabled: {err}");
                return;
            }
        };

        loop {
            while let Some(event) = watcher.poll_next_event() {
                if tx.send(event).is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(450));
        }
    });

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                crate::watcher::WatchEvent::CreatedOrModified(path) => {
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    if !path.exists() || !path.is_file() {
                        continue;
                    }

                    let hash = match DocumentIndex::compute_file_hash(&path) {
                        Some(h) => h,
                        None => continue,
                    };

                    let path_str = path.display().to_string();
                    let is_duplicate_download = {
                        let mut index = state.index.lock().await;
                        if let Some(existing) = index.find_by_hash(&hash) {
                            if !existing.physical_paths.contains(&path_str) {
                                let mut dup_item = existing.clone();
                                dup_item.path = path_str.clone();
                                let backup_index = (*index).clone();
                                index.upsert(dup_item);
                                match index.save() {
                                    Ok(_) => true,
                                    Err(e) => {
                                        *index = backup_index;
                                        eprintln!("[Watcher] Failed to save index: {e}");
                                        false
                                    }
                                }
                            } else {
                                // Already known physical path
                                continue;
                            }
                        } else {
                            false
                        }
                    };

                    if is_duplicate_download {
                        emit_index_event(&state.events_tx, "duplicate_detected", path.file_name().and_then(|s| s.to_str()));
                        continue;
                    }

                    let ai = state.get_ai();
                    if let Some(item) = analyze_and_build_item(&path, &ai).await {
                        let file_name = item.file.clone();
                        let save_ok = {
                            let mut index = state.index.lock().await;
                            let backup_index = (*index).clone();
                            index.upsert(item);
                            match index.save() {
                                Ok(_) => true,
                                Err(e) => {
                                    *index = backup_index;
                                    eprintln!("[Watcher] Failed to save index: {e}");
                                    false
                                }
                            }
                        };
                        if save_ok {
                            emit_index_event(&state.events_tx, "watcher", Some(&file_name));
                        }
                    }
                }
                crate::watcher::WatchEvent::Removed(path) => {
                    let changed = {
                        let mut index = state.index.lock().await;
                        let backup_index = (*index).clone();
                        if index.remove_path(&path) {
                            match index.save() {
                                Ok(_) => true,
                                Err(e) => {
                                    *index = backup_index;
                                    eprintln!("[Watcher] Failed to save index on remove: {e}");
                                    false
                                }
                            }
                        } else {
                            false
                        }
                    };
                    if changed {
                        emit_index_event(&state.events_tx, "file_removed", path.file_name().and_then(|s| s.to_str()));
                    }
                }
            }
        }
    });
}

fn emit_index_event(tx: &broadcast::Sender<String>, reason: &str, file: Option<&str>) {
    let payload = serde_json::json!({
        "type": "index_updated",
        "reason": reason,
        "file": file,
    });
    let _ = tx.send(payload.to_string());
}

fn static_asset_response(body: &'static str, content_type: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .header(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn serve_ui(State(state): State<AppState>) -> impl IntoResponse {
    // Under H01: Do NOT include configured API key in public HTML!
    // In remote mode or if explicit token was configured, require bootstrap in UI:
    // Only pass browser session token if running locally on loopback without explicit secret key.
    let token_for_ui = if !state.is_remote && state.api_token.is_none() {
        state.session_token.as_str()
    } else {
        ""
    };

    let token_meta = if !token_for_ui.is_empty() {
        format!(r#"<meta name="axel-token" content="{}">"#, token_for_ui)
    } else {
        String::new()
    };

    let injected_html = UI_HTML.replace("<head>", &format!("<head>\n    {token_meta}"));
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
        .header(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));

    if !token_for_ui.is_empty() {
        if let Ok(cookie_val) = HeaderValue::from_str(&format!(
            "axel_session={}; Path=/; HttpOnly; SameSite=Strict",
            token_for_ui
        )) {
            resp = resp.header(header::SET_COOKIE, cookie_val);
        }
    }

    resp.body(Body::from(injected_html))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[derive(Deserialize)]
struct AuthSessionRequest {
    token: String,
}

async fn create_auth_session(
    State(state): State<AppState>,
    Json(payload): Json<AuthSessionRequest>,
) -> Result<Response, (StatusCode, String)> {
    let t = payload.token.trim();
    let matches_api = state.api_token.as_deref().map_or(false, |k| k == t);
    let matches_session = !state.is_remote && t == state.session_token;
    if !matches_api && !matches_session {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credential".to_string()));
    }

    // A10: Authenticated session lifecycle for remote clients with per-client token and expiration
    let client_token = generate_secure_token();
    let expires_at = std::time::Instant::now() + std::time::Duration::from_secs(86400); // 24h
    {
        let mut sessions = state.remote_sessions.write().await;
        let now = std::time::Instant::now();
        sessions.retain(|_, exp| *exp > now);
        sessions.insert(client_token.clone(), expires_at);
    }

    let cookie = format!(
        "axel_session={}; Path=/; HttpOnly; SameSite=Strict",
        client_token
    );

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
        )
        .body(Body::from(format!(
            r#"{{"authenticated":true,"session_token":"{}"}}"#,
            client_token
        )))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

async fn logout_session(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    if let Some(token) = extract_token_from_headers(&headers) {
        let mut sessions = state.remote_sessions.write().await;
        sessions.remove(&token);
    }
    let expired_cookie = "axel_session=; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT; HttpOnly; SameSite=Strict";
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::SET_COOKIE, HeaderValue::from_static(expired_cookie))
        .body(Body::from(r#"{"logged_out":true}"#))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()))
}

async fn serve_css() -> impl IntoResponse {
    static_asset_response(UI_CSS, "text/css; charset=utf-8")
}

async fn serve_js() -> impl IntoResponse {
    static_asset_response(UI_JS, "application/javascript; charset=utf-8")
}

async fn sse_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events_tx.subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(msg) => return Some((Ok(Event::default().data(msg)), rx)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn get_graph(State(state): State<AppState>) -> Json<GraphSnapshot> {
    let items = {
        let index = state.index.lock().await;
        index.all().to_vec()
    };
    Json(build_graph_snapshot(&items))
}

async fn list_documents(State(state): State<AppState>) -> Json<Vec<IndexItem>> {
    let index = state.index.lock().await;
    Json(index.all().to_vec())
}

async fn get_document_detail(
    State(state): State<AppState>,
    AxumPath(hash): AxumPath<String>,
) -> Result<Json<DocumentDetail>, (StatusCode, String)> {
    let item = {
        let index = state.index.lock().await;
        index
            .find_by_hash(&hash)
            .cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "Document not found".to_string()))?
    };

    let workspace = state.get_config().input_folder.clone();
    // Run verification & text extraction off the main Tokio executor to prevent thread blocking (H02, H11)
    let detail = tokio::task::spawn_blocking(move || {
        crate::engine::build_document_detail_verified(&item, &workspace)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Detail task panicked: {e}")))?;

    Ok(Json(detail))
}

#[derive(Deserialize)]
struct DeleteParams {
    delete_file: Option<bool>,
    permanent: Option<bool>,
}

async fn delete_document(
    State(state): State<AppState>,
    AxumPath(hash): AxumPath<String>,
    Query(params): Query<DeleteParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cfg = state.get_config();
    let outcome = {
        let mut index = state.index.lock().await;
        delete_document_from_index(
            &hash,
            params.delete_file.unwrap_or(false),
            params.permanent.unwrap_or(false),
            &cfg,
            &mut index,
        )
        .map_err(|e| {
            if e == "Document not found" {
                (StatusCode::NOT_FOUND, e)
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, e)
            }
        })?
    };

    emit_index_event(&state.events_tx, "delete", Some(&outcome.file));

    Ok(Json(serde_json::json!({
        "deleted": outcome.deleted,
        "index_deleted": outcome.index_deleted,
        "file_deleted": outcome.file_removed_from_disk,
        "deletion_method": outcome.disk_removal_method,
        "hash": outcome.hash,
        "file": outcome.file,
        "disk_error": outcome.error,
    })))
}

fn open_and_verify_document_handle(
    item: &IndexItem,
    workspace: &Path,
    max_bytes: u64,
) -> Result<(std::fs::File, PathBuf, u64), (StatusCode, String)> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek, SeekFrom};

    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.push(PathBuf::from(&item.path));
    for alias in &item.physical_paths {
        let ap = PathBuf::from(alias);
        if !candidates.contains(&ap) {
            candidates.push(ap);
        }
    }

    let mut found_oversized = false;
    let mut actual_len = 0u64;

    for cand in candidates {
        let resolved = crate::config::resolve_workspace_path(&cand);
        if !resolved.exists() || !crate::engine::is_path_inside_workspace(&resolved, workspace) {
            continue;
        }

        let mut file = match std::fs::File::open(&resolved) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let meta = match file.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !meta.is_file() {
            continue;
        }

        let file_len = meta.len();
        if file_len > max_bytes {
            found_oversized = true;
            actual_len = file_len;
            continue;
        }

        // Validate hash on the open file handle before streaming
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut read_ok = true;
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    read_ok = false;
                    break;
                }
            }
        }

        if !read_ok {
            continue;
        }

        let computed_hash = format!("{:x}", hasher.finalize());
        if computed_hash == item.hash {
            if file.seek(SeekFrom::Start(0)).is_ok() {
                return Ok((file, resolved, file_len));
            }
        }
    }

    if found_oversized {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "File size ({} MB) exceeds single-response download ceiling of {} MB",
                actual_len / (1024 * 1024),
                max_bytes / (1024 * 1024)
            ),
        ));
    }

    Err((
        StatusCode::NOT_FOUND,
        "File missing on disk or security verification failed (file hash modified / outside workspace)".to_string(),
    ))
}

async fn get_document_raw(
    State(state): State<AppState>,
    AxumPath(hash): AxumPath<String>,
) -> Result<Response, (StatusCode, String)> {
    let item = {
        let index = state.index.lock().await;
        index
            .find_by_hash(&hash)
            .cloned()
            .ok_or_else(|| (StatusCode::NOT_FOUND, "Document not found".to_string()))?
    };

    // A11: Check size, validate hash, and stream from a SINGLE open file handle without reopening
    const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
    let (std_file, valid_path, file_len) =
        open_and_verify_document_handle(&item, &state.get_config().input_folder, MAX_DOWNLOAD_BYTES)?;

    let tokio_file = tokio::fs::File::from_std(std_file);
    let stream = tokio_util::io::ReaderStream::new(tokio_file);
    let body = Body::from_stream(stream);

    let content_type = match valid_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("json") => "application/json; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("md") | Some("txt") => "text/plain; charset=utf-8",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        _ => "application/octet-stream",
    };

    let safe_name = item.file.replace('"', "");
    let disposition = format!("inline; filename=\"{safe_name}\"");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, file_len.to_string())
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Response error: {e}")))
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchApiResponse {
    query: String,
    total: usize,
    results: Vec<crate::index::SearchResult>,
}

async fn search_documents(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Json<SearchApiResponse> {
    let q = params.q.unwrap_or_default();
    let limit = params.limit.unwrap_or(state.get_config().top_k_results);

    let index = state.index.lock().await;
    let all_matches = index.search(&q, usize::MAX);
    let total = all_matches.len();
    let results: Vec<crate::index::SearchResult> = all_matches.into_iter().take(limit).collect();

    Json(SearchApiResponse {
        query: q,
        total,
        results,
    })
}

async fn get_stats(State(state): State<AppState>) -> Json<SystemStats> {
    let cfg = state.get_config();
    let index = state.index.lock().await;
    Json(build_system_stats(&cfg, &index))
}

async fn trigger_scan(State(state): State<AppState>) -> Result<Json<ScanSummary>, (StatusCode, String)> {
    let existing_hashes: HashSet<String> = {
        let index = state.index.lock().await;
        index.all().iter().map(|i| i.hash.clone()).collect()
    };

    let cfg = state.get_config();
    let ai = state.get_ai();
    let (summary, items_to_insert) =
        scan_folder_into_items(&cfg.input_folder, &existing_hashes, &ai, false)
            .await;

    if !items_to_insert.is_empty() {
        let mut index = state.index.lock().await;
        let backup_index = (*index).clone();
        for item in items_to_insert {
            index.upsert(item);
        }
        if let Err(e) = index.save() {
            *index = backup_index;
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to persist index after scan: {e}"),
            ));
        }
        emit_index_event(&state.events_tx, "scan", None);
    }

    Ok(Json(summary))
}

#[derive(Serialize)]
struct UploadResponse {
    count: usize,
    uploaded: Vec<IndexItem>,
}

async fn upload_document(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    let cfg = state.get_config();
    let ai = state.get_ai();

    if !cfg.input_folder.exists() {
        let _ = std::fs::create_dir_all(&cfg.input_folder);
    }

    let mut uploaded = Vec::new();
    let mut created_files_on_disk: Vec<PathBuf> = Vec::new();
    let mut next_field_error = None;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                next_field_error = Some(e);
                break;
            }
        };

        let raw_name = match field.file_name() {
            Some(name) => name.to_string(),
            None => continue,
        };

        let safe_name = match Path::new(&raw_name)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        {
            Some(s) => s.to_string(),
            None => {
                for p in &created_files_on_disk {
                    let _ = std::fs::remove_file(p);
                }
                return Err((StatusCode::BAD_REQUEST, "Invalid upload filename".to_string()));
            }
        };

        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                for p in &created_files_on_disk {
                    let _ = std::fs::remove_file(p);
                }
                return Err((StatusCode::BAD_REQUEST, format!("Failed to read upload stream: {e}")));
            }
        };

        // H13: Enforce upload size ceiling (100 MB per file)
        const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;
        if data.len() > MAX_UPLOAD_BYTES {
            for p in &created_files_on_disk {
                let _ = std::fs::remove_file(p);
            }
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "Uploaded file exceeds single-file size ceiling of 100 MB ({} MB received)",
                    data.len() / (1024 * 1024)
                ),
            ));
        }

        use sha2::Digest;
        let mut upload_hasher = sha2::Sha256::new();
        upload_hasher.update(&data);
        let upload_hash = format!("{:x}", upload_hasher.finalize());

        // A08: Stage file in hidden temporary staging file, flush & sync, then publish atomically
        let (mut staging_file, staging_path) = match crate::engine::create_workspace_staging_file(&cfg.input_folder) {
            Ok(res) => res,
            Err(e) => {
                for p in &created_files_on_disk {
                    let _ = std::fs::remove_file(p);
                }
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create staging file: {e}"),
                ));
            }
        };

        use std::io::Write;
        if let Err(e) = staging_file.write_all(&data).and_then(|_| staging_file.flush()).and_then(|_| staging_file.sync_all()) {
            let _ = std::fs::remove_file(&staging_path);
            for p in &created_files_on_disk {
                let _ = std::fs::remove_file(p);
            }
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write upload data to staging file: {e}"),
            ));
        }
        drop(staging_file);

        let published = match crate::engine::publish_staged_workspace_file(
            &staging_path,
            &cfg.input_folder,
            &safe_name,
            Some(&upload_hash),
            &cfg.input_folder,
        ) {
            Ok(res) => res,
            Err(e) => {
                let _ = std::fs::remove_file(&staging_path);
                for p in &created_files_on_disk {
                    let _ = std::fs::remove_file(p);
                }
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to publish staged upload: {e}"),
                ));
            }
        };

        let final_target_path = published.path;
        let final_file_name = published.file_name;

        if !published.reused_existing {
            created_files_on_disk.push(final_target_path.clone());
        }

        // M15 & H15: Run unified analysis (handling both code files and document files)
        let item = if let Some(analyzed) = analyze_and_build_item(&final_target_path, &ai).await {
            analyzed
        } else {
            let (text, _) = extract_text(&final_target_path);
            let ai_res = ai.analyze_document(&text, &final_file_name, None).await;
            IndexItem::new(
                final_file_name,
                ai_res.category,
                ai_res.keywords,
                Vec::new(),
                final_target_path.display().to_string(),
                upload_hash,
                ai_res.virtual_paths,
            )
        };

        uploaded.push(item);
    }

    if let Some(err) = next_field_error {
        for p in &created_files_on_disk {
            let _ = std::fs::remove_file(p);
        }
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Failed to parse multipart upload stream: {err}"),
        ));
    }

    if uploaded.is_empty() {
        for p in &created_files_on_disk {
            let _ = std::fs::remove_file(p);
        }
        return Err((
            StatusCode::BAD_REQUEST,
            "No file provided in multipart upload".to_string(),
        ));
    }

    // A13: Atomic index mutation and disk persistence rollback
    let save_res = {
        let mut index = state.index.lock().await;
        let backup_index = (*index).clone();
        for item in &uploaded {
            index.upsert(item.clone());
        }
        if let Err(e) = index.save() {
            *index = backup_index;
            Err(e)
        } else {
            Ok(())
        }
    };

    if let Err(e) = save_res {
        for p in &created_files_on_disk {
            let _ = std::fs::remove_file(p);
        }
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to persist index after upload: {e}"),
        ));
    }

    emit_index_event(
        &state.events_tx,
        "upload",
        uploaded.first().map(|i| i.file.as_str()),
    );

    Ok(Json(UploadResponse {
        count: uploaded.len(),
        uploaded,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub groq_api_key_configured: bool,
    pub groq_api_key_masked: Option<String>,
    pub model_name: String,
    pub model_fallbacks: Vec<String>,
    pub input_folder: String,
    pub index_file: String,
    pub max_keywords: usize,
    pub top_k_results: usize,
    pub port: u16,
    pub host: String,
    pub force_offline: bool,
    pub enable_cloud_code: bool,
    pub enable_ocr: bool,
    pub max_file_size_mb: usize,
    pub excluded_patterns: Vec<String>,
    pub theme: String,
    pub has_seen_onboarding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    pub groq_api_key: Option<String>,
    pub model_name: Option<String>,
    pub model_fallbacks: Option<Vec<String>>,
    pub force_offline: Option<bool>,
    pub enable_cloud_code: Option<bool>,
    pub enable_ocr: Option<bool>,
    pub max_file_size_mb: Option<usize>,
    pub excluded_patterns: Option<Vec<String>>,
    pub theme: Option<String>,
    pub has_seen_onboarding: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestKeyRequest {
    pub api_key: Option<String>,
}

async fn get_settings(State(state): State<AppState>) -> Json<SettingsResponse> {
    let cfg = state.get_config();
    let groq_api_key_configured = cfg.groq_api_key.as_ref().map_or(false, |k| !k.is_empty());
    let groq_api_key_masked = cfg.groq_api_key.as_ref().map(|k| mask_token(k));

    Json(SettingsResponse {
        groq_api_key_configured,
        groq_api_key_masked,
        model_name: cfg.model_name,
        model_fallbacks: cfg.model_fallbacks,
        input_folder: cfg.input_folder.display().to_string(),
        index_file: cfg.index_file.display().to_string(),
        max_keywords: cfg.max_keywords,
        top_k_results: cfg.top_k_results,
        port: cfg.port,
        host: cfg.host,
        force_offline: cfg.force_offline,
        enable_cloud_code: cfg.enable_cloud_code,
        enable_ocr: cfg.enable_ocr,
        max_file_size_mb: cfg.max_file_size_mb,
        excluded_patterns: cfg.excluded_patterns,
        theme: cfg.theme,
        has_seen_onboarding: cfg.has_seen_onboarding,
    })
}

async fn update_settings(
    State(state): State<AppState>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, (StatusCode, String)> {
    let mut ai_needs_reload = false;
    let updated_config = {
        let mut cfg = state.config.write().map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Config lock poisoned: {e}"))
        })?;

        if let Some(key) = payload.groq_api_key {
            let trimmed = key.trim().to_string();
            if trimmed.is_empty() {
                cfg.groq_api_key = None;
            } else {
                cfg.groq_api_key = Some(trimmed);
            }
            ai_needs_reload = true;
        }

        if let Some(model) = payload.model_name {
            if !model.trim().is_empty() {
                cfg.model_name = model.trim().to_string();
                ai_needs_reload = true;
            }
        }

        if let Some(fallbacks) = payload.model_fallbacks {
            cfg.model_fallbacks = fallbacks;
            ai_needs_reload = true;
        }

        if let Some(offline) = payload.force_offline {
            cfg.force_offline = offline;
        }

        if let Some(cloud_code) = payload.enable_cloud_code {
            cfg.enable_cloud_code = cloud_code;
        }

        if let Some(ocr) = payload.enable_ocr {
            cfg.enable_ocr = ocr;
        }

        if let Some(max_size) = payload.max_file_size_mb {
            cfg.max_file_size_mb = max_size;
        }

        if let Some(patterns) = payload.excluded_patterns {
            cfg.excluded_patterns = patterns;
        }

        if let Some(theme) = payload.theme {
            cfg.theme = theme;
        }

        if let Some(seen) = payload.has_seen_onboarding {
            cfg.has_seen_onboarding = seen;
        }

        if let Err(e) = cfg.save() {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to persist settings: {e}"),
            ));
        }

        cfg.clone()
    };

    if ai_needs_reload {
        let new_ai = Arc::new(AiEngine::new(&updated_config));
        if let Ok(mut ai_lock) = state.ai.write() {
            *ai_lock = new_ai;
        }
    }

    emit_index_event(&state.events_tx, "settings_updated", None);

    let groq_api_key_configured = updated_config.groq_api_key.as_ref().map_or(false, |k| !k.is_empty());
    let groq_api_key_masked = updated_config.groq_api_key.as_ref().map(|k| mask_token(k));

    Ok(Json(SettingsResponse {
        groq_api_key_configured,
        groq_api_key_masked,
        model_name: updated_config.model_name,
        model_fallbacks: updated_config.model_fallbacks,
        input_folder: updated_config.input_folder.display().to_string(),
        index_file: updated_config.index_file.display().to_string(),
        max_keywords: updated_config.max_keywords,
        top_k_results: updated_config.top_k_results,
        port: updated_config.port,
        host: updated_config.host,
        force_offline: updated_config.force_offline,
        enable_cloud_code: updated_config.enable_cloud_code,
        enable_ocr: updated_config.enable_ocr,
        max_file_size_mb: updated_config.max_file_size_mb,
        excluded_patterns: updated_config.excluded_patterns,
        theme: updated_config.theme,
        has_seen_onboarding: updated_config.has_seen_onboarding,
    }))
}

async fn test_api_key_endpoint(
    State(state): State<AppState>,
    Json(payload): Json<TestKeyRequest>,
) -> Result<Json<crate::ai::GroqKeyVerification>, (StatusCode, String)> {
    let key = if let Some(ref k) = payload.api_key {
        let trimmed = k.trim();
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else {
            state.get_config().groq_api_key.unwrap_or_default()
        }
    } else {
        state.get_config().groq_api_key.unwrap_or_default()
    };

    if key.is_empty() {
        return Ok(Json(crate::ai::GroqKeyVerification {
            valid: false,
            latency_ms: 0,
            available_models: Vec::new(),
            message: "No API key provided or configured".to_string(),
        }));
    }

    let verification = crate::ai::test_groq_api_key(&key).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(verification))
}
