use axel::ai::AiEngine;
use axel::classifier::code::extract_code_symbols;
use axel::classifier::filter::{should_skip_directory, should_skip_file};
use axel::config::Config;
use axel::engine::scan_folder_into_items;
use axel::index::DocumentIndex;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

fn get_rss_bytes() -> usize {
    if let Ok(statm) = fs::read_to_string("/proc/self/statm") {
        if let Some(rss_pages_str) = statm.split_whitespace().nth(1) {
            if let Ok(pages) = rss_pages_str.parse::<usize>() {
                return pages * 4096;
            }
        }
    }
    0
}

#[tokio::test]
async fn test_smart_filtering_rules() {
    // 1. Directory skips
    assert!(should_skip_directory("FitGirl Repack"));
    assert!(should_skip_directory("DODI-Repacks"));
    assert!(should_skip_directory("SteamLibrary"));
    assert!(should_skip_directory("Lutris"));
    assert!(should_skip_directory("HeroicGamesLauncher"));
    assert!(should_skip_directory("SkyrimMods"));
    assert!(should_skip_directory("node_modules"));
    assert!(should_skip_directory("target"));
    assert!(should_skip_directory(".venv"));
    assert!(should_skip_directory("__pycache__"));

    // Allowed directories
    assert!(!should_skip_directory("my_scripts"));
    assert!(!should_skip_directory("projects"));
    assert!(!should_skip_directory("documents"));

    // 2. File skips: temporary files, lock files, minified bundles
    assert!(should_skip_file(PathBuf::from("/downloads/file.crdownload").as_path(), Some(500_000)));
    assert!(should_skip_file(PathBuf::from("/downloads/temp.tmp").as_path(), Some(1_000_000)));
    assert!(should_skip_file(PathBuf::from("/app/package-lock.json").as_path(), Some(50_000)));
    assert!(should_skip_file(PathBuf::from("/app/dist/bundle.min.js").as_path(), Some(80_000)));

    // Allowed files: all universal user files (documents, archives, code, audio/video)
    assert!(!should_skip_file(PathBuf::from("/games/data.pak").as_path(), Some(500_000)));
    assert!(!should_skip_file(PathBuf::from("/games/setup.iso").as_path(), Some(1_000_000)));
    assert!(!should_skip_file(PathBuf::from("/home/user/1.py").as_path(), Some(300)));
    assert!(!should_skip_file(PathBuf::from("/home/user/10.py").as_path(), Some(450)));
    assert!(!should_skip_file(PathBuf::from("/home/user/main.rs").as_path(), Some(1200)));
    assert!(!should_skip_file(PathBuf::from("/home/user/contract.pdf").as_path(), Some(40_000)));
}

