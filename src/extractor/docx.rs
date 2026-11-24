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
    // First attempt binary string/stream extraction (A17)
    if let Some(binary_text) = extract_binary_doc_text(path) {
        if !binary_text.is_empty() {
            return Ok(binary_text);
        }
    }

    // Report metadata-only extraction with filename and path tokens if binary extraction yields nothing
    let metadata_text = extract_office_metadata_fallback(path);
    Ok(format!("[Metadata Extraction Fallback]\n{metadata_text}"))
}

fn extract_docx_zip(archive: &mut ZipArchive<File>) -> Result<String, String> {
    // 1. Primary document body (PRIORITIZED FIRST so headers never exhaust budget)
    let primary_body = match archive.by_name("word/document.xml") {
        Ok(mut doc_xml) => {
            let mut doc_content = String::new();
            doc_xml
                .by_ref()
                .take(MAX_ENTRY_DECOMPRESS_BYTES)
                .read_to_string(&mut doc_content)
                .map_err(|e| format!("Failed to read word/document.xml: {e}"))?;
            strip_docx_xml(&doc_content)
        }
        Err(e) => return Err(format!("DOCX missing word/document.xml: {e}")),
    };

    let mut combined_text = String::new();

    // 2. Extract from headers if budget allows
    let mut header_text = String::new();
    for i in 1..=5 {
        if primary_body.len() + header_text.len() >= MAX_TOTAL_EXTRACT_BYTES {
            break;
        }
        let name = format!("word/header{i}.xml");
        if let Ok(mut entry) = archive.by_name(&name) {
            let mut content = String::new();
            if entry.by_ref().take(MAX_ENTRY_DECOMPRESS_BYTES).read_to_string(&mut content).is_ok() {
                let parsed = strip_docx_xml(&content);
                if !parsed.is_empty() {
                    header_text.push_str(&parsed);
                    header_text.push('\n');
                }
            }
        }
    }

    if !header_text.is_empty() {
        combined_text.push_str(&header_text);
    }
    combined_text.push_str(&primary_body);
    combined_text.push('\n');

    // 3. Extract from footers if budget allows
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
            let is_closing = tag_name.starts_with('/');
            let is_self_closing = tag_name.ends_with('/');
            let clean = if is_closing { &tag_name[1..] } else { tag_name };
            let clean = clean.trim_end_matches('/');
            let token = clean.split_whitespace().next().unwrap_or(clean);
            // A17/A18: Decouple from specific namespace prefix by inspecting local name
            let local_name = token.split(':').last().unwrap_or(token);

            if !is_closing && local_name == "t" {
                if is_self_closing {
                    in_text_node = false;
                } else {
                    in_text_node = true;
                }
            } else if is_closing && local_name == "t" {
                in_text_node = false;
                result.push_str(&buffer);
                buffer.clear();
            } else if (is_closing && (local_name == "p" || local_name == "tr"))
                || (!is_closing && local_name == "br")
            {
                result.push('\n');
            } else if !is_closing && local_name == "tab" {
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

pub fn strip_odt_xml(xml: &str) -> String {
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
            let is_closing = tag_name.starts_with('/');
            let clean = if is_closing { &tag_name[1..] } else { tag_name };
            let clean = clean.trim_end_matches('/');
            let token = clean.split_whitespace().next().unwrap_or(clean);
            // A17: Decouple from specific namespace prefix by inspecting local name
            let local_name = token.split(':').last().unwrap_or(token);

            if is_closing && (local_name == "p" || local_name == "h") {
                result.push('\n');
            } else if !is_closing && (local_name == "line-break" || local_name == "tab") {
                if local_name == "tab" {
                    result.push(' ');
                } else {
                    result.push('\n');
                }
            }
        } else if !in_tag {
            result.push(ch);
        } else {
            current_tag.push(ch);
        }
    }

    decode_xml_entities(&result)
}

