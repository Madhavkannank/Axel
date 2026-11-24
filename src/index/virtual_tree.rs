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

fn is_file_like_segment(seg: &str) -> bool {
    if let Some(pos) = seg.rfind('.') {
        if pos > 0 && pos < seg.len() - 1 {
            let ext = &seg[pos + 1..];
            return ext.len() >= 1
                && ext.len() <= 5
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic());
        }
    }
    false
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

    // 1. Ingest dynamic LLM-discovered semantic paths, sanitizing spurious file extensions
    for p in suggested_paths {
        let clean = p.trim().trim_matches('/');
        if clean.is_empty() {
            continue;
        }

        let raw_segments: Vec<&str> = clean
            .split('/')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if raw_segments.is_empty() {
            continue;
        }

        // Filter directory segments:
        // - Segments matching `file_name` are the leaf item itself, not a directory.
        // - Spurious suggested filenames with extensions (e.g. "Syllabus.pdf") must not be treated as folders.
        let mut dir_segments: Vec<&str> = Vec::new();
        for &seg in &raw_segments {
            if seg == file_name {
                continue;
            }
            if is_file_like_segment(seg) {
                continue;
            }
            dir_segments.push(seg);
        }

        if !dir_segments.is_empty() {
            let folder_path = dir_segments.join("/");
            virtual_paths.push(format!("{folder_path}/{file_name}"));
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

    #[test]
    fn test_compute_virtual_paths_strips_spurious_file_extensions() {
        let file_name = "2021-MSc Data Science - Syllabus Book.pdf";
        let suggested = vec![
            "Academics/Data Science/Curriculum and Syllabi.pdf".to_string(),
            "Data Science/CIT M.Sc Data Science Syllabus.pdf/2021-MSc Data Science - Syllabus Book.pdf".to_string(),
            "Data Science/Curriculum and Syllabi.pdf".to_string(),
        ];

        let result = compute_virtual_paths(file_name, "Data Science", &suggested, "pdf");

        // "Curriculum and Syllabi.pdf" and "CIT M.Sc Data Science Syllabus.pdf" must NEVER be directory folders!
        for path in &result {
            assert!(
                !path.contains(".pdf/"),
                "Virtual path should not contain a .pdf folder segment: {path}"
            );
            assert!(
                path.ends_with(file_name),
                "Virtual path must end with actual file name: {path}"
            );
        }

        assert!(result.contains(&format!("Academics/Data Science/{file_name}")));
        assert!(result.contains(&format!("Data Science/{file_name}")));
    }
}
