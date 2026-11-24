use crate::ai::AiEngine;
use crate::classifier::{
    is_script_extension, should_skip_directory, should_skip_file, TfidfEngine,
};
use crate::config::{resolve_workspace_path, Config};
use crate::extractor::{
    detect_file_type, extract_raw_text, extract_text, ocr,
};
use crate::index::{DocumentIndex, FileMetadataTag, IndexItem};
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
pub struct FileProperties {
    pub file_name: String,
    pub absolute_path: String,
    pub parent_dir: String,
    pub file_size_bytes: u64,
    pub formatted_size: String,
    pub file_type_desc: String,
    pub created_str: String,
    pub modified_str: String,
    pub accessed_str: String,
    pub permissions_mode: String,
    pub is_readonly: bool,
    pub is_symlink: bool,
    pub has_xattrs: bool,
    pub xattr_category: Option<String>,
    pub xattr_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDetail {
    pub item: IndexItem,
    pub text_preview: String,
    pub file_exists: bool,
    pub file_type: String,
    pub size_bytes: Option<u64>,
    pub properties: Option<FileProperties>,
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
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOutcome {
    pub deleted: bool,
    pub index_deleted: bool,
    pub file_removed_from_disk: bool,
    pub disk_removal_method: Option<String>,
    pub hash: String,
    pub file: String,
    pub error: Option<String>,
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

pub fn resolve_verified_document_path(item: &IndexItem, workspace: &Path) -> Option<PathBuf> {
    // Collect candidate paths: primary path first, followed by all physical aliases (M03)
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(&item.path));
    for alias in &item.physical_paths {
        let ap = PathBuf::from(alias);
        if !candidates.contains(&ap) {
            candidates.push(ap);
        }
    }

    // Check candidates in order. Check existence first so canonicalization does not fail on missing primary (M03)
    for cand in candidates {
        let resolved = resolve_workspace_path(&cand);
        if resolved.exists() && is_path_inside_workspace(&resolved, workspace) {
            // H11: Check file size ceiling (100MB) before full hashing
            if let Ok(meta) = std::fs::symlink_metadata(&resolved) {
                if !meta.is_file() {
                    continue;
                }
                const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;
                if meta.len() > MAX_DOWNLOAD_BYTES {
                    continue;
                }
            }
            if let Some(chash) = DocumentIndex::compute_file_hash(&resolved) {
                if chash == item.hash {
                    return Some(resolved);
                }
            }
        }
    }

    None
}

pub fn build_document_detail_verified(item: &IndexItem, workspace: &Path) -> DocumentDetail {
    let verified_path = resolve_verified_document_path(item, workspace);
    let file_exists = verified_path.is_some();
    let effective_path = verified_path
        .clone()
        .unwrap_or_else(|| resolve_workspace_path(Path::new(&item.path)));
    let file_type = detect_file_type(&effective_path).as_str().to_string();
    let size_bytes = if file_exists {
        std::fs::metadata(&effective_path).map(|m| m.len()).ok()
    } else {
        None
    };

    let text_preview = if file_exists {
        let (text, _) = extract_text(&effective_path);
        if text.trim().is_empty() {
            "(No extractable text found in this document — indexed via filename & metadata)".to_string()
        } else if text.chars().count() > 4000 {
            let truncated: String = text.chars().take(4000).collect();
            format!("{truncated}\n\n[... preview truncated to 4,000 characters ...]")
        } else {
            text
        }
    } else {
        "(Original document file is no longer accessible or has been modified on disk)".to_string()
    };

    let properties = if file_exists {
        extract_file_properties(&effective_path)
    } else {
        None
    };

    DocumentDetail {
        item: item.clone(),
        text_preview,
        file_exists,
        file_type,
        size_bytes,
        properties,
    }
}

pub fn build_document_detail(item: &IndexItem) -> DocumentDetail {
    let workspace = Path::new(&item.path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    build_document_detail_verified(item, workspace)
}

fn format_int_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    let bytes = s.as_bytes();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}

pub fn format_bytes_precise(bytes: u64) -> String {
    let commas = format_int_with_commas(bytes);
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB ({commas} bytes)", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!(
            "{:.2} MB ({commas} bytes)",
            bytes as f64 / (1024.0 * 1024.0)
        )
    } else {
        format!(
            "{:.2} GB ({commas} bytes)",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    }
}

#[cfg(unix)]
fn format_unix_mode(mode: u32) -> String {
    let mut s = String::with_capacity(10);
    s.push(if mode & 0o40000 != 0 { 'd' } else { '-' });
    s.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o200 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o100 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o020 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o010 != 0 { 'x' } else { '-' });
    s.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    s.push(if mode & 0o002 != 0 { 'w' } else { '-' });
    s.push(if mode & 0o001 != 0 { 'x' } else { '-' });
    s
}