fn is_ole_metadata_token(s: &str) -> bool {
    matches!(
        s,
        "Root Entry"
            | "WordDocument"
            | "1Table"
            | "0Table"
            | "SummaryInformation"
            | "DocumentSummaryInformation"
            | "CompObj"
            | "Data"
    )
}

pub fn extract_binary_doc_text(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.take(MAX_ENTRY_DECOMPRESS_BYTES).read_to_end(&mut buffer).ok()?;

    let mut runs: Vec<String> = Vec::new();

    // 1. Scan for UTF-16LE runs (Word 97-2003 often stores document text in 16-bit unicode runs)
    let mut i = 0;
    while i + 1 < buffer.len() {
        let mut u16_chars = Vec::new();
        while i + 1 < buffer.len() {
            let lo = buffer[i];
            let hi = buffer[i + 1];
            if hi == 0 && (lo >= 32 && lo <= 126 || lo == b'\t' || lo == b'\n' || lo == b'\r') {
                u16_chars.push(lo as char);
                i += 2;
            } else {
                break;
            }
        }
        if u16_chars.len() >= 4 {
            let s: String = u16_chars.into_iter().collect();
            let trimmed = s.trim();
            if trimmed.len() >= 4 && !is_ole_metadata_token(trimmed) {
                runs.push(trimmed.to_string());
            }
        } else {
            i += 2;
        }
    }

    // 2. Scan for ASCII runs
    let mut j = 0;
    while j < buffer.len() {
        let start = j;
        while j < buffer.len()
            && (buffer[j] >= 32 && buffer[j] <= 126
                || buffer[j] == b'\t'
                || buffer[j] == b'\n'
                || buffer[j] == b'\r')
        {
            j += 1;
        }
        if j - start >= 4 {
            if let Ok(s) = std::str::from_utf8(&buffer[start..j]) {
                let trimmed = s.trim();
                if trimmed.len() >= 4 && !is_ole_metadata_token(trimmed) {
                    runs.push(trimmed.to_string());
                }
            }
        } else {
            j += 1;
        }
    }

    if runs.is_empty() {
        return None;
    }

    let joined = runs.join("\n");
    if joined.trim().len() >= 10 {
        Some(joined)
    } else {
        None
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_odt_xml_decoupled_namespaces() {
        let xml_standard = "<office:body><office:text><text:p>Standard ODF paragraph.</text:p><text:h>Heading 1</text:h></office:text></office:body>";
        let parsed = strip_odt_xml(xml_standard);
        assert!(parsed.contains("Standard ODF paragraph."));
        assert!(parsed.contains("Heading 1"));

        let xml_custom = "<office:body><office:text><myprefix:p>Custom prefix paragraph.</myprefix:p><h>Default ns heading</h></office:text></office:body>";
        let parsed_custom = strip_odt_xml(xml_custom);
        assert!(parsed_custom.contains("Custom prefix paragraph."));
        assert!(parsed_custom.contains("Default ns heading"));
    }

    #[test]
    fn test_binary_doc_text_extraction() {
        use std::io::Write;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let doc_path = std::env::temp_dir().join(format!("test_legacy_{now}.doc"));

        let mut file = File::create(&doc_path).unwrap();
        // Write binary prefix
        file.write_all(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]).unwrap();
        // Write ASCII text run
        file.write_all(b"Confidential Project Specification Alpha\x00\x00").unwrap();
        // Write UTF-16LE text run: "System Architecture"
        let utf16_str: Vec<u8> = "System Architecture"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        file.write_all(&utf16_str).unwrap();
        file.flush().unwrap();

        let extracted = extract_binary_doc_text(&doc_path);
        let _ = std::fs::remove_file(&doc_path);

        assert!(extracted.is_some());
        let text = extracted.unwrap();
        assert!(text.contains("Confidential Project Specification Alpha"));
        assert!(text.contains("System Architecture"));
    }
}
