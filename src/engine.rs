use crate::ai::AiEngine;
use crate::classifier::{
    is_script_extension, should_skip_directory, should_skip_file, TfidfEngine,
};
use crate::config::{resolve_workspace_path, Config};
use crate::extractor::{
    detect_file_type, extract_raw_text, extract_text, ocr, preprocess_text, FileType,
};
use crate::index::{DocumentIndex, IndexItem};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphHub {
    pub id: String,
    pub category: String,
    pub count: usize,
    pub top_keywords: Vec<String>,
    pub accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub hash: String,
    pub file: String,
    pub category: String,
    pub file_type: String,
    pub keywords: Vec<String>,
    pub path: String,
    pub indexed_at: Option<String>,
    pub size_bytes: Option<u64>,
    pub file_exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub shared_keywords: Vec<String>,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub hash: String,
    pub file: String,
    pub category: String,
    pub file_type: String,
    pub indexed_at: Option<String>,
    pub keywords_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphSnapshot {
    pub hubs: Vec<GraphHub>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub activity: Vec<ActivityEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDetail {
    pub item: IndexItem,
    pub text_preview: String,
    pub file_exists: bool,
    pub file_type: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub total_documents: usize,
    pub groq_enabled: bool,
    pub tesseract_ready: bool,
    pub active_model: String,
    pub input_folder: String,
    pub index_file: String,
    pub categories: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub scanned: usize,
    pub indexed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOutcome {
    pub deleted: bool,
    pub hash: String,
    pub file: String,
    pub file_removed_from_disk: bool,
}

pub fn category_accent(category: &str) -> &'static str {
    const PALETTE: [&str; 6] = ["amber", "violet", "cyan", "emerald", "rose", "indigo"];
    let clean = category.trim().to_lowercase();
    let sum: usize = clean.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize));
    PALETTE[sum % PALETTE.len()]
}

fn is_generic_link_keyword(kw: &str) -> bool {
    matches!(
        kw,
        "document"
            | "documents"
            | "file"
            | "files"
            | "records"
            | "record"
            | "data"
            | "report"
            | "reports"
            | "research"
            | "system"
            | "systems"
            | "using"
            | "present"
            | "summary"
            | "notes"
            | "page"
            | "pages"
            | "text"
            | "pdf"
            | "docx"
            | "txt"
            | "image"
            | "general"
            | "daily"
            | "work"
            | "project"
            | "projects"
            | "based"
            | "section"
            | "chapter"
            | "part"
            | "study"
            | "analysis"
            | "information"
            | "details"
    )
}

