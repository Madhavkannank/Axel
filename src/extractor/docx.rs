use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub fn extract_text<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open DOCX: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid DOCX zip archive: {e}"))?;

    let mut combined_text = String::new();

    // 1. Extract from headers if present
    for i in 1..=5 {
        let name = format!("word/header{i}.xml");
        if let Ok(mut entry) = archive.by_name(&name) {
            let mut content = String::new();
            if entry.read_to_string(&mut content).is_ok() {
                let parsed = strip_docx_xml(&content);
                if !parsed.is_empty() {
                    combined_text.push_str(&parsed);
                    combined_text.push('\n');
                }
            }
        }
    }

    // 2. Primary document body
    {
        let mut doc_xml = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("DOCX missing word/document.xml: {e}"))?;

        let mut doc_content = String::new();
        doc_xml
            .read_to_string(&mut doc_content)
            .map_err(|e| format!("Failed to read word/document.xml: {e}"))?;

        let parsed = strip_docx_xml(&doc_content);
        combined_text.push_str(&parsed);
        combined_text.push('\n');
    }

    // 3. Extract from footers if present
    for i in 1..=5 {
        let name = format!("word/footer{i}.xml");
        if let Ok(mut entry) = archive.by_name(&name) {
            let mut content = String::new();
            if entry.read_to_string(&mut content).is_ok() {
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
                // If it is self-closing <w:t/>, it contains no text
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

    // Flush any leftover buffer if tag didn't close cleanly
    if !buffer.is_empty() {
        result.push_str(&buffer);
    }

    // Decode standard XML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
