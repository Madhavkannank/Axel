
import json
import os
import hashlib
import numpy as np
from difflib import SequenceMatcher
from collections import Counter
from config import INPUT_FOLDER, MAX_KEYWORDS
INDEX_FILE = 'axel_index.json'

_index_cache = None
_index_dirty = False

def _load_index():
    global _index_cache
    if _index_cache is not None:
        return _index_cache
    if os.path.exists(INDEX_FILE):
        try:
            with open(INDEX_FILE, 'r', encoding='utf-8') as f:
                _index_cache = json.load(f)
                return _index_cache
        except Exception:
            _index_cache = []
            return _index_cache
    _index_cache = []
    return _index_cache

def _save_index():
    global _index_cache, _index_dirty
    if _index_cache is not None and _index_dirty:
        with open(INDEX_FILE, 'w', encoding='utf-8') as f:
            json.dump(_index_cache, f, indent=2)
        _index_dirty = False

def get_index():
    return _load_index()

def flush_index():
    _save_index()

def compute_file_hash(file_path):
    """Compute SHA256 hash of a file."""
    h = hashlib.sha256()
    try:
        with open(file_path, 'rb') as f:
            while True:
                chunk = f.read(8192)
                if not chunk:
                    break
                h.update(chunk)
        return h.hexdigest()
    except Exception:
        return None


def _input_folder_root():
    """Return the absolute root path for the configured input folder."""
    return os.path.abspath(INPUT_FOLDER)


def _item_in_input_folder(item):
    """Check whether an indexed item belongs to the configured input folder."""
    item_path = str(item.get('path', '')).strip()
    if not item_path:
        return False

    input_root = _input_folder_root()
    abs_item_path = os.path.abspath(item_path)

    try:
        if os.path.commonpath([abs_item_path, input_root]) == input_root:
            return True
    except ValueError:
        pass

    normalized_item = os.path.normpath(item_path).lower()
    normalized_root = os.path.normpath(INPUT_FOLDER).lower()
    return normalized_item.startswith(normalized_root)

def find_by_hash(file_hash):
    idx = get_index()
    for item in idx:
        if item.get('hash') == file_hash:
            return item
    return None

def update_index(filename, category, keywords, file_path, embedding=None, file_hash=None):
    """Updates the index with new file results, avoiding duplicates by hash."""
    global _index_dirty
    idx = get_index()
    if file_hash is None:
        file_hash = compute_file_hash(file_path)
    updated = False
    for item in idx:
        if item.get('hash') == file_hash:
            item['file'] = filename
            item['category'] = category
            item['keywords'] = keywords
            item['path'] = file_path
            if embedding is not None:
                item['embedding'] = embedding
            updated = True
            break
    if not updated:
        new_item = {
            "file": filename,
            "category": category,
            "keywords": keywords,
            "path": file_path,
            "hash": file_hash
        }
        if embedding is not None:
            new_item['embedding'] = embedding
        idx.append(new_item)
    _index_dirty = True
    _save_index()

def search_index(query):
    idx = get_index()
    query = (query or "").strip().lower()
    if not query:
        return []

    query_terms = [term for term in query.split() if term]
    results = []
    suggestions = []

    for item in idx:
        if not _item_in_input_folder(item):
            continue

        category = str(item.get('category', '')).lower()
        file_name = str(item.get('file', '')).lower()
        path = str(item.get('path', '')).lower()
        keywords = [str(k).lower() for k in item.get('keywords', [])]

        category_match = query in category
        file_match = query in file_name or query in path
        keyword_match = any(query in keyword for keyword in keywords)

        # Allow looser matching on individual query terms so short searches like
        # "room" can still find files with keyword or filename variants.
        token_match = False
        if query_terms:
            searchable_text = " ".join([category, file_name, path, " ".join(keywords)])
            token_hits = sum(1 for term in query_terms if term in searchable_text)
            token_match = token_hits > 0

        if category_match or file_match or keyword_match or token_match:
            matched_item = dict(item)
            matched_item['score'] = _build_search_score(query, category, file_name, path, keywords)
            matched_item['match_type'] = 'exact'
            results.append(matched_item)
        else:
            fuzzy_score = max(
                _best_similarity(query, [category]),
                _best_similarity(query, [file_name]),
                _best_similarity(query, [path]),
                _best_similarity(query, keywords),
            )
            if fuzzy_score >= 0.35:
                suggested_item = dict(item)
                suggested_item['score'] = fuzzy_score
                suggested_item['match_type'] = 'suggested'
                suggestions.append(suggested_item)

    if results:
        results.sort(key=lambda item: item.get('score', 0), reverse=True)
        return results

    suggestions.sort(key=lambda item: item.get('score', 0), reverse=True)
    return suggestions[:5]

