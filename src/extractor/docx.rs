use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub fn extract_text<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open DOCX: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Invalid DOCX zip archive: {e}"))?;

    let mut doc_xml = match archive.by_name("word/document.xml") {
        Ok(entry) => entry,
        Err(e) => return Err(format!("DOCX missing word/document.xml: {e}")),
    };

    let mut xml_content = String::new();
    doc_xml
        .read_to_string(&mut xml_content)
        .map_err(|e| format!("Failed to read word/document.xml: {e}"))?;

    Ok(strip_docx_xml(&xml_content))
}

fn strip_docx_xml(xml: &str) -> String {
    let mut in_tag = false;
    let mut in_text_node = false;
    let mut current_tag = String::new();
    let mut result = String::new();
    let mut buffer = String::new();

    let chars: Vec<char> = xml.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        if ch == '<' {
            in_tag = true;
            current_tag.clear();
        } else if ch == '>' {
            in_tag = false;
            let tag_name = current_tag.trim();
            if tag_name.starts_with("w:t") {
                in_text_node = true;
            } else if tag_name == "/w:t" {
                in_text_node = false;
                result.push_str(&buffer);
                buffer.clear();
            } else if tag_name == "/w:p" || tag_name == "w:br" || tag_name == "/w:tr" {
                result.push('\n');
            }
        } else if in_tag {
            current_tag.push(ch);
        } else if in_text_node {
            buffer.push(ch);
        }
        i += 1;
    }

    // Decode standard XML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}
