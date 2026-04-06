import json
from groq import Groq
from config import GROQ_API_KEY, MODEL_NAME, MODEL_FALLBACKS


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

def generate_keywords(text, ml_category=None, ml_keywords=None):
    """Sends text to Groq API and returns strict JSON output with model fallback."""
    fallback_response = {"category": "Unknown", "keywords": []}
    
    if not text:
        return fallback_response

    if _key_missing_or_placeholder():
        print("[WARNING] GROQ_API_KEY missing/placeholder. Using safe fallback.")
        return fallback_response

    try:
        client = Groq(api_key=GROQ_API_KEY, timeout=15)
    except Exception as e:
        print(f"[ERROR] Groq Initialisation: {e}")
        return fallback_response
    
    ml_context = ""
    if ml_category and ml_keywords:
        ml_context = f"\n    I already have a machine learning guess:\n    Suggested ML Category: {ml_category}\n    Suggested ML Keywords: {', '.join(ml_keywords)}\n    Use the ML suggestions to guide your output if they seem correct. Otherwise, improve them.\n"

    prompt = f"""
    Analyze the following text and provide a category and a list of MAXIMUM 5 keywords.{ml_context}
    You MUST output STRICT JSON in the following format ONLY:
    {{
        "category": "...",
        "keywords": ["...", "..."]
    }}
    Do NOT output any other text or explanation.

    Text to analyze:
    {text}
    """
    
    stricter_prompt = f"""
    Your previous response failed JSON parsing. 
    Provide ONLY a valid raw JSON object with EXACTLY the keys "category" (string) and "keywords" (array of strings). 
    NO markdown formatting, NO explanation, NO backticks.
    Text: {text}
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

    File name: {file_name}
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
    File name: {file_name}
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
