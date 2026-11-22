use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

/// Maximum bytes of subtitle/transcript text to ingest to keep RAM strictly bounded (< 256 KB)
const MAX_SUBTITLE_BYTES: usize = 256 * 1024;

/// Result of probing a video file
#[derive(Debug, Clone, Default)]
pub struct VideoIntelligence {
    pub duration_secs: Option<f64>,
    pub resolution: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub chapters: Vec<String>,
    pub subtitle_text: String,
    pub format_tags: Vec<String>,
}

/// Extract searchable textual content and metadata from a video file.
/// Designed for long videos (movies, multi-hour lectures, presentations):
/// - Probes container headers via `ffprobe` without decoding raw video frames (< 1 MB RAM)
/// - Ingests sidecar (.srt, .vtt) or embedded subtitles for full-dialogue keyword extraction
/// - Never loads gigabytes of video into RAM
pub fn extract_video_text<P: AsRef<Path>>(path: P) -> (String, VideoIntelligence) {
    let path = path.as_ref();
    let mut intel = VideoIntelligence::default();
    let mut text_blocks = Vec::new();

    // 1. Check for sidecar subtitle files (.srt, .vtt, .sub) beside the video
    if let Some(sidecar_text) = find_and_read_sidecar_subtitles(path) {
        if !sidecar_text.is_empty() {
            intel.subtitle_text = sidecar_text.clone();
            text_blocks.push(format!("Transcript:\n{sidecar_text}"));
        }
    }

    // 2. Probe container headers via ffprobe (if available)
    if let Ok(ffprobe_intel) = probe_container_headers(path) {
        if let Some(title) = &ffprobe_intel.title {
            text_blocks.push(format!("Title: {title}"));
            intel.title = Some(title.clone());
        }
        if let Some(desc) = &ffprobe_intel.description {
            text_blocks.push(format!("Description: {desc}"));
            intel.description = Some(desc.clone());
        }
        if !ffprobe_intel.chapters.is_empty() {
            let chapter_str = ffprobe_intel.chapters.join(" | ");
            text_blocks.push(format!("Chapters: {chapter_str}"));
            intel.chapters = ffprobe_intel.chapters;
        }
        if !ffprobe_intel.format_tags.is_empty() {
            text_blocks.push(format!("Tags: {}", ffprobe_intel.format_tags.join(", ")));
            intel.format_tags = ffprobe_intel.format_tags;
        }
        intel.duration_secs = ffprobe_intel.duration_secs;
        intel.resolution = ffprobe_intel.resolution;
    }

    // 3. If sidecar subtitle was not found, attempt embedded subtitle extraction via ffmpeg
    if intel.subtitle_text.is_empty() {
        if let Some(embedded_sub) = extract_embedded_subtitle_stream(path) {
            intel.subtitle_text = embedded_sub.clone();
            text_blocks.push(format!("Subtitles:\n{embedded_sub}"));
        }
    }

    // 4. Filename tokens (fallback / augmentation)
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let cleaned_name = stem.replace(['_', '.', '-'], " ");
        text_blocks.push(format!("Filename: {cleaned_name}"));
    }

    let combined_text = text_blocks.join("\n\n");
    (combined_text, intel)
}

/// Find and parse sidecar subtitle files like `video.srt`, `video.en.srt`, `video.vtt`
pub fn find_and_read_sidecar_subtitles(video_path: &Path) -> Option<String> {
    let parent = video_path.parent()?;
    let stem = video_path.file_stem()?.to_str()?;

    let candidates = [
        parent.join(format!("{stem}.srt")),
        parent.join(format!("{stem}.en.srt")),
        parent.join(format!("{stem}.vtt")),
        parent.join(format!("{stem}.en.vtt")),
        parent.join(format!("{stem}.sub")),
    ];

    for candidate in &candidates {
        if candidate.exists() && candidate.is_file() {
            if let Ok(content) = read_subtitle_file(candidate) {
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
    }

    None
}

/// Read and clean an .srt or .vtt subtitle file, stripping timestamp cues and indices
pub fn read_subtitle_file(sub_path: &Path) -> Result<String, std::io::Error> {
    let file = File::open(sub_path)?;
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut handle = file.take(MAX_SUBTITLE_BYTES as u64);
    handle.read_to_end(&mut buf)?;

    let raw = String::from_utf8_lossy(&buf);
    Ok(clean_subtitle_cues(&raw))
}

/// Strip numeric counters, WEBVTT headers, and timestamp lines (e.g. 00:01:23,456 --> 00:01:26,789)
pub fn clean_subtitle_cues(raw: &str) -> String {
    let mut cleaned_lines = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed == "WEBVTT"
            || trimmed.starts_with("NOTE")
            || trimmed.starts_with("STYLE")
        {
            continue;
        }

        // Skip timestamp lines: "00:01:20.000 --> 00:01:23.000" or "00:01:20,000 --> 00:01:23,000"
        if trimmed.contains("-->") {
            continue;
        }

        // Skip numeric index lines (e.g. "1", "24")
        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        // Strip HTML-like subtitle tags: <i>, </b>, <font color="...">
        let no_tags = strip_xml_tags(trimmed);
        if !no_tags.is_empty() {
            cleaned_lines.push(no_tags);
        }
    }

    cleaned_lines.join(" ")
}

fn strip_xml_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        if c == '<' {
            inside = true;
        } else if c == '>' {
            inside = false;
        } else if !inside {
            out.push(c);
        }
    }
    out
}