pub fn extract_file_properties(path: &Path) -> Option<FileProperties> {
    let md = std::fs::symlink_metadata(path).ok()?;
    let is_symlink = md.file_type().is_symlink();
    let actual_md = if is_symlink {
        std::fs::metadata(path).unwrap_or_else(|_| md.clone())
    } else {
        md.clone()
    };
    let file_size_bytes = actual_md.len();
    let formatted_size = format_bytes_precise(file_size_bytes);

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let absolute_path = std::fs::canonicalize(path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());

    let parent_dir = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".to_string());

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let file_type_desc = match ext.as_str() {
        "pdf" => "PDF Document (.pdf)".to_string(),
        "docx" | "doc" => "Word Document (.docx)".to_string(),
        "txt" => "Plain Text Document (.txt)".to_string(),
        "md" => "Markdown Document (.md)".to_string(),
        "mkv" => "Matroska Video (.mkv)".to_string(),
        "mp4" => "MPEG-4 Video (.mp4)".to_string(),
        "webm" => "WebM Video (.webm)".to_string(),
        "mp3" => "MP3 Audio (.mp3)".to_string(),
        "wav" => "WAV Audio (.wav)".to_string(),
        "png" => "PNG Image (.png)".to_string(),
        "jpg" | "jpeg" => "JPEG Image (.jpg)".to_string(),
        "webp" => "WebP Image (.webp)".to_string(),
        "py" => "Python Script (.py)".to_string(),
        "rs" => "Rust Source (.rs)".to_string(),
        "js" | "ts" => "JavaScript/TypeScript Code".to_string(),
        "zip" | "tar" | "gz" | "7z" => "Compressed Archive".to_string(),
        "iso" => "Disk Image (.iso)".to_string(),
        _ => {
            if ext.is_empty() {
                "File".to_string()
            } else {
                format!("{} File (.{})", ext.to_uppercase(), ext)
            }
        }
    };

    let modified_str = actual_md
        .modified()
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%b %d, %Y  %I:%M %p").to_string()
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let created_str = actual_md
        .created()
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%b %d, %Y  %I:%M %p").to_string()
        })
        .unwrap_or_else(|| modified_str.clone());

    let accessed_str = actual_md
        .accessed()
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%b %d, %Y  %I:%M %p").to_string()
        })
        .unwrap_or_else(|| "Unknown".to_string());

    #[cfg(unix)]
    let permissions_mode = {
        use std::os::unix::fs::PermissionsExt;
        let mode = actual_md.permissions().mode();
        format!("{:04o} ({})", mode & 0o777, format_unix_mode(mode))
    };
    #[cfg(not(unix))]
    let permissions_mode = if actual_md.permissions().readonly() {
        "Read-only".to_string()
    } else {
        "Read and Write".to_string()
    };

    let is_readonly = actual_md.permissions().readonly();

    let tag = crate::index::metadata::FileMetadataTag::read_from_file(path);
    let xattr_category = tag.as_ref().map(|t| t.category.clone());
    let xattr_keywords = tag.map(|t| t.keywords).unwrap_or_default();
    let has_xattrs = xattr_category.is_some() || !xattr_keywords.is_empty();

    Some(FileProperties {
        file_name,
        absolute_path,
        parent_dir,
        file_size_bytes,
        formatted_size,
        file_type_desc,
        created_str,
        modified_str,
        accessed_str,
        permissions_mode,
        is_readonly,
        is_symlink,
        has_xattrs,
        xattr_category,
        xattr_keywords,
    })
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

