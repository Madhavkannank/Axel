import json
from ml.classifier import predict_category
from ml.keywords import extract_keywords

with open('/tmp/test_data.json', 'r', encoding='utf-8') as f:
    test_cases = json.load(f)

correct = 0
total = len(test_cases)

print("="*50)
print("TEST RESULTS")
print("="*50)

for idx, case in enumerate(test_cases, 1):
    text = case['text']
    true_cat = case['category']
    if true_cat == "Notes": true_cat = "Personal"
    
    pred_cat, conf = predict_category(text)
    keywords = extract_keywords(text)
    
    is_correct = (pred_cat == true_cat)
    if is_correct:
        correct += 1
        status = "PASS"
    else:
        status = f"FAIL (Pred: {pred_cat})"
        
    print(f"[{idx}/{total}] {status} | Conf: {conf:.2f} | True: {true_cat}")
    print(f"   Text: {text}")
    print(f"   Keywords: {keywords}")

accuracy = correct / total * 100
print("="*50)
print(f"Final Validation Accuracy: {accuracy:.2f}% ({correct}/{total})")
print("="*50)
