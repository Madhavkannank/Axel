import json
import re
from groq import Groq
from config import GROQ_API_KEY, MODEL_NAME, MODEL_FALLBACKS


def redact_sensitive_data(text):
    """Shields PII, government IDs, and credentials before cloud transmission (H10)."""
    if not text:
        return ""
    # Redact Aadhaar: 12 digits, often formatted as 4-4-4
    text = re.sub(r'\b\d{4}\s?\d{4}\s?\d{4}\b', '[REDACTED_AADHAAR]', text)
    # Redact PAN: 5 letters, 4 digits, 1 letter
    text = re.sub(r'\b[A-Z]{5}[0-9]{4}[A-Z]\b', '[REDACTED_PAN]', text)
    # Redact US SSN: 3-2-4 digits
    text = re.sub(r'\b\d{3}-\d{2}-\d{4}\b', '[REDACTED_SSN]', text)
    # Redact Credit Cards: 16 digits
    text = re.sub(r'\b(?:\d{4}[ -]?){3}\d{4}\b', '[REDACTED_CARD]', text)
    # Redact Private Keys and API Keys
    text = re.sub(r'-----BEGIN (?:RSA )?PRIVATE KEY-----[\s\S]*?-----END (?:RSA )?PRIVATE KEY-----', '[REDACTED_PRIVATE_KEY]', text)
    text = re.sub(r'\b(?:gsk_|sk_|ghp_|AKIA)[A-Za-z0-9_]{16,}\b', '[REDACTED_KEY]', text)
    text = re.sub(r'(?i)\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|password|passwd)\s*[:=]\s*["\']?[A-Za-z0-9_\-\.]{8,}["\']?', '[REDACTED_CREDENTIAL]', text)
    # Redact Email addresses
    text = re.sub(r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b', '[REDACTED_EMAIL]', text)
    return text


def is_identity_document(text, file_name=""):
    """Detects high-risk government IDs that must never be transmitted to external cloud LLMs (H10)."""
    combined = (str(text) + " " + str(file_name)).lower()
    strong_markers = [
        "aadhaar card", "unique identification authority", "uidai",
        "income tax department", "permanent account number card", "pan card",
        "election commission of india", "voter identity card", "epic no",
        "republic of india passport", "driver license", "driving licence",
        "social security card", "social security administration",
        "identity card", "national id card", "citizen card",
    ]
    return any(marker in combined for marker in strong_markers)


def _is_connection_error(exc):
    msg = str(exc).lower()
    return (
        "connection error" in msg
        or "timed out" in msg
        or "name or service not known" in msg
        or "temporary failure in name resolution" in msg
        or "connection refused" in msg
    )


def _is_auth_error(exc):
    msg = str(exc).lower()
    return "401" in msg or "invalid_api_key" in msg


def _key_missing_or_placeholder():
    key = (GROQ_API_KEY or "").strip()
    return (not key) or key == "your-groq-api-key-here"


def _request_with_model_fallback(client, prompt, stricter_prompt, max_tokens, context_label="Groq"):
    """Try primary and fallback models. Fail fast on auth/network failures."""
    models_to_try = [MODEL_NAME] + [m for m in MODEL_FALLBACKS if m != MODEL_NAME]

    for model in models_to_try:
        parse_attempted = False
        for attempt in range(2):
            try:
                response = client.chat.completions.create(
                    messages=[
                        {
                            "role": "user",
                            "content": prompt if attempt == 0 else stricter_prompt,
                        }
                    ],
                    model=model,
                    temperature=0.2,
                    max_tokens=max_tokens,
                )

                content = response.choices[0].message.content
                parsed_data = safe_parse_llm_response(content)
                parse_attempted = True

                if parsed_data:
                    if model != MODEL_NAME:
                        print(f"[INFO] Groq fallback model succeeded: {model}")
                    return parsed_data

            except Exception as e:
                if _is_auth_error(e):
                    print("[WARNING] Groq auth failed (invalid API key). Using safe fallback.")
                    return None
                if _is_connection_error(e):
                    print("[WARNING] Groq unreachable (network/connection issue). Using safe fallback.")
                    return None

                print(f"[ERROR] {context_label} model={model} attempt={attempt + 1} error: {e}")

        if parse_attempted:
            print(f"[WARNING] Model failed to return parseable JSON: {model}")

    return None

def safe_parse_llm_response(response_text):
    """Safely extracts JSON from LLM response strings."""
    try:
        if not response_text:
            return None

        start_idx = response_text.find('{')
        end_idx = response_text.rfind('}')
        if start_idx != -1 and end_idx != -1:
            json_str = response_text[start_idx:end_idx+1]
            data = json.loads(json_str)
            if "category" in data and "keywords" in data:
                return data
    except Exception:
        pass
    return None

def generate_keywords(text, ml_category=None, ml_keywords=None, file_name="", full_text=None):
    """Sends text to Groq API and returns strict JSON output with model fallback."""
    fallback_response = {"category": "Unknown", "keywords": []}

    inspect_text = full_text if full_text is not None else text
    if not inspect_text:
        return fallback_response

    # Privacy shield: Check for high-risk identity documents on UNTRUNCATED text (A14)
    if is_identity_document(inspect_text, file_name):
        print("[PRIVACY] High-risk government identity document detected. Cloud LLM transmission prevented (H10).")
        return {"category": "Government ID", "keywords": ["identity", "confidential", "local_only"]}

    # Redact PII and credentials on the complete text before cloud excerpting/transmission
    safe_untruncated = redact_sensitive_data(inspect_text)
    from utils.file_handler import preprocess_text
    safe_text = preprocess_text(safe_untruncated)

    if _key_missing_or_placeholder():
        print("[WARNING] GROQ_API_KEY missing/placeholder. Using safe fallback.")
        return fallback_response

    try:
        client = Groq(api_key=GROQ_API_KEY, timeout=15)
    except Exception as e:
        print(f"[ERROR] Groq Initialisation: {e}")
        return fallback_response

    ml_context = ""
    if ml_category or ml_keywords:
        clean_cat = redact_sensitive_data(str(ml_category or ""))
        clean_kws = [redact_sensitive_data(str(k)) for k in (ml_keywords or [])]
        ml_context = f"\n    I already have a machine learning guess:\n    Suggested ML Category: {clean_cat}\n    Suggested ML Keywords: {', '.join(clean_kws)}\n    Use the ML suggestions to guide your output if they seem correct. Otherwise, improve them.\n"

    prompt = f"""
    Analyze the following text and provide a category and a list of MAXIMUM 5 keywords. The keywords are used to add a tag into the meta data for faster searching using general terms ,
    analyse the texts correctly , and provide the keywords , dont choose unusual words {ml_context}
    You MUST output STRICT JSON in the following format ONLY:
    {{
        "category": "...",
        "keywords": ["...", "..."]
    }}
    Do NOT output any other text or explanation.

    Text to analyze:
    {safe_text}
    """

    stricter_prompt = f"""
    Your previous response failed JSON parsing.
    Provide ONLY a valid raw JSON object with EXACTLY the keys "category" (string) and "keywords" (array of strings).
    NO markdown formatting, NO explanation, NO backticks.
    Text: {safe_text}
    """

    parsed_data = _request_with_model_fallback(
        client=client,
        prompt=prompt,
        stricter_prompt=stricter_prompt,
        max_tokens=1024,
        context_label="Groq API",
    )

    return parsed_data if parsed_data else fallback_response


def generate_keywords_from_file_context(file_name, file_type, reason=""):
    """Uses Groq LLM to infer category/keywords when extracted text is unavailable."""
    fallback_response = {
        "category": "Unknown",
        "keywords": [file_type, "unreadable", "review"]
    }

    # Privacy check on file_name
    if is_identity_document("", file_name):
        print("[PRIVACY] High-risk filename detected. Cloud LLM transmission prevented (H10).")
        return {"category": "Government ID", "keywords": ["identity", "confidential", "local_only"]}

    clean_file_name = redact_sensitive_data(file_name)

    if _key_missing_or_placeholder():
        print("[WARNING] GROQ_API_KEY missing/placeholder. Using safe fallback.")
        return fallback_response

    try:
        client = Groq(api_key=GROQ_API_KEY, timeout=15)
    except Exception as e:
        print(f"[ERROR] Groq Initialisation: {e}")
        return fallback_response

    prompt = f"""
    OCR extraction failed or returned empty text.
    Infer a best-effort category and up to 5 keywords from available file context.
    Be conservative and generic when uncertain.

    File name: {clean_file_name}
    File type: {file_type}
    Failure reason: {reason}

    Output STRICT JSON only in this format:
    {{
      "category": "...",
      "keywords": ["...", "..."]
    }}
    """

    stricter_prompt = f"""
    Return ONLY valid raw JSON with keys "category" and "keywords".
    No markdown and no explanation.
    File name: {clean_file_name}
    File type: {file_type}
    """

    parsed_data = _request_with_model_fallback(
        client=client,
        prompt=prompt,
        stricter_prompt=stricter_prompt,
        max_tokens=512,
        context_label="Groq context fallback",
    )

    return parsed_data if parsed_data else fallback_response
