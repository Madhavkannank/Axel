use std::path::Path;

pub fn extract_text<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let path = path.as_ref();
    pdf_extract::extract_text(path).map_err(|e| format!("PDF extraction error: {e}"))
}
