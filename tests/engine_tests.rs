use axel::ai::AiEngine;
use axel::classifier::KeywordExtractor;
use axel::config::Config;
use axel::extractor::docx::strip_docx_xml;
use axel::extractor::{detect_file_type, preprocess_text, FileType};
use axel::index::{DocumentIndex, IndexItem};

#[tokio::test]
async fn test_ai_document_analysis_pii_and_keywords() {
    let mut config = Config::load();
    config.groq_api_key = None; // Offline mode test
    let ai = AiEngine::new(&config);

    let doc = "Quarterly consulting invoice for client services. Billing amount: $4,500. Due in 30 days.";
    let res = ai.analyze_document(doc, "invoice.txt", None).await;
    assert_eq!(res.category, "General");
    assert!(res.keywords.contains(&"invoice".to_string()) || res.keywords.contains(&"consulting".to_string()));
}

#[test]
fn test_keyword_extraction() {
    let extractor = KeywordExtractor::new();
    let text = "Hospital discharge summary and prescription for patient Jane Smith. Doctor prescribed antibiotics and dosage.";
    let keywords = extractor.extract(text, 5);

    assert!(keywords.len() >= 3);
    assert!(keywords.len() <= 5);
    assert!(keywords.contains(&"prescription".to_string()) || keywords.contains(&"hospital".to_string()));
}

#[test]
fn test_file_type_detection() {
    assert_eq!(detect_file_type("report.pdf"), FileType::Pdf);
    assert_eq!(detect_file_type("contract.docx"), FileType::Docx);
    assert_eq!(detect_file_type("photo.jpeg"), FileType::Image);
    assert_eq!(detect_file_type("lecture_calculus.mp4"), FileType::Video);
    assert_eq!(detect_file_type("presentation.mkv"), FileType::Video);
    assert_eq!(detect_file_type("notes.txt"), FileType::Text);
    assert_eq!(detect_file_type("data.json"), FileType::Text);
    assert_eq!(detect_file_type("sheet.csv"), FileType::Text);
    assert_eq!(detect_file_type("archive.tar.gz"), FileType::Unsupported);
}

#[test]
fn test_preprocess_text_utf8_multibyte_safety() {
    // Generate a long text containing multi-byte characters:
    // emojis (4 bytes), Chinese (3 bytes), Hindi (3 bytes), Euro (3 bytes), curly quotes (3 bytes)
    let base_snippet = "🦀 Rust Document Intelligence 智慧文档 दस्तावेज़ €100 “quote” — dash. ";
    let repeated = base_snippet.repeat(100); // ~6,000 chars, well over 3,000 threshold

    // Must not panic on UTF-8 character boundary
    let processed = preprocess_text(&repeated);
    assert!(processed.contains("..."));
    assert!(processed.chars().count() <= 3100);
}

#[test]
fn test_docx_strip_xml() {
    let xml = r#"
        <w:p>
            <w:t>Header Title</w:t>
        </w:p>
        <w:p>
            <w:t xml:space="preserve">This is paragraph 1 &amp; details </w:t>
            <w:t/>
            <w:br/>
            <w:t>with second line &lt;important&gt;</w:t>
        </w:p>
    "#;

    let stripped = strip_docx_xml(xml);
    assert!(stripped.contains("Header Title"));
    assert!(stripped.contains("This is paragraph 1 & details"));
    assert!(stripped.contains("with second line <important>"));
    // Ensure self-closing tag didn't corrupt subsequent text
    assert!(!stripped.contains("<w:"));
}