pub fn build_graph_snapshot(items: &[IndexItem]) -> GraphSnapshot {
    let mut by_category: BTreeMap<String, Vec<&IndexItem>> = BTreeMap::new();
    for item in items {
        by_category
            .entry(item.category.clone())
            .or_default()
            .push(item);
    }

    let mut hubs: Vec<GraphHub> = by_category
        .iter()
        .map(|(category, cat_items)| {
            let mut kw_freq: HashMap<String, usize> = HashMap::new();
            for item in cat_items {
                for kw in &item.keywords {
                    *kw_freq.entry(kw.to_lowercase()).or_insert(0) += 1;
                }
            }
            let mut kws: Vec<(String, usize)> = kw_freq.into_iter().collect();
            kws.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let top_keywords = kws.into_iter().take(5).map(|(k, _)| k).collect();

            GraphHub {
                id: format!("hub:{category}"),
                category: category.clone(),
                count: cat_items.len(),
                top_keywords,
                accent: category_accent(category).to_string(),
            }
        })
        .collect();

    hubs.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.category.cmp(&b.category)));

    let mut nodes = Vec::with_capacity(items.len());
    let mut edges = Vec::new();

    for item in items {
        let path = resolve_workspace_path(Path::new(&item.path));
        let file_exists = path.exists();
        let size_bytes = if file_exists {
            std::fs::metadata(&path).map(|m| m.len()).ok()
        } else {
            None
        };
        let file_type = detect_file_type(&path).as_str().to_string();

        // Primary structural edge: every document belongs cleanly to its Category Hub
        edges.push(GraphEdge {
            source: format!("hub:{}", item.category),
            target: item.hash.clone(),
            kind: "cluster".to_string(),
            shared_keywords: Vec::new(),
            weight: 1.0,
        });

        nodes.push(GraphNode {
            hash: item.hash.clone(),
            file: item.file.clone(),
            category: item.category.clone(),
            file_type,
            keywords: item.keywords.clone(),
            path: path.display().to_string(),
            indexed_at: item.indexed_at.clone(),
            size_bytes,
            file_exists,
        });
    }

    // Strict semantic edges: ONLY connect documents when >= 2 strong non-generic domain
    // keywords overlap AND Jaccard similarity >= 0.22 (prevents noisy single-word links).
    let mut hub_shared_kws: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for i in 0..items.len() {
        let kws_a: HashSet<String> = items[i]
            .keywords
            .iter()
            .map(|k| k.to_lowercase())
            .filter(|k| !is_generic_link_keyword(k))
            .collect();
        if kws_a.is_empty() {
            continue;
        }

        for j in (i + 1)..items.len() {
            let kws_b: HashSet<String> = items[j]
                .keywords
                .iter()
                .map(|k| k.to_lowercase())
                .filter(|k| !is_generic_link_keyword(k))
                .collect();
            if kws_b.is_empty() {
                continue;
            }

            let mut shared: Vec<String> = kws_a.intersection(&kws_b).cloned().collect();
            shared.sort();

            let union_len = kws_a.union(&kws_b).count().max(1);
            let jaccard = shared.len() as f64 / union_len as f64;

            if shared.len() >= 2 && jaccard >= 0.22 {
                let weight = (shared.len() as f64 / 4.0).min(1.0);
                edges.push(GraphEdge {
                    source: items[i].hash.clone(),
                    target: items[j].hash.clone(),
                    kind: "semantic".to_string(),
                    shared_keywords: shared.clone(),
                    weight,
                });

                if items[i].category != items[j].category {
                    let (h1, h2) = if items[i].category < items[j].category {
                        (
                            format!("hub:{}", items[i].category),
                            format!("hub:{}", items[j].category),
                        )
                    } else {
                        (
                            format!("hub:{}", items[j].category),
                            format!("hub:{}", items[i].category),
                        )
                    };
                    hub_shared_kws
                        .entry((h1, h2))
                        .or_default()
                        .extend(shared);
                }
            }
        }
    }

    let mut hub_pairs: HashSet<(String, String)> = HashSet::new();
    for ((h1, h2), kws) in hub_shared_kws {
        let mut list: Vec<String> = kws.into_iter().collect();
        list.sort();
        list.truncate(4);
        hub_pairs.insert((h1.clone(), h2.clone()));
        edges.push(GraphEdge {
            source: h1,
            target: h2,
            kind: "pipeline".to_string(),
            shared_keywords: list,
            weight: 0.85,
        });
    }

    // Clean open constellation chain connecting hubs (no closed ring crossing the canvas)
    if hubs.len() > 1 {
        for i in 0..(hubs.len() - 1) {
            let (h1, h2) = if hubs[i].id < hubs[i + 1].id {
                (hubs[i].id.clone(), hubs[i + 1].id.clone())
            } else {
                (hubs[i + 1].id.clone(), hubs[i].id.clone())
            };
            if !hub_pairs.contains(&(h1.clone(), h2.clone())) {
                edges.push(GraphEdge {
                    source: hubs[i].id.clone(),
                    target: hubs[i + 1].id.clone(),
                    kind: "backbone".to_string(),
                    shared_keywords: Vec::new(),
                    weight: 0.35,
                });
            }
        }
    }

    let mut activity: Vec<ActivityEntry> = nodes
        .iter()
        .map(|n| ActivityEntry {
            hash: n.hash.clone(),
            file: n.file.clone(),
            category: n.category.clone(),
            file_type: n.file_type.clone(),
            indexed_at: n.indexed_at.clone(),
            keywords_count: n.keywords.len(),
        })
        .collect();

    activity.sort_by(|a, b| b.indexed_at.cmp(&a.indexed_at));

    GraphSnapshot {
        hubs,
        nodes,
        edges,
        activity,
    }
}

