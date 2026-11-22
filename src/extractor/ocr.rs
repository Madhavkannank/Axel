use std::path::Path;
use std::process::Command;

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

pub fn extract_text<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let path = path.as_ref();
    let bin = match find_tesseract_binary() {
        Some(b) => b,
        None => {
            return Err("Tesseract OCR binary not found on system PATH. Install tesseract to enable image OCR.".to_string());
        }
    };

    let output = Command::new(bin)
        .arg(path)
        .arg("stdout")
        .arg("--oem")
        .arg("1")
        .output()
        .map_err(|e| format!("Failed to execute Tesseract: {e}"))?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(text)
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("Tesseract failed: {err}"))
    }
}
