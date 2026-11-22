use crate::ai::AiEngine;
use crate::config::Config;
use crate::extractor::{detect_file_type, extract_text, FileType};
use crate::index::{DocumentIndex, IndexItem};
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub index: Arc<Mutex<DocumentIndex>>,
    pub ai: Arc<AiEngine>,
}

pub async fn run_server(config: Config, open_browser: bool) -> Result<(), Box<dyn std::error::Error>> {
    let port = config.port;
    let index = Arc::new(Mutex::new(DocumentIndex::load(&config.index_file)));
    let ai = Arc::new(AiEngine::new(&config));

    let state = AppState {
        config: config.clone(),
        index,
        ai,
    };

    let app = Router::new()
        .route("/", get(serve_ui))
        .route("/api/documents", get(list_documents))
        .route("/api/search", get(search_documents))
        .route("/api/stats", get(get_stats))
        .route("/api/scan", post(trigger_scan))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let local_url = format!("http://localhost:{port}");
    println!("\n=======================================================");
    println!("  AXEL UI & REST API Server running at: {local_url}");
    println!("  Dashboard: {local_url}");
    println!("  API Docs:  {local_url}/api/documents");
    println!("=======================================================\n");

    if open_browser {
        let _ = open::that(&local_url);
    }

    axum::serve(listener, app).await?;
    Ok(())
}

async fn serve_ui() -> impl IntoResponse {
    Html(UI_HTML)
}

async fn list_documents(State(state): State<AppState>) -> Json<Vec<IndexItem>> {
    let index = state.index.lock().await;
    Json(index.all().to_vec())
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

#[derive(Serialize)]
struct StatsResponse {
    total_documents: usize,
    groq_enabled: bool,
    active_model: String,
    input_folder: String,
    categories: std::collections::HashMap<String, usize>,
}

async fn get_stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let index = state.index.lock().await;
    let mut categories = std::collections::HashMap::new();

    for item in index.all() {
        *categories.entry(item.category.clone()).or_insert(0) += 1;
    }

    Json(StatsResponse {
        total_documents: index.count(),
        groq_enabled: state.config.groq_api_key.is_some(),
        active_model: state.config.model_name.clone(),
        input_folder: state.config.input_folder.display().to_string(),
        categories,
    })
}

#[derive(Serialize)]
struct ScanResultResponse {
    scanned: usize,
    indexed: usize,
    skipped: usize,
}

async fn trigger_scan(State(state): State<AppState>) -> Json<ScanResultResponse> {
    let folder = state.config.input_folder.clone();
    let mut scanned = 0;
    let mut indexed = 0;
    let mut skipped = 0;

    if folder.exists() && folder.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    scanned += 1;
                    let file_type = detect_file_type(&path);
                    if file_type == FileType::Unsupported {
                        skipped += 1;
                        continue;
                    }

                    let hash = match DocumentIndex::compute_file_hash(&path) {
                        Some(h) => h,
                        None => {
                            skipped += 1;
                            continue;
                        }
                    };

                    {
                        let index = state.index.lock().await;
                        if index.find_by_hash(&hash).is_some() {
                            skipped += 1;
                            continue;
                        }
                    }

                    let file_name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("document")
                        .to_string();

                    let (text, _) = extract_text(&path);
                    let ai_res = state.ai.analyze_document(&text, &file_name).await;

                    let item = IndexItem {
                        file: file_name,
                        category: ai_res.category,
                        keywords: ai_res.keywords,
                        path: path.display().to_string(),
                        hash,
                        indexed_at: Some(chrono::Utc::now().to_rfc3339()),
                    };

                    let mut index = state.index.lock().await;
                    index.upsert(item);
                    let _ = index.save();
                    indexed += 1;
                }
            }
        }
    }

    Json(ScanResultResponse {
        scanned,
        indexed,
        skipped,
    })
}

const UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>AXEL - Document Intelligence Engine</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #0b0f17;
      --surface: #121824;
      --surface-border: #1e293b;
      --surface-hover: #172133;
      --primary: #3b82f6;
      --primary-hover: #2563eb;
      --primary-glow: rgba(59, 130, 246, 0.25);
      --accent: #10b981;
      --text: #f8fafc;
      --text-muted: #94a3b8;
      --card-bg: rgba(18, 24, 36, 0.7);
      --card-border: rgba(255, 255, 255, 0.08);
      --font: 'Plus Jakarta Sans', -apple-system, BlinkMacSystemFont, sans-serif;
      --font-mono: 'JetBrains Mono', monospace;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg);
      color: var(--text);
      font-family: var(--font);
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      background-image:
        radial-gradient(at 0% 0%, rgba(59, 130, 246, 0.12) 0px, transparent 50%),
        radial-gradient(at 100% 100%, rgba(16, 185, 129, 0.08) 0px, transparent 50%);
    }
    header {
      padding: 1.25rem 2rem;
      border-bottom: 1px solid var(--surface-border);
      display: flex;
      justify-content: space-between;
      align-items: center;
      backdrop-filter: blur(12px);
      background: rgba(11, 15, 23, 0.8);
      position: sticky;
      top: 0;
      z-index: 50;
    }
    .logo {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-weight: 800;
      font-size: 1.25rem;
      letter-spacing: -0.02em;
    }
    .logo-badge {
      background: linear-gradient(135deg, var(--primary), var(--accent));
      color: white;
      padding: 0.2rem 0.6rem;
      border-radius: 6px;
      font-size: 0.8rem;
      font-weight: 700;
      box-shadow: 0 0 16px var(--primary-glow);
    }
    .status-bar {
      display: flex;
      align-items: center;
      gap: 1.5rem;
      font-size: 0.875rem;
      color: var(--text-muted);
    }
    .status-pill {
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.35rem 0.75rem;
      border-radius: 9999px;
      background: rgba(16, 185, 129, 0.1);
      color: #34d399;
      border: 1px solid rgba(16, 185, 129, 0.2);
      font-size: 0.75rem;
      font-weight: 600;
    }
    .status-dot {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: currentColor;
    }
    main {
      flex: 1;
      max-width: 1200px;
      width: 100%;
      margin: 0 auto;
      padding: 2.5rem 1.5rem;
      display: flex;
      flex-direction: column;
      gap: 2rem;
    }
    .hero {
      text-align: center;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 0.75rem;
    }
    .hero h1 {
      font-size: 2.25rem;
      font-weight: 800;
      letter-spacing: -0.03em;
    }
    .hero p {
      color: var(--text-muted);
      max-width: 600px;
      line-height: 1.5;
    }
    .search-box {
      width: 100%;
      max-width: 760px;
      margin: 0.5rem auto 0;
      position: relative;
    }
    .search-input {
      width: 100%;
      background: var(--surface);
      border: 1px solid var(--surface-border);
      border-radius: 12px;
      padding: 1.1rem 1.5rem 1.1rem 3.2rem;
      font-size: 1.05rem;
      color: var(--text);
      font-family: inherit;
      outline: none;
      transition: all 0.2s ease;
      box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
    }
    .search-input:focus {
      border-color: var(--primary);
      box-shadow: 0 0 0 3px var(--primary-glow);
    }
    .search-icon {
      position: absolute;
      left: 1.25rem;
      top: 50%;
      transform: translateY(-50%);
      color: var(--text-muted);
      pointer-events: none;
      font-size: 1.1rem;
    }
    .actions-row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 1rem;
    }
    .btn {
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.6rem 1.2rem;
      border-radius: 8px;
      font-size: 0.875rem;
      font-weight: 600;
      cursor: pointer;
      border: none;
      transition: all 0.2s ease;
      font-family: inherit;
    }
    .btn-primary {
      background: var(--primary);
      color: white;
    }
    .btn-primary:hover {
      background: var(--primary-hover);
    }
    .btn-outline {
      background: transparent;
      border: 1px solid var(--surface-border);
      color: var(--text);
    }
    .btn-outline:hover {
      background: var(--surface);
    }
    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 1rem;
    }
    .stat-card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 1.25rem;
      backdrop-filter: blur(8px);
    }
    .stat-label {
      font-size: 0.8rem;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.05em;
      font-weight: 600;
    }
    .stat-value {
      font-size: 1.75rem;
      font-weight: 800;
      margin-top: 0.4rem;
      letter-spacing: -0.02em;
    }
    .document-list {
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
    }
    .doc-card {
      background: var(--surface);
      border: 1px solid var(--surface-border);
      border-radius: 12px;
      padding: 1.25rem;
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
      transition: all 0.2s ease;
    }
    .doc-card:hover {
      border-color: rgba(59, 130, 246, 0.4);
      background: var(--surface-hover);
      transform: translateY(-2px);
    }
    .doc-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 0.5rem;
    }
    .doc-title {
      font-weight: 700;
      font-size: 1.05rem;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .doc-badge {
      background: rgba(59, 130, 246, 0.15);
      color: #60a5fa;
      border: 1px solid rgba(59, 130, 246, 0.3);
      padding: 0.2rem 0.6rem;
      border-radius: 6px;
      font-size: 0.75rem;
      font-weight: 700;
      text-transform: uppercase;
    }
    .doc-path {
      font-family: var(--font-mono);
      font-size: 0.75rem;
      color: var(--text-muted);
      word-break: break-all;
    }
    .doc-keywords {
      display: flex;
      flex-wrap: wrap;
      gap: 0.4rem;
    }
    .keyword-pill {
      background: rgba(255, 255, 255, 0.05);
      color: var(--text-muted);
      border: 1px solid rgba(255, 255, 255, 0.08);
      padding: 0.2rem 0.55rem;
      border-radius: 9999px;
      font-size: 0.75rem;
      font-family: var(--font-mono);
    }
    .empty-state {
      text-align: center;
      padding: 4rem 1rem;
      color: var(--text-muted);
      background: var(--surface);
      border: 1px dashed var(--surface-border);
      border-radius: 12px;
    }
    footer {
      padding: 1.5rem;
      text-align: center;
      font-size: 0.8rem;
      color: var(--text-muted);
      border-top: 1px solid var(--surface-border);
    }
  </style>
</head>
<body>
  <header>
    <div class="logo">
      <span>AXEL</span>
      <span class="logo-badge">Rust v0.2.0</span>
    </div>
    <div class="status-bar">
      <div class="status-pill" id="aiStatusPill">
        <span class="status-dot"></span>
        <span id="aiStatusText">AI Ready</span>
      </div>
      <div id="statsDocsCount">0 Documents</div>
    </div>
  </header>

  <main>
    <div class="hero">
      <h1>Automated Document Intelligence</h1>
      <p>Instant categorization, deep OCR extraction, semantic keyword labeling, and vector-precision document retrieval powered by native Rust.</p>
    </div>

    <div class="search-box">
      <span class="search-icon">🔍</span>
      <input
        type="text"
        id="searchInput"
        class="search-input"
        placeholder="Search documents by keyword, category, file name, or topic..."
        autofocus
      />
    </div>

    <div class="actions-row">
      <div style="font-size: 0.9rem; color: var(--text-muted);" id="resultsHeading">All Indexed Documents</div>
      <button class="btn btn-primary" id="scanBtn" onclick="runScan()">⚡ Scan Document Folder</button>
    </div>

    <div class="stats-grid" id="statsGrid"></div>

    <div class="document-list" id="documentList">
      <div class="empty-state">Loading document index...</div>
    </div>
  </main>

  <footer>
    AXEL • High-Performance Document Extraction & Labeling • Built with Rust & Axum
  </footer>

  <script>
    let allDocuments = [];

    async function loadStats() {
      try {
        const res = await fetch('/api/stats');
        const data = await res.json();

        document.getElementById('statsDocsCount').textContent = `${data.total_documents} Documents`;
        document.getElementById('aiStatusText').textContent = data.groq_enabled ? `Groq (${data.active_model})` : 'Local AI Rules';

        const statsGrid = document.getElementById('statsGrid');
        statsGrid.innerHTML = `
          <div class="stat-card">
            <div class="stat-label">Total Indexed</div>
            <div class="stat-value">${data.total_documents}</div>
          </div>
          <div class="stat-card">
            <div class="stat-label">Active Model</div>
            <div class="stat-value" style="font-size: 1.1rem; line-height: 2rem;">${data.groq_enabled ? 'Groq LLaMA' : 'Local Rule Engine'}</div>
          </div>
          <div class="stat-card">
            <div class="stat-label">Watched Folder</div>
            <div class="stat-value" style="font-size: 0.95rem; font-family: var(--font-mono); line-height: 2rem;">${data.input_folder}</div>
          </div>
        `;
      } catch (e) {
        console.error('Failed to load stats', e);
      }
    }

    async function loadDocuments() {
      try {
        const res = await fetch('/api/documents');
        allDocuments = await res.json();
        renderDocuments(allDocuments);
      } catch (e) {
        document.getElementById('documentList').innerHTML = `<div class="empty-state">Failed to connect to AXEL API.</div>`;
      }
    }

    function renderDocuments(docs) {
      const list = document.getElementById('documentList');
      if (!docs || docs.length === 0) {
        list.innerHTML = `<div class="empty-state">No documents indexed yet. Drop files into your watched documents folder or click <strong>Scan Document Folder</strong> above!</div>`;
        return;
      }

      list.innerHTML = docs.map(doc => {
        const item = doc.item || doc;
        const score = doc.score ? `<span style="font-size: 0.8rem; color: #34d399; font-weight: 700;">Relevance: ${(doc.score * 100).toFixed(0)}%</span>` : '';
        const kws = (item.keywords || []).map(k => `<span class="keyword-pill">#${k}</span>`).join('');
        return `
          <div class="doc-card">
            <div class="doc-header">
              <div class="doc-title">
                <span>📄 ${escapeHtml(item.file)}</span>
                <span class="doc-badge">${escapeHtml(item.category || 'General')}</span>
              </div>
              ${score}
            </div>
            <div class="doc-path">${escapeHtml(item.path)}</div>
            <div class="doc-keywords">${kws}</div>
          </div>
        `;
      }).join('');
    }

    let searchDebounceTimer = null;
    document.getElementById('searchInput').addEventListener('input', (e) => {
      const q = e.target.value.trim();
      clearTimeout(searchDebounceTimer);
      searchDebounceTimer = setTimeout(async () => {
        if (!q) {
          document.getElementById('resultsHeading').textContent = 'All Indexed Documents';
          renderDocuments(allDocuments);
          return;
        }

        document.getElementById('resultsHeading').textContent = `Search results for "${q}"`;
        try {
          const res = await fetch(`/api/search?q=${encodeURIComponent(q)}`);
          const data = await res.json();
          renderDocuments(data.results);
        } catch (e) {
          console.error('Search failed', e);
        }
      }, 150);
    });

    async function runScan() {
      const btn = document.getElementById('scanBtn');
      btn.disabled = true;
      btn.textContent = '⏳ Scanning...';
      try {
        const res = await fetch('/api/scan', { method: 'POST' });
        const result = await res.json();
        alert(`Scan Complete!\nScanned: ${result.scanned}\nNewly Indexed: ${result.indexed}\nSkipped: ${result.skipped}`);
        await loadStats();
        await loadDocuments();
      } catch (e) {
        alert('Failed to scan documents: ' + e);
      } finally {
        btn.disabled = false;
        btn.textContent = '⚡ Scan Document Folder';
      }
    }

    function escapeHtml(text) {
      if (!text) return '';
      return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#039;");
    }

    loadStats();
    loadDocuments();
  </script>
</body>
</html>
"#;