pub fn build_document_detail(item: &IndexItem) -> DocumentDetail {
    let path = resolve_workspace_path(Path::new(&item.path));
    let file_exists = path.exists();
    let file_type = detect_file_type(&path).as_str().to_string();
    let size_bytes = if file_exists {
        std::fs::metadata(&path).map(|m| m.len()).ok()
    } else {
        None
    };

    let text_preview = if file_exists {
        let (text, _) = extract_text(&path);
        if text.trim().is_empty() {
            "(No extractable text found in this document — indexed via filename & metadata)".to_string()
        } else if text.chars().count() > 4000 {
            let truncated: String = text.chars().take(4000).collect();
            format!("{truncated}\n\n[... preview truncated to 4,000 characters ...]")
        } else {
            text
        }
    } else {
        "(Original document file is no longer accessible at this path)".to_string()
    };

    DocumentDetail {
        item: item.clone(),
        text_preview,
        file_exists,
        file_type,
        size_bytes,
    }
}

pub fn build_system_stats(config: &Config, index: &DocumentIndex) -> SystemStats {
    let mut categories = HashMap::new();
    for item in index.all() {
        *categories.entry(item.category.clone()).or_insert(0) += 1;
    }

    SystemStats {
        total_documents: index.count(),
        groq_enabled: config.groq_api_key.is_some(),
        tesseract_ready: ocr::find_tesseract_binary().is_some(),
        active_model: config.model_name.clone(),
        input_folder: resolve_workspace_path(&config.input_folder)
            .display()
            .to_string(),
        index_file: config.index_file.display().to_string(),
        categories,
    }
}

pub fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    collect_files_bounded(dir, files, 0, 8, 2000);
    files.sort();
}

fn collect_files_bounded(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    depth: usize,
    max_depth: usize,
    max_files: usize,
) {
    if depth > max_depth || files.len() >= max_files || !dir.is_dir() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        if files.len() >= max_files {
            break;
        }
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if ft.is_dir() {
            if !should_skip_directory(&name_str) {
                collect_files_bounded(&path, files, depth + 1, max_depth, max_files);
            }
        } else if ft.is_file() {
            let size = entry.metadata().map(|m| m.len()).ok();
            if !should_skip_file(&path, size) {
                files.push(path);
            }
        }
    }
}

pub async fn analyze_and_build_item(path: &Path, ai: &AiEngine) -> Option<IndexItem> {
    analyze_and_build_item_with_tfidf(path, ai, None).await
}

pub async fn analyze_and_build_item_with_tfidf(
    path: &Path,
    ai: &AiEngine,
    mut tfidf: Option<&mut TfidfEngine>,
) -> Option<IndexItem> {
    let resolved = resolve_workspace_path(path);
    if !resolved.exists() || detect_file_type(&resolved) == FileType::Unsupported {
        return None;
    }

    let ext = resolved
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let path_for_extract = resolved.clone();
    let extracted = tokio::task::spawn_blocking(move || {
        let hash = DocumentIndex::compute_file_hash(&path_for_extract)?;
        let (raw_text, _) = extract_raw_text(&path_for_extract);
        Some((hash, raw_text))
    })
    .await
    .ok()??;

    let (hash, raw_text) = extracted;
    let file_name = resolved
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();

    if is_script_extension(&ext) {
        let mut tfidf_kws = Vec::new();
        if let Some(engine) = tfidf.as_deref_mut() {
            engine.register_document_terms(&raw_text, &[]);
            tfidf_kws = engine.extract_keywords(&raw_text, &[], 6);
        }

        let (ai_res, symbols) = ai.analyze_code_script(&raw_text, &file_name, &ext, &tfidf_kws).await;

        Some(IndexItem::new(
            file_name,
            ai_res.category,
            ai_res.keywords,
            symbols.symbols,
            resolved.display().to_string(),
            hash,
            ai_res.virtual_paths,
        ))
    } else {
        let mut tfidf_kws = Vec::new();
        if let Some(engine) = tfidf.as_deref_mut() {
            engine.register_document_terms(&raw_text, &[]);
            tfidf_kws = engine.extract_keywords(&raw_text, &[], 6);
        }

        let preprocessed = preprocess_text(&raw_text);
        let ai_res = ai.analyze_document(&preprocessed, &file_name, Some(&tfidf_kws)).await;

        Some(IndexItem::new(
            file_name,
            ai_res.category,
            ai_res.keywords,
            Vec::new(),
            resolved.display().to_string(),
            hash,
            ai_res.virtual_paths,
        ))
    }
}

