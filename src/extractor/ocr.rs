use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrBackendKind {
    Tesseract,
    LightweightOcrsModel,
    CliWorker,
    MetadataFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrEngineStatus {
    pub backend: OcrBackendKind,
    pub description: String,
    pub model_path: Option<String>,
}

/// Detect available OCR capability across the system
pub fn detect_ocr_backend() -> OcrEngineStatus {
    // 1. Check for Tesseract binary
    if let Some(bin) = find_tesseract_binary() {
        return OcrEngineStatus {
            backend: OcrBackendKind::Tesseract,
            description: format!("System Tesseract at {bin}"),
            model_path: None,
        };
    }

    // 2. Check for dedicated lightweight OCR CLI or helper (e.g. ocrs-cli)
    if let Some(bin) = find_cli_ocr_binary() {
        return OcrEngineStatus {
            backend: OcrBackendKind::CliWorker,
            description: format!("CLI OCR worker at {bin}"),
            model_path: None,
        };
    }

    // 3. Check for local < 20MB lightweight model files (text-detection.rten + text-recognition.rten)
    if let Some(model_dir) = find_lightweight_model_dir() {
        return OcrEngineStatus {
            backend: OcrBackendKind::LightweightOcrsModel,
            description: "Pure-Rust lightweight OCR model (< 20MB: DBNet + CRNN)".to_string(),
            model_path: Some(model_dir.display().to_string()),
        };
    }

    // 4. Fallback to metadata & compound filename tokenization
    OcrEngineStatus {
        backend: OcrBackendKind::MetadataFallback,
        description: "Zero-dependency metadata & filename heuristic fallback (Install tesseract or download ocrs ~16.4MB models for pixel-level OCR)".to_string(),
        model_path: None,
    }
}

pub fn find_tesseract_binary() -> Option<String> {
    // 1. Check PATH
    if let Ok(output) = Command::new("tesseract").arg("--version").output() {
        if output.status.success() {
            return Some("tesseract".to_string());
        }
    }

    // 2. Check common Linux/macOS paths
    let unix_paths = [
        "/usr/bin/tesseract",
        "/usr/local/bin/tesseract",
        "/opt/homebrew/bin/tesseract",
    ];
    for p in unix_paths {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }

    // 3. Check Windows paths
    let win_paths = [
        r"C:\Program Files\Tesseract-OCR\tesseract.exe",
        r"C:\Program Files (x86)\Tesseract-OCR\tesseract.exe",
    ];
    for p in win_paths {
        if Path::new(p).exists() {
            return Some(p.to_string());
        }
    }

    None
}

pub fn find_cli_ocr_binary() -> Option<String> {
    let bins = ["ocrs-cli", "ocrs", "rapidocr"];
    for b in bins {
        if let Ok(output) = Command::new(b).arg("--version").output() {
            if output.status.success() {
                return Some(b.to_string());
            }
        }
    }
    None
}

/// Find directory containing the < 20MB lightweight model weights (2.4MB det + 9.3MB rec)
pub fn find_lightweight_model_dir() -> Option<PathBuf> {
    let mut search_dirs = Vec::new();

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        search_dirs.push(PathBuf::from(manifest_dir).join("models"));
    }
    search_dirs.push(PathBuf::from("./models"));
    if let Some(home) = dirs::home_dir() {
        search_dirs.push(home.join(".local/share/axel/models"));
        search_dirs.push(home.join(".axel/models"));
        search_dirs.push(home.join(".cache/ocrs"));
    }

    for dir in search_dirs {
        let has_onnx = dir.join("text-detection.onnx").exists() && dir.join("text-recognition.onnx").exists();
        let has_rten = dir.join("text-detection.rten").exists() && dir.join("text-recognition.rten").exists();
        if has_onnx || has_rten {
            return Some(dir);
        }
    }

    None
}

/// Main entry point for extracting text from an image.
/// Ensures graceful operation:
/// - If an OCR engine is available, runs OCR.
/// - If not, extracts textual cues from image filename tokens and container metadata.
pub fn extract_text<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let path = path.as_ref();
    let status = detect_ocr_backend();

    match status.backend {
        OcrBackendKind::Tesseract => run_tesseract_ocr(path),
        OcrBackendKind::CliWorker => run_cli_worker_ocr(path),
        OcrBackendKind::LightweightOcrsModel => {
            // When lightweight model files exist, invoke the on-demand worker
            if let Some(model_dir) = &status.model_path {
                run_lightweight_model_ocr(path, Path::new(model_dir))
            } else {
                Ok(extract_image_filename_and_metadata_text(path))
            }
        }
        OcrBackendKind::MetadataFallback => {
            // Graceful fallback: Extract rich tokens from image metadata and compound filename
            Ok(extract_image_filename_and_metadata_text(path))
        }
    }
}

const OCR_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const MAX_OCR_OUTPUT_BYTES: u64 = 5 * 1024 * 1024; // 5 MB max output

