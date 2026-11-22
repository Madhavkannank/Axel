use std::collections::{HashMap, HashSet};

pub struct RuleClassifier {
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
        let mut rules = HashMap::new();

        rules.insert("Government ID", [
            "aadhaar", "pan", "passport", "licence", "license", "uidai", "voter", "id", "dob",
            "nationality", "father", "address", "government", "identity", "card", "cardholder",
            "republic", "india", "dl", "epic", "resident", "enrollment", "male", "female"
        ].into_iter().collect());

        rules.insert("Invoice", [
            "invoice", "receipt", "bill", "gst", "gstin", "qty", "items", "price", "paid",
            "order", "total", "subtotal", "tax", "hsn", "sac", "cgst", "sgst", "igst",
            "due", "payment", "vendor", "billed", "shipping", "discount", "amount", "unit"
        ].into_iter().collect());

        rules.insert("Finance", [
            "bank", "salary", "payslip", "debit", "credit", "statement", "balance", "upi",
            "txn", "transactions", "deposit", "withdrawal", "account", "ifsc", "interest",
            "fund", "mutual", "portfolio", "dividend", "equity", "neft", "rtgs", "imps", "cheque"
        ].into_iter().collect());

        rules.insert("Academic", [
            "assignment", "lecture", "research", "thesis", "project", "regression", "classification",
            "dataset", "analysis", "university", "college", "semester", "syllabus", "course",
            "professor", "student", "grade", "examination", "paper", "abstract", "curriculum"
        ].into_iter().collect());

        rules.insert("Resume", [
            "resume", "curriculum", "vitae", "candidate", "skills", "experience", "internship",
            "achievements", "profile", "education", "technologies", "bachelor", "master",
            "summary", "certifications", "projects", "employment", "developer", "engineer"
        ].into_iter().collect());

        rules.insert("Legal", [
            "agreement", "affidavit", "clause", "contract", "declaration", "witness", "legal",
            "notice", "property", "transfer", "jurisdiction", "indemnity", "tenant", "landlord",
            "lease", "notary", "stamp", "arbitration", "covenant", "deed", "terms"
        ].into_iter().collect());

        rules.insert("Medical", [
            "prescription", "doctor", "diagnosis", "hospital", "discharge", "dosage", "lab",
            "hemoglobin", "scan", "treatment", "patient", "clinical", "medicine", "tablets",
            "clinic", "physician", "pathology", "symptoms", "radiology", "blood", "report"
        ].into_iter().collect());

        rules.insert("Travel", [
            "train", "flight", "boarding", "ticket", "pnr", "coach", "route", "checkin",
            "checkout", "hotel", "seat", "airline", "booking", "terminal", "gate",
            "departure", "arrival", "passenger", "itinerary", "visa", "reservation"
        ].into_iter().collect());

        rules.insert("Personal", [
            "todo", "notes", "reminder", "daily", "meeting", "friend", "tasks", "pending",
            "completed", "journal", "diary", "idea", "shopping", "recipe", "family", "personal"
        ].into_iter().collect());

        rules.insert("Technical", [
            "api", "database", "code", "python", "docker", "function", "server", "frontend",
            "backend", "deployment", "rust", "javascript", "typescript", "microservice",
            "container", "git", "linux", "cloud", "aws", "gcp", "architecture", "query"
        ].into_iter().collect());

        let noise = [
            "the", "and", "this", "is", "document", "for", "of", "file", "record",
            "notice", "attention", "official", "please", "see", "attached", "below",
            "summary", "data", "info", "confidential", "page", "signature", "authorized",
            "date", "amount", "copy", "printed", "scanned", "version", "final", "latest",
            "original", "digital", "archived", "verified", "signed", "photocopy", "report"
        ].into_iter().collect();

        Self { rules, noise }
    }

    pub fn classify(&self, text: &str) -> (String, f64) {
        if text.trim().is_empty() {
            return ("Unknown".to_string(), 0.0);
        }

        let words: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 2 && !self.noise.contains(w))
            .map(|s| s.to_string())
            .collect();

        if words.is_empty() {
            return ("General".to_string(), 0.1);
        }

        let mut scores: HashMap<&'static str, usize> = HashMap::new();
        for word in &words {
            for (category, kws) in &self.rules {
                if kws.contains(word.as_str()) {
                    *scores.entry(category).or_insert(0) += 1;
                }
            }
        }

        if let Some((&best_cat, &count)) = scores.iter().max_by_key(|(_, &v)| v) {
            let confidence = ((count as f64) / 10.0).min(1.0).max(0.3);
            (best_cat.to_string(), confidence)
        } else {
            ("General".to_string(), 0.2)
        }
    }
}
