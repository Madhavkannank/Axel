import os

BASE_DIR = os.path.dirname(os.path.abspath(__file__))


def _read_dotenv_values(dotenv_path=".env"):
	"""Read key=value pairs from .env into a dict (source of truth)."""
	dotenv_path = dotenv_path if os.path.isabs(dotenv_path) else os.path.join(BASE_DIR, dotenv_path)
	values = {}
	if not os.path.exists(dotenv_path):
		return values

	try:
		with open(dotenv_path, "r", encoding="utf-8") as f:
			for raw_line in f:
				line = raw_line.strip()
				if not line or line.startswith("#") or "=" not in line:
					continue

				key, value = line.split("=", 1)
				key = key.strip()
				value = value.strip().strip('"').strip("'")
				if key:
					values[key] = value
	except Exception as e:
		print(f"[WARNING] Failed to read .env file: {e}")

	return values


def _get_alias(values, keys, default=""):
	"""Retrieve configuration value from environment or dotenv using alias priority."""
	for key in keys:
		env_val = os.environ.get(key, "").strip()
		if env_val:
			return env_val
		dotenv_val = values.get(key, "").strip()
		if dotenv_val:
			return dotenv_val
	return default


_DOTENV_VALUES = _read_dotenv_values()

# Optional for offline / local-only semantic mode (M08)
_raw_key = _get_alias(_DOTENV_VALUES, ["AXEL_GROQ_API_KEY", "GROQ_API_KEY"], "")
GROQ_API_KEY = _raw_key if _raw_key and _raw_key != "your-groq-api-key-here" else None

MODEL_NAME = _get_alias(_DOTENV_VALUES, ["AXEL_MODEL_NAME", "MODEL_NAME"], "llama-3.3-70b-versatile")
MODEL_FALLBACKS = [
	m.strip()
	for m in _get_alias(_DOTENV_VALUES, ["AXEL_GROQ_FALLBACK_MODELS", "GROQ_FALLBACK_MODELS"], "").split(",")
	if m.strip()
]
INPUT_FOLDER = _get_alias(_DOTENV_VALUES, ["AXEL_INPUT_FOLDER", "INPUT_FOLDER"], "./documents")
MAX_KEYWORDS = int(_get_alias(_DOTENV_VALUES, ["AXEL_MAX_KEYWORDS", "MAX_KEYWORDS"], "5"))
TOP_K_RESULTS = int(_get_alias(_DOTENV_VALUES, ["AXEL_TOP_K_RESULTS", "TOP_K_RESULTS"], "5"))