/// Probe container headers using ffprobe (streaming JSON format)
fn probe_container_headers(video_path: &Path) -> Result<VideoIntelligence, String> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg("-show_chapters")
        .arg(video_path)
        .output()
        .map_err(|e| format!("Failed to spawn ffprobe: {e}"))?;

    if !output.status.success() {
        return Err("ffprobe exited with non-zero status".to_string());
    }

    let json_val: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Invalid ffprobe JSON output: {e}"))?;

    let mut intel = VideoIntelligence::default();

    // Parse format tags (Title, Description, Comment, etc.)
    if let Some(format_obj) = json_val.get("format") {
        if let Some(duration_str) = format_obj.get("duration").and_then(|d| d.as_str()) {
            intel.duration_secs = duration_str.parse::<f64>().ok();
        }

        if let Some(tags) = format_obj.get("tags").and_then(|t| t.as_object()) {
            for (k, v) in tags {
                let k_lower = k.to_lowercase();
                if let Some(val_str) = v.as_str() {
                    match k_lower.as_str() {
                        "title" => intel.title = Some(val_str.to_string()),
                        "description" | "comment" => intel.description = Some(val_str.to_string()),
                        _ => {
                            if !val_str.trim().is_empty() && val_str.len() < 100 {
                                intel.format_tags.push(format!("{k}: {val_str}"));
                            }
                        }
                    }
                }
            }
        }
    }

    // Parse streams to find resolution and audio/subtitle tracks
    if let Some(streams) = json_val.get("streams").and_then(|s| s.as_array()) {
        for s in streams {
            let codec_type = s.get("codec_type").and_then(|c| c.as_str()).unwrap_or_default();
            if codec_type == "video" && intel.resolution.is_none() {
                if let (Some(w), Some(h)) = (s.get("width").and_then(|w| w.as_i64()), s.get("height").and_then(|h| h.as_i64())) {
                    intel.resolution = Some(format!("{w}x{h}"));
                }
            }
        }
    }

    // Parse chapter titles
    if let Some(chapters) = json_val.get("chapters").and_then(|c| c.as_array()) {
        for ch in chapters {
            if let Some(title) = ch.get("tags").and_then(|t| t.get("title")).and_then(|t| t.as_str()) {
                if !title.trim().is_empty() {
                    intel.chapters.push(title.trim().to_string());
                }
            }
        }
    }

    Ok(intel)
}

/// Extract up to 128 KB of embedded subtitle stream using ffmpeg without video transcoding
fn extract_embedded_subtitle_stream(video_path: &Path) -> Option<String> {
    let output = Command::new("ffmpeg")
        .arg("-v")
        .arg("quiet")
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg("0:s:0") // First subtitle stream
        .arg("-f")
        .arg("webvtt")
        .arg("-") // Stream to stdout
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    // Limit buffer to 128KB to maintain <10MB RAM
    let max_len = output.stdout.len().min(128 * 1024);
    let raw = String::from_utf8_lossy(&output.stdout[..max_len]);
    let cleaned = clean_subtitle_cues(&raw);
    if cleaned.trim().is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_subtitle_cues_srt() {
        let sample_srt = r#"
1
00:00:01,000 --> 00:00:04,500
Welcome to Calculus 101. Today we discuss derivatives.

2
00:00:05,000 --> 00:00:09,000
<i>The limit definition of a derivative is fundamental.</i>
"#;
        let cleaned = clean_subtitle_cues(sample_srt);
        assert!(cleaned.contains("Welcome to Calculus 101"));
        assert!(cleaned.contains("Today we discuss derivatives"));
        assert!(cleaned.contains("The limit definition of a derivative is fundamental"));
        assert!(!cleaned.contains("-->"));
        assert!(!cleaned.contains("<i>"));
    }

    #[test]
    fn test_clean_subtitle_cues_vtt() {
        let sample_vtt = r#"WEBVTT
NOTE This is a lecture transcript

1
00:01:10.500 --> 00:01:15.000
Fourier transform converts time domain to frequency domain.
"#;
        let cleaned = clean_subtitle_cues(sample_vtt);
        assert!(cleaned.contains("Fourier transform converts time domain to frequency domain"));
        assert!(!cleaned.contains("WEBVTT"));
        assert!(!cleaned.contains("NOTE"));
    }
}
