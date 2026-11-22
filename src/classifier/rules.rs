use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct RuleClassifier {
    strong_rules: HashMap<&'static str, HashSet<&'static str>>,
    rules: HashMap<&'static str, HashSet<&'static str>>,
    noise: HashSet<&'static str>,
}

impl Default for RuleClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleClassifier {
    pub fn new() -> Self {
        let mut strong_rules = HashMap::new();
        let mut rules = HashMap::new();

        strong_rules.insert(
            "Government ID",
            ["aadhaar", "uidai", "passport", "epic", "pan", "voter", "cardholder", "dob"]
                .into_iter()
                .collect(),
        );
        rules.insert(
            "Government ID",
            [
                "aadhaar", "pan", "passport", "licence", "license", "uidai", "voter", "dob",
                "nationality", "father", "address", "government", "identity", "cardholder",
                "republic", "india", "epic", "resident", "enrollment",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Invoice",
            ["invoice", "gstin", "receipt", "subtotal", "cgst", "sgst", "igst", "billed"]
                .into_iter()
                .collect(),
        );
        rules.insert(
            "Invoice",
            [
                "invoice", "receipt", "bill", "gst", "gstin", "qty", "price", "paid",
                "total", "subtotal", "tax", "hsn", "sac", "cgst", "sgst", "igst",
                "due", "payment", "vendor", "billed", "shipping", "discount", "amount", "unit",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Finance",
            ["payslip", "ifsc", "neft", "rtgs", "imps", "dividend", "equity", "portfolio"]
                .into_iter()
                .collect(),
        );
        rules.insert(
            "Finance",
            [
                "bank", "salary", "payslip", "debit", "credit", "statement", "balance", "upi",
                "txn", "transactions", "deposit", "withdrawal", "account", "ifsc", "interest",
                "fund", "mutual", "portfolio", "dividend", "equity", "neft", "rtgs", "imps", "cheque",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Academic",
            [
                "syllabus", "curriculum", "semester", "thesis", "examination", "qpaper",
                "midsem", "endsem", "lecture", "assignment", "university", "professor",
            ]
            .into_iter()
            .collect(),
        );
        rules.insert(
            "Academic",
            [
                "assignment", "lecture", "research", "thesis", "regression", "classification",
                "dataset", "university", "college", "semester", "syllabus", "course",
                "professor", "student", "grade", "examination", "paper", "abstract", "curriculum",
                "qpaper", "midsem", "endsem", "marks", "credits",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Resume",
            ["resume", "curriculum", "vitae", "cv", "internship", "certifications", "employment"]
                .into_iter()
                .collect(),
        );
        rules.insert(
            "Resume",
            [
                "resume", "curriculum", "vitae", "cv", "candidate", "skills", "experience",
                "internship", "achievements", "profile", "education", "technologies", "bachelor",
                "master", "certifications", "projects", "employment", "developer", "engineer",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Legal",
            [
                "affidavit", "indemnity", "jurisdiction", "arbitration", "covenant",
                "notary", "nda", "disclosure", "landlord", "tenant",
            ]
            .into_iter()
            .collect(),
        );
        rules.insert(
            "Legal",
            [
                "agreement", "affidavit", "clause", "contract", "declaration", "witness", "legal",
                "property", "transfer", "jurisdiction", "indemnity", "tenant", "landlord",
                "lease", "notary", "stamp", "arbitration", "covenant", "deed", "terms", "nda",
                "disclosure",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Medical",
            [
                "prescription", "diagnosis", "dosage", "hemoglobin", "pathology",
                "radiology", "physician", "symptoms", "discharge",
            ]
            .into_iter()
            .collect(),
        );
        rules.insert(
            "Medical",
            [
                "prescription", "doctor", "diagnosis", "hospital", "discharge", "dosage", "lab",
                "hemoglobin", "scan", "treatment", "patient", "clinical", "medicine", "tablets",
                "clinic", "physician", "pathology", "symptoms", "radiology", "blood",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Travel",
            ["pnr", "boarding", "itinerary", "airline", "reservation", "visa"]
                .into_iter()
                .collect(),
        );
        rules.insert(
            "Travel",
            [
                "train", "flight", "boarding", "ticket", "pnr", "coach", "route", "checkin",
                "checkout", "hotel", "seat", "airline", "booking", "terminal", "gate",
                "departure", "arrival", "passenger", "itinerary", "visa", "reservation",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Personal",
            ["journal", "diary", "recipe", "story", "script", "kadha", "todo", "shopping"]
                .into_iter()
                .collect(),
        );
        rules.insert(
            "Personal",
            [
                "todo", "notes", "reminder", "daily", "friend", "tasks", "pending",
                "completed", "journal", "diary", "idea", "shopping", "recipe", "family",
                "personal", "story", "script", "chapter", "scene",
            ]
            .into_iter()
            .collect(),
        );

        strong_rules.insert(
            "Technical",
            [
                "rust", "python", "typescript", "javascript", "microservice", "kubernetes",
                "docker", "postgresql", "api", "backend", "frontend", "compiler",
            ]
            .into_iter()
            .collect(),
        );
        rules.insert(
            "Technical",
            [
                "api", "database", "code", "python", "docker", "function", "server", "frontend",
                "backend", "deployment", "rust", "javascript", "typescript", "microservice",
                "container", "git", "linux", "cloud", "aws", "gcp", "architecture", "query",
                "algorithm", "problem", "input", "output", "test", "wires", "chips",
            ]
            .into_iter()
            .collect(),
        );

        let noise = [
            "the", "and", "this", "is", "document", "for", "of", "file", "record",
            "notice", "attention", "official", "please", "see", "attached", "below",
            "summary", "data", "info", "confidential", "page", "signature", "authorized",
            "date", "amount", "copy", "printed", "scanned", "version", "final", "latest",
            "original", "digital", "archived", "verified", "signed", "photocopy", "report",
        ]
        .into_iter()
        .collect();

        Self {
            strong_rules,
            rules,
            noise,
        }
    }

    pub fn classify(&self, text: &str) -> (String, f64) {
        self.classify_with_filename(text, "")
    }

    pub fn classify_with_filename(&self, text: &str, file_name: &str) -> (String, f64) {
        if text.trim().is_empty() && file_name.trim().is_empty() {
            return ("Unknown".to_string(), 0.0);
        }

        let mut scores: HashMap<&'static str, usize> = HashMap::new();
        let mut distinct_hits: HashMap<&'static str, HashSet<String>> = HashMap::new();

        // 1. Extension signal for source code / config / media
        if !file_name.is_empty() {
            let ext = Path::new(file_name)
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            if matches!(
                ext.as_str(),
                "py" | "rs" | "js" | "jsx" | "ts" | "tsx" | "go" | "java" | "c" | "cpp"
                    | "cc" | "h" | "hpp" | "sh" | "bash" | "zsh" | "sql" | "toml" | "yaml"
                    | "yml" | "css" | "scss" | "html" | "htm" | "json" | "jsonl"
            ) {
                *scores.entry("Technical").or_insert(0) += 6;
            } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tiff" | "tif") {
                *scores.entry("Media").or_insert(0) += 4;
            }

            // 2. Filename token signals (high precision)
            for token in file_name
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() >= 2 && !self.noise.contains(w))
            {
                for (&category, kws) in &self.rules {
                    if kws.contains(token) {
                        let weight = if self
                            .strong_rules
                            .get(category)
                            .map(|s| s.contains(token))
                            .unwrap_or(false)
                        {
                            8
                        } else {
                            4
                        };
                        *scores.entry(category).or_insert(0) += weight;
                        distinct_hits
                            .entry(category)
                            .or_default()
                            .insert(token.to_string());
                    }
                }
            }
        }

        // 3. Body text scoring (weighted by strong domain terms + distinct term diversity)
        let words: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 2 && !self.noise.contains(w))
            .map(|s| s.to_string())
            .collect();

        for word in &words {
            for (&category, kws) in &self.rules {
                if kws.contains(word.as_str()) {
                    let is_strong = self
                        .strong_rules
                        .get(category)
                        .map(|s| s.contains(word.as_str()))
                        .unwrap_or(false);
                    *scores.entry(category).or_insert(0) += if is_strong { 3 } else { 1 };
                    distinct_hits
                        .entry(category)
                        .or_default()
                        .insert(word.clone());
                }
            }
        }

        // Bonus for distinct keyword diversity so one repeated word doesn't hijack category
        for (cat, hits) in &distinct_hits {
            if hits.len() >= 2 {
                *scores.entry(cat).or_insert(0) += hits.len() * 2;
            }
        }

        if let Some((&best_cat, &score)) = scores
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        {
            if score >= 3 {
                let confidence = ((score as f64) / 18.0).clamp(0.35, 1.0);
                return (best_cat.to_string(), confidence);
            }
        }

        ("General".to_string(), 0.2)
    }
}