pub async fn scan_folder_into_items(
    folder: &Path,
    existing_hashes: &HashSet<String>,
    ai: &AiEngine,
    force: bool,
) -> (ScanSummary, Vec<IndexItem>) {
    scan_folder_with_progress(folder, existing_hashes, ai, force, |_, _, _| {}).await
}

pub async fn scan_folder_with_progress<F>(
    folder: &Path,
    existing_hashes: &HashSet<String>,
    ai: &AiEngine,
    force: bool,
    mut on_progress: F,
) -> (ScanSummary, Vec<IndexItem>)
where
    F: FnMut(usize, usize, &str),
{
    let folder_owned = resolve_workspace_path(folder);
    let folder_for_task = folder_owned.clone();
    let candidate_files = tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        collect_files_recursive(&folder_for_task, &mut files);
        files
    })
    .await
    .unwrap_or_default();

    let total = candidate_files.len();
    let mut scanned = 0;
    let mut indexed = 0;
    let mut skipped = 0;
    let mut items_to_insert = Vec::new();

    let tfidf_path = folder_owned.join(".axel_tfidf.json");
    let mut tfidf = TfidfEngine::new(Some(tfidf_path));

    for (idx, path) in candidate_files.into_iter().enumerate() {
        scanned += 1;
        let file_label = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("document")
            .to_string();

        if detect_file_type(&path) == FileType::Unsupported {
            skipped += 1;
            continue;
        }

        let path_for_hash = path.clone();
        let hash = tokio::task::spawn_blocking(move || DocumentIndex::compute_file_hash(&path_for_hash))
            .await
            .ok()
            .flatten();

        let Some(hash) = hash else {
            skipped += 1;
            continue;
        };

        if !force && existing_hashes.contains(&hash) {
            skipped += 1;
            continue;
        }

        on_progress(idx + 1, total, &file_label);

        if let Some(item) = analyze_and_build_item_with_tfidf(&path, ai, Some(&mut tfidf)).await {
            items_to_insert.push(item);
            indexed += 1;
        } else {
            skipped += 1;
        }
    }

    let _ = tfidf.save();

    (
        ScanSummary {
            scanned,
            indexed,
            skipped,
        },
        items_to_insert,
    )
}

pub async fn ingest_local_files(
    paths: &[PathBuf],
    workspace_folder: &Path,
    ai: &AiEngine,
    copy_into_workspace: bool,
) -> Result<Vec<IndexItem>, String> {
    let workspace_resolved = resolve_workspace_path(workspace_folder);
    if copy_into_workspace && !workspace_resolved.exists() {
        std::fs::create_dir_all(&workspace_resolved)
            .map_err(|e| format!("Failed to create workspace folder: {e}"))?;
    }

    let mut ingested = Vec::new();

    for src_path in paths {
        if !src_path.exists() || !src_path.is_file() {
            continue;
        }
        if detect_file_type(src_path) == FileType::Unsupported {
            continue;
        }

        let target_path = if copy_into_workspace {
            let file_name = src_path
                .file_name()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty() && *s != "." && *s != "..")
                .ok_or_else(|| "Invalid filename".to_string())?;

            let dest = workspace_resolved.join(file_name);
            let same_file = match (
                std::fs::canonicalize(src_path),
                std::fs::canonicalize(&dest),
            ) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            };

            if !same_file {
                std::fs::copy(src_path, &dest)
                    .map_err(|e| format!("Failed to copy {}: {e}", src_path.display()))?;
            }
            dest
        } else {
            src_path.clone()
        };

        if let Some(item) = analyze_and_build_item(&target_path, ai).await {
            ingested.push(item);
        }
    }

    Ok(ingested)
}

