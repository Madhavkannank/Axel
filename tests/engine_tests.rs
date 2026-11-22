use axel::classifier::{KeywordExtractor, RuleClassifier};
use axel::extractor::{detect_file_type, FileType};
use axel::index::{DocumentIndex, IndexItem};

#[test]
fn test_rule_classification() {
    let classifier = RuleClassifier::new();

    let (cat, conf) = classifier.classify("Tax Invoice #1024 GSTIN items qty total price paid");
    assert_eq!(cat, "Invoice");
    assert!(conf >= 0.3);

    let (cat, conf) = classifier.classify("Patient medical prescription doctor dosage hospital clinic");
    assert_eq!(cat, "Medical");
    assert!(conf >= 0.3);

    let (cat, conf) = classifier.classify("Candidate curriculum vitae developer engineer experience skills");
    assert_eq!(cat, "Resume");
    assert!(conf >= 0.3);

    let (cat, conf) = classifier.classify("Government Aadhaar card UIDAI resident identity DOB");
    assert_eq!(cat, "Government ID");
    assert!(conf >= 0.3);
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
    assert_eq!(detect_file_type("notes.txt"), FileType::Text);
    assert_eq!(detect_file_type("archive.tar.gz"), FileType::Unsupported);
}

#[test]
fn test_document_index_search() {
    let mut index = DocumentIndex::load("non_existent_test_index.json");

    let item = IndexItem {
        file: "tax_invoice_2026.pdf".to_string(),
        category: "Invoice".to_string(),
        keywords: vec!["invoice".to_string(), "consulting".to_string(), "billing".to_string()],
        path: "/docs/tax_invoice_2026.pdf".to_string(),
        hash: "dummyhash123".to_string(),
        indexed_at: Some("2026-11-22T00:00:00Z".to_string()),
    };

    index.upsert(item);
    assert_eq!(index.count(), 1);

    let search_res = index.search("invoice", 5);
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].item.file, "tax_invoice_2026.pdf");
    assert_eq!(search_res[0].match_type, "exact");

    let empty_res = index.search("nonexistenttermxyz", 5);
    assert!(empty_res.is_empty());
}
