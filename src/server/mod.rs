use crate::ai::AiEngine;
use crate::config::Config;
use crate::engine::{
    analyze_and_build_item, build_document_detail, build_graph_snapshot, build_system_stats,
    delete_document_from_index, scan_folder_into_items, DocumentDetail, GraphSnapshot, ScanSummary,
    SystemStats,
};
use crate::extractor::extract_text;
use crate::index::{DocumentIndex, IndexItem};
use crate::watcher::FolderWatcher;
use axum::{
    body::Body,
    extract::{Multipart, Path as AxumPath, Query, State},
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
    pub config: Config,
    pub index: Arc<Mutex<DocumentIndex>>,
    pub ai: Arc<AiEngine>,
    pub events_tx: broadcast::Sender<String>,
    pub session_token: String,
    pub is_explicit_token: bool,
    pub is_remote: bool,
}

pub fn generate_secure_token() -> String {
    use sha2::{Digest, Sha256};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut hasher = Sha256::new();
    hasher.update(b"axel_token_secret_");
    hasher.update(now.to_le_bytes());
    hasher.update(pid.to_le_bytes());
    let addr = &now as *const u128 as usize;
    hasher.update(addr.to_le_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn is_request_authorized(
    headers: &axum::http::HeaderMap,
    uri: &axum::http::Uri,
    expected_token: &str,
) -> bool {
    // 1. Authorization: Bearer <token>
    if let Some(auth_val) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth_val.strip_prefix("Bearer ") {
            if token.trim() == expected_token {
                return true;
            }
        }
    }

    // 2. X-API-Key: <token>
    if let Some(key_val) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        if key_val.trim() == expected_token {
            return true;
        }
    }

    // 3. Query parameter: ?token=<token>
    if let Some(query) = uri.query() {
        for pair in query.split('&') {
            if let Some(token) = pair.strip_prefix("token=") {
                if token == expected_token {
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
    let method = req.method();

    // Static assets and UI are always accessible
    if path == "/" || path == "/styles.css" || path == "/app.js" {
        return next.run(req).await;
    }

    let is_sensitive = path.ends_with("/raw")
        || path == "/api/scan"
        || path == "/api/upload"
        || *method == axum::http::Method::DELETE;

    // Remote binding and explicit token require auth for all API routes.
    // Sensitive mutating/download routes always require auth even on loopback (H01).
    let requires_auth = state.is_explicit_token || state.is_remote || is_sensitive;

    if requires_auth && !is_request_authorized(req.headers(), req.uri(), &state.session_token) {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"error":"Unauthorized: Valid API token required"}"#))
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
                    // Always allow localhost and loopback interfaces
                    if orig_str.starts_with("http://localhost")
                        || orig_str.starts_with("http://127.0.0.1")
                        || orig_str.starts_with("https://localhost")
                        || orig_str.starts_with("https://127.0.0.1")
                    {
                        return true;
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
            axum::http::HeaderName::from_static("x-api-key"),
        ]);

    Router::new()
        .route("/", get(serve_ui))
        .route("/styles.css", get(serve_css))
        .route("/app.js", get(serve_js))
        .route("/api/graph", get(get_graph))
        .route("/api/documents", get(list_documents))
        .route("/api/documents/:hash", get(get_document_detail).delete(delete_document))
        .route("/api/documents/:hash/raw", get(get_document_raw))
        .route("/api/search", get(search_documents))
        .route("/api/stats", get(get_stats))
        .route("/api/scan", post(trigger_scan))
        .route("/api/upload", post(upload_document))
        .route("/api/events", get(sse_events))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(cors)
        .with_state(state)
}

pub async fn run_server(config: Config, open_browser: bool) -> Result<(), Box<dyn std::error::Error>> {
    let port = config.port;
    let host = config.host.clone();
    let is_remote = host != "127.0.0.1" && host != "localhost" && host != "::1";
    let is_explicit_token = config.api_token.is_some();
    let session_token = config
        .api_token
        .clone()
        .unwrap_or_else(generate_secure_token);

    let index = Arc::new(Mutex::new(DocumentIndex::load(&config.index_file)));
    let ai = Arc::new(AiEngine::new(&config));
    let (events_tx, _) = broadcast::channel::<String>(64);

    let state = AppState {
        config: config.clone(),
        index,
        ai,
        events_tx,
        session_token: session_token.clone(),
        is_explicit_token,
        is_remote,
    };

    spawn_background_watcher(state.clone());

    let app = create_router(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let bound_port = listener.local_addr()?.port();

    let local_url = if host == "0.0.0.0" {
        format!("http://localhost:{bound_port}")
    } else {
        format!("http://{host}:{bound_port}")
    };

    println!("\n=======================================================");
    println!("  AXEL Server running at: {local_url}");
    if is_remote {
        println!("  [SECURITY NOTICE] Server is bound to remote host '{host}'");
        if !is_explicit_token {
            println!("  [WARN] No AXEL_API_KEY was specified. Generated remote session token:");
            println!("         {session_token}");
            println!("  All API requests from remote origins must include:");
            println!("         Header: Authorization: Bearer {session_token}");
        }
    } else {
        println!("  Loopback interface active (127.0.0.1).");
        println!("  Local session token: {session_token}");
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
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();

    std::thread::spawn(move || {
        let watcher = match FolderWatcher::new(&watch_dir) {
            Ok(w) => w,
            Err(err) => {
                eprintln!("[Watcher] Disabled: {err}");
                return;
            }
        };

        loop {
            while let Some(path) = watcher.poll_next_file() {
                if tx.send(path).is_err() {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(450));
        }
    });

    tokio::spawn(async move {
        while let Some(path) = rx.recv().await {
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
                        let _ = index.save();
                        true
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
                {
                    let mut index = state.index.lock().await;
                    index.upsert(item);
                    let _ = index.save();
                }
                emit_index_event(&state.events_tx, "watcher", Some(&file_name));
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
    let token_meta = format!(r#"<meta name="axel-token" content="{}">"#, state.session_token);
    let injected_html = UI_HTML.replace("<head>", &format!("<head>\n    {token_meta}"));
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))
        .header(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))
        .body(Body::from(injected_html))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
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

    Ok(Json(build_document_detail(&item)))
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

    let path = Path::new(&item.path);

    // H07: Validate workspace boundary
    if !crate::engine::is_path_inside_workspace(path, &state.config.input_folder) {
        return Err((
            StatusCode::FORBIDDEN,
            "Access denied: Target document path is outside workspace boundaries".to_string(),
        ));
    }

    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "File missing on disk".to_string()));
    }

    // H06: Verify disk file hash matches document hash to prevent serving modified/tampered bytes
    let valid_path = match DocumentIndex::compute_file_hash(path) {
        Some(ref chash) if chash == &hash => path.to_path_buf(),
        _ => {
            // Check registered physical aliases
            let mut found = None;
            for alias in &item.physical_paths {
                let apath = Path::new(alias);
                if apath.exists() && crate::engine::is_path_inside_workspace(apath, &state.config.input_folder) {
                    if let Some(ref ahash) = DocumentIndex::compute_file_hash(apath) {
                        if ahash == &hash {
                            found = Some(apath.to_path_buf());
                            break;
                        }
                    }
                }
            }
            found.ok_or_else(|| {
                (
                    StatusCode::CONFLICT,
                    "Security verification failed: File on disk has been modified and no longer matches indexed hash".to_string(),
                )
            })?
        }
    };

    // H13: Check file size ceiling (100 MB)
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

    let bytes = tokio::fs::read(&valid_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read file: {e}")))?;

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
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(bytes))
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

async fn trigger_scan(State(state): State<AppState>) -> Json<ScanSummary> {
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
        let _ = index.save();
        emit_index_event(&state.events_tx, "scan", None);
    }

    Json(summary)
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

    while let Ok(Some(field)) = multipart.next_field().await {
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

        let default_target = state.config.input_folder.join(&safe_name);

        let (final_target_path, final_file_name) = if default_target.exists() {
            // Check if existing file is already identical
            if let Some(existing_hash) = DocumentIndex::compute_file_hash(&default_target) {
                if existing_hash == upload_hash {
                    // Exact duplicate already exists at destination; reuse safely without overwriting
                    (default_target, safe_name)
                } else {
                    // Different file with the same name exists! Prevent overwrite data loss (H02).
                    crate::engine::resolve_collision_safe_path(&state.config.input_folder, &safe_name)
                }
            } else {
                crate::engine::resolve_collision_safe_path(&state.config.input_folder, &safe_name)
            }
        } else if std::fs::symlink_metadata(&default_target).is_ok() {
            // Existing symlink target! Prevent following symlink (H02)
            crate::engine::resolve_collision_safe_path(&state.config.input_folder, &safe_name)
        } else {
            (default_target, safe_name)
        };

        // If target file doesn't already exist with identical content, write via secure staging file
        if !final_target_path.exists() {
            static UPLOAD_STAGE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let count = UPLOAD_STAGE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let staging_name = format!(".tmp_upload_{}_{}_{}.tmp", std::process::id(), now, count);
            let staging_path = state.config.input_folder.join(&staging_name);

            // Write using create_new(true) to ensure we never follow existing symlinks
            let _ = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staging_path)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create upload staging file: {e}")))?;

            tokio::fs::write(&staging_path, &data)
                .await
                .map_err(|e| {
                    let _ = std::fs::remove_file(&staging_path);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write upload data: {e}"))
                })?;

            tokio::fs::rename(&staging_path, &final_target_path)
                .await
                .map_err(|e| {
                    let _ = std::fs::remove_file(&staging_path);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to finalize upload file: {e}"))
                })?;
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
        let _ = index.save();
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