#[test]
fn test_document_index_search_hybrid_blending() {
    let mut index = DocumentIndex::load("non_existent_test_index.json");

    let item1 = IndexItem {
        file: "tax_invoice_2026.pdf".to_string(),
        category: "Invoice".to_string(),
        keywords: vec!["invoice".to_string(), "consulting".to_string(), "billing".to_string()],
        symbols: Vec::new(),
        path: "/docs/tax_invoice_2026.pdf".to_string(),
        hash: "hash1".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    let item2 = IndexItem {
        file: "invoyce_statement.pdf".to_string(), // slight typo for fuzzy matching
        category: "Invoice".to_string(),
        keywords: vec!["invoyce".to_string(), "receipt".to_string()],
        symbols: Vec::new(),
        path: "/docs/invoyce_statement.pdf".to_string(),
        hash: "hash2".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    index.upsert(item1);
    index.upsert(item2);
    assert_eq!(index.count(), 2);

    let search_res = index.search("invoice", 5);
    assert!(!search_res.is_empty());
    // Highest score should be exact match
    assert_eq!(search_res[0].item.file, "tax_invoice_2026.pdf");
    assert!(search_res[0].score >= 0.9);

    let empty_res = index.search("nonexistenttermxyz", 5);
    assert!(empty_res.is_empty());
}

#[tokio::test]
async fn test_ai_filename_fallback_when_empty() {
    let mut config = Config::load();
    config.groq_api_key = None; // Offline fallback without hardcoded domain bias
    let ai = AiEngine::new(&config);

    // Empty text, but filename contains statistical tokens
    let res = ai.analyze_document("", "hospital_prescription_patient_jane.pdf", None).await;
    assert_eq!(res.category, "General");
    assert!(res.keywords.contains(&"prescription".to_string()) || res.keywords.contains(&"hospital".to_string()));
}

#[test]
fn test_document_index_deduplication() {
    let mut index = DocumentIndex::load("non_existent_test_index.json");

    let item = IndexItem {
        file: "contract.docx".to_string(),
        category: "Legal".to_string(),
        keywords: vec!["agreement".to_string(), "lease".to_string()],
        symbols: Vec::new(),
        path: "/docs/contract.docx".to_string(),
        hash: "contracthash123".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    index.upsert(item.clone());
    assert_eq!(index.count(), 1);

    // Re-upsert with same path but updated keywords
    let mut updated_item = item;
    updated_item.keywords.push("notary".to_string());
    index.upsert(updated_item);

    assert_eq!(index.count(), 1); // Must not duplicate
    assert_eq!(index.all()[0].keywords.len(), 3);
}

#[test]
fn test_document_index_remove_by_hash() {
    let mut index = DocumentIndex::load("non_existent_test_index.json");

    let item = IndexItem {
        file: "temp_note.txt".to_string(),
        category: "General".to_string(),
        keywords: vec!["note".to_string()],
        symbols: Vec::new(),
        path: "/docs/temp_note.txt".to_string(),
        hash: "removehash999".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
        ..Default::default()
    };

    index.upsert(item);
    assert_eq!(index.count(), 1);

    let removed = index.remove_by_hash("removehash999");
    assert!(removed.is_some());
    assert_eq!(index.count(), 0);
    assert!(index.remove_by_hash("missing").is_none());
}

#[test]
fn test_video_sidecar_subtitle_extraction() {
    let temp_dir = std::env::temp_dir().join(format!("axel_video_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let video_file = temp_dir.join("mit_calculus_lecture_01.mp4");
    let srt_file = temp_dir.join("mit_calculus_lecture_01.srt");

    // Create a dummy video file (empty or tiny)
    std::fs::write(&video_file, b"dummy video container header").unwrap();

    // Create sidecar .srt
    let srt_content = r#"
1
00:00:01,000 --> 00:00:05,000
Welcome to Calculus. Today we introduce derivatives and limits.

2
00:00:06,000 --> 00:00:10,000
The rate of change of a function is given by the derivative.
"#;
    std::fs::write(&srt_file, srt_content).unwrap();

    let (text, file_type) = axel::extractor::extract_text(&video_file);
    assert_eq!(file_type, FileType::Video);
    assert!(text.contains("Welcome to Calculus"));
    assert!(text.contains("derivatives and limits"));
    assert!(!text.contains("-->"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_watcher_filtering_and_duplicate_safety() {
    use axel::classifier::filter::{should_skip_directory, should_skip_file};
    use std::path::Path;

    // 1. Directories that watcher must automatically ignore
    assert!(should_skip_directory("SteamLibrary"));
    assert!(should_skip_directory("steamapps"));
    assert!(should_skip_directory("node_modules"));
    assert!(should_skip_directory("__pycache__"));
    assert!(should_skip_directory(".venv"));
    assert!(should_skip_directory("mods"));

    // 2. Binary and game files that watcher must ignore
    assert!(should_skip_file(Path::new("game.pak"), Some(1024)));
    assert!(should_skip_file(Path::new("setup.iso"), Some(1024)));
    assert!(should_skip_file(Path::new("bundle.min.js"), Some(1024)));
    assert!(should_skip_file(Path::new("package-lock.json"), Some(1024)));

    // 3. Normal user files that watcher MUST process
    assert!(!should_skip_file(Path::new("calculus_notes.pdf"), Some(1024)));
    assert!(!should_skip_file(Path::new("1.py"), Some(1024)));
    assert!(!should_skip_file(Path::new("lecture.mp4"), Some(1024)));
}

#[tokio::test]
async fn test_pan_and_aadhaar_photo_classification_and_multi_pointer() {
    use axel::classifier::privacy::is_high_risk_identity_document;
    use axel::index::virtual_tree::compute_virtual_paths;

    let mut config = Config::load();
    config.groq_api_key = None; // Offline mode for local unit test
    let ai = AiEngine::new(&config);

    // Case 1: Photo of PAN Card (even with generic camera filename IMG_0492.jpg)
    let ocr_pan_text = "INCOME TAX DEPARTMENT GOVT OF INDIA \n Permanent Account Number \n ABCDE1234F \n Name: John Doe \n Father: Bob Doe \n DOB: 14/08/1992";
    let is_pan_high_risk = is_high_risk_identity_document(ocr_pan_text, "IMG_0492.jpg");
    assert!(is_pan_high_risk, "PAN card must trigger high-risk identity detection");

    let pan_analysis = ai.analyze_document(ocr_pan_text, "IMG_0492.jpg", None).await;
    assert_eq!(pan_analysis.category, "Government ID");
    assert!(pan_analysis.keywords.contains(&"pan".to_string()));

    let llm_pan_vpaths = vec![
        "Identity & Documents/Government ID/PAN Card".to_string(),
        "Media/Document Scans".to_string(),
    ];
    let pan_vpaths = compute_virtual_paths("IMG_0492.jpg", &pan_analysis.category, &llm_pan_vpaths, "jpg");
    assert!(pan_vpaths.iter().any(|p| p.starts_with("Identity & Documents/Government ID/PAN Card/IMG_0492.jpg")));
    assert!(pan_vpaths.iter().any(|p| p.starts_with("Media/Document Scans/IMG_0492.jpg")));

    // Case 2: Photo of Aadhaar Card (generic filename photo_scan.png)
    let ocr_aadhaar_text = "GOVERNMENT OF INDIA \n Unique Identification Authority of India \n 9876 5432 1098 \n Resident Identity \n DOB: 01/01/1990";
    let is_aadhaar_high_risk = is_high_risk_identity_document(ocr_aadhaar_text, "photo_scan.png");
    assert!(is_aadhaar_high_risk, "Aadhaar must trigger high-risk identity detection");

    let aadhaar_analysis = ai.analyze_document(ocr_aadhaar_text, "photo_scan.png", None).await;
    assert_eq!(aadhaar_analysis.category, "Government ID");
    assert!(aadhaar_analysis.keywords.contains(&"aadhaar".to_string()));

    let llm_aadhaar_vpaths = vec![
        "Identity & Documents/Government ID/Aadhaar Card".to_string(),
        "Media/Document Scans".to_string(),
    ];
    let aadhaar_vpaths = compute_virtual_paths("photo_scan.png", &aadhaar_analysis.category, &llm_aadhaar_vpaths, "png");
    assert!(aadhaar_vpaths.iter().any(|p| p.starts_with("Identity & Documents/Government ID/Aadhaar Card/photo_scan.png")));
    assert!(aadhaar_vpaths.iter().any(|p| p.starts_with("Media/Document Scans/photo_scan.png")));

    // Case 3: Offline fallback without suggested LLM paths
    let fallback_paths = compute_virtual_paths("photo_scan.png", &aadhaar_analysis.category, &[], "png");
    assert_eq!(fallback_paths, vec!["Government ID/photo_scan.png"]);
}

