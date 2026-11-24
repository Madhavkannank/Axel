use std::path::Path;

/// Directories to skip: applications, games, repacks, modpacks, caches, build artifacts, and vendor trees.
const SKIP_DIR_EXACT: &[&str] = &[
    // Build artifacts and package managers
    "target",
    "node_modules",
    "__pycache__",
    ".git",
    ".svn",
    ".hg",
    ".venv",
    "venv",
    "env",
    ".gradle",
    ".cargo",
    ".rustup",
    ".nuget",
    "vendor",
    "bower_components",
    "dist",
    "build",
    "out",
    "pkg",
    "bin",
    "obj",
    ".next",
    ".nuxt",
    ".cache",
    ".turbo",
    // OS and application installation paths
    "Applications",
    "Library",
    "System",
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "AppData",
    "drive_c",
    "pfx",
    "dosdevices",
    "flatpak",
    "snap",
    // Game distribution & runners
    ".steam",
    "steamapps",
    "compatdata",
    "lutris",
    "heroic",
    "Epic Games",
    "GOG Games",
    "Origin Games",
    "Ubisoft",
    "Battle.net",
    "Riot Games",
    // Asset / shader / engine blobs
    "shadercache",
    "shaders",
    "directx",
    "redist",
    "_CommonRedist",
    "vcredist",
    "textures",
    "meshes",
    "soundbanks",
    "raw_footage",
];

const SKIP_DIR_CONTAINS: &[&str] = &[
    // Game distribution, runners, and libraries
    "steam",
    "steamlibrary",
    "steamapps",
    "lutris",
    "heroic",
    "epicgames",
    "goggames",
    // Repack signatures
    "fitgirl",
    "dodi",
    "repack",
    "skidrow",
    "codex",
    "empress",
    "goldberg",
    "cpy",
    "plaza",
    "rune",
    "tenoke",
    "elamigos",
    "kaos",
    "crack",
    // Game modding
    "modpack",
    "modpacks",
    "mods",
    "savegame",
    "shadercache",
];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "flv", "wmv", "webm", "m4v",
];

/// Supported document extensions
const DOC_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "doc", "odt", "rtf", "txt", "md", "markdown", "rst", "org", "tex", "bib",
    "csv", "tsv", "json", "jsonl", "yaml", "yml", "toml", "xml",
];

/// Supported user code / script extensions
const SCRIPT_EXTENSIONS: &[&str] = &[
    "py", "rs", "js", "mjs", "cjs", "ts", "tsx", "jsx", "c", "cpp", "cc", "cxx", "h", "hpp",
    "hxx", "go", "java", "sh", "bash", "zsh", "sql", "html", "htm", "css", "scss", "lua", "rb",
    "php", "r", "kt", "swift", "pl", "pm",
];

/// Supported image extensions (document scans, receipts, etc.)
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "tiff", "tif"];

fn extract_dir_tokens(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = name.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
        } else if c.is_uppercase() && !current.is_empty() && chars[i - 1].is_lowercase() {
            tokens.push(current.to_lowercase());
            current.clear();
            current.push(c);
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }
    tokens
}

/// Determine if a directory should be skipped completely during traversal.
pub fn should_skip_directory(name: &str) -> bool {
    // Hidden directories (starts with '.')
    if name.starts_with('.') && name != "." && name != ".." {
        return true;
    }

    let lower = name.to_lowercase();

    // Check user-configured explicit includes (AXEL_INCLUDE_DIRS overrides default exclusions)
    if let Ok(includes) = std::env::var("AXEL_INCLUDE_DIRS") {
        for incl in includes.split(',') {
            let clean = incl.trim().to_lowercase();
            if !clean.is_empty() && (lower == clean || lower.contains(&clean)) {
                return false;
            }
        }
    }

    // Check user-configured custom excludes (AXEL_EXCLUDE_DIRS)
    if let Ok(excludes) = std::env::var("AXEL_EXCLUDE_DIRS") {
        for excl in excludes.split(',') {
            let clean = excl.trim().to_lowercase();
            if !clean.is_empty() && (lower == clean || lower.contains(&clean)) {
                return true;
            }
        }
    }

    // Check exact name matches
    if SKIP_DIR_EXACT.iter().any(|&s| s.eq_ignore_ascii_case(&lower)) {
        return true;
    }

    // Delimited and camelCase token analysis to prevent false substring matches (e.g. "Pruned Records" must NOT match "rune")
    let tokens = extract_dir_tokens(name);

    for &sub in SKIP_DIR_CONTAINS {
        // Exact token match or whole lower match
        if tokens.iter().any(|t| t == sub) || lower == sub {
            return true;
        }

        // Token prefix matches (e.g. token "repacks" starts with "repack", "steam" starts with "steam")
        if (sub == "repack" || sub == "steam" || sub == "heroic" || sub == "lutris" || sub == "modpack" || sub == "shadercache" || sub == "fitgirl" || sub == "dodi")
            && tokens.iter().any(|t| t.starts_with(sub))
        {
            return true;
        }

        // Distinct compound keywords in full string
        if (sub == "steam" || sub == "shadercache" || sub == "modpack" || sub == "savegame" || sub == "heroic" || sub == "lutris" || sub == "fitgirl" || sub == "dodi")
            && lower.contains(sub)
        {
            return true;
        }
    }

    false
}

