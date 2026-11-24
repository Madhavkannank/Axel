use crate::index::IndexItem;
use std::path::Path;
use std::time::SystemTime;

pub const XATTR_CATEGORY: &str = "user.axel.category";
pub const XATTR_KEYWORDS: &str = "user.axel.keywords";
pub const XATTR_VPATHS: &str = "user.axel.vpaths";
pub const XATTR_HASH: &str = "user.axel.hash";
pub const XATTR_MTIME: &str = "user.axel.mtime";
pub const XATTR_VERSION: &str = "user.axel.version";

pub struct FileMetadataTag;

impl FileMetadataTag {
    /// Attempt to read cached tag metadata directly from the file's extended attributes.
    /// Returns Some(IndexItem) if valid dynamic tags exist and file mtime matches,
    /// allowing instant sub-millisecond skips without disk reads or re-analysis.
    pub fn read_from_file(path: &Path) -> Option<IndexItem> {
        if !path.exists() || !path.is_file() {
            return None;
        }

        // 1. Check if category exists
        let cat_bytes = xattr::get(path, XATTR_CATEGORY).ok()??;
        let category = String::from_utf8(cat_bytes).ok()?;
        let trimmed_cat = category.trim();

        // Never accept empty or legacy "General" tag
        if trimmed_cat.is_empty()
            || trimmed_cat.eq_ignore_ascii_case("general")
            || trimmed_cat.eq_ignore_ascii_case("documents/general")
        {
            return None;
        }

        // 2. Validate mtime freshness against current filesystem state
        let fs_metadata = std::fs::metadata(path).ok()?;
        let current_mtime = fs_metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(mtime_bytes) = xattr::get(path, XATTR_MTIME).ok().flatten() {
            if let Ok(mtime_str) = std::str::from_utf8(&mtime_bytes) {
                if let Ok(recorded_mtime) = mtime_str.trim().parse::<u64>() {
                    if recorded_mtime != current_mtime && current_mtime > 0 {
                        // File was modified on disk since tagging; must re-index
                        return None;
                    }
                }
            }
        }

        // 3. Read keywords from xattr
        let keywords: Vec<String> = if let Some(kw_bytes) = xattr::get(path, XATTR_KEYWORDS).ok().flatten() {
            if let Ok(kw_str) = std::str::from_utf8(&kw_bytes) {
                if let Ok(parsed) = serde_json::from_str::<Vec<String>>(kw_str) {
                    parsed
                } else {
                    kw_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // 4. Read virtual paths from xattr
        let virtual_paths: Vec<String> = if let Some(vp_bytes) = xattr::get(path, XATTR_VPATHS).ok().flatten() {
            if let Ok(vp_str) = std::str::from_utf8(&vp_bytes) {
                if let Ok(parsed) = serde_json::from_str::<Vec<String>>(vp_str) {
                    parsed
                } else {
                    vp_str
                        .split('|')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // 5. Read file hash from xattr
        let hash = if let Some(h_bytes) = xattr::get(path, XATTR_HASH).ok().flatten() {
            String::from_utf8(h_bytes).unwrap_or_default()
        } else {
            String::new()
        };

        if hash.is_empty() {
            return None;
        }

        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("document")
            .to_string();

        Some(IndexItem {
            file: file_name,
            category: trimmed_cat.to_string(),
            keywords,
            symbols: Vec::new(),
            path: path.to_string_lossy().to_string(),
            hash,
            indexed_at: Some(chrono::Utc::now().to_rfc3339()),
            virtual_paths,
            physical_paths: vec![path.to_string_lossy().to_string()],
            is_duplicate: false,
        })
    }

    /// Write semantic tags, keywords, virtual paths, and compound hash directly
    /// to the file's extended attributes so the file permanently carries its own metadata.
    pub fn write_to_file(
        path: &Path,
        category: &str,
        keywords: &[String],
        virtual_paths: &[String],
        hash: &str,
    ) -> Result<(), std::io::Error> {
        if !path.exists() || !path.is_file() {
            return Ok(());
        }

        let trimmed_cat = category.trim();
        // Never write legacy "General" to file metadata
        if trimmed_cat.is_empty()
            || trimmed_cat.eq_ignore_ascii_case("general")
            || trimmed_cat.eq_ignore_ascii_case("documents/general")
        {
            return Ok(());
        }

        // Store category
        let _ = xattr::set(path, XATTR_CATEGORY, trimmed_cat.as_bytes());

        // Store keywords as JSON array
        if let Ok(kw_json) = serde_json::to_string(keywords) {
            let _ = xattr::set(path, XATTR_KEYWORDS, kw_json.as_bytes());
        }

        // Store virtual paths as JSON array
        if let Ok(vp_json) = serde_json::to_string(virtual_paths) {
            let _ = xattr::set(path, XATTR_VPATHS, vp_json.as_bytes());
        }

        // Store hash
        if !hash.is_empty() {
            let _ = xattr::set(path, XATTR_HASH, hash.as_bytes());
        }

        // Store mtime
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(d) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                    let mtime_str = d.as_secs().to_string();
                    let _ = xattr::set(path, XATTR_MTIME, mtime_str.as_bytes());
                }
            }
        }

        // Version 2 schema
        let _ = xattr::set(path, XATTR_VERSION, b"2");

        Ok(())
    }

    /// Remove any cached extended attributes from the file
    pub fn clear_file_tags(path: &Path) {
        let _ = xattr::remove(path, XATTR_CATEGORY);
        let _ = xattr::remove(path, XATTR_KEYWORDS);
        let _ = xattr::remove(path, XATTR_VPATHS);
        let _ = xattr::remove(path, XATTR_HASH);
        let _ = xattr::remove(path, XATTR_MTIME);
        let _ = xattr::remove(path, XATTR_VERSION);
    }
}
