use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

const MAX_ENTRY_DECOMPRESS_BYTES: u64 = 10 * 1024 * 1024;
const MAX_TOTAL_EXTRACT_BYTES: usize = 20 * 1024 * 1024;

/// Main extraction entry point for office documents (DOCX, ODT, RTF, and legacy DOC).
/// Returns extracted text, or reports metadata-only extraction when container is binary/unparseable (M11).
pub fn extract_text<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|e| format!("Cannot open document: {e}"))?;

    // 1. Try reading as ZIP archive (DOCX, ODT)
    if let Ok(mut archive) = ZipArchive::new(file) {
        // Case 1A: DOCX format (word/document.xml)
        if archive.by_name("word/document.xml").is_ok() {
            return extract_docx_zip(&mut archive);
        }
        // Case 1B: ODF/ODT format (content.xml)
        if archive.by_name("content.xml").is_ok() {
            return extract_odt_zip(&mut archive);
        }
    }

    // 2. Try RTF format (starts with "{\rtf")
    if let Ok(mut rtf_file) = File::open(path) {
        let mut header = [0u8; 5];
        if rtf_file.read_exact(&mut header).is_ok() && &header == b"{\\rtf" {
            let mut rtf_content = String::new();
            if File::open(path)
                .and_then(|f| f.take(MAX_ENTRY_DECOMPRESS_BYTES).read_to_string(&mut rtf_content))
                .is_ok()
            {
                let parsed = strip_rtf(&rtf_content);
                if !parsed.is_empty() {
                    return Ok(parsed);
                }
            }
        }
    }

    // 3. Fallback for legacy binary formats (e.g. .doc, corrupted zips)
    // Report metadata-only extraction with filename and path tokens
    let metadata_text = extract_office_metadata_fallback(path);
    Ok(format!("[Metadata Extraction Fallback]\n{metadata_text}"))
}

fn extract_docx_zip(archive: &mut ZipArchive<File>) -> Result<String, String> {
    let mut combined_text = String::new();

    // 1. Extract from headers if present
    for i in 1..=5 {
        if combined_text.len() >= MAX_TOTAL_EXTRACT_BYTES {
            break;
        }
        let name = format!("word/header{i}.xml");
        if let Ok(mut entry) = archive.by_name(&name) {
            let mut content = String::new();
            if entry.by_ref().take(MAX_ENTRY_DECOMPRESS_BYTES).read_to_string(&mut content).is_ok() {
                let parsed = strip_docx_xml(&content);
                if !parsed.is_empty() {
                    combined_text.push_str(&parsed);
                    combined_text.push('\n');
                }
            }
        }
    }

    // 2. Primary document body
    if combined_text.len() < MAX_TOTAL_EXTRACT_BYTES {
        let mut doc_xml = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("DOCX missing word/document.xml: {e}"))?;

        let mut doc_content = String::new();
        doc_xml
            .by_ref()
            .take(MAX_ENTRY_DECOMPRESS_BYTES)
            .read_to_string(&mut doc_content)
            .map_err(|e| format!("Failed to read word/document.xml: {e}"))?;

        let parsed = strip_docx_xml(&doc_content);
        combined_text.push_str(&parsed);
        combined_text.push('\n');
    }

    // 3. Extract from footers if present
    for i in 1..=5 {
        if combined_text.len() >= MAX_TOTAL_EXTRACT_BYTES {
            break;
        }
        let name = format!("word/footer{i}.xml");
        if let Ok(mut entry) = archive.by_name(&name) {
            let mut content = String::new();
            if entry.by_ref().take(MAX_ENTRY_DECOMPRESS_BYTES).read_to_string(&mut content).is_ok() {
                let parsed = strip_docx_xml(&content);
                if !parsed.is_empty() {
                    combined_text.push_str(&parsed);
                    combined_text.push('\n');
                }
            }
        }
    }

    Ok(combined_text.trim().to_string())
}

fn extract_odt_zip(archive: &mut ZipArchive<File>) -> Result<String, String> {
    let mut content_xml = archive
        .by_name("content.xml")
        .map_err(|e| format!("ODT missing content.xml: {e}"))?;

    let mut content = String::new();
    content_xml
        .by_ref()
        .take(MAX_ENTRY_DECOMPRESS_BYTES)
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read content.xml: {e}"))?;

    let parsed = strip_odt_xml(&content);
    Ok(parsed.trim().to_string())
}

