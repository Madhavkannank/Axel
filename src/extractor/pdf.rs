use std::path::Path;
use std::process::Command;

pub fn extract_text<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let path = path.as_ref();

    // 1. Try pure-Rust pdf_extract inside catch_unwind so malformed PDFs never crash the worker
    let path_owned = path.to_path_buf();
    let rust_attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        pdf_extract::extract_text(&path_owned)
    }));

    if let Ok(Ok(text)) = rust_attempt {
        if !text.trim().is_empty() {
            return Ok(text);
        }
    }

    // 2. Fallback to system `pdftotext` (Poppler) if installed — handles many PDFs pdf-extract misses
    if let Some(poppler_text) = extract_with_pdftotext(path) {
        if !poppler_text.trim().is_empty() {
            return Ok(poppler_text);
        }
    }

    Err("PDF contains no extractable text layer (may be scanned image PDF)".to_string())
}

fn extract_with_pdftotext(path: &Path) -> Option<String> {
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg("-nopgbrk")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}
