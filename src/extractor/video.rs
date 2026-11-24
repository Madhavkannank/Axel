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

/// Parse television, webseries, and media filename stems into (Series Title, Season Episode, Episode Title)
pub fn parse_media_filename_metadata(stem: &str) -> (Option<String>, Option<String>, Option<String>) {
    let clean_stem = stem.trim();
    let parts: Vec<&str> = clean_stem
        .split(|c: char| c == '_' || c == '.' || c == '-')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut season_ep_idx = None;
    let mut season_ep_str = None;

    for (idx, part) in parts.iter().enumerate() {
        let p_upper = part.to_uppercase();
        // Pattern 1: S01E02 or S1E2
        if p_upper.starts_with('S') && p_upper.contains('E') {
            let rest = &p_upper[1..];
            if let Some(e_idx) = rest.find('E') {
                let s_num = &rest[..e_idx];
                let e_num = &rest[e_idx + 1..];
                if s_num.chars().all(|c| c.is_ascii_digit())
                    && e_num.chars().all(|c| c.is_ascii_digit())
                    && !s_num.is_empty()
                    && !e_num.is_empty()
                {
                    season_ep_idx = Some(idx);
                    season_ep_str = Some(format!(
                        "Season {}, Episode {}",
                        s_num.trim_start_matches('0'),
                        e_num.trim_start_matches('0')
                    ));
                    break;
                }
            }
        }
        // Pattern 2: 1x02 or 01x02
        if let Some(x_idx) = p_upper.find('X') {
            let s_num = &p_upper[..x_idx];
            let e_num = &p_upper[x_idx + 1..];
            if s_num.chars().all(|c| c.is_ascii_digit())
                && e_num.chars().all(|c| c.is_ascii_digit())
                && !s_num.is_empty()
                && !e_num.is_empty()
            {
                season_ep_idx = Some(idx);
                season_ep_str = Some(format!(
                    "Season {}, Episode {}",
                    s_num.trim_start_matches('0'),
                    e_num.trim_start_matches('0')
                ));
                break;
            }
        }
        // Pattern 3: EP02 or EPISODE 2
        if (p_upper.starts_with("EP") || p_upper.starts_with("EPISODE")) && idx > 0 {
            let num_part = p_upper.trim_start_matches("EPISODE").trim_start_matches("EP");
            if num_part.chars().all(|c| c.is_ascii_digit()) && !num_part.is_empty() {
                season_ep_idx = Some(idx);
                season_ep_str = Some(format!("Episode {}", num_part.trim_start_matches('0')));
                break;
            }
        }
    }

    if let Some(idx) = season_ep_idx {
        let series_raw = &parts[..idx];
        // Filter out release group bracket tags like [Judas] or (HorribleSubs)
        let filtered_series: Vec<&str> = series_raw
            .iter()
            .copied()
            .filter(|tok| !(tok.starts_with('[') && tok.ends_with(']')) && !(tok.starts_with('(') && tok.ends_with(')')))
            .collect();

        let series_title = if !filtered_series.is_empty() {
            Some(filtered_series.join(" "))
        } else if !series_raw.is_empty() {
            Some(series_raw.join(" "))
        } else {
            None
        };

        // Clean episode title by truncating at standard release/codec/resolution tags
        let ep_raw = &parts[idx + 1..];
        let mut clean_ep_tokens = Vec::new();
        const CODEC_TAGS: &[&str] = &[
            "1080P", "720P", "4K", "2160P", "480P", "X264", "X265", "HEVC", "H264", "H265",
            "10BIT", "8BIT", "WEBRIP", "BLURAY", "HDTV", "WEBDL", "WEB", "DUAL", "AUDIO",
            "GER", "ENG", "ITA", "SPA", "FRA", "RUS", "HIN", "6CH", "5.1", "AAC", "DTS",
            "AC3", "REPACK", "PROPER", "REMUX", "HDR", "SDR", "MULTI", "SUBS",
        ];

        for tok in ep_raw {
            let tok_upper = tok.to_uppercase();
            if CODEC_TAGS.contains(&tok_upper.as_str()) {
                break;
            }
            clean_ep_tokens.push(*tok);
        }

        let episode_title = if !clean_ep_tokens.is_empty() {
            Some(clean_ep_tokens.join(" "))
        } else {
            None
        };

        (series_title, season_ep_str, episode_title)
    } else {
        (None, None, None)
    }
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

    // 1. Primary Title & Metadata from Filename (Highest semantic priority)
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let (series_title, season_ep, episode_title) = parse_media_filename_metadata(stem);
        if let Some(st) = series_title {
            text_blocks.push(format!("Series Title: {st}"));
            if intel.title.is_none() {
                intel.title = Some(st);
            }
        }
        if let Some(se) = season_ep {
            text_blocks.push(format!("Series Episode: {se}"));
        }
        if let Some(ep) = episode_title {
            text_blocks.push(format!("Episode Name: {ep}"));
        }
        let cleaned_name = stem.replace(['_', '.', '-'], " ");
        text_blocks.push(format!("Filename: {cleaned_name}"));
    }

    // 2. Probe container headers via ffprobe (if available)
    if let Ok(ffprobe_intel) = probe_container_headers(path) {
        if let Some(title) = &ffprobe_intel.title {
            if intel.title.is_none() {
                intel.title = Some(title.clone());
            }
            text_blocks.push(format!("Title: {title}"));
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

    // 3. Subtitles / Transcript (Semantic enrichment, bounded to 2000 chars so dialogue never drowns title)
    if let Some(sidecar_text) = find_and_read_sidecar_subtitles(path) {
        if !sidecar_text.is_empty() {
            let bounded_sub: String = sidecar_text.chars().take(2000).collect();
            intel.subtitle_text = bounded_sub.clone();
            text_blocks.push(format!("Transcript:\n{bounded_sub}"));
        }
    } else if let Some(embedded_sub) = extract_embedded_subtitle_stream(path) {
        if !embedded_sub.is_empty() {
            let bounded_sub: String = embedded_sub.chars().take(2000).collect();
            intel.subtitle_text = bounded_sub.clone();
            text_blocks.push(format!("Subtitles:\n{bounded_sub}"));
        }
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
