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
pub fn compute_virtual_paths(
    file_name: &str,
    category: &str,
    keywords: &[String],
    ext: &str,
) -> Vec<String> {
    let mut virtual_paths = Vec::new();
    let lower_file = file_name.to_lowercase();
    let lower_kws: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
    let ext_lower = ext.to_lowercase();

    // 1. Check Primary Functional Taxonomy (Study Materials, Financial, Code, etc.)
    let is_study = is_academic_or_study(category, &lower_file, &lower_kws);
    let is_finance = is_financial_document(category, &lower_file, &lower_kws);
    let is_code = is_programming_code(&ext_lower, category);
    let is_photo_image = is_image_format(&ext_lower);
    let is_video = is_video_format(&ext_lower);

    // Path 1: Academic / Study Materials Hierarchy
    if is_study {
        let subject = detect_academic_subject(&lower_file, &lower_kws);
        let sub_role = detect_study_sub_role(&lower_file, &lower_kws);
        virtual_paths.push(format!("Study Materials/{subject}/{sub_role}/{file_name}"));
    }

    // Path 2: Financial Hierarchy
    if is_finance {
        let fin_role = detect_finance_sub_role(&lower_file, &lower_kws);
        let entity = detect_entity_or_year(&lower_file, &lower_kws);
        virtual_paths.push(format!("Finance/{fin_role}/{entity}/{file_name}"));
    }

    // Path 3: Code Hierarchy
    if is_code {
        let domain = detect_code_domain(&lower_file, &lower_kws);
        let lang = language_from_ext(&ext_lower);
        virtual_paths.push(format!("Code/{domain}/{lang}/{file_name}"));
    }

    // Path 4: Media Format Pointers (e.g. if an image is ALSO a study material)
    if is_photo_image {
        let photo_kind = if is_study {
            "Document Scans"
        } else {
            "Photos"
        };
        virtual_paths.push(format!("Media/{photo_kind}/{file_name}"));
    }

    // Path 5: Video Format Pointers (e.g. if a video is a lecture vs movie)
    if is_video {
        let video_kind = if is_study {
            "Educational Lectures"
        } else if lower_file.contains("1080p")
            || lower_file.contains("2160p")
            || lower_file.contains("bluray")
            || lower_file.contains("movie")
        {
            "Movies"
        } else {
            "Clips"
        };
        virtual_paths.push(format!("Media/Videos/{video_kind}/{file_name}"));
    }

    // Fallback: If no specialized paths triggered, use Category / Format / File
    if virtual_paths.is_empty() {
        let cat_clean = if category.is_empty() { "General" } else { category };
        virtual_paths.push(format!("{cat_clean}/{file_name}"));
    }

    virtual_paths.sort();
    virtual_paths.dedup();
    virtual_paths
}

fn is_academic_or_study(category: &str, file: &str, kws: &[String]) -> bool {
    let cat_lower = category.to_lowercase();
    if cat_lower.contains("academic") || cat_lower.contains("study") || cat_lower.contains("education") {
        return true;
    }
    let terms = [
        "lecture", "notes", "question paper", "exam", "midterm", "final", "syllabus",
        "assignment", "homework", "textbook", "chapter", "module", "curriculum", "unit",
        "calculus", "physics", "chemistry", "biology", "algebra", "ece", "cs229",
    ];
    terms.iter().any(|&t| file.contains(t) || kws.iter().any(|k| k.contains(t)))
}

fn detect_academic_subject(file: &str, kws: &[String]) -> String {
    let subjects = [
        ("Calculus", &["calculus", "derivative", "integral", "differential", "multivariable"][..]),
        ("Mathematics", &["math", "algebra", "geometry", "trigonometry", "matrix", "linear"][..]),
        ("Physics", &["physics", "mechanics", "thermodynamics", "optics", "quantum", "electromagnetism"][..]),
        ("Computer Science", &["computer science", "algorithm", "data structure", "compiler", "operating system", "dbms", "networks", "machine learning"][..]),
        ("Electrical & ECE", &["ece", "circuits", "analog", "signals", "microprocessor", "vlsi", "digital logic"][..]),
        ("Chemistry", &["chemistry", "organic", "inorganic", "molecules", "reaction"][..]),
        ("Biology", &["biology", "genetics", "cellular", "anatomy", "physiology"][..]),
        ("Economics", &["economics", "macroeconomics", "microeconomics", "econometrics"][..]),
    ];

    for (name, triggers) in subjects {
        if triggers.iter().any(|&tr| file.contains(tr) || kws.iter().any(|k| k.contains(tr))) {
            return name.to_string();
        }
    }
    "General Studies".to_string()
}

fn detect_study_sub_role(file: &str, kws: &[String]) -> String {
    if file.contains("question") || file.contains("qp") || file.contains("exam") || file.contains("test")
        || kws.iter().any(|k| k.contains("question") || k.contains("exam"))
    {
        "Question Papers".to_string()
    } else if file.contains("assignment") || file.contains("homework") || file.contains("lab")
        || kws.iter().any(|k| k.contains("assignment") || k.contains("lab"))
    {
        "Assignments & Labs".to_string()
    } else if file.contains("lecture") || file.contains("slide") || file.contains("ppt")
        || kws.iter().any(|k| k.contains("lecture") || k.contains("slide"))
    {
        "Lecture Materials".to_string()
    } else if file.contains("syllabus") || file.contains("curriculum") {
        "Syllabus & Guides".to_string()
    } else {
        "Notes & Reference".to_string()
    }
}