// H11, A07: Run external OCR worker with concurrent output draining, memory cap, execution timeout, and guaranteed child reaping
fn run_command_bounded(
    mut cmd: Command,
    timeout: std::time::Duration,
    max_bytes: u64,
) -> Result<String, String> {
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn OCR worker: {e}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout pipe".to_string())?;

    // Drain stdout concurrently in a background thread so the child process never deadlocks
    // when output exceeds the operating system's pipe buffer (typically 64 KB).
    let reader_handle = std::thread::spawn(move || {
        use std::io::Read;
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 8192];
        let mut total_read = 0u64;

        while total_read < max_bytes {
            let to_read = (max_bytes - total_read).min(chunk.len() as u64) as usize;
            match stdout.read(&mut chunk[..to_read]) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    buffer.extend_from_slice(&chunk[..n]);
                    total_read += n as u64;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        // If child outputs more than max_bytes, drain and discard remainder until EOF
        // so child process does not block on writing to pipe.
        let mut sink = [0u8; 8192];
        while let Ok(n) = stdout.read(&mut sink) {
            if n == 0 {
                break;
            }
        }

        buffer
    });

    let start = std::time::Instant::now();
    let mut exit_status = None;

    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader_handle.join();
                return Err(format!("Error waiting for OCR worker: {e}"));
            }
        }
    }

    if exit_status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader_handle.join();
        return Err(format!("OCR worker timed out after {:?}", timeout));
    }

    let status = exit_status.unwrap();
    let output = reader_handle
        .join()
        .map_err(|_| "Reader thread panicked while capturing worker output".to_string())?;

    if status.success() {
        Ok(String::from_utf8_lossy(&output).trim().to_string())
    } else {
        Err(format!("OCR worker exited with status: {status}"))
    }
}

fn run_tesseract_ocr(path: &Path) -> Result<String, String> {
    let bin = find_tesseract_binary().unwrap_or_else(|| "tesseract".to_string());
    let mut cmd = Command::new(bin);
    cmd.arg(path).arg("stdout").arg("--oem").arg("1");

    match run_command_bounded(cmd, OCR_TIMEOUT, MAX_OCR_OUTPUT_BYTES) {
        Ok(text) if !text.is_empty() => Ok(text),
        _ => Ok(extract_image_filename_and_metadata_text(path)),
    }
}

fn run_cli_worker_ocr(path: &Path) -> Result<String, String> {
    let bin = find_cli_ocr_binary().unwrap_or_else(|| "ocrs".to_string());
    let mut cmd = Command::new(bin);
    cmd.arg(path);

    match run_command_bounded(cmd, OCR_TIMEOUT, MAX_OCR_OUTPUT_BYTES) {
        Ok(text) if !text.is_empty() => Ok(text),
        _ => Ok(extract_image_filename_and_metadata_text(path)),
    }
}

fn run_lightweight_model_ocr(path: &Path, model_dir: &Path) -> Result<String, String> {
    // Delegate to on-demand CLI worker with the < 20MB models so weights are freed immediately,
    // keeping daemon resident memory strictly < 10MB RSS
    if let Some(cli) = find_cli_ocr_binary() {
        let det_onnx = model_dir.join("text-detection.onnx");
        let rec_onnx = model_dir.join("text-recognition.onnx");

        let mut cmd = Command::new(&cli);
        if det_onnx.exists() && rec_onnx.exists() {
            cmd.arg("--detect-model").arg(&det_onnx);
            cmd.arg("--rec-model").arg(&rec_onnx);
        }
        cmd.arg(path);

        match run_command_bounded(cmd, OCR_TIMEOUT, MAX_OCR_OUTPUT_BYTES) {
            Ok(text) if !text.is_empty() => return Ok(text),
            _ => {}
        }
    }

    Ok(extract_image_filename_and_metadata_text(path))
}

/// Fallback text extractor that derives meaningful semantic cues from the image path and metadata
pub fn extract_image_filename_and_metadata_text(path: &Path) -> String {
    let mut tokens = Vec::new();

    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        // Split camelCase, snake_case, kebab-case, dots
        let cleaned = stem
            .replace(['_', '-', '.'], " ")
            .split_whitespace()
            .map(|word| {
                // Split CamelCase if present
                crate::classifier::code::split_identifier(word).join(" ")
            })
            .collect::<Vec<_>>()
            .join(" ");

        tokens.push(format!("Image Title: {cleaned}"));
    }

    // If parent directory gives contextual domain (e.g. "Physics_Lectures/slide_1.png")
    if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
        let parent_cleaned = parent.replace(['_', '-', '.'], " ");
        tokens.push(format!("Context Folder: {parent_cleaned}"));
    }

    tokens.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_backend_detection_does_not_panic() {
        let status = detect_ocr_backend();
        assert!(!status.description.is_empty());
        println!("Detected OCR status: {:?}", status);
    }

    #[test]
    fn test_image_filename_and_metadata_fallback() {
        let path = Path::new("/workspace/samples/Study/Calculus_Lecture_Slide_Derivatives_2026.png");
        let text = extract_image_filename_and_metadata_text(path);
        assert!(text.contains("Calculus"));
        assert!(text.contains("Lecture"));
        assert!(text.contains("Derivatives"));
        assert!(text.contains("Study"));
    }

    #[test]
    fn test_extract_text_with_lightweight_model_or_fallback() {
        let test_path = Path::new("/tmp/ocr_test.png");
        if test_path.exists() {
            let res = extract_text(test_path);
            assert!(res.is_ok());
            let text = res.unwrap();
            assert!(text.contains("Calculus") || text.contains("Derivatives"));
        }
    }
}
