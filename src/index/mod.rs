use crate::config::resolve_workspace_path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub mod virtual_tree;
pub use virtual_tree::{
    build_virtual_tree, compute_virtual_paths, DuplicateCluster, VirtualFolder,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    #[serde(default)]
    pub virtual_paths: Vec<String>,
    #[serde(default)]
    pub physical_paths: Vec<String>,
    #[serde(default)]
    pub is_duplicate: bool,
}

impl IndexItem {
    pub fn new(
        file: String,
        category: String,
        keywords: Vec<String>,
        symbols: Vec<String>,
        path: String,
        hash: String,
        virtual_paths: Vec<String>,
    ) -> Self {
        let ext = Path::new(&file)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let computed_virtual_paths = compute_virtual_paths(&file, &category, &virtual_paths, ext);
        let physical_paths = vec![path.clone()];
        Self {
            file,
            category,
            keywords,
            symbols,
            path,
            hash,
            indexed_at: Some(chrono::Utc::now().to_rfc3339()),
            virtual_paths: computed_virtual_paths,
            physical_paths,
            is_duplicate: false,
        }
    }
}

impl AsRef<IndexItem> for IndexItem {
    fn as_ref(&self) -> &IndexItem {
        self
    }
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

    pub fn items(&self) -> &[IndexItem] {
        &self.items
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

    pub fn upsert(&mut self, mut item: IndexItem) {
        let ext = Path::new(&item.file)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if item.virtual_paths.is_empty() {
            item.virtual_paths = compute_virtual_paths(&item.file, &item.category, &[], ext);
        }

        if item.physical_paths.is_empty() {
            item.physical_paths.push(item.path.clone());
        }

        let item_resolved = resolve_workspace_path(Path::new(&item.path));
        if let Some(pos) = self.items.iter().position(|i| i.hash == item.hash) {
            let existing = &mut self.items[pos];
            let new_path = item.path.clone();

            // Track duplicate physical download locations
            if !existing.physical_paths.contains(&new_path) {
                existing.physical_paths.push(new_path);
                existing.is_duplicate = true;
            }

            // Merge virtual paths
            for vp in item.virtual_paths {
                if !existing.virtual_paths.contains(&vp) {
                    existing.virtual_paths.push(vp);
                }
            }

            // Update category and keywords if new item has richer metadata
            if item.keywords.len() > existing.keywords.len() {
                existing.keywords = item.keywords;
            }
            if !item.symbols.is_empty() && existing.symbols.is_empty() {
                existing.symbols = item.symbols;
            }
        } else if let Some(pos) = self.items.iter().position(|i| {
            i.path == item.path || resolve_workspace_path(Path::new(&i.path)) == item_resolved
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

    pub fn remove_physical_copy(&mut self, hash: &str, path_to_remove: &str) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i.hash == hash) {
            let item = &mut self.items[pos];
            let before = item.physical_paths.len();
            item.physical_paths.retain(|p| p != path_to_remove);
            if item.physical_paths.len() < before {
                item.is_duplicate = item.physical_paths.len() > 1;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    pub fn items_mut(&mut self) -> &mut Vec<IndexItem> {
        self.dirty = true;
        &mut self.items
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
        let mut filter_dup = false;
        let mut filter_symbols = Vec::new();
        let mut filter_keywords = Vec::new();
        let mut filter_cats = Vec::new();
        let mut plain_terms = Vec::new();

        for term in &query_terms {
            if *term == "is:dup" || *term == "dup:" || *term == "•" {
                filter_dup = true;
            } else if let Some(cat) = term.strip_prefix("@cat:") {
                if !cat.is_empty() {
                    filter_cats.push(cat);
                }
            } else if let Some(sym) = term
                .strip_prefix("@fn:")
                .or_else(|| term.strip_prefix("@var:"))
                .or_else(|| term.strip_prefix('@'))
            {
                if !sym.is_empty() {
                    filter_symbols.push(sym);
                }
            } else if let Some(kw) = term.strip_prefix('#') {
                if !kw.is_empty() {
                    filter_keywords.push(kw);
                }
            } else {
                plain_terms.push(*term);
            }
        }

        let mut exact_matches = Vec::new();
        let mut fuzzy_matches = Vec::new();

        for item in &self.items {
            let cat_lower = item.category.to_lowercase();
            let file_lower = item.file.to_lowercase();
            let path_lower = item.path.to_lowercase();
            let kws_lower: Vec<String> = item.keywords.iter().map(|k| k.to_lowercase()).collect();
            let symbols_lower: Vec<String> = item.symbols.iter().map(|s| s.to_lowercase()).collect();

            // 1. Strict filter checks if specific prefixes were specified
            if filter_dup && (!item.is_duplicate && item.physical_paths.len() <= 1) {
                continue;
            }
            if !filter_cats.is_empty() && !filter_cats.iter().any(|c| cat_lower.contains(c)) {
                continue;
            }
            if !filter_symbols.is_empty()
                && !filter_symbols
                    .iter()
                    .any(|s| symbols_lower.iter().any(|sym| sym.contains(s)))
            {
                continue;
            }
            if !filter_keywords.is_empty()
                && !filter_keywords
                    .iter()
                    .any(|k| kws_lower.iter().any(|kw| kw.contains(k)))
            {
                continue;
            }

            let mut score: f64 = 0.0;
            let mut match_type = "partial";

            // If query consists entirely of valid prefix filters that passed
            if plain_terms.is_empty()
                && (filter_dup
                    || !filter_symbols.is_empty()
                    || !filter_keywords.is_empty()
                    || !filter_cats.is_empty())
            {
                score = 0.95;
                match_type = "filter";
            }

            // Keyword and symbol boosts
            if !filter_symbols.is_empty() {
                if filter_symbols.iter().any(|s| symbols_lower.iter().any(|sym| sym == s)) {
                    score = score.max(0.98);
                    match_type = "exact_symbol";
                } else {
                    score = score.max(0.92);
                }
            }
            if !filter_keywords.is_empty() {
                if filter_keywords.iter().any(|k| kws_lower.iter().any(|kw| kw == k)) {
                    score = score.max(0.97);
                    match_type = "exact_kw";
                } else {
                    score = score.max(0.91);
                }
            }

            // Plain term matches
            if !plain_terms.is_empty() {
                let joined_plain = plain_terms.join(" ");
                if cat_lower.contains(&joined_plain) {
                    score = score.max(1.0);
                    match_type = "exact";
                }
                if file_lower.contains(&joined_plain) {
                    score = score.max(0.95);
                    match_type = "exact";
                }
                if symbols_lower.iter().any(|s| s == &joined_plain) {
                    score = score.max(0.96);
                    match_type = "exact";
                } else if symbols_lower.iter().any(|s| s.contains(&joined_plain)) {
                    score = score.max(0.91);
                    match_type = "exact";
                }
                if kws_lower.iter().any(|k| k.contains(&joined_plain)) {
                    score = score.max(0.9);
                    match_type = "exact";
                }
                let is_path_query = joined_plain.contains('/') || joined_plain.contains('\\');
                if (is_path_query && path_lower.contains(&joined_plain))
                    || (!is_path_query
                        && joined_plain.len() >= 4
                        && Path::new(&item.path)
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_lowercase())
                            .as_deref()
                            == Some(&joined_plain))
                {
                    score = score.max(0.85);
                }

                let term_hits = plain_terms
                    .iter()
                    .filter(|&t| {
                        cat_lower.contains(t)
                            || file_lower.contains(t)
                            || kws_lower.iter().any(|k| k.contains(t))
                            || symbols_lower.iter().any(|s| s.contains(t))
                    })
                    .count();

                if term_hits > 0 {
                    let term_score = 0.7 + (term_hits as f64 / plain_terms.len() as f64) * 0.25;
                    score = score.max(term_score);
                }
            }

            if score > 0.0 {
                exact_matches.push(SearchResult {
                    item: item.clone(),
                    score,
                    match_type: match_type.to_string(),
                });
            } else if !plain_terms.is_empty() {
                let joined_plain = plain_terms.join(" ");
                let best_sim = kws_lower
                    .iter()
                    .map(|k| strsim::jaro_winkler(&joined_plain, k))
                    .chain(symbols_lower.iter().map(|s| strsim::jaro_winkler(&joined_plain, s)))
                    .chain(std::iter::once(strsim::jaro_winkler(&joined_plain, &file_lower)))
                    .chain(std::iter::once(strsim::jaro_winkler(&joined_plain, &cat_lower)))
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
                virtual_paths: Vec::new(),
                physical_paths: vec!["/home/maddy/Dl/10.py".to_string()],
                is_duplicate: false,
            },
            IndexItem {
                file: "1.py".to_string(),
                category: "Code/Math".to_string(),
                keywords: vec!["calculator".to_string(), "arithmetic".to_string(), "math".to_string()],
                symbols: vec!["add".to_string(), "subtract".to_string(), "multiply".to_string()],
                path: "/home/maddy/Dl/1.py".to_string(),
                hash: "h1".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/home/maddy/Dl/1.py".to_string()],
                is_duplicate: false,
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

    #[test]
    fn test_duplicate_download_tracking_and_virtual_pointers() {
        let mut index = DocumentIndex::from_items(Vec::new());

        // File downloaded first time as notes.pdf
        let item1 = IndexItem {
            file: "calculus_notes.pdf".to_string(),
            category: "Academic".to_string(),
            keywords: vec!["calculus".to_string(), "derivatives".to_string(), "notes".to_string()],
            symbols: Vec::new(),
            path: "/home/maddy/Downloads/calculus_notes.pdf".to_string(),
            hash: "same_sha256_hash_123".to_string(),
            indexed_at: None,
            virtual_paths: vec!["Study Materials/Calculus/Notes".to_string()],
            physical_paths: Vec::new(),
            is_duplicate: false,
        };

        index.upsert(item1);
        assert_eq!(index.count(), 1);
        assert!(!index.all()[0].is_duplicate);
        assert_eq!(index.all()[0].physical_paths.len(), 1);

        // Same file downloaded second time as calculus_notes (1).pdf
        let item2 = IndexItem {
            file: "calculus_notes (1).pdf".to_string(),
            category: "Academic".to_string(),
            keywords: vec!["calculus".to_string()],
            symbols: Vec::new(),
            path: "/home/maddy/Downloads/calculus_notes (1).pdf".to_string(),
            hash: "same_sha256_hash_123".to_string(),
            indexed_at: None,
            virtual_paths: Vec::new(),
            physical_paths: Vec::new(),
            is_duplicate: false,
        };

        index.upsert(item2);

        // Count must still be 1 (unified, no duplicate clutter!)
        assert_eq!(index.count(), 1);
        let canonical = &index.all()[0];

        // Must be marked as duplicate with 2 physical paths tracked
        assert!(canonical.is_duplicate);
        assert_eq!(canonical.physical_paths.len(), 2);
        assert!(canonical.physical_paths.contains(&"/home/maddy/Downloads/calculus_notes.pdf".to_string()));
        assert!(canonical.physical_paths.contains(&"/home/maddy/Downloads/calculus_notes (1).pdf".to_string()));

        // Virtual pointers auto-computed
        assert!(!canonical.virtual_paths.is_empty());
        assert!(canonical.virtual_paths[0].starts_with("Study Materials/Calculus/"));
    }

    #[test]
    fn test_structured_query_grammar() {
        let items = vec![
            IndexItem {
                file: "10.py".to_string(),
                category: "Code/Vision".to_string(),
                keywords: vec!["computer vision".to_string(), "opencv".to_string()],
                symbols: vec!["sobel_x".to_string(), "sobel_y".to_string(), "image_path".to_string()],
                path: "/home/maddy/Dl/10.py".to_string(),
                hash: "h10".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/home/maddy/Dl/10.py".to_string()],
                is_duplicate: false,
            },
            IndexItem {
                file: "1.py".to_string(),
                category: "Code/Math".to_string(),
                keywords: vec!["calculator".to_string(), "arithmetic".to_string()],
                symbols: vec!["add".to_string(), "subtract".to_string()],
                path: "/home/maddy/Dl/1.py".to_string(),
                hash: "h1".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/home/maddy/Dl/1.py".to_string(), "/home/maddy/Downloads/1.py".to_string()],
                is_duplicate: true,
            },
        ];

        let index = DocumentIndex::from_items(items);

        // 1. Filter by duplicate badge/tag
        let dups = index.search("is:dup", 10);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].item.file, "1.py");

        // 2. Filter by AST function symbol
        let sym_search = index.search("@fn:sobel_x", 10);
        assert_eq!(sym_search.len(), 1);
        assert_eq!(sym_search[0].item.file, "10.py");

        // 3. Filter by keyword chip
        let kw_search = index.search("#opencv", 10);
        assert_eq!(kw_search.len(), 1);
        assert_eq!(kw_search[0].item.file, "10.py");

        // 4. Combined query: category filter + plain term
        let cat_search = index.search("@cat:vision sobel", 10);
        assert_eq!(cat_search.len(), 1);
        assert_eq!(cat_search[0].item.file, "10.py");
    }
}
