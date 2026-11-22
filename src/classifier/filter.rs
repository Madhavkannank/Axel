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
    "common",
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

const SKIP_FILE_EXTENSIONS: &[&str] = &[
    // Game archives, disk images, VM images, ROMs
    "iso", "bin", "cue", "dmg", "vdi", "vmdk", "qcow2", "pak", "vpk", "bsp", "wad", "dat",
    "sav", "save", "rom", "nes", "sfc", "gba", "nds", "xci", "nsp",
    // Compiled binaries and system libraries
    "exe", "msi", "dll", "so", "dylib", "class", "pyc", "pyo", "pyd", "o", "obj", "a", "lib",
    "jar", "war", "apk", "deb", "rpm", "cab", "elf",
    // Multipart archive fragments
    "part1.rar", "part01.rar", "r00", "r01", "7z", "rar",
    // Heavy media and 3D meshes (not document text or code)
    "mp4", "mkv", "avi", "mov", "flv", "wmv", "webm", "mp3", "wav", "flac", "ogg", "m4a",
    "aac", "fbx", "blend", "stl", "gltf", "glb", "unitypackage", "torrent",
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

/// Determine if a directory should be skipped completely during traversal.
pub fn should_skip_directory(name: &str) -> bool {
    // Hidden directories (starts with '.')
    if name.starts_with('.') && name != "." && name != ".." {
        return true;
    }

    let lower = name.to_lowercase();

    // Check exact name matches
    if SKIP_DIR_EXACT.iter().any(|&s| s.eq_ignore_ascii_case(&lower)) {
        return true;
    }

    // Check substring matches (repacks, modpacks, game saves, cracks)
    if SKIP_DIR_CONTAINS.iter().any(|&sub| lower.contains(sub)) {
        return true;
    }

    false
}

/// Determine if a file should be skipped.
pub fn should_skip_file(path: &Path, file_size: Option<u64>) -> bool {
    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };

    // Hidden or temporary files
    if file_name.starts_with('.') || file_name.starts_with('~') || file_name.starts_with("updated_") {
        return true;
    }

    let lower = file_name.to_lowercase();

    // Minified code, source maps, and dependency lock files (megabytes of noise)
    if lower.ends_with(".min.js")
        || lower.ends_with(".min.css")
        || lower.ends_with(".bundle.js")
        || lower.ends_with(".map")
        || lower.ends_with(".lock")
        || lower == "package-lock.json"
        || lower == "yarn.lock"
        || lower == "pnpm-lock.yaml"
        || lower == "cargo.lock"
    {
        return true;
    }

    // Check file extension
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if SKIP_FILE_EXTENSIONS.iter().any(|&e| e == ext) {
        return true;
    }

    // File size filter: skip any single file > 10MB to guarantee < 10MB RSS memory ceiling
    if let Some(size) = file_size {
        if size > 10 * 1024 * 1024 {
            return true;
        }
        // Text / script files larger than 1MB are almost always generated dumps or logs
        if (is_script_extension(&ext) || is_doc_extension(&ext)) && size > 1024 * 1024 {
            return true;
        }
    }

    // Check if it's a known processable extension
    if !is_doc_extension(&ext) && !is_script_extension(&ext) && !is_image_extension(&ext) {
        return true;
    }

    false
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
    fn test_skip_game_and_binary_files() {
        assert!(should_skip_file(Path::new("game.pak"), None));
        assert!(should_skip_file(Path::new("installer.iso"), None));
        assert!(should_skip_file(Path::new("payload.exe"), None));
        assert!(should_skip_file(Path::new("app.min.js"), None));
        assert!(should_skip_file(Path::new("package-lock.json"), None));
        assert!(should_skip_file(Path::new("Cargo.lock"), None));
    }

    #[test]
    fn test_allow_normal_docs_and_scripts() {
        assert!(!should_skip_file(Path::new("invoice.pdf"), Some(50_000)));
        assert!(!should_skip_file(Path::new("thesis.docx"), Some(80_000)));
        assert!(!should_skip_file(Path::new("1.py"), Some(600)));
        assert!(!should_skip_file(Path::new("main.rs"), Some(4000)));
        assert!(!should_skip_file(Path::new("script.js"), Some(1200)));
    }
}
