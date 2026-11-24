pub mod docx;
pub mod ocr;
pub mod pdf;
pub mod video;

use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Pdf,
    Docx,
    Image,
    Video,
    Audio,
    Archive,
    Text,
    Binary,
}

impl FileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileType::Pdf => "pdf",
            FileType::Docx => "docx",
            FileType::Image => "image",
            FileType::Video => "video",
            FileType::Audio => "audio",
            FileType::Archive => "archive",
            FileType::Text => "text",
            FileType::Binary => "binary",
        }
    }
}

pub fn detect_file_type<P: AsRef<Path>>(path: P) -> FileType {
    let path = path.as_ref();
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return FileType::Binary,
    };

    match ext.as_str() {
        "pdf" => FileType::Pdf,
        "docx" | "doc" | "odt" | "rtf" => FileType::Docx,
        "jpg" | "jpeg" | "png" | "tiff" | "tif" | "bmp" | "webp" | "gif" | "svg" | "ico" => FileType::Image,
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "m4v" | "wmv" => FileType::Video,
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" | "wma" => FileType::Audio,
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" | "tgz" => FileType::Archive,
        "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "jsonl" | "yaml" | "yml"
        | "toml" | "xml" | "html" | "htm" | "css" | "scss" | "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx"
        | "py" | "rs" | "go" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hxx" | "sh" | "bash"
        | "zsh" | "sql" | "log" | "ini" | "cfg" | "conf" | "tex" | "bib" | "org"
        | "rst" | "diff" | "patch" | "lua" | "rb" | "php" | "r" | "kt" | "swift" | "pl" | "pm" => FileType::Text,
        _ => FileType::Binary,
    }
}

pub fn extract_text<P: AsRef<Path>>(path: P) -> (String, FileType) {
    extract_raw_text(path)
}

pub fn extract_raw_text<P: AsRef<Path>>(path: P) -> (String, FileType) {
    let path = path.as_ref();
    let file_type = detect_file_type(path);

    let raw_text = match file_type {
        FileType::Pdf => pdf::extract_text(path).unwrap_or_default(),
        FileType::Docx => docx::extract_text(path).unwrap_or_default(),
        FileType::Image => ocr::extract_text(path).unwrap_or_default(),
        FileType::Video => video::extract_video_text(path).0,
        FileType::Audio => extract_audio_metadata(path),
        FileType::Archive => extract_archive_metadata(path),
        FileType::Text => read_text_capped(path, 256 * 1024),
        FileType::Binary => extract_binary_metadata(path),
    };

    (raw_text, file_type)
}

fn extract_audio_metadata(path: &Path) -> String {
    let mut parts = Vec::new();
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let clean = stem.replace(['_', '-', '.'], " ");
        parts.push(format!("Audio: {clean}"));
    }
    parts.join("\n")
}

fn extract_archive_metadata(path: &Path) -> String {
    let mut parts = Vec::new();
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let clean = stem.replace(['_', '-', '.'], " ");
        parts.push(format!("Archive: {clean}"));
    }
    parts.join("\n")
}

fn extract_binary_metadata(path: &Path) -> String {
    let mut parts = Vec::new();
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let clean = stem.replace(['_', '-', '.'], " ");
        parts.push(format!("File: {clean}"));
    }
    parts.join("\n")
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
