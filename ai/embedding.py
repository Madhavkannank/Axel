try:
    from sentence_transformers import SentenceTransformer
except Exception as import_error:
    SentenceTransformer = None
    _IMPORT_ERROR = import_error
else:
    _IMPORT_ERROR = None

# Global variable to cache the model across calls
_model = None

def get_model():
    """Loads the sentence-transformers model lazily to avoid unnecessary loading."""
    global _model
    if _model is None:
        if SentenceTransformer is None:
            raise RuntimeError(f"SentenceTransformer unavailable: {_IMPORT_ERROR}")
        print("[INFO] Loading semantic embedding model (all-MiniLM-L6-v2)...")
        try:
            _model = SentenceTransformer('all-MiniLM-L6-v2')
        except Exception as e:
            _model = None
            raise RuntimeError(f"Failed to load semantic embedding model: {e}") from e
    return _model

def generate_embedding(text):
    """Generates a vector embedding for the given text. Returns a list of floats."""
    if not text:
        return []
    try:
        model = get_model()
        # encode() returns a numpy array, we convert it to a standard Python list for JSON compatibility
        embedding = model.encode(text)
        return embedding.tolist()
    except Exception as e:
        print(f"[WARNING] Semantic embedding unavailable: {e}")
        return []
