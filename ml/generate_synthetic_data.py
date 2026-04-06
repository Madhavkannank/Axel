import json
import random
import os
import string

domains = {
"Government ID": ["aadhaar card", "pan card", "passport", "driving license", "voter id", "ration card", "national id card", "resident permit", "work permit", "social security card", "citizenship certificate", "immigration card", "employee id card", "student id card", "army id card"],
"Invoice": ["electricity bill", "water bill", "gas bill", "internet bill", "mobile bill", "restaurant bill", "hotel invoice", "grocery bill", "pharmacy bill", "amazon invoice", "flipkart receipt", "service invoice", "repair bill", "tax invoice", "subscription invoice"],
"Finance": ["bank statement", "credit card statement", "debit card statement", "salary slip", "payslip", "income tax return", "form 16", "loan statement", "emi schedule", "insurance premium receipt", "mutual fund statement", "stock transaction report", "upi transaction receipt", "cheque deposit slip", "balance certificate"],
"Academic": ["assignment report", "lab record", "project report", "thesis document", "research paper", "lecture notes", "class notes", "exam question paper", "answer sheet", "study notes", "course material", "seminar paper", "presentation notes", "syllabus document", "reference notes"],
"Resume": ["resume", "curriculum vitae", "job application", "cover letter", "portfolio document", "internship application", "candidate profile", "professional bio", "linkedin profile export", "experience summary"],
"Legal": ["rental agreement", "lease agreement", "sale deed", "property document", "affidavit", "legal notice", "contract agreement", "nda agreement", "partnership deed", "court order", "case filing document", "will document", "power of attorney", "compliance document"],
"Medical": ["medical prescription", "lab report", "blood test report", "xray report", "mri scan report", "discharge summary", "health checkup report", "diagnostic report", "medical bill", "pharmacy prescription", "vaccination certificate", "covid test report", "insurance claim form"],
"Travel": ["flight ticket", "boarding pass", "train ticket", "bus ticket", "hotel booking confirmation", "travel itinerary", "visa document", "travel insurance document", "trip receipt", "cab booking receipt"],
"Personal": ["personal notes", "todo list", "meeting notes", "diary entry", "journal notes", "shopping list", "grocery list", "reminder notes", "budget notes", "planning notes"],
"Recipe": ["chicken curry recipe", "vegetable biryani recipe", "pasta recipe", "cake recipe", "dessert recipe", "soup recipe", "salad recipe", "breakfast recipe", "snack recipe", "juice recipe"],
"Book": ["data science book", "machine learning book", "python programming book", "history book", "novel ebook", "science textbook", "math textbook", "engineering handbook", "reference book", "tutorial ebook"],
"Communication": ["email thread export", "whatsapp chat export", "sms conversation", "support ticket conversation", "customer chat transcript", "meeting transcript", "call summary notes"],
"Certificates": ["birth certificate", "death certificate", "marriage certificate", "degree certificate", "diploma certificate", "course completion certificate", "experience certificate", "bonafide certificate", "transfer certificate"],
"Business": ["purchase order", "sales report", "inventory report", "profit report", "expense report", "business proposal", "quotation document", "invoice summary", "client report", "audit report"]
}

filler_words = "the and this is document for of file record notice attention official please see attached below summary report data info confidential page signature authorized date amount".split()

qualifiers = ["scanned copy", "original document", "digital copy", "official record", "verified document", "signed document", "final version", "archived copy", "latest version", "printed copy", "photocopy", "fax"]

data = []
for domain, items in domains.items():
    for item in items:
        # Generate 40 highly noisy instances per item
        for _ in range(40):
            qual = random.choice(qualifiers) if random.random() < 0.5 else ""
            
            num_fillers = random.randint(5, 20)
            fillers = random.choices(filler_words, k=num_fillers)
            
            if random.random() < 0.5:
                fillers.append(f"{random.randint(1,28)}/{random.randint(1,12)}/202{random.randint(0,5)}")
            if random.random() < 0.5:
                fillers.append(f"${random.uniform(10, 5000):.2f}")
            if random.random() < 0.5:
                fillers.append(f"ID{''.join(random.choices(string.digits, k=5))}")
                
            components = item.split() + qual.split() + fillers
            random.shuffle(components)
            
            text = " ".join(components)
            if random.random() < 0.3:
                chars = list(text)
                for i in range(len(chars)):
                    if random.random() < 0.05 and chars[i] in 'O0l1S5 ':
                        if chars[i] == 'O': chars[i] = '0'
                        elif chars[i] == '0': chars[i] = 'O'
                        elif chars[i] == 'l': chars[i] = '1'
                        elif chars[i] == '1': chars[i] = 'l'
                        elif chars[i] == 'S': chars[i] = '5'
                        elif chars[i] == '5': chars[i] = 'S'
                text = "".join(chars)
            
            data.append({
                "text": text,
                "category": domain
            })

os.makedirs('ml', exist_ok=True)
with open("ml/data.json", "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)

print(f"✅ Generated {len(data)} randomized bag-of-words samples")
