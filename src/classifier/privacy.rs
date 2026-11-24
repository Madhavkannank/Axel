use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Default)]
pub struct RedactionSummary {
    pub aadhaar_count: usize,
    pub pan_count: usize,
    pub ssn_count: usize,
    pub card_count: usize,
    pub phone_count: usize,
    pub email_count: usize,
    pub name_count: usize,
    pub dob_count: usize,
}

impl RedactionSummary {
    pub fn total_redactions(&self) -> usize {
        self.aadhaar_count
            + self.pan_count
            + self.ssn_count
            + self.card_count
            + self.phone_count
            + self.email_count
            + self.name_count
            + self.dob_count
    }

    pub fn has_sensitive_id(&self) -> bool {
        self.aadhaar_count > 0 || self.pan_count > 0 || self.ssn_count > 0
    }
}

static RE_AADHAAR: OnceLock<Regex> = OnceLock::new();
static RE_PAN: OnceLock<Regex> = OnceLock::new();
static RE_SSN: OnceLock<Regex> = OnceLock::new();
static RE_CREDIT_CARD: OnceLock<Regex> = OnceLock::new();
static RE_PHONE: OnceLock<Regex> = OnceLock::new();
static RE_EMAIL: OnceLock<Regex> = OnceLock::new();
static RE_NAME_FIELD: OnceLock<Regex> = OnceLock::new();
static RE_DOB_FIELD: OnceLock<Regex> = OnceLock::new();

fn get_re_aadhaar() -> &'static Regex {
    RE_AADHAAR.get_or_init(|| {
        Regex::new(r"\b[2-9]\d{3}[ -]\d{4}[ -]\d{4}\b|\b[2-9]\d{11}\b").expect("aadhaar regex")
    })
}

fn get_re_pan() -> &'static Regex {
    RE_PAN.get_or_init(|| Regex::new(r"\b[A-Z]{5}[0-9]{4}[A-Z]\b").expect("pan regex"))
}

fn get_re_ssn() -> &'static Regex {
    RE_SSN.get_or_init(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("ssn regex"))
}

fn get_re_credit_card() -> &'static Regex {
    RE_CREDIT_CARD.get_or_init(|| Regex::new(r"\b(?:\d{4}[ -]?){3}\d{4}\b").expect("cc regex"))
}

fn get_re_phone() -> &'static Regex {
    RE_PHONE.get_or_init(|| {
        Regex::new(r"(?:\+?91[\-\s]?|0[\-\s]?)?[6-9]\d{9}\b|\b(?:\+?1[\-\s]?)?\(?\d{3}\)?[\-\s]?\d{3}[\-\s]?\d{4}\b")
            .expect("phone regex")
    })
}

fn get_re_email() -> &'static Regex {
    RE_EMAIL.get_or_init(|| {
        Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").expect("email regex")
    })
}

fn get_re_name_field() -> &'static Regex {
    RE_NAME_FIELD.get_or_init(|| {
        Regex::new(r"(?i)\b(?:Name|Full Name|Father's Name|Mother's Name|Cardholder|Applicant|Resident Name|Holder)\s*[:：\-]\s*([A-Za-z \t\.]{2,40})")
            .expect("name regex")
    })
}

fn get_re_dob_field() -> &'static Regex {
    RE_DOB_FIELD.get_or_init(|| {
        Regex::new(r"(?i)\b(?:DOB|Date of Birth|Birth Date|Year of Birth|YOB)\s*[:：\-]\s*([0-9]{1,4}[\/\-\.][0-9]{1,2}[\/\-\.][0-9]{2,4})")
            .expect("dob regex")
    })
}

