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
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tower_http::cors::CorsLayer;

const UI_HTML: &str = include_str!("../../ui/index.html");
const UI_CSS: &str = include_str!("../../ui/styles.css");
const UI_JS: &str = include_str!("../../ui/app.js");

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub index: Arc<Mutex<DocumentIndex>>,
    pub ai: Arc<AiEngine>,
    pub events_tx: broadcast::Sender<String>,
    pub api_token: Option<String>,
    pub session_token: String,
    pub is_remote: bool,
}

pub fn generate_secure_token() -> String {
    let mut buf = [0u8; 32];
    if getrandom::getrandom(&mut buf).is_ok() {
        return buf.iter().map(|b| format!("{:02x}", b)).collect();
    }
    use sha2::{Digest, Sha256};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut hasher = Sha256::new();
    hasher.update(b"axel_secure_token_fallback_");
    hasher.update(now.to_le_bytes());
    hasher.update(pid.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn is_request_authorized(
    headers: &axum::http::HeaderMap,
    expected_api_token: Option<&str>,
    expected_session_token: &str,
) -> bool {
    // 1. Authorization: Bearer <token>
    if let Some(auth_val) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth_val.strip_prefix("Bearer ") {
            let t = token.trim();
            if t == expected_session_token || expected_api_token.map_or(false, |k| t == k) {
                return true;
            }
        }
    }

    // 2. X-API-Key: <token>
    if let Some(key_val) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let t = key_val.trim();
        if t == expected_session_token || expected_api_token.map_or(false, |k| t == k) {
            return true;
        }
    }

    // 3. Cookie: axel_session=<token>
    if let Some(cookie_hdr) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for cookie in cookie_hdr.split(';') {
            let cookie = cookie.trim();
            if let Some(val) = cookie.strip_prefix("axel_session=") {
                if val == expected_session_token {
                    return true;
                }
            }
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

    // H01 & H02: Require authorization for all API routes
    if !is_request_authorized(
        req.headers(),
        state.api_token.as_deref(),
        &state.session_token,
    ) {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"error":"Unauthorized: Valid API token or session required"}"#))
            .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response());
    }

    next.run(req).await
}

pub fn create_router(state: AppState) -> Router {
    let allowed_origins_list = state.config.allowed_origins.clone();
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
        .route("/api/graph", get(get_graph))
        .route("/api/documents", get(list_documents))
        .route("/api/documents/:hash", get(get_document_detail).delete(delete_document))
        .route("/api/documents/:hash/raw", get(get_document_raw))
        .route("/api/search", get(search_documents))
        .route("/api/stats", get(get_stats))
        .route("/api/scan", post(trigger_scan))
        .route("/api/upload", post(upload_document))
        .route("/api/events", get(sse_events))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
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

    let state = AppState {
        config: config.clone(),
        index,
        ai,
        events_tx,
        api_token: api_token.clone(),
        session_token: session_token.clone(),
        is_remote,
    };

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
    let watch_dir = state.config.input_folder.clone();
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
                                index.upsert(dup_item);
                                match index.save() {
                                    Ok(_) => true,
                                    Err(e) => {
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

                    if let Some(item) = analyze_and_build_item(&path, &state.ai).await {
                        let file_name = item.file.clone();
                        let save_ok = {
                            let mut index = state.index.lock().await;
                            index.upsert(item);
                            match index.save() {
                                Ok(_) => true,
                                Err(e) => {
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
                        if index.remove_path(&path) {
                            let _ = index.save();
                            true
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
    let matches_session = t == state.session_token;
    if !matches_api && !matches_session {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credential".to_string()));
    }

    let cookie = format!(
        "axel_session={}; Path=/; HttpOnly; SameSite=Strict",
        state.session_token
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
            state.session_token
        )))
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

    let workspace = state.config.input_folder.clone();
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
    let outcome = {
        let mut index = state.index.lock().await;
        delete_document_from_index(
            &hash,
            params.delete_file.unwrap_or(false),
            params.permanent.unwrap_or(false),
            &state.config,
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

    // H02, M03: Resolve verified live path with workspace containment and SHA-256 hash validation
    let valid_path = crate::engine::resolve_verified_document_path(&item, &state.config.input_folder)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "File missing on disk or security verification failed (file hash modified / outside workspace)".to_string(),
            )
        })?;

    // H11: Check file size ceiling (100 MB) BEFORE reading into memory
    let meta = tokio::fs::metadata(&valid_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to stat file: {e}")))?;

    const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
    if meta.len() > MAX_DOWNLOAD_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "File size ({} MB) exceeds single-response download ceiling of 100 MB",
                meta.len() / (1024 * 1024)
            ),
        ));
    }

    let file = tokio::fs::File::open(&valid_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to open file: {e}")))?;

    let stream = tokio_util::io::ReaderStream::new(file);
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
        .header(header::CONTENT_LENGTH, meta.len().to_string())
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
    let limit = params.limit.unwrap_or(state.config.top_k_results);

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
    let index = state.index.lock().await;
    Json(build_system_stats(&state.config, &index))
}

async fn trigger_scan(State(state): State<AppState>) -> Result<Json<ScanSummary>, (StatusCode, String)> {
    let existing_hashes: HashSet<String> = {
        let index = state.index.lock().await;
        index.all().iter().map(|i| i.hash.clone()).collect()
    };

    let (summary, items_to_insert) =
        scan_folder_into_items(&state.config.input_folder, &existing_hashes, &state.ai, false)
            .await;

    if !items_to_insert.is_empty() {
        let mut index = state.index.lock().await;
        for item in items_to_insert {
            index.upsert(item);
        }
        index.save().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to persist index after scan: {e}"),
            )
        })?;
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
    if !state.config.input_folder.exists() {
        let _ = std::fs::create_dir_all(&state.config.input_folder);
    }

    let mut uploaded = Vec::new();
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

        let safe_name = Path::new(&raw_name)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty() && *s != "." && *s != "..")
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid upload filename".to_string()))?
            .to_string();

        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to read upload stream: {e}")))?;

        // H13: Enforce upload size ceiling (100 MB)
        const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;
        if data.len() > MAX_UPLOAD_BYTES {
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

        // H04: Atomic file publishing without replacement using exclusive creation handles
        let exclusive = crate::engine::create_exclusive_workspace_file(
            &state.config.input_folder,
            &safe_name,
            Some(&upload_hash),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create destination file: {e}")))?;

        let final_target_path = exclusive.path;
        let final_file_name = exclusive.file_name;

        if !exclusive.reused_existing {
            use std::io::Write;
            let mut file = exclusive.file;
            if let Err(e) = file.write_all(&data) {
                let _ = std::fs::remove_file(&final_target_path);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to write upload data: {e}"),
                ));
            }
            if let Err(e) = file.flush() {
                let _ = std::fs::remove_file(&final_target_path);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to flush upload data: {e}"),
                ));
            }
        }

        // M15 & H15: Run unified analysis (handling both code files and document files)
        let item = if let Some(analyzed) = analyze_and_build_item(&final_target_path, &state.ai).await {
            analyzed
        } else {
            let (text, _) = extract_text(&final_target_path);
            let ai_res = state.ai.analyze_document(&text, &final_file_name, None).await;
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
        if uploaded.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Failed to parse multipart upload stream: {err}"),
            ));
        }
    }

    if uploaded.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No file provided in multipart upload".to_string(),
        ));
    }

    {
        let mut index = state.index.lock().await;
        for item in &uploaded {
            index.upsert(item.clone());
        }
        index.save().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to persist index after upload: {e}"),
            )
        })?;
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
