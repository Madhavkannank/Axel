use crate::config::resolve_workspace_path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexItem {
    pub file: String,
    pub category: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
    pub path: String,
    pub hash: String,
    #[serde(default)]
    pub indexed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub item: IndexItem,
    pub score: f64,
    pub match_type: String, // "exact", "partial", "suggested"
}

pub struct DocumentIndex {
    file_path: PathBuf,
    items: Vec<IndexItem>,
    dirty: bool,
}

impl DocumentIndex {
    pub fn load<P: AsRef<Path>>(index_path: P) -> Self {
        let file_path = index_path.as_ref().to_path_buf();
        let items = if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                serde_json::from_str(&content).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Self {
            file_path,
            items,
            dirty: false,
        }
    }

    pub fn from_items(items: Vec<IndexItem>) -> Self {
        Self {
            file_path: PathBuf::new(),
            items,
            dirty: false,
        }
    }

    pub fn compute_file_hash<P: AsRef<Path>>(path: P) -> Option<String> {
        let mut file = File::open(path).ok()?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).ok()?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Some(format!("{:x}", hasher.finalize()))
    }

    pub fn find_by_hash(&self, hash: &str) -> Option<&IndexItem> {
        self.items.iter().find(|item| item.hash == hash)
    }

    #[allow(dead_code)]
    pub fn find_by_path(&self, path: &str) -> Option<&IndexItem> {
        let target = resolve_workspace_path(Path::new(path));
        self.items
            .iter()
            .find(|item| item.path == path || resolve_workspace_path(Path::new(&item.path)) == target)
    }

    pub fn upsert(&mut self, item: IndexItem) {
        let item_resolved = resolve_workspace_path(Path::new(&item.path));
        if let Some(pos) = self.items.iter().position(|i| {
            i.hash == item.hash
                || i.path == item.path
                || resolve_workspace_path(Path::new(&i.path)) == item_resolved
        }) {
            self.items[pos] = item;
        } else {
            self.items.push(item);
        }
        self.dirty = true;
    }

    pub fn remove_by_hash(&mut self, hash: &str) -> Option<IndexItem> {
        if let Some(pos) = self.items.iter().position(|i| i.hash == hash) {
            let removed = self.items.remove(pos);
            self.dirty = true;
            Some(removed)
        } else {
            None
        }
    }

    /// Remove entries whose files no longer exist on disk or live outside `workspace`.
    pub fn prune_missing_in_workspace(&mut self, workspace: &Path) -> usize {
        let base = resolve_workspace_path(workspace);
        let before = self.items.len();
        self.items.retain(|item| {
            let p = resolve_workspace_path(Path::new(&item.path));
            p.exists() && p.is_file() && p.starts_with(&base)
        });
        let removed = before.saturating_sub(self.items.len());
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if !self.dirty {
            return Ok(());
        }

        if let Some(parent) = self.file_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let json = serde_json::to_string_pretty(&self.items)
            .map_err(std::io::Error::other)?;

        let temp_path = format!("{}.tmp", self.file_path.display());
        std::fs::write(&temp_path, json)?;
        std::fs::rename(temp_path, &self.file_path)?;

        self.dirty = false;
        Ok(())
    }

    pub fn all(&self) -> &[IndexItem] {
        &self.items
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_clean = query.trim().to_lowercase();
        if query_clean.is_empty() {
            return Vec::new();
        }

        let query_terms: Vec<&str> = query_clean.split_whitespace().collect();
        let mut exact_matches = Vec::new();
        let mut fuzzy_matches = Vec::new();

        for item in &self.items {
            let cat_lower = item.category.to_lowercase();
            let file_lower = item.file.to_lowercase();
            let path_lower = item.path.to_lowercase();
            let kws_lower: Vec<String> = item.keywords.iter().map(|k| k.to_lowercase()).collect();
            let symbols_lower: Vec<String> = item.symbols.iter().map(|s| s.to_lowercase()).collect();

            let mut score: f64 = 0.0;
            let mut match_type = "partial";

            if cat_lower.contains(&query_clean) {
                score = score.max(1.0);
                match_type = "exact";
            }
            if file_lower.contains(&query_clean) {
                score = score.max(0.95);
                match_type = "exact";
            }
            if symbols_lower.iter().any(|s| s == &query_clean) {
                score = score.max(0.96);
                match_type = "exact";
            } else if symbols_lower.iter().any(|s| s.contains(&query_clean)) {
                score = score.max(0.91);
                match_type = "exact";
            }
            if kws_lower.iter().any(|k| k.contains(&query_clean)) {
                score = score.max(0.9);
                match_type = "exact";
            }
            let is_path_query = query_clean.contains('/') || query_clean.contains('\\');
            if (is_path_query && path_lower.contains(&query_clean))
                || (!is_path_query
                    && query_clean.len() >= 4
                    && Path::new(&item.path)
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_lowercase())
                        .as_deref()
                        == Some(&query_clean))
            {
                score = score.max(0.85);
            }

            let term_hits = query_terms
                .iter()
                .filter(|&t| {
                    cat_lower.contains(t)
                        || file_lower.contains(t)
                        || kws_lower.iter().any(|k| k.contains(t))
                        || symbols_lower.iter().any(|s| s.contains(t))
                })
                .count();

            if term_hits > 0 {
                let term_score = 0.7 + (term_hits as f64 / query_terms.len() as f64) * 0.25;
                score = score.max(term_score);
            }

            if score > 0.0 {
                exact_matches.push(SearchResult {
                    item: item.clone(),
                    score,
                    match_type: match_type.to_string(),
                });
            } else {
                let best_sim = kws_lower
                    .iter()
                    .map(|k| strsim::jaro_winkler(&query_clean, k))
                    .chain(symbols_lower.iter().map(|s| strsim::jaro_winkler(&query_clean, s)))
                    .chain(std::iter::once(strsim::jaro_winkler(&query_clean, &file_lower)))
                    .chain(std::iter::once(strsim::jaro_winkler(&query_clean, &cat_lower)))
                    .fold(0.0f64, |acc, v| acc.max(v));

                if best_sim >= 0.70 {
                    fuzzy_matches.push(SearchResult {
                        item: item.clone(),
                        score: best_sim * 0.7,
                        match_type: "suggested".to_string(),
                    });
                }
            }
        }

        let mut all_results = Vec::new();
        all_results.extend(exact_matches);
        all_results.extend(fuzzy_matches);

        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for r in all_results {
            if seen.insert(r.item.hash.clone()) {
                deduped.push(r);
            }
        }

        deduped.truncate(limit);
        deduped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_by_variable_or_function_symbol() {
        let items = vec![
            IndexItem {
                file: "10.py".to_string(),
                category: "Code/Vision".to_string(),
                keywords: vec!["computer vision".to_string(), "opencv".to_string()],
                symbols: vec!["sobel_x".to_string(), "sobel_y".to_string(), "image_path".to_string()],
                path: "/home/maddy/Dl/10.py".to_string(),
                hash: "h10".to_string(),
                indexed_at: None,
            },
            IndexItem {
                file: "1.py".to_string(),
                category: "Code/Math".to_string(),
                keywords: vec!["calculator".to_string(), "arithmetic".to_string(), "math".to_string()],
                symbols: vec!["add".to_string(), "subtract".to_string(), "multiply".to_string()],
                path: "/home/maddy/Dl/1.py".to_string(),
                hash: "h1".to_string(),
                indexed_at: None,
            },
        ];

        let index = DocumentIndex::from_items(items);

        // 1. Search by exact variable name in 10.py
        let res_var = index.search("sobel_x", 5);
        assert_eq!(res_var.len(), 1);
        assert_eq!(res_var[0].item.file, "10.py");

        // 2. Search by concept keyword "calculator" in 1.py
        let res_concept = index.search("calculator", 5);
        assert_eq!(res_concept.len(), 1);
        assert_eq!(res_concept[0].item.file, "1.py");

        // 3. Search by function name "add" in 1.py
        let res_func = index.search("add", 5);
        assert_eq!(res_func.len(), 1);
        assert_eq!(res_func[0].item.file, "1.py");
    }
}
