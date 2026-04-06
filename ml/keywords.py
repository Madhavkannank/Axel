import re
from sklearn.feature_extraction.text import TfidfVectorizer

def extract_keywords(text, top_n=5):
    """
    Extract keywords using TF-IDF, filtering out stopwords, short words, and numbers.
    """
    if not text or not str(text).strip():
        return []

    # Simple heuristic to keep words that are alphanumeric, >2 chars, and no numbers
    def custom_preprocessor(t):
        # lower case
        t = t.lower()
        # remove numbers and special characters, keep only letters
        t = re.sub(r'[^a-z\s]', ' ', t)
        return t
        
    try:
        vectorizer = TfidfVectorizer(
            stop_words='english',
            preprocessor=custom_preprocessor,
            token_pattern=r'(?u)\b[a-z]{3,}\b'  # >=3 chars, letters only
        )
        
        tfidf_matrix = vectorizer.fit_transform([text])
        feature_names = vectorizer.get_feature_names_out()
        
        # Get tf-idf scores
        scores = tfidf_matrix.toarray()[0]
        
        # Sort by score
        sorted_indices = scores.argsort()[::-1]
        
        keywords = []
        for idx in sorted_indices:
            if scores[idx] > 0:
                kw = feature_names[idx]
                if kw not in keywords:
                    keywords.append(kw)
            if len(keywords) >= top_n:
                break
                
        return keywords[:top_n]
    except ValueError:
        # Happens if vocab is empty after stop words/short words filter
        return []
    except Exception as e:
        print(f"[ERROR] Keyword extraction failed: {e}")
        return []