/// Determine if a file should be skipped.
pub fn should_skip_file(path: &Path, _file_size: Option<u64>) -> bool {
    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };

    // Hidden or temporary files
    if file_name.starts_with('.') || file_name.starts_with('~') || file_name.starts_with("updated_") {
        return true;
    }

    let lower = file_name.to_lowercase();

    // Minified code, source maps, in-progress downloads, and dependency lock files
    if lower.ends_with(".crdownload")
        || lower.ends_with(".part")
        || lower.ends_with(".tmp")
        || lower.ends_with(".swp")
        || lower.ends_with(".min.js")
        || lower.ends_with(".min.css")
        || lower.ends_with(".bundle.js")
        || lower.ends_with(".map")
        || lower == "package-lock.json"
        || lower == "yarn.lock"
        || lower == "pnpm-lock.yaml"
        || lower == "cargo.lock"
    {
        return true;
    }

    false
}

pub fn is_video_extension(ext: &str) -> bool {
    VIDEO_EXTENSIONS.contains(&ext)
}

pub fn is_doc_extension(ext: &str) -> bool {
    DOC_EXTENSIONS.contains(&ext)
}

pub fn is_script_extension(ext: &str) -> bool {
    SCRIPT_EXTENSIONS.contains(&ext)
}

pub fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_directories() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory("target"));
        assert!(should_skip_directory(".venv"));
        assert!(should_skip_directory("FitGirl Repack"));
        assert!(should_skip_directory("DODI-Repacks"));
        assert!(should_skip_directory("steamapps"));
        assert!(should_skip_directory("ModPacks"));
        assert!(!should_skip_directory("Documents"));
        assert!(!should_skip_directory("src"));
        assert!(!should_skip_directory("scripts"));
        assert!(!should_skip_directory("Dl"));
    }

    #[test]
    fn test_skip_temporary_and_lock_files() {
        assert!(should_skip_file(Path::new("download.crdownload"), None));
        assert!(should_skip_file(Path::new("file.part"), None));
        assert!(should_skip_file(Path::new("cache.tmp"), None));
        assert!(should_skip_file(Path::new("app.min.js"), None));
        assert!(should_skip_file(Path::new("package-lock.json"), None));
        assert!(should_skip_file(Path::new("Cargo.lock"), None));
    }

    #[test]
    fn test_allow_universal_files() {
        assert!(!should_skip_file(Path::new("invoice.pdf"), Some(50_000)));
        assert!(!should_skip_file(Path::new("thesis.docx"), Some(80_000)));
        assert!(!should_skip_file(Path::new("5thSemQPs.pdf"), Some(15_000_000)));
        assert!(!should_skip_file(Path::new("installer.iso"), Some(2_000_000_000)));
        assert!(!should_skip_file(Path::new("archive.zip"), Some(500_000)));
        assert!(!should_skip_file(Path::new("song.mp3"), Some(5_000_000)));
        assert!(!should_skip_file(Path::new("1.py"), Some(600)));
        assert!(!should_skip_file(Path::new("main.rs"), Some(4000)));
        assert!(!should_skip_file(Path::new("script.js"), Some(1200)));
    }
}
