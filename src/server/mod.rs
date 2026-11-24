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
}

pub async fn run_server(config: Config, open_browser: bool) -> Result<(), Box<dyn std::error::Error>> {
    let port = config.port;
    let index = Arc::new(Mutex::new(DocumentIndex::load(&config.index_file)));
    let ai = Arc::new(AiEngine::new(&config));
    let (events_tx, _) = broadcast::channel::<String>(64);

    let state = AppState {
        config: config.clone(),
        index,
        ai,
        events_tx,
    };

    spawn_background_watcher(state.clone());

    let app = Router::new()
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
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let local_url = format!("http://localhost:{port}");
    println!("\n=======================================================");
    println!("  AXEL Headless Server & REST API running at: {local_url}");
    println!("  Graph API: {local_url}/api/graph");
    println!("=======================================================\n");

    if open_browser {
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

async fn serve_ui() -> impl IntoResponse {
    static_asset_response(UI_HTML, "text/html; charset=utf-8")
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
        "hash": outcome.hash,
        "file": outcome.file,
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
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "File missing on disk".to_string()));
    }

    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read file: {e}")))?;

    let content_type = match path
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
    let limit = params.limit.unwrap_or(20);

    let index = state.index.lock().await;
    let results = index.search(&q, limit);

    Json(SearchApiResponse {
        query: q,
        total: results.len(),
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

        let target_path = state.config.input_folder.join(&safe_name);
        tokio::fs::write(&target_path, &data)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {e}")))?;

        let hash = DocumentIndex::compute_file_hash(&target_path).ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to compute file hash".to_string(),
            )
        })?;

        let (text, _) = extract_text(&target_path);
        let ai_res = state.ai.analyze_document(&text, &safe_name, None).await;

        let item = IndexItem::new(
            safe_name,
            ai_res.category,
            ai_res.keywords,
            Vec::new(),
            target_path.display().to_string(),
            hash,
            ai_res.virtual_paths,
        );

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