/// Check if a document text or title represents a sensitive government identity document
/// that should NEVER be transmitted to external cloud LLM APIs.
pub fn is_high_risk_identity_document(text: &str, file_name: &str) -> bool {
    let lower_file = file_name.to_lowercase();

    // Check if filename suggests an identity card (using bounded tokens to avoid substrings in words like "japan", "company")
    if lower_file.contains("aadhaar")
        || lower_file.contains("aadhar")
        || lower_file.contains("passport")
        || lower_file.contains("voter")
        || lower_file.contains("ssn")
        || lower_file.contains("pan_card")
        || lower_file.contains("pancard")
        || lower_file.starts_with("pan.")
        || lower_file.starts_with("pan_")
        || lower_file.contains("_pan.")
        || lower_file.contains("_pan_")
        || lower_file.contains("-pan.")
        || lower_file.contains("-pan-")
    {
        return true;
    }

    let lower_text = text.to_lowercase();

    // Contextual identity markers
    let has_aadhaar_context = lower_text.contains("unique identification authority")
        || lower_text.contains("uidai")
        || lower_text.contains("enrolment no")
        || lower_text.contains("resident identity")
        || lower_text.contains("mera aadhaar");

    let has_pan_context = lower_text.contains("income tax department")
        || lower_text.contains("permanent account number")
        || (lower_text.contains("govt of india") && lower_text.contains("father's name"));

    let has_ssn_context = lower_text.contains("social security administration")
        || lower_text.contains("social security card")
        || lower_text.contains("social security act");

    let has_passport_context = (lower_text.contains("passport")
        && (lower_text.contains("republic of india") || lower_text.contains("type p") || lower_text.contains("passport no")))
        || lower_text.contains("diplomatic passport");

    let has_voter_context = (lower_text.contains("voter") || lower_text.contains("elector"))
        && (lower_text.contains("election commission") || lower_text.contains("elector photo identity") || lower_text.contains("epic no"));

    // High risk ID requires genuine issuing context combined with pattern match or identity terms.
    // We never allow generic resume terms to override genuine government ID indicators (H11).
    if has_aadhaar_context && (get_re_aadhaar().is_match(text) || lower_text.contains("aadhaar") || lower_text.contains("aadhar")) {
        return true;
    }
    if has_pan_context && (get_re_pan().is_match(text) || lower_text.contains("pan")) {
        return true;
    }
    if has_ssn_context && get_re_ssn().is_match(text) {
        return true;
    }
    if has_passport_context || has_voter_context {
        return true;
    }

    false
}

static RE_PRIVATE_KEY: OnceLock<Regex> = OnceLock::new();
static RE_API_KEY: OnceLock<Regex> = OnceLock::new();
static RE_AWS_KEY: OnceLock<Regex> = OnceLock::new();
static RE_JWT: OnceLock<Regex> = OnceLock::new();
static RE_SECRET_ASSIGN: OnceLock<Regex> = OnceLock::new();