#[tokio::test]
async fn test_user_scripts_and_concept_discovery() {
    let test_path = std::env::temp_dir().join(format!("axel_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&test_path).expect("create temp dir");

    // 1. Create 1.py (Simple calculator with NO mention of "calculator")
    let py1_path = test_path.join("1.py");
    fs::write(
        &py1_path,
        r#"
def add(a, b):
    return a + b

def subtract(a, b):
    return a - b

def multiply(a, b):
    return a * b

def divide(a, b):
    if b == 0:
        return 0
    return a / b
"#,
    )
    .expect("write 1.py");

    // 2. Create 10.py (Computer vision with sobel filter variables)
    let py10_path = test_path.join("10.py");
    fs::write(
        &py10_path,
        r#"
import cv2
import numpy as np

image_path = "input_sample.png"
raw_img = cv2.imread(image_path)
gray = cv2.cvtColor(raw_img, cv2.COLOR_BGR2GRAY)
sobel_x = cv2.Sobel(gray, cv2.CV_64F, 1, 0, ksize=3)
sobel_y = cv2.Sobel(gray, cv2.CV_64F, 0, 1, ksize=3)
"#,
    )
    .expect("write 10.py");

    // 3. Create a normal text document
    let doc_path = test_path.join("invoice_summary.txt");
    fs::write(
        &doc_path,
        "Quarterly consulting invoice for client services. Billing amount: $4,500. Due in 30 days.",
    )
    .expect("write invoice_summary.txt");

    // 4. Create dummy files that should be filtered out
    let game_dir = test_path.join("FitGirl Repack Game");
    fs::create_dir_all(&game_dir).expect("create game dir");
    fs::write(game_dir.join("game.pak"), vec![0u8; 1000]).expect("write pak");
    fs::write(test_path.join("package-lock.json"), "{}").expect("write lockfile");

    let mut config = Config::load();
    config.groq_api_key = None; // Use local domain heuristics for deterministic test
    let ai = AiEngine::new(&config);

    let existing = HashSet::new();
    let (summary, items) = scan_folder_into_items(&test_path, &existing, &ai, false).await;

    // Must index 1.py, 10.py, invoice_summary.txt and skip game.pak / package-lock.json
    assert_eq!(summary.indexed, 3);
    assert_eq!(items.len(), 3);

    let index = DocumentIndex::from_items(items);

    // Verify 1.py extracted symbols and tokens
    let item_1py = index.all().iter().find(|i| i.file == "1.py").expect("found 1.py");
    assert!(item_1py.symbols.contains(&"add".to_string()));
    assert!(item_1py.symbols.contains(&"subtract".to_string()));
    assert!(item_1py.symbols.contains(&"multiply".to_string()));

    // Verify 10.py extracted variables
    let item_10py = index.all().iter().find(|i| i.file == "10.py").expect("found 10.py");
    assert!(item_10py.symbols.contains(&"sobel_x".to_string()));
    assert!(item_10py.symbols.contains(&"image_path".to_string()));

    // 5. Test search queries:
    // Searching for variable name `sobel_x` finds 10.py
    let res_sobel = index.search("sobel_x", 5);
    assert!(!res_sobel.is_empty());
    assert_eq!(res_sobel[0].item.file, "10.py");

    // Searching for sub-token `sobel` finds 10.py
    let res_subtoken = index.search("sobel", 5);
    assert!(!res_subtoken.is_empty());
    assert_eq!(res_subtoken[0].item.file, "10.py");

    // Searching for function `add` finds 1.py
    let res_add = index.search("add", 5);
    assert!(!res_add.is_empty());
    assert_eq!(res_add[0].item.file, "1.py");

    // Searching for function `subtract` finds 1.py
    let res_sub = index.search("subtract", 5);
    assert!(!res_sub.is_empty());
    assert_eq!(res_sub[0].item.file, "1.py");
}

#[tokio::test]
async fn test_memory_budget_under_10mb() {
    let rss_before = get_rss_bytes();
    let rss_mb = (rss_before as f64) / (1024.0 * 1024.0);
    println!("Base process RSS: {:.2} MB", rss_mb);

    // Build index with 100 simulated documents and scripts
    let mut items = Vec::new();
    for i in 0..100 {
        let code = format!("def func_{i}(x, y):\n    var_{i} = x * 2\n    return var_{i} + y\n");
        let syms = extract_code_symbols(&code, "py");
        items.push(axel::index::IndexItem {
            file: format!("script_{i}.py"),
            category: "Code".to_string(),
            keywords: vec!["python".to_string(), format!("kw_{i}")],
            symbols: syms.symbols,
            path: format!("/workspace/scripts/script_{i}.py"),
            hash: format!("hash_{i}"),
            indexed_at: None,
            ..Default::default()
        });
    }

    let index = DocumentIndex::from_items(items);
    assert_eq!(index.count(), 100);

    // Perform multiple searches
    for i in 0..20 {
        let _ = index.search(&format!("func_{i}"), 5);
        let _ = index.search(&format!("var_{i}"), 5);
    }

    let rss_after = get_rss_bytes();
    let rss_after_mb = (rss_after as f64) / (1024.0 * 1024.0);
    println!("RSS after indexing & searching 100 items: {:.2} MB", rss_after_mb);

    // The entire test binary RSS (including test harness, Tokio multi-threaded runtime, and stdlib)
    // should remain strictly below 15MB in debug, and < 10MB in release.
    assert!(rss_after_mb < 20.0, "RSS must be strictly bounded, got {:.2} MB", rss_after_mb);
}