pub fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> bool {
    let max_files = std::env::var("AXEL_MAX_SCAN_FILES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);
    let max_depth = std::env::var("AXEL_MAX_SCAN_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);

    let mut truncated = false;
    collect_files_bounded(dir, files, 0, max_depth, max_files, &mut truncated);
    files.sort();
    truncated
}

fn collect_files_bounded(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    depth: usize,
    max_depth: usize,
    max_files: usize,
    truncated: &mut bool,
) {
    if files.len() >= max_files {
        *truncated = true;
        return;
    }
    if depth > max_depth {
        *truncated = true;
        return;
    }
    if !dir.is_dir() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        if files.len() >= max_files {
            *truncated = true;
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
                collect_files_bounded(&path, files, depth + 1, max_depth, max_files, truncated);
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
    if !resolved.exists() || !resolved.is_file() {
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

    let item = if is_script_extension(&ext) {
        // Redact secrets and credentials before TF-IDF registration (H10, M12)
        let (sanitized_code, _) = crate::classifier::privacy::redact_code_secrets_and_pii(&raw_text);
        let mut tfidf_kws = Vec::new();
        if let Some(engine) = tfidf.as_deref_mut() {
            engine.register_document_terms(&sanitized_code, &[]);
            tfidf_kws = engine.extract_keywords(&sanitized_code, &[], 6);
        }

        let (ai_res, symbols) = ai.analyze_code_script(&raw_text, &file_name, &ext, &tfidf_kws).await;

        IndexItem::new(
            file_name,
            ai_res.category,
            ai_res.keywords,
            symbols.symbols,
            resolved.display().to_string(),
            hash,
            ai_res.virtual_paths,
        )
    } else {
        // Redact sensitive personal data before TF-IDF registration (H11, M12)
        let (sanitized_full_text, _) = crate::classifier::privacy::redact_sensitive_pii(&raw_text);
        let mut tfidf_kws = Vec::new();
        if let Some(engine) = tfidf.as_deref_mut() {
            engine.register_document_terms(&sanitized_full_text, &[]);
            tfidf_kws = engine.extract_keywords(&sanitized_full_text, &[], 6);
        }

        // Pass full text into analyze_document so identity & privacy checks inspect complete text (H11)
        let ai_res = ai.analyze_document(&raw_text, &file_name, Some(&tfidf_kws)).await;

        IndexItem::new(
            file_name,
            ai_res.category,
            ai_res.keywords,
            Vec::new(),
            resolved.display().to_string(),
            hash,
            ai_res.virtual_paths,
        )
    };

    // Permanently write dynamic tags directly into the file's extended attributes
    let _ = FileMetadataTag::write_to_file(
        &resolved,
        &item.category,
        &item.keywords,
        &item.virtual_paths,
        &item.hash,
    );

    Some(item)
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
    let (candidate_files, was_truncated) = tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        let truncated = collect_files_recursive(&folder_for_task, &mut files);
        (files, truncated)
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

        // Fast metadata path: If the file already carries valid dynamic tags in its metadata (xattr),
        // we can skip expensive disk reading, hashing, OCR, and classification entirely!
        if !force {
            if let Some(tagged_item) = FileMetadataTag::read_from_file(&path) {
                if !existing_hashes.contains(&tagged_item.hash) {
                    items_to_insert.push(tagged_item);
                    indexed += 1;
                } else {
                    // M05: Register duplicate physical path as alias without repeating OCR / LLM
                    let mut dup_item = IndexItem::empty();
                    dup_item.hash = tagged_item.hash;
                    dup_item.path = path.to_string_lossy().to_string();
                    dup_item.file = path.file_name().and_then(|s| s.to_str()).unwrap_or("file").to_string();
                    dup_item.physical_paths = vec![dup_item.path.clone()];
                    items_to_insert.push(dup_item);
                    skipped += 1;
                }
                continue;
            }
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

        if !force && (existing_hashes.contains(&hash) || items_to_insert.iter().any(|i| i.hash == hash && !i.category.is_empty())) {
            // M05: Register duplicate physical path as alias without repeating OCR / LLM
            let mut dup_item = IndexItem::empty();
            dup_item.hash = hash;
            dup_item.path = path.to_string_lossy().to_string();
            dup_item.file = path.file_name().and_then(|s| s.to_str()).unwrap_or("file").to_string();
            dup_item.physical_paths = vec![dup_item.path.clone()];
            items_to_insert.push(dup_item);
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
            truncated: was_truncated,
        },
        items_to_insert,
    )
}

/// Compares two files on disk byte-for-byte to verify exact bit-level identity.
/// Returns Ok(true) only if both files exist, are regular files, have identical sizes,
/// represent different filesystem paths/inodes, and match completely from byte 0 to EOF.
pub fn are_files_identical<P1: AsRef<Path>, P2: AsRef<Path>>(
    p1: P1,
    p2: P2,
) -> Result<bool, std::io::Error> {
    use std::io::Read;

    let p1 = p1.as_ref();
    let p2 = p2.as_ref();

    let meta1 = std::fs::symlink_metadata(p1)?;
    let meta2 = std::fs::symlink_metadata(p2)?;

    if !meta1.is_file() || !meta2.is_file() {
        return Ok(false);
    }

    if meta1.len() != meta2.len() {
        return Ok(false);
    }

    // Ensure we are not comparing the exact same file/inode on disk
    if let (Ok(c1), Ok(c2)) = (std::fs::canonicalize(p1), std::fs::canonicalize(p2)) {
        if c1 == c2 {
            return Ok(false);
        }
    }

    if meta1.len() == 0 {
        return Ok(true);
    }

    let mut f1 = std::fs::File::open(p1)?;
    let mut f2 = std::fs::File::open(p2)?;

    let mut buf1 = [0u8; 65536];
    let mut buf2 = [0u8; 65536];

    loop {
        let n1 = f1.read(&mut buf1)?;
        let n2 = f2.read(&mut buf2)?;

        if n1 != n2 {
            return Ok(false);
        }
        if n1 == 0 {
            break;
        }
        if buf1[..n1] != buf2[..n2] {
            return Ok(false);
        }
    }

    Ok(true)
}

pub struct SafeExclusiveFile {
    pub file: std::fs::File,
    pub path: PathBuf,
    pub file_name: String,
    pub reused_existing: bool,
}

pub struct PublishedWorkspaceFile {
    pub path: PathBuf,
    pub file_name: String,
    pub reused_existing: bool,
}

/// Creates a hidden staging file in `target_dir` (e.g. `.axel_stage_<pid>_<nanos>.tmp`).
/// Axel's filter specifically skips files starting with `.` or ending in `.tmp`,
/// ensuring watchers and scans never index an incomplete partial file (A08).
pub fn create_workspace_staging_file(target_dir: &Path) -> Result<(std::fs::File, PathBuf), std::io::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();

    for attempt in 0..1000 {
        let staging_name = format!(".axel_stage_{pid}_{now}_{attempt}.tmp");
        let staging_path = target_dir.join(&staging_name);
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&staging_path)
        {
            Ok(file) => return Ok((file, staging_path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "Failed to allocate temporary staging file in workspace",
    ))
}

/// Publishes a fully-written staged file into `target_dir` atomically without replacement (A08).
/// If `expected_hash` is provided and an existing candidate matches through its single open handle
/// inside the workspace boundary, that candidate is reused and the staging file is discarded.
pub fn publish_staged_workspace_file(
    staging_path: &Path,
    target_dir: &Path,
    original_name: &str,
    expected_hash: Option<&str>,
    workspace: &Path,
) -> Result<PublishedWorkspaceFile, std::io::Error> {
    let p = Path::new(original_name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");

    for counter in 0..=10000 {
        let candidate_name = if counter == 0 {
            original_name.to_string()
        } else if ext.is_empty() {
            format!("{stem}_{counter}")
        } else {
            format!("{stem}_{counter}.{ext}")
        };

        let candidate_path = target_dir.join(&candidate_name);

        // Check if candidate exists (or symlink)
        if let Ok(_sym_meta) = std::fs::symlink_metadata(&candidate_path) {
            // Candidate already exists. If expected_hash is provided, verify candidate
            // through a single open file handle and boundary check (A08).
            if let Some(expected) = expected_hash {
                // Ensure candidate does not escape workspace via symlink
                let resolved = crate::config::resolve_workspace_path(&candidate_path);
                if is_path_inside_workspace(&resolved, workspace) {
                    if let Ok(mut cand_file) = std::fs::File::open(&resolved) {
                        if let Ok(meta) = cand_file.metadata() {
                            if meta.is_file() {
                                use sha2::{Digest, Sha256};
                                use std::io::Read;
                                let mut hasher = Sha256::new();
                                let mut buf = [0u8; 65536];
                                let mut ok = true;
                                loop {
                                    match cand_file.read(&mut buf) {
                                        Ok(0) => break,
                                        Ok(n) => hasher.update(&buf[..n]),
                                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                                        Err(_) => {
                                            ok = false;
                                            break;
                                        }
                                    }
                                }
                                if ok {
                                    let cand_hash = format!("{:x}", hasher.finalize());
                                    if cand_hash == expected {
                                        // Candidate matches exactly! Discard staging file and reuse candidate.
                                        let _ = std::fs::remove_file(staging_path);
                                        return Ok(PublishedWorkspaceFile {
                                            path: candidate_path,
                                            file_name: candidate_name,
                                            reused_existing: true,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Collision or hash mismatch: try next candidate
            continue;
        }

        // Candidate does not exist. Atomically publish without replacement.
        // On Unix, hard_link fails with AlreadyExists if candidate_path appeared concurrently.
        #[cfg(unix)]
        {
            match std::fs::hard_link(staging_path, &candidate_path) {
                Ok(_) => {
                    let _ = std::fs::remove_file(staging_path);
                    return Ok(PublishedWorkspaceFile {
                        path: candidate_path,
                        file_name: candidate_name,
                        reused_existing: false,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    continue;
                }
                Err(_) => {
                    // Fall back to rename if hard link is unsupported (e.g. cross-filesystem)
                    if std::fs::rename(staging_path, &candidate_path).is_ok() {
                        return Ok(PublishedWorkspaceFile {
                            path: candidate_path,
                            file_name: candidate_name,
                            reused_existing: false,
                        });
                    }
                }
            }
        }

        #[cfg(not(unix))]
        {
            if std::fs::rename(staging_path, &candidate_path).is_ok() {
                return Ok(PublishedWorkspaceFile {
                    path: candidate_path,
                    file_name: candidate_name,
                    reused_existing: false,
                });
            }
        }
    }

    let _ = std::fs::remove_file(staging_path);
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("Exhausted candidate filenames for {original_name} in {}", target_dir.display()),
    ))
}

/// Publishes a file into `target_dir` atomically without replacement by opening the handle
/// with exclusive creation (`create_new(true)`) and retrying collisions (H04).
/// If `expected_hash` is provided and a candidate already exists with exact identical content,
/// it reuses that candidate safely.
pub fn create_exclusive_workspace_file(
    target_dir: &Path,
    original_name: &str,
    expected_hash: Option<&str>,
) -> Result<SafeExclusiveFile, std::io::Error> {
    let p = Path::new(original_name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");

    for counter in 0..=10000 {
        let candidate_name = if counter == 0 {
            original_name.to_string()
        } else if ext.is_empty() {
            format!("{stem}_{counter}")
        } else {
            format!("{stem}_{counter}.{ext}")
        };

        let candidate_path = target_dir.join(&candidate_name);

        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&candidate_path)
        {
            Ok(file) => {
                return Ok(SafeExclusiveFile {
                    file,
                    path: candidate_path,
                    file_name: candidate_name,
                    reused_existing: false,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if let Some(expected) = expected_hash {
                    if let Some(existing_hash) = DocumentIndex::compute_file_hash(&candidate_path) {
                        if existing_hash == expected {
                            if let Ok(file) = std::fs::File::open(&candidate_path) {
                                return Ok(SafeExclusiveFile {
                                    file,
                                    path: candidate_path,
                                    file_name: candidate_name,
                                    reused_existing: true,
                                });
                            }
                        }
                    }
                }
                // Collision with another file or concurrent writer; retry with next candidate name
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("Exhausted candidate filenames for {original_name} in {}", target_dir.display()),
    ))
}

/// Finds a non-colliding destination filename in `target_dir` for `original_name`.
/// If `original_name` does not exist in `target_dir`, returns `(target_dir.join(original_name), original_name)`.
/// If it exists (or is a symlink), generates `<stem>_1.<ext>`, `<stem>_2.<ext>`, etc.,
/// until an unused filename is found.
pub fn resolve_collision_safe_path(target_dir: &Path, original_name: &str) -> (PathBuf, String) {
    let p = Path::new(original_name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");

    let initial_dest = target_dir.join(original_name);
    if std::fs::symlink_metadata(&initial_dest).is_err() {
        return (initial_dest, original_name.to_string());
    }

    let mut counter = 1;
    loop {
        let candidate_name = if ext.is_empty() {
            format!("{stem}_{counter}")
        } else {
            format!("{stem}_{counter}.{ext}")
        };
        let candidate_path = target_dir.join(&candidate_name);
        if std::fs::symlink_metadata(&candidate_path).is_err() {
            return (candidate_path, candidate_name);
        }
        counter += 1;
    }
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

        let target_path = if copy_into_workspace {
            let file_name = src_path
                .file_name()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty() && *s != "." && *s != "..")
                .ok_or_else(|| "Invalid filename".to_string())?;

            let default_dest = workspace_resolved.join(file_name);
            let same_file = match (
                std::fs::canonicalize(src_path),
                std::fs::canonicalize(&default_dest),
            ) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            };

            if same_file {
                // File is already in the workspace at the destination
                default_dest
            } else {
                let src_hash = DocumentIndex::compute_file_hash(src_path);
                // A08: Stage in hidden file first, sync completely, then publish atomically without replacement
                let (mut staging_file, staging_path) = create_workspace_staging_file(&workspace_resolved)
                    .map_err(|e| format!("Failed to create staging file: {e}"))?;
                let mut src_file = std::fs::File::open(src_path)
                    .map_err(|e| format!("Failed to open source file {}: {e}", src_path.display()))?;
                std::io::copy(&mut src_file, &mut staging_file)
                    .map_err(|e| {
                        let _ = std::fs::remove_file(&staging_path);
                        format!("Failed to copy file to staging {}: {e}", staging_path.display())
                    })?;
                staging_file.sync_all()
                    .map_err(|e| {
                        let _ = std::fs::remove_file(&staging_path);
                        format!("Failed to sync staging file: {e}")
                    })?;
                drop(staging_file);

                let published = publish_staged_workspace_file(
                    &staging_path,
                    &workspace_resolved,
                    file_name,
                    src_hash.as_deref(),
                    &workspace_resolved,
                ).map_err(|e| format!("Failed to publish file: {e}"))?;

                published.path
            }
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
    let resolved_workspace = crate::config::resolve_workspace_path(workspace_folder);
    let resolved_path = if let Ok(c) = std::fs::canonicalize(path) {
        c
    } else {
        // If the path does not exist on disk (e.g. vanished/removed file or rename source),
        // try canonicalizing its deepest existing parent to resolve symlinks in the path prefix.
        let mut ancestors_stack = Vec::new();
        let mut curr = path;
        let mut found_parent = None;
        while let Some(parent) = curr.parent() {
            if let Some(file_name) = curr.file_name() {
                ancestors_stack.push(file_name);
            }
            if let Ok(canon_parent) = std::fs::canonicalize(parent) {
                let mut resolved = canon_parent;
                for comp in ancestors_stack.into_iter().rev() {
                    resolved.push(comp);
                }
                found_parent = Some(resolved);
                break;
            }
            curr = parent;
        }
        found_parent.unwrap_or_else(|| crate::config::resolve_workspace_path(path))
    };

    resolved_path.starts_with(&resolved_workspace)
}

pub fn trash_file_safely(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("File does not exist on disk".to_string());
    }
    trash::delete(path).map_err(|e| format!("Failed to move file to trash: {e}"))
}

pub fn delete_document_from_index(
    hash: &str,
    delete_file: bool,
    permanent: bool,
    config: &Config,
    index: &mut DocumentIndex,
) -> Result<DeleteOutcome, String> {
    let item = index
        .find_by_hash(hash)
        .cloned()
        .ok_or_else(|| "Document not found".to_string())?;

    let mut file_removed_from_disk = false;
    let mut disk_removal_method = None;

    let target_path_buf = PathBuf::from(&item.path);
    let target_path = target_path_buf.as_path();

    if delete_file {
        if !is_path_inside_workspace(target_path, &config.input_folder) {
            return Err("Target file is outside workspace boundaries; disk deletion aborted.".to_string());
        }
        if !target_path.exists() {
            return Err("Target file does not exist on disk.".to_string());
        }

        // Verify current disk contents still match indexed hash before deletion
        match DocumentIndex::compute_file_hash(target_path) {
            Some(ref current_hash) if current_hash == hash => {}
            Some(_) => {
                return Err("Security abort: File contents on disk have been modified since indexing. Refusing to delete to prevent data loss.".to_string());
            }
            None => {
                return Err("Security abort: Could not read target file to verify hash.".to_string());
            }
        }

        // Pre-check verified live aliases remaining in physical_paths
        let mut remaining_live_copies = Vec::new();
        let target_resolved = resolve_workspace_path(target_path);
        for alias in &item.physical_paths {
            let apath = resolve_workspace_path(Path::new(alias));
            if apath != target_resolved
                && is_path_inside_workspace(&apath, &config.input_folder)
                && apath.exists()
            {
                if let Some(chash) = DocumentIndex::compute_file_hash(&apath) {
                    if chash == *hash {
                        remaining_live_copies.push(alias.clone());
                    }
                }
            }
        }

        // H09: Recoverable deletion workflow:
        // Stage removal by moving target file to a temporary staging name in the same parent directory
        static DEL_STAGE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let count = DEL_STAGE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
        let stage_del_name = format!(".tmp_del_{}_{}_{}.tmp", std::process::id(), now, count);
        let stage_del_path = parent.join(stage_del_name);

        std::fs::rename(target_path, &stage_del_path)
            .map_err(|e| format!("Failed to stage file for deletion: {e}"))?;

        // Attempt deletion from staging
        if permanent {
            if let Err(e) = std::fs::remove_file(&stage_del_path) {
                let rollback_res = std::fs::rename(&stage_del_path, target_path);
                let rollback_msg = match rollback_res {
                    Ok(_) => "Staged file successfully restored to original path.".to_string(),
                    Err(re) => format!(
                        "CRITICAL: Failed to restore staged file to original path: {re}. File remains at {}",
                        stage_del_path.display()
                    ),
                };
                return Err(format!("Failed to permanently remove staged file: {e}. {rollback_msg}"));
            }
            file_removed_from_disk = true;
            disk_removal_method = Some("permanent".to_string());
        } else {
            match trash::delete(&stage_del_path) {
                Ok(_) => {
                    file_removed_from_disk = true;
                    disk_removal_method = Some("trash".to_string());
                }
                Err(trash_err) => {
                    // A01: NEVER permanently remove on trash failure without explicit consent!
                    // Restore original file and surface any rollback failure.
                    let rollback_res = std::fs::rename(&stage_del_path, target_path);
                    let rollback_msg = match rollback_res {
                        Ok(_) => "File successfully restored to original path.".to_string(),
                        Err(re) => format!(
                            "CRITICAL: Failed to restore file from staging: {re}. File remains at {}",
                            stage_del_path.display()
                        ),
                    };
                    return Err(format!(
                        "Failed to move file to OS trash: {trash_err}. Refusing permanent deletion without explicit consent. {rollback_msg}"
                    ));
                }
            }
        }

        // File removed from disk. Now perform index update with verified alias promotion (A05)
        let verified_promoted = remaining_live_copies.first().map(|s| s.as_str());
        let index_updated = if !remaining_live_copies.is_empty() {
            index.remove_physical_copy_with_promotion(hash, &item.path, verified_promoted)
        } else {
            index.remove_by_hash(hash).is_some()
        };

        if !index_updated {
            return Err("Document not found in index".to_string());
        }

        // Save index to disk
        if let Err(save_err) = index.save() {
            // A05: Roll back live in-memory index on save failure!
            index.restore_item(item.clone());
            return Err(format!("Failed to save index after file deletion: {save_err}"));
        }
    } else {
        // Only removing from index without deleting file on disk
        let removed = index
            .remove_by_hash(hash)
            .ok_or_else(|| "Document not found".to_string())?;
        if let Err(save_err) = index.save() {
            index.restore_item(removed);
            return Err(format!("Failed to save index: {save_err}"));
        }
    }

    let final_item = index.find_by_hash(hash);
    let (final_file, index_deleted) = match final_item {
        Some(fi) => (fi.file.clone(), false),
        None => (item.file.clone(), true),
    };

    Ok(DeleteOutcome {
        deleted: true,
        index_deleted,
        file_removed_from_disk,
        disk_removal_method,
        hash: item.hash,
        file: final_file,
        error: None,
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
