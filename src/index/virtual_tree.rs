use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A Virtual Directory node representing a smart, non-duplicating semantic folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualFolder {
    pub name: String,
    pub path: String,
    pub subfolders: BTreeMap<String, VirtualFolder>,
    /// Hashes of items that have a virtual pointer inside this folder
    pub item_hashes: Vec<String>,
}

impl VirtualFolder {
    pub fn new(name: &str, path: &str) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            subfolders: BTreeMap::new(),
            item_hashes: Vec::new(),
        }
    }

    /// Insert an item pointer at a given virtual path segments, e.g. ["Study Materials", "Calculus", "Notes"]
    pub fn insert_pointer(&mut self, segments: &[&str], item_hash: &str) {
        if segments.is_empty() {
            if !self.item_hashes.contains(&item_hash.to_string()) {
                self.item_hashes.push(item_hash.to_string());
            }
            return;
        }

        let first = segments[0];
        let remaining = &segments[1..];
        let sub_path = if self.path.is_empty() {
            first.to_string()
        } else {
            format!("{}/{}", self.path, first)
        };

        let child = self
            .subfolders
            .entry(first.to_string())
            .or_insert_with(|| VirtualFolder::new(first, &sub_path));

        child.insert_pointer(remaining, item_hash);
    }

    /// Total count of unique item pointers contained in this folder and all subfolders.
    pub fn total_item_count(&self) -> usize {
        let mut set = std::collections::HashSet::new();
        self.collect_hashes(&mut set);
        set.len()
    }

    fn collect_hashes(&self, set: &mut std::collections::HashSet<String>) {
        for h in &self.item_hashes {
            set.insert(h.clone());
        }
        for sub in self.subfolders.values() {
            sub.collect_hashes(set);
        }
    }
}

/// Information about duplicates across the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCluster {
    pub hash: String,
    pub canonical_file: String,
    pub physical_paths: Vec<String>,
    pub copy_count: usize,
}

/// Compute dynamic virtual paths (multiple virtual pointers) for a document.
/// Enables a single physical file to appear in multiple logical categories without copying.
/// Pointers are generated dynamically by the LLM (or supplied taxonomy) without hardcoded rules.
pub fn compute_virtual_paths(
    file_name: &str,
    category: &str,
    suggested_paths: &[String],
    _ext: &str,
) -> Vec<String> {
    let mut virtual_paths = Vec::new();

    // 1. Ingest dynamic LLM-discovered semantic paths
    for p in suggested_paths {
        let clean = p.trim().trim_matches('/');
        if !clean.is_empty() {
            if clean.ends_with(file_name) {
                virtual_paths.push(clean.to_string());
            } else {
                virtual_paths.push(format!("{clean}/{file_name}"));
            }
        }
    }

    // 2. Dynamic Fallback: If no paths were provided by LLM/taxonomy, map directly to Category / File
    if virtual_paths.is_empty() {
        let cat_clean = if category.trim().is_empty() { "General" } else { category.trim() };
        virtual_paths.push(format!("{cat_clean}/{file_name}"));
    }

    virtual_paths.sort();
    virtual_paths.dedup();
    virtual_paths
}

/// Construct the complete Virtual Directory Tree from a set of indexed items.
pub fn build_virtual_tree<T: AsRef<crate::index::IndexItem>>(items: &[T]) -> VirtualFolder {
    let mut root = VirtualFolder::new("Root", "");

    for item_ref in items {
        let item = item_ref.as_ref();
        let ext = Path::new(&item.file)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        let paths = if item.virtual_paths.is_empty() {
            compute_virtual_paths(&item.file, &item.category, &[], ext)
        } else {
            compute_virtual_paths(&item.file, &item.category, &item.virtual_paths, ext)
        };

        for vp in paths {
            let segments: Vec<&str> = vp.split('/').collect();
            // The last segment is the file name itself, so directory segments are before the last
            if segments.len() > 1 {
                let dir_segments = &segments[..segments.len() - 1];
                root.insert_pointer(dir_segments, &item.hash);
            } else {
                root.insert_pointer(&["Uncategorized"], &item.hash);
            }
        }
    }

    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photo_of_study_material_gets_multi_pointer() {
        let llm_paths = vec![
            "Study Materials/Calculus/Lecture Materials".to_string(),
            "Media/Document Scans".to_string(),
        ];
        let paths = compute_virtual_paths("IMG_2026_lecture_slide.jpg", "Study Material", &llm_paths, "jpg");

        // Appears in both Study Materials AND Media/Document Scans via dynamic LLM pointers
        assert!(paths.iter().any(|p| p.starts_with("Study Materials/Calculus/Lecture Materials/")));
        assert!(paths.iter().any(|p| p.starts_with("Media/Document Scans/")));
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_virtual_tree_hierarchical_structure() {
        let items = vec![
            crate::index::IndexItem {
                file: "calculus_exam_2026.pdf".to_string(),
                category: "Academic".to_string(),
                keywords: vec!["calculus".to_string(), "exam".to_string(), "question paper".to_string()],
                symbols: Vec::new(),
                path: "/downloads/calculus_exam_2026.pdf".to_string(),
                hash: "h_calc".to_string(),
                indexed_at: None,
                virtual_paths: vec!["Study Materials/Calculus/Exams".to_string()],
                physical_paths: vec!["/downloads/calculus_exam_2026.pdf".to_string()],
                is_duplicate: false,
            },
            crate::index::IndexItem {
                file: "matrix_notes.jpg".to_string(),
                category: "Study".to_string(),
                keywords: vec!["mathematics".to_string(), "matrix".to_string(), "notes".to_string()],
                symbols: Vec::new(),
                path: "/downloads/matrix_notes.jpg".to_string(),
                hash: "h_matrix".to_string(),
                indexed_at: None,
                virtual_paths: vec![
                    "Study Materials/Mathematics/Notes".to_string(),
                    "Media/Document Scans".to_string(),
                ],
                physical_paths: vec!["/downloads/matrix_notes.jpg".to_string()],
                is_duplicate: false,
            },
        ];

        let tree = build_virtual_tree(&items);

        // Verify root contains "Study Materials" and "Media" (matrix_notes.jpg has Media/Document Scans pointer)
        assert!(tree.subfolders.contains_key("Study Materials"));
        assert!(tree.subfolders.contains_key("Media"));

        let study = &tree.subfolders["Study Materials"];
        assert!(study.subfolders.contains_key("Calculus"));
        assert!(study.subfolders.contains_key("Mathematics"));

        // Verify total unique item pointers
        assert_eq!(tree.total_item_count(), 2);
    }
}
