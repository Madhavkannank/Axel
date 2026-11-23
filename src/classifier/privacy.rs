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
    RE_AADHAAR.get_or_init(|| Regex::new(r"\b[2-9]\d{3}[ -]?\d{4}[ -]?\d{4}\b").expect("aadhaar regex"))
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
        Regex::new(r"(?:\+?91[\-\s]?)?[6-9]\d{9}\b|\b(?:\+?1[\-\s]?)?\(?\d{3}\)?[\-\s]?\d{3}[\-\s]?\d{4}\b")
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

    // Check if filename suggests an identity card
    if lower_file.contains("aadhaar")
        || lower_file.contains("aadhar")
        || lower_file.contains("pan")
        || lower_file.contains("passport")
        || lower_file.contains("ssn")
        || lower_file.contains("voter")
    {
        return true;
    }

    // Check if PAN, Aadhaar, or SSN pattern exists in text
    get_re_pan().is_match(text) || get_re_aadhaar().is_match(text) || get_re_ssn().is_match(text)
}

/// Redact sensitive PII (Aadhaar, PAN, SSN, Credit Cards, Personal Names, Phone, DOB)
/// before any text is processed or sent anywhere.
pub fn redact_sensitive_pii(input: &str) -> (String, RedactionSummary) {
    let mut summary = RedactionSummary::default();
    let mut text = input.to_string();

    // 1. Redact Aadhaar Numbers
    let re_aadhaar = get_re_aadhaar();
    summary.aadhaar_count = re_aadhaar.find_iter(&text).count();
    if summary.aadhaar_count > 0 {
        text = re_aadhaar.replace_all(&text, "[REDACTED_AADHAAR]").to_string();
    }

    // 2. Redact PAN Numbers
    let re_pan = get_re_pan();
    summary.pan_count = re_pan.find_iter(&text).count();
    if summary.pan_count > 0 {
        text = re_pan.replace_all(&text, "[REDACTED_PAN]").to_string();
    }

    // 3. Redact SSN
    let re_ssn = get_re_ssn();
    summary.ssn_count = re_ssn.find_iter(&text).count();
    if summary.ssn_count > 0 {
        text = re_ssn.replace_all(&text, "[REDACTED_SSN]").to_string();
    }

    // 4. Redact Credit / Debit Card Numbers
    let re_card = get_re_credit_card();
    // Exclude strings that look like date ranges
    let card_matches: Vec<_> = re_card.find_iter(&text).map(|m| m.as_str().to_string()).collect();
    for card_str in card_matches {
        let digits: String = card_str.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 13 && digits.len() <= 19 {
            text = text.replace(&card_str, "[REDACTED_CARD]");
            summary.card_count += 1;
        }
    }

    // 5. Redact Phone Numbers
    let re_phone = get_re_phone();
    summary.phone_count = re_phone.find_iter(&text).count();
    if summary.phone_count > 0 {
        text = re_phone.replace_all(&text, "[REDACTED_PHONE]").to_string();
    }

    // 6. Redact Email Addresses
    let re_email = get_re_email();
    summary.email_count = re_email.find_iter(&text).count();
    if summary.email_count > 0 {
        text = re_email.replace_all(&text, "[REDACTED_EMAIL]").to_string();
    }

    // 7. Redact Explicit Name Fields ("Name: John Doe" -> "Name: [REDACTED_NAME]")
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

    // 8. Redact Explicit Date of Birth Fields ("DOB: 12/04/1998" -> "DOB: [REDACTED_DOB]")
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
}