/// Redact code secrets (API keys, private keys, authorization tokens, passwords, JWTs)
/// and PII from source code before any potential cloud transmission (H10).
pub fn redact_code_secrets_and_pii(input: &str) -> (String, bool) {
    let mut text = input.to_string();
    let mut found_secrets = false;

    // 1. Private keys: RSA, EC, OpenSSH, PGP, Generic private keys
    let re_priv_key = RE_PRIVATE_KEY.get_or_init(|| {
        Regex::new(r"(?s)-----BEGIN [A-Z0-9_\- ]*PRIVATE KEY-----.*?-----END [A-Z0-9_\- ]*PRIVATE KEY-----")
            .expect("privkey regex")
    });
    if re_priv_key.is_match(&text) {
        text = re_priv_key.replace_all(&text, "[REDACTED_PRIVATE_KEY]").to_string();
        found_secrets = true;
    }

    // 2. High-entropy API keys and tokens (e.g. sk_live_..., ghp_..., xoxb-...)
    let re_api_key = RE_API_KEY.get_or_init(|| {
        Regex::new(r"(?i)\b(?:sk_live|sk_test|pk_live|pk_test|ghp|gho|ghu|ghs|ghr|glpat|xoxb|xoxp|xapp|sq0csp)_[A-Za-z0-9_\-]{16,}\b")
            .expect("apikey regex")
    });
    if re_api_key.is_match(&text) {
        text = re_api_key.replace_all(&text, "[REDACTED_API_KEY]").to_string();
        found_secrets = true;
    }

    // 3. AWS Access Key IDs
    let re_aws = RE_AWS_KEY.get_or_init(|| {
        Regex::new(r"\bAKIA[0-9A-Z]{16}\b").expect("aws regex")
    });
    if re_aws.is_match(&text) {
        text = re_aws.replace_all(&text, "[REDACTED_AWS_KEY]").to_string();
        found_secrets = true;
    }

    // 4. JWT tokens
    let re_jwt = RE_JWT.get_or_init(|| {
        Regex::new(r"\beyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\b")
            .expect("jwt regex")
    });
    if re_jwt.is_match(&text) {
        text = re_jwt.replace_all(&text, "[REDACTED_JWT]").to_string();
        found_secrets = true;
    }

    // 5. Configured passwords or secrets assigned in code (e.g. dbPassword = "...", api_secret: "...")
    let re_secret_assign = RE_SECRET_ASSIGN.get_or_init(|| {
        Regex::new(r#"(?i)\b([A-Za-z0-9_\-]*(?:password|passwd|pwd|secret|auth_token|access_token|api_key|apikey))\s*([:=])\s*["']([^"'\r\n\t]{6,})["']"#)
            .expect("secret assign regex")
    });
    if re_secret_assign.is_match(&text) {
        text = re_secret_assign
            .replace_all(&text, |caps: &regex::Captures| {
                let var = &caps[1];
                let sep = &caps[2];
                let val = &caps[3];
                if val.starts_with("[REDACTED_") {
                    caps[0].to_string()
                } else {
                    format!("{var} {sep} \"[REDACTED_SECRET]\"")
                }
            })
            .to_string();
        found_secrets = true;
    }

    // 6. PII redaction (emails, phones, personal names, SSNs, cards)
    let (sanitized_pii, pii_summary) = redact_sensitive_pii(&text);
    if pii_summary.total_redactions() > 0 {
        found_secrets = true;
    }

    (sanitized_pii, found_secrets)
}

/// Filters extracted code symbols and tokens to ensure no secrets or credential fragments remain (H10).
pub fn sanitize_code_symbols(symbols: &[String]) -> Vec<String> {
    let forbidden_substrings = [
        "secret", "password", "passwd", "apikey", "api_key", "token", "private_key",
        "redacted", "credential", "auth_token", "access_token",
    ];
    symbols
        .iter()
        .filter(|sym| {
            let lower = sym.to_lowercase();
            !forbidden_substrings.iter().any(|&f| lower.contains(f))
        })
        .cloned()
        .collect()
}

/// Redact sensitive PII (Aadhaar, PAN, SSN, Credit Cards, Personal Names, Phone, DOB)
/// before any text is processed or sent anywhere.
pub fn redact_sensitive_pii(input: &str) -> (String, RedactionSummary) {
    let mut summary = RedactionSummary::default();
    let mut text = input.to_string();

    // 1. Redact Explicit Name Fields ("Name: John Doe" -> "Name: [REDACTED_NAME]")
    let re_name = get_re_name_field();
    let mut names_to_replace = Vec::new();
    for caps in re_name.captures_iter(&text) {
        if let Some(m) = caps.get(1) {
            let val = m.as_str().trim();
            if !val.is_empty() && val.len() >= 3 {
                names_to_replace.push(val.to_string());
            }
        }
    }
    summary.name_count = names_to_replace.len();
    for n in names_to_replace {
        text = text.replace(&n, "[REDACTED_NAME]");
    }

    // 2. Redact Explicit Date of Birth Fields ("DOB: 12/04/1998" -> "DOB: [REDACTED_DOB]")
    let re_dob = get_re_dob_field();
    let mut dobs_to_replace = Vec::new();
    for caps in re_dob.captures_iter(&text) {
        if let Some(m) = caps.get(1) {
            let val = m.as_str().trim();
            if !val.is_empty() {
                dobs_to_replace.push(val.to_string());
            }
        }
    }
    summary.dob_count = dobs_to_replace.len();
    for d in dobs_to_replace {
        text = text.replace(&d, "[REDACTED_DOB]");
    }

    // 3. Redact Email Addresses
    let re_email = get_re_email();
    summary.email_count = re_email.find_iter(&text).count();
    if summary.email_count > 0 {
        text = re_email.replace_all(&text, "[REDACTED_EMAIL]").to_string();
    }

    // 4. Redact Phone Numbers (processed BEFORE Aadhaar to prevent 12-digit numbers
    // with 91 country code like +91 9876543210 from falsely triggering Aadhaar)
    let re_phone = get_re_phone();
    summary.phone_count = re_phone.find_iter(&text).count();
    if summary.phone_count > 0 {
        text = re_phone.replace_all(&text, "[REDACTED_PHONE]").to_string();
    }

    // 5. Redact Credit / Debit Card Numbers
    let re_card = get_re_credit_card();
    let card_matches: Vec<_> = re_card.find_iter(&text).map(|m| m.as_str().to_string()).collect();
    for card_str in card_matches {
        let digits: String = card_str.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 13 && digits.len() <= 19 {
            text = text.replace(&card_str, "[REDACTED_CARD]");
            summary.card_count += 1;
        }
    }

    // 6. Redact SSN
    let re_ssn = get_re_ssn();
    summary.ssn_count = re_ssn.find_iter(&text).count();
    if summary.ssn_count > 0 {
        text = re_ssn.replace_all(&text, "[REDACTED_SSN]").to_string();
    }

    // 7. Redact PAN Numbers
    let re_pan = get_re_pan();
    summary.pan_count = re_pan.find_iter(&text).count();
    if summary.pan_count > 0 {
        text = re_pan.replace_all(&text, "[REDACTED_PAN]").to_string();
    }

    // 8. Redact Aadhaar Numbers
    let re_aadhaar = get_re_aadhaar();
    // Exclude strings that were already matched or look like phone prefixes
    let aadhaar_matches: Vec<_> = re_aadhaar
        .find_iter(&text)
        .map(|m| m.as_str().to_string())
        .filter(|s| {
            let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            // A 12-digit number starting with 91 followed by 6-9 is an Indian phone number
            if digits.len() == 12 && digits.starts_with("91") {
                let third_digit = digits.chars().nth(2).unwrap_or('0');
                if ('6'..='9').contains(&third_digit) {
                    return false;
                }
            }
            true
        })
        .collect();

    for a_str in aadhaar_matches {
        text = text.replace(&a_str, "[REDACTED_AADHAAR]");
        summary.aadhaar_count += 1;
    }

    (text, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_aadhaar_and_pan_and_names() {
        let sample = "Government of India UIDAI\n\
                      Name: Rajesh Kumar Sharma\n\
                      Father's Name: Suresh Sharma\n\
                      DOB: 15/08/1992\n\
                      Aadhaar Number: 9876 5432 1098\n\
                      PAN: ABCDE1234F\n\
                      Phone: 9876543210\n\
                      Email: rajesh.sharma@example.com";

        assert!(is_high_risk_identity_document(sample, "aadhaar_card.pdf"));

        let (redacted, summary) = redact_sensitive_pii(sample);

        // Verify none of the sensitive values remain in the string
        assert!(!redacted.contains("9876 5432 1098"));
        assert!(!redacted.contains("ABCDE1234F"));
        assert!(!redacted.contains("Rajesh Kumar Sharma"));
        assert!(!redacted.contains("Suresh Sharma"));
        assert!(!redacted.contains("15/08/1992"));
        assert!(!redacted.contains("9876543210"));
        assert!(!redacted.contains("rajesh.sharma@example.com"));

        // Verify placeholders are present
        assert!(redacted.contains("[REDACTED_AADHAAR]"));
        assert!(redacted.contains("[REDACTED_PAN]"));
        assert!(redacted.contains("[REDACTED_NAME]"));
        assert!(redacted.contains("[REDACTED_DOB]"));
        assert!(redacted.contains("[REDACTED_PHONE]"));
        assert!(redacted.contains("[REDACTED_EMAIL]"));

        assert_eq!(summary.aadhaar_count, 1);
        assert_eq!(summary.pan_count, 1);
        assert!(summary.name_count >= 1);
    }

    #[test]
    fn test_normal_document_not_flagged_as_high_risk_id() {
        let sample = "Quarterly Business Review. Revenue increased by 20% in Q3. Contact sales team.";
        assert!(!is_high_risk_identity_document(sample, "qbr_report.docx"));

        let (redacted, summary) = redact_sensitive_pii(sample);
        assert_eq!(summary.total_redactions(), 0);
        assert_eq!(redacted, sample);
    }

    #[test]
    fn test_resume_cv_not_flagged_as_identity_card() {
        let cv_text = "Madhav Kannan K\n\
                       madhavkannank2903@gmail.com\n\
                       +916383908577\n\
                       github.com/Madhavkannank\n\
                       Objective: Data Science graduate student specializing in Machine Learning, Computer Vision.\n\
                       Education: Anna University B.Tech 2021-2025.\n\
                       Skills: Python, Rust, PyTorch, CUDA.\n\
                       Projects: Edge Detection with OpenCV Sobel filter.";

        assert!(!is_high_risk_identity_document(cv_text, "Madhav_Kannan_K_CV.pdf"));

        let (redacted, summary) = redact_sensitive_pii(cv_text);
        assert!(!is_high_risk_identity_document(&redacted, "Madhav_Kannan_K_CV.pdf"));
        assert_eq!(summary.aadhaar_count, 0, "Phone number must NOT be counted as Aadhaar");
        assert_eq!(summary.pan_count, 0);
        assert!(summary.phone_count >= 1, "Phone number must be properly recognized");
        assert!(summary.email_count >= 1, "Email must be redacted");
        assert!(redacted.contains("[REDACTED_PHONE]"));
        assert!(redacted.contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn test_redact_code_secrets_and_credentials() {
        let dummy_key = format!("{}_{}", "sk_test", "1234567890abcdef12345678");
        let code_sample = format!(
            r#"
            // Configuration for backend service
            const apiKey = "{dummy_key}";
            const awsKey = "AKIAIOSFODNN7EXAMPLE";
            const dbPassword = "super_secret_production_password_2026";
            const token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
            const contact = "admin@internal.example.corp";

            function connect() {{
                console.log("Connecting...");
            }}
        "#
        );

        let (redacted, has_secrets) = redact_code_secrets_and_pii(&code_sample);
        assert!(has_secrets, "Code containing API keys and passwords must flag has_secrets");
        assert!(!redacted.contains(&dummy_key));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!redacted.contains("super_secret_production_password_2026"));
        assert!(!redacted.contains("admin@internal.example.corp"));
        assert!(redacted.contains("[REDACTED_API_KEY]"));
        assert!(redacted.contains("[REDACTED_AWS_KEY]"));
        assert!(redacted.contains("[REDACTED_SECRET]"));
        assert!(redacted.contains("[REDACTED_EMAIL]"));

        let symbols = vec![
            "connect".to_string(),
            "apiKey".to_string(),
            "dbPassword".to_string(),
            "authToken".to_string(),
            "calculateMetrics".to_string(),
        ];
        let cleaned = sanitize_code_symbols(&symbols);
        assert_eq!(cleaned, vec!["connect".to_string(), "calculateMetrics".to_string()]);
    }
}