def cosine_similarity(vec1, vec2):
    v1 = np.array(vec1)
    v2 = np.array(vec2)
    norm1 = np.linalg.norm(v1)
    norm2 = np.linalg.norm(v2)
    if norm1 == 0 or norm2 == 0:
        return 0.0
    return float(np.dot(v1, v2) / (norm1 * norm2))

def keyword_overlap_score(query, keywords):
    # Simple overlap: count of query words in keywords
    query_words = set(query.lower().split())
    kw_set = set([k.lower() for k in keywords])
    if not kw_set:
        return 0.0
    overlap = len(query_words & kw_set)
    return overlap / len(kw_set)


def _best_similarity(query, values):
    """Return the best fuzzy similarity between a query and a list of values."""
    best = 0.0
    for value in values:
        value = str(value).lower()
        if not value:
            continue
        best = max(best, SequenceMatcher(None, query, value).ratio())
    return best


def _build_search_score(query, category, file_name, path, keywords):
    """Build a simple relevance score for keyword search results."""
    score = 0.0
    if query in category:
        score = max(score, 1.0)
    if query in file_name:
        score = max(score, 0.95)
    if query in path:
        score = max(score, 0.9)
    if any(query in keyword for keyword in keywords):
        score = max(score, 0.85)

    token_hits = sum(1 for term in query.split() if term and term in " ".join([category, file_name, path, " ".join(keywords)]))
    if token_hits:
        score = max(score, 0.75 + min(token_hits * 0.05, 0.2))

    return score

def semantic_search_index(query_embedding, query_text=None, top_k=5):
    idx = get_index()
    results = []

    use_embedding = bool(query_embedding)
    for item in idx:
        if not _item_in_input_folder(item):
            continue

        emb = item.get("embedding")
        category = str(item.get('category', '')).lower()
        file_name = str(item.get('file', '')).lower()
        path = str(item.get('path', '')).lower()
        keywords = [str(k).lower() for k in item.get('keywords', [])]

        if use_embedding and emb:
            cos_score = cosine_similarity(query_embedding, emb)
            kw_score = 0.0
            if query_text and item.get('keywords'):
                kw_score = keyword_overlap_score(query_text, item['keywords'])
            hybrid_score = 0.7 * cos_score + 0.3 * kw_score
            result_item = dict(item)
            result_item["score"] = hybrid_score
            result_item["match_type"] = 'exact'
            results.append(result_item)
        elif query_text:
            # Fallback when embeddings are unavailable: rank using metadata and keyword overlap.
            fuzzy_score = max(
                _best_similarity(query_text.lower(), [category]),
                _best_similarity(query_text.lower(), [file_name]),
                _best_similarity(query_text.lower(), [path]),
                _best_similarity(query_text.lower(), keywords),
            )
            kw_score = keyword_overlap_score(query_text, item.get('keywords', [])) if item.get('keywords') else 0.0
            token_score = 0.0
            if query_text.lower() in category or query_text.lower() in file_name or query_text.lower() in path:
                token_score = 1.0
            hybrid_score = max(fuzzy_score, kw_score, token_score)
            if hybrid_score > 0:
                result_item = dict(item)
                result_item["score"] = hybrid_score
                result_item["match_type"] = 'suggested'
                results.append(result_item)
    results.sort(key=lambda x: x.get("score", 0), reverse=True)
    return results[:top_k]

def get_stats():
    idx = get_index()
    total_files = len(idx)
    categories = [item.get('category', '') for item in idx]
    cat_counter = Counter(categories)
    most_common_cat, most_common_count = cat_counter.most_common(1)[0] if cat_counter else (None, 0)
    return {
        'total_files': total_files,
        'total_categories': len(cat_counter),
        'most_common_category': most_common_cat,
        'most_common_count': most_common_count
    }

def rebuild_index():
    global _index_cache, _index_dirty
    _index_cache = []
    _index_dirty = True
    if not os.path.exists(INPUT_FOLDER):
        return
    for fname in os.listdir(INPUT_FOLDER):
        fpath = os.path.join(INPUT_FOLDER, fname)
        if not os.path.isfile(fpath):
            continue
        # The main pipeline should be called externally to process and re-index
        pass
    _save_index()