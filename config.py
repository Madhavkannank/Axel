import os


BASE_DIR = os.path.dirname(os.path.abspath(__file__))


def _read_dotenv_values(dotenv_path=".env"):
	"""Read key=value pairs from .env into a dict (source of truth)."""
	dotenv_path = dotenv_path if os.path.isabs(dotenv_path) else os.path.join(BASE_DIR, dotenv_path)
	values = {}
	if not os.path.exists(dotenv_path):
		print(f"[WARNING] .env not found at: {dotenv_path}")
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


def _get_required(values, key):
	"""Get required config from .env or raise a clear error."""
	val = values.get(key, "").strip()
	if not val:
		raise ValueError(f"Missing required .env key: {key}")
	return val


def _get_optional(values, key, default=""):
	"""Get optional config from .env."""
	val = values.get(key, "").strip()
	return val if val else default


_DOTENV_VALUES = _read_dotenv_values()

GROQ_API_KEY = _get_required(_DOTENV_VALUES, "GROQ_API_KEY")
MODEL_NAME = _get_required(_DOTENV_VALUES, "MODEL_NAME")
MODEL_FALLBACKS = [
	m.strip()
	for m in _get_optional(_DOTENV_VALUES, "AXEL_GROQ_FALLBACK_MODELS", "").split(",")
	if m.strip()
]
INPUT_FOLDER = _get_required(_DOTENV_VALUES, "AXEL_INPUT_FOLDER")
MAX_KEYWORDS = int(_get_required(_DOTENV_VALUES, "AXEL_MAX_KEYWORDS"))
TOP_K_RESULTS = int(_get_required(_DOTENV_VALUES, "AXEL_TOP_K_RESULTS"))
