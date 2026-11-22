pub mod docx;
pub mod ocr;
pub mod pdf;

use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Pdf,
    Docx,
    Image,
    Text,
    Unsupported,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Pdf => "pdf",
            FileType::Docx => "docx",
            FileType::Image => "image",
            FileType::Text => "text",
            FileType::Unsupported => "unsupported",
        }
    }
}

pub fn detect_file_type<P: AsRef<Path>>(path: P) -> FileType {
    let path = path.as_ref();
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return FileType::Unsupported,
    };

    match ext.as_str() {
        "pdf" => FileType::Pdf,
        "docx" => FileType::Docx,
        "jpg" | "jpeg" | "png" | "tiff" | "tif" | "bmp" | "webp" | "gif" => FileType::Image,
        "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "jsonl" | "yaml" | "yml"
        | "toml" | "xml" | "html" | "htm" | "css" | "scss" | "js" | "jsx" | "ts" | "tsx"
        | "py" | "rs" | "go" | "java" | "c" | "cpp" | "cc" | "h" | "hpp" | "sh" | "bash"
        | "zsh" | "sql" | "log" | "ini" | "cfg" | "conf" | "tex" | "bib" | "rtf" | "org"
        | "rst" => FileType::Text,
        _ => FileType::Unsupported,
    }
}

pub fn extract_text<P: AsRef<Path>>(path: P) -> (String, FileType) {
    let (raw_text, file_type) = extract_raw_text(path);
    let preprocessed = preprocess_text(&raw_text);
    (preprocessed, file_type)
}

pub fn extract_raw_text<P: AsRef<Path>>(path: P) -> (String, FileType) {
    let path = path.as_ref();
    let file_type = detect_file_type(path);

    let raw_text = match file_type {
        FileType::Pdf => pdf::extract_text(path).unwrap_or_default(),
        FileType::Docx => docx::extract_text(path).unwrap_or_default(),
        FileType::Image => ocr::extract_text(path).unwrap_or_default(),
        FileType::Text => read_text_capped(path, 256 * 1024),
        FileType::Unsupported => String::new(),
    };

    (raw_text, file_type)
}

fn read_text_capped(path: &Path, max_bytes: usize) -> String {
    let Ok(file) = File::open(path) else {
        return String::new();
    };
    let mut buf = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut handle = file.take(max_bytes as u64);
    if handle.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

pub fn preprocess_text(text: &str) -> String {
    let cleaned = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let char_count = cleaned.chars().count();
    if char_count > 3000 {
        let first: String = cleaned.chars().take(1500).collect();
        let last: String = cleaned.chars().skip(char_count - 1500).collect();
        format!("{first} ... {last}")
    } else {
        cleaned
    }
}
