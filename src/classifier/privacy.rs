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

    let has_passport_context = lower_text.contains("republic of india")
        || lower_text.contains("passport no")
        || lower_text.contains("type p");

    let has_voter_context = lower_text.contains("election commission")
        || lower_text.contains("elector photo identity")
        || lower_text.contains("voter id");

    // If document is clearly a professional resume, curriculum vitae, or academic paper,
    // it is not a government identity card.
    let is_resume_or_academic = (lower_text.contains("curriculum vitae")
        || lower_text.contains("resume")
        || lower_text.contains("objective")
        || lower_text.contains("experience")
        || lower_text.contains("skills")
        || lower_text.contains("education"))
        && !has_aadhaar_context
        && !has_pan_context;

    if is_resume_or_academic {
        return false;
    }

    // High risk ID requires genuine issuing context combined with pattern match or identity terms
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
}
