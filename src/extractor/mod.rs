pub mod docx;
pub mod ocr;
pub mod pdf;

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
        "jpg" | "jpeg" | "png" | "tiff" | "bmp" | "webp" => FileType::Image,
        "txt" | "md" | "csv" | "json" => FileType::Text,
        _ => FileType::Unsupported,
    }
}

pub fn extract_text<P: AsRef<Path>>(path: P) -> (String, FileType) {
    let path = path.as_ref();
    let file_type = detect_file_type(path);

    let raw_text = match file_type {
        FileType::Pdf => pdf::extract_text(path).unwrap_or_default(),
        FileType::Docx => docx::extract_text(path).unwrap_or_default(),
        FileType::Image => ocr::extract_text(path).unwrap_or_default(),
        FileType::Text => std::fs::read_to_string(path).unwrap_or_default(),
        FileType::Unsupported => String::new(),
    };

    let preprocessed = preprocess_text(&raw_text);
    (preprocessed, file_type)
}

pub fn preprocess_text(text: &str) -> String {
    let cleaned = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.len() > 3000 {
        let first = &cleaned[..1500];
        let last = &cleaned[cleaned.len() - 1500..];
        format!("{first} ... {last}")
    } else {
        cleaned
    }
}