pub fn is_path_inside_workspace(path: &Path, workspace_folder: &Path) -> bool {
    match (
        std::fs::canonicalize(workspace_folder),
        std::fs::canonicalize(path),
    ) {
        (Ok(base), Ok(target)) => target.starts_with(&base),
        _ => false,
    }
}

pub fn delete_document_from_index(
    hash: &str,
    delete_file: bool,
    config: &Config,
    index: &mut DocumentIndex,
) -> Result<DeleteOutcome, String> {
    let removed = index
        .remove_by_hash(hash)
        .ok_or_else(|| "Document not found".to_string())?;

    index
        .save()
        .map_err(|e| format!("Failed to save index: {e}"))?;

    let mut file_removed_from_disk = false;
    if delete_file {
        let target_path = Path::new(&removed.path);
        if is_path_inside_workspace(target_path, &config.input_folder)
            && std::fs::remove_file(target_path).is_ok()
        {
            file_removed_from_disk = true;
        }
    }

    Ok(DeleteOutcome {
        deleted: true,
        hash: removed.hash,
        file: removed.file,
        file_removed_from_disk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_graph_snapshot_avoids_single_generic_keyword_links() {
        let items = vec![
            IndexItem {
                file: "discharge.txt".to_string(),
                category: "Medical".to_string(),
                keywords: vec!["patient".to_string(), "research".to_string()],
                symbols: Vec::new(),
                path: "documents/discharge.txt".to_string(),
                hash: "h1".to_string(),
                indexed_at: Some("2026-11-22T10:00:00Z".to_string()),
                ..Default::default()
            },
            IndexItem {
                file: "paper.txt".to_string(),
                category: "Academic".to_string(),
                keywords: vec!["transformers".to_string(), "research".to_string()],
                symbols: Vec::new(),
                path: "documents/paper.txt".to_string(),
                hash: "h2".to_string(),
                indexed_at: Some("2026-11-22T11:00:00Z".to_string()),
                ..Default::default()
            },
        ];

        let snap = build_graph_snapshot(&items);
        assert_eq!(snap.hubs.len(), 2);
        assert_eq!(snap.nodes.len(), 2);
        assert!(snap.edges.iter().any(|e| e.kind == "cluster"));
        // Single generic keyword ("research") must NOT falsely connect unrelated files
        assert!(!snap.edges.iter().any(|e| e.kind == "semantic"));
        assert_eq!(snap.activity[0].hash, "h2");
    }

    #[test]
    fn test_build_graph_snapshot_connects_strong_multi_keyword_matches() {
        let items = vec![
            IndexItem {
                file: "invoice_q1.txt".to_string(),
                category: "Invoice".to_string(),
                keywords: vec!["gstin".to_string(), "acme".to_string(), "subtotal".to_string()],
                symbols: Vec::new(),
                path: "documents/invoice_q1.txt".to_string(),
                hash: "h1".to_string(),
                indexed_at: None,
                ..Default::default()
            },
            IndexItem {
                file: "vendor_contract.txt".to_string(),
                category: "Legal".to_string(),
                keywords: vec!["gstin".to_string(), "acme".to_string(), "clause".to_string()],
                symbols: Vec::new(),
                path: "documents/vendor_contract.txt".to_string(),
                hash: "h2".to_string(),
                indexed_at: None,
                ..Default::default()
            },
        ];

        let snap = build_graph_snapshot(&items);
        assert!(snap
            .edges
            .iter()
            .any(|e| e.kind == "semantic" && e.shared_keywords == vec!["acme".to_string(), "gstin".to_string()]));
        assert!(snap.edges.iter().any(|e| e.kind == "pipeline"));
    }

    #[test]
    fn test_category_accent_mapping() {
        const PALETTE: [&str; 6] = ["amber", "violet", "cyan", "emerald", "rose", "indigo"];
        let accent1 = category_accent("Invoice");
        let accent2 = category_accent("invoice");
        assert_eq!(accent1, accent2);
        assert!(PALETTE.contains(&accent1));

        let accent_custom = category_accent("Custom Dynamic Category From LLM");
        assert!(PALETTE.contains(&accent_custom));
    }
}