pub fn strip_docx_xml(xml: &str) -> String {
    let mut in_tag = false;
    let mut in_text_node = false;
    let mut current_tag = String::new();
    let mut result = String::new();
    let mut buffer = String::new();

    for ch in xml.chars() {
        if ch == '<' {
            in_tag = true;
            current_tag.clear();
        } else if ch == '>' {
            in_tag = false;
            let tag_name = current_tag.trim();

            if tag_name.starts_with("w:t") {
                if tag_name.ends_with('/') {
                    in_text_node = false;
                } else {
                    in_text_node = true;
                }
            } else if tag_name == "/w:t" {
                in_text_node = false;
                result.push_str(&buffer);
                buffer.clear();
            } else if tag_name == "/w:p" || tag_name == "w:br" || tag_name == "w:br/" || tag_name == "/w:tr" {
                result.push('\n');
            } else if tag_name == "w:tab" || tag_name == "w:tab/" {
                result.push(' ');
            }
        } else if in_tag {
            current_tag.push(ch);
        } else if in_text_node {
            buffer.push(ch);
        }
    }

    if !buffer.is_empty() {
        result.push_str(&buffer);
    }

    decode_xml_entities(&result)
}

fn strip_odt_xml(xml: &str) -> String {
    let mut in_tag = false;
    let mut current_tag = String::new();
    let mut result = String::new();

    for ch in xml.chars() {
        if ch == '<' {
            in_tag = true;
            current_tag.clear();
        } else if ch == '>' {
            in_tag = false;
            let tag_name = current_tag.trim();
            if tag_name.starts_with("/text:p")
                || tag_name.starts_with("/text:h")
                || tag_name.starts_with("text:line-break")
            {
                result.push('\n');
            } else if tag_name.starts_with("text:tab") {
                result.push(' ');
            }
        } else if !in_tag {
            result.push(ch);
        } else {
            current_tag.push(ch);
        }
    }

    decode_xml_entities(&result)
}

fn strip_rtf(rtf: &str) -> String {
    let mut result = String::new();
    let mut in_control = false;

    for ch in rtf.chars() {
        if ch == '\\' {
            in_control = true;
        } else if in_control {
            if ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t' || ch == '\\' || ch == '{' || ch == '}' {
                in_control = false;
                if ch == '\n' || ch == '\r' {
                    result.push(' ');
                }
            }
        } else if ch != '{' && ch != '}' {
            result.push(ch);
        }
    }

    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn decode_xml_entities(input: &str) -> String {
    let mut s = input
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");

    if s.contains("&#") {
        if let Ok(re_dec) = regex::Regex::new(r"&#(\d+);") {
            s = re_dec
                .replace_all(&s, |caps: &regex::Captures| {
                    if let Ok(code) = caps[1].parse::<u32>() {
                        if let Some(ch) = char::from_u32(code) {
                            return ch.to_string();
                        }
                    }
                    caps[0].to_string()
                })
                .to_string();
        }

        if let Ok(re_hex) = regex::Regex::new(r"(?i)&#x([0-9a-f]+);") {
            s = re_hex
                .replace_all(&s, |caps: &regex::Captures| {
                    if let Ok(code) = u32::from_str_radix(&caps[1], 16) {
                        if let Some(ch) = char::from_u32(code) {
                            return ch.to_string();
                        }
                    }
                    caps[0].to_string()
                })
                .to_string();
        }
    }
    s
}

fn extract_office_metadata_fallback(path: &Path) -> String {
    let mut tokens = Vec::new();
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let cleaned = stem
            .replace(['_', '-', '.'], " ")
            .split_whitespace()
            .map(|w| crate::classifier::code::split_identifier(w).join(" "))
            .collect::<Vec<_>>()
            .join(" ");
        tokens.push(format!("Document Title: {cleaned}"));
    }
    if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
        let parent_cleaned = parent.replace(['_', '-', '.'], " ");
        tokens.push(format!("Folder: {parent_cleaned}"));
    }
    tokens.join("\n")
}
