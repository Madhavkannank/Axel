import json
import os
import re
import joblib
from sklearn.feature_extraction.text import TfidfVectorizer
from sklearn.linear_model import LogisticRegression
from sklearn.pipeline import Pipeline

APP_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MODEL_PATH = os.path.join(APP_DIR, "ml", "model.pkl")
DATA_PATH = os.path.join(APP_DIR, "ml", "data.json")

_classifier_pipeline = None

# Generic document-noise tokens found across many categories.
NOISE_TOKENS = {
    "the", "and", "this", "is", "document", "for", "of", "file", "record",
    "notice", "attention", "official", "please", "see", "attached", "below",
    "summary", "data", "info", "confidential", "page", "signature", "authorized",
    "date", "amount", "copy", "printed", "scanned", "version", "final", "latest",
    "original", "digital", "archived", "verified", "signed", "photocopy", "report"
}

RULE_KEYWORDS = {
    "Government ID": {
        "aadhaar", "pan", "passport", "licence", "license", "uidai", "voter", "id", "dob", "nationality"
    },
    "Invoice": {
        "invoice", "receipt", "bill", "gst", "qty", "items", "price", "paid", "order", "total"
    },
    "Finance": {
        "bank", "salary", "payslip", "debit", "credit", "statement", "balance", "upi", "txn", "transactions"
    },
    "Academic": {
        "assignment", "lecture", "research", "thesis", "project", "regression", "classification", "dataset", "analysis"
    },
    "Resume": {
        "resume", "curriculum", "vitae", "candidate", "skills", "experience", "internship", "achievements", "profile"
    },
    "Legal": {
        "agreement", "affidavit", "clause", "contract", "declaration", "witness", "legal", "notice", "property", "transfer"
    },
    "Medical": {
        "prescription", "doctor", "diagnosis", "hospital", "discharge", "dosage", "lab", "hemoglobin", "scan", "treatment"
    },
    "Travel": {
        "train", "flight", "boarding", "ticket", "pnr", "coach", "route", "checkin", "checkout", "hotel"
    },
    "Personal": {
        "todo", "notes", "reminder", "daily", "meeting", "friend", "tasks", "pending", "completed", "journal"
    },
    "Recipe": {
        "recipe", "ingredients", "cook", "baking", "cake", "pasta", "sauce", "kitchen", "masala", "boil"
    },
    "Book": {
        "book", "ebook", "chapter", "theory", "textbook", "references", "concepts", "learning", "exercises"
    },
    "Communication": {
        "whatsapp", "chat", "messages", "sms", "email", "reply", "forward", "thread", "conversation", "transcript"
    }
}


def _normalize_text(text):
    """Normalize OCR-like text and suppress generic noise for classification."""
    if not text:
        return ""

    text = str(text).lower()

    # Normalize common OCR confusions before token filtering.
    text = text.replace("0", "o").replace("1", "l").replace("5", "s")

    # Keep alphabetic tokens only and remove generic tokens that are not discriminative.
    tokens = re.findall(r"[a-z]{2,}", text)
    tokens = [t for t in tokens if t not in NOISE_TOKENS]

    return " ".join(tokens)


def _rule_based_category(cleaned_text):
    """Return category when strong keyword evidence is present; else None."""
    if not cleaned_text:
        return None, 0.0

    tokens = set(cleaned_text.split())
    if not tokens:
        return None, 0.0

    scores = {}
    for category, kws in RULE_KEYWORDS.items():
        score = len(tokens & kws)
        if score > 0:
            scores[category] = score

    if not scores:
        return None, 0.0

    ranked = sorted(scores.items(), key=lambda x: x[1], reverse=True)
    top_cat, top_score = ranked[0]
    second_score = ranked[1][1] if len(ranked) > 1 else 0

    # Strong signal: at least 2 matches, or clear margin with at least one match.
    if top_score >= 2 or (top_score >= 1 and top_score >= second_score + 1):
        confidence = min(0.95, 0.45 + (top_score * 0.12))
        return top_cat, confidence

    return None, 0.0

def train_model():
    global _classifier_pipeline
    print("[INFO] Initializing/Training ML Classifier...")
    if not os.path.exists(DATA_PATH):
        print(f"[ERROR] Training data not found at {DATA_PATH}")
        return False
        
    try:
        with open(DATA_PATH, 'r', encoding='utf-8') as f:
            data = json.load(f)
            
        texts = [_normalize_text(item['text']) for item in data]
        labels = [item['category'] for item in data]

        # If normalization removes everything for any sample, keep a tiny fallback token.
        texts = [t if t.strip() else "unknowntext" for t in texts]
        
        _classifier_pipeline = Pipeline([
            ('tfidf', TfidfVectorizer(
                stop_words='english',
                max_features=5000,
                min_df=2,
                ngram_range=(1, 2),
                sublinear_tf=True
            )),
            ('clf', LogisticRegression(
                C=2.0,
                max_iter=2000,
                class_weight='balanced',
                random_state=42
            ))
        ])
        
        _classifier_pipeline.fit(texts, labels)
        
        # Ensure ml dir exists
        os.makedirs(os.path.dirname(MODEL_PATH), exist_ok=True)
        joblib.dump(_classifier_pipeline, MODEL_PATH)
        print(f"[INFO] ML Classifier trained on {len(texts)} samples.")
        return True
    except Exception as e:
        print(f"[ERROR] Failed to train ML Classifier: {e}")
        return False

def predict_category(text):
    global _classifier_pipeline
    if _classifier_pipeline is None:
        try:
            canonical_model = os.path.realpath(MODEL_PATH)
            canonical_app = os.path.realpath(APP_DIR)
            if os.path.exists(MODEL_PATH) and canonical_model.startswith(canonical_app):
                _classifier_pipeline = joblib.load(MODEL_PATH)
            else:
                train_model()
        except Exception:
            try:
                train_model()
            except Exception:
                pass

    if _classifier_pipeline is None or not str(text).strip():
        return "Unknown", 0.0

    try:
        cleaned = _normalize_text(text)
        rule_cat, rule_conf = _rule_based_category(cleaned)
        if rule_cat:
            return rule_cat, rule_conf

        if not cleaned:
            cleaned = str(text)

        probs = _classifier_pipeline.predict_proba([cleaned])[0]
        max_idx = probs.argmax()
        confidence = probs[max_idx]
        category = _classifier_pipeline.classes_[max_idx]
        return category, confidence
    except Exception as e:
        print(f"[ERROR] Prediction failed: {e}")
        return "Unknown", 0.0
