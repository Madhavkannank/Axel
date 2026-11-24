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
    use std::io::Read;
    let mut cmd = Command::new("pdftotext");
    cmd.arg("-layout")
        .arg("-nopgbrk")
        .arg(path)
        .arg("-")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;

    // A11: Bound process output during capture (5 MB max)
    const MAX_PDF_TEXT_BYTES: u64 = 5 * 1024 * 1024;
    let reader_handle = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 8192];
        let mut total_read = 0u64;
        while total_read < MAX_PDF_TEXT_BYTES {
            let to_read = (MAX_PDF_TEXT_BYTES - total_read).min(chunk.len() as u64) as usize;
            match stdout.read(&mut chunk[..to_read]) {
                Ok(0) => break,
                Ok(n) => {
                    buffer.extend_from_slice(&chunk[..n]);
                    total_read += n as u64;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        let mut sink = [0u8; 8192];
        while let Ok(n) = stdout.read(&mut sink) {
            if n == 0 {
                break;
            }
        }
        buffer
    });

    let timeout = std::time::Duration::from_secs(15);
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
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader_handle.join();
                return None;
            }
        }
    }

    if exit_status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader_handle.join();
        return None;
    }

    let status = exit_status.unwrap();
    let output = reader_handle.join().ok()?;
    if status.success() {
        let text = String::from_utf8_lossy(&output).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}