fn is_financial_document(category: &str, file: &str, kws: &[String]) -> bool {
    let cat_lower = category.to_lowercase();
    if cat_lower.contains("invoice") || cat_lower.contains("finance") || cat_lower.contains("receipt") {
        return true;
    }
    let terms = ["invoice", "receipt", "billing", "tax", "gst", "statement", "payroll", "salary"];
    terms.iter().any(|&t| file.contains(t) || kws.iter().any(|k| k.contains(t)))
}

fn detect_finance_sub_role(file: &str, kws: &[String]) -> String {
    if file.contains("receipt") || kws.iter().any(|k| k.contains("receipt")) {
        "Receipts".to_string()
    } else if file.contains("tax") || file.contains("gst") || kws.iter().any(|k| k.contains("tax")) {
        "Tax & Filings".to_string()
    } else if file.contains("statement") || kws.iter().any(|k| k.contains("statement")) {
        "Bank Statements".to_string()
    } else {
        "Invoices".to_string()
    }
}

fn detect_entity_or_year(file: &str, kws: &[String]) -> String {
    // Check for years 2020..2030
    for year in 2020..=2030 {
        let yr_str = year.to_string();
        if file.contains(&yr_str) || kws.iter().any(|k| k.contains(&yr_str)) {
            return yr_str;
        }
    }
    "Corporate & Client".to_string()
}

fn is_programming_code(ext: &str, category: &str) -> bool {
    let cat_lower = category.to_lowercase();
    if cat_lower.contains("code") || cat_lower.contains("technical") {
        return true;
    }
    matches!(
        ext,
        "py" | "rs" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "cpp" | "java" | "sh" | "sql" | "html" | "css"
    )
}

fn detect_code_domain(file: &str, kws: &[String]) -> String {
    let domains = [
        ("Computer Vision", &["vision", "cv2", "opencv", "sobel", "image", "filter", "yolo"][..]),
        ("Mathematics & ML", &["calc", "math", "add", "subtract", "matrix", "tensor", "torch", "numpy"][..]),
        ("Web & Frontend", &["web", "react", "html", "css", "vue", "frontend", "dom"][..]),
        ("Backend & APIs", &["api", "server", "endpoint", "database", "sql", "axum", "express"][..]),
        ("Automation & Tools", &["script", "bot", "crawler", "scraper", "cli", "tool"][..]),
    ];

    for (name, triggers) in domains {
        if triggers.iter().any(|&tr| file.contains(tr) || kws.iter().any(|k| k.contains(tr))) {
            return name.to_string();
        }
    }
    "Utilities".to_string()
}

fn language_from_ext(ext: &str) -> &'static str {
    match ext {
        "py" => "Python",
        "rs" => "Rust",
        "js" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "go" => "Go",
        "c" | "h" => "C",
        "cpp" | "hpp" => "C++",
        "java" => "Java",
        "sh" | "bash" => "Shell",
        "sql" => "SQL",
        _ => "General Code",
    }
}

fn is_image_format(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tiff" | "tif")
}

fn is_video_format(ext: &str) -> bool {
    matches!(ext, "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv")
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
            compute_virtual_paths(&item.file, &item.category, &item.keywords, ext)
        } else {
            item.virtual_paths.clone()
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
        let kws = vec![
            "calculus".to_string(),
            "derivative".to_string(),
            "lecture notes".to_string(),
        ];
        let paths = compute_virtual_paths("IMG_2026_lecture_slide.jpg", "Study Material", &kws, "jpg");

        // Must appear in both Study Materials AND Media/Photos!
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
                path: "/home/maddy/Downloads/calculus_exam_2026.pdf".to_string(),
                hash: "h_calc".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/home/maddy/Downloads/calculus_exam_2026.pdf".to_string()],
                is_duplicate: false,
            },
            crate::index::IndexItem {
                file: "matrix_notes.jpg".to_string(),
                category: "Study".to_string(),
                keywords: vec!["mathematics".to_string(), "matrix".to_string(), "notes".to_string()],
                symbols: Vec::new(),
                path: "/home/maddy/Downloads/matrix_notes.jpg".to_string(),
                hash: "h_matrix".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/home/maddy/Downloads/matrix_notes.jpg".to_string()],
                is_duplicate: false,
            },
        ];

        let tree = build_virtual_tree(&items);

        // Verify root contains "Study Materials" and "Media"
        assert!(tree.subfolders.contains_key("Study Materials"));
        assert!(tree.subfolders.contains_key("Media"));

        let study = &tree.subfolders["Study Materials"];
        assert!(study.subfolders.contains_key("Calculus"));
        assert!(study.subfolders.contains_key("Mathematics"));

        // Verify total unique item pointers
        assert_eq!(tree.total_item_count(), 2);
    }
}
