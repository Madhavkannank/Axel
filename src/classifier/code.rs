use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct CodeSymbols {
    /// Exact symbol names (functions, classes, variables) e.g. "sobel_x", "image_path", "add"
    pub symbols: Vec<String>,
    /// Split identifier words e.g. "sobel", "image", "path"
    pub tokens: Vec<String>,
    /// Inferred domain hints (e.g. "calculator", "vision", "web", "ml")
    pub domain_hints: Vec<String>,
}

/// Extract symbols (functions, classes, variables, imports) from code text.
pub fn extract_code_symbols(code: &str, ext: &str) -> CodeSymbols {
    let mut raw_symbols = HashSet::new();
    let mut split_tokens = HashSet::new();

    // Line-by-line lightweight parser without heavy AST dependencies (keeps RAM < 10MB)
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        match ext {
            "py" => extract_python_line(trimmed, &mut raw_symbols),
            "rs" => extract_rust_line(trimmed, &mut raw_symbols),
            "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" => {
                extract_js_ts_line(trimmed, &mut raw_symbols)
            }
            "go" => extract_go_line(trimmed, &mut raw_symbols),
            "c" | "cpp" | "cc" | "h" | "hpp" | "java" => {
                extract_c_like_line(trimmed, &mut raw_symbols)
            }
            _ => extract_generic_code_line(trimmed, &mut raw_symbols),
        }
    }

    // Split compound identifiers (snake_case and camelCase)
    for sym in &raw_symbols {
        let parts = split_identifier(sym);
        for part in parts {
            if part.len() >= 2 && !is_code_keyword(&part) {
                split_tokens.insert(part.to_lowercase());
            }
        }
    }

    let domain_hints = detect_code_domain_hints(code, &raw_symbols, &split_tokens);

    let mut symbols_vec: Vec<String> = raw_symbols.into_iter().collect();
    symbols_vec.sort();

    let mut tokens_vec: Vec<String> = split_tokens.into_iter().collect();
    tokens_vec.sort();

    CodeSymbols {
        symbols: symbols_vec,
        tokens: tokens_vec,
        domain_hints,
    }
}

fn extract_python_line(line: &str, out: &mut HashSet<String>) {
    // 1. Functions: def func_name(
    if let Some(rest) = line.strip_prefix("def ") {
        if let Some(name) = rest.split('(').next() {
            let name = name.trim();
            if is_valid_ident(name) {
                out.insert(name.to_string());
            }
        }
    }
    // 2. Classes: class ClassName:
    else if let Some(rest) = line.strip_prefix("class ") {
        if let Some(name) = rest.split(['(', ':']).next() {
            let name = name.trim();
            if is_valid_ident(name) {
                out.insert(name.to_string());
            }
        }
    }
    // 3. Imports: import foo, from foo import bar
    else if let Some(rest) = line.strip_prefix("import ") {
        for part in rest.split(',') {
            let mod_name = part.trim().split_whitespace().next().unwrap_or("");
            if is_valid_ident(mod_name) {
                out.insert(mod_name.to_string());
            }
        }
    } else if let Some(rest) = line.strip_prefix("from ") {
        if let Some(mod_name) = rest.split_whitespace().next() {
            if is_valid_ident(mod_name) {
                out.insert(mod_name.to_string());
            }
        }
    }
    // 4. Assignments: var_name = ...
    else if line.contains('=') && !line.starts_with("if ") && !line.starts_with("while ") {
        if let Some(lhs) = line.split('=').next() {
            let lhs = lhs.trim();
            // multiple assignments: a, b = ...
            for item in lhs.split(',') {
                let var = item.trim();
                if is_valid_ident(var) && !is_code_keyword(var) {
                    out.insert(var.to_string());
                }
            }
        }
    }
}

fn extract_rust_line(line: &str, out: &mut HashSet<String>) {
    let clean = if let Some(rest) = line.strip_prefix("pub ") {
        rest
    } else {
        line
    };

    if let Some(rest) = clean.strip_prefix("fn ") {
        if let Some(name) = rest.split(['(', '<']).next() {
            let name = name.trim();
            if is_valid_ident(name) {
                out.insert(name.to_string());
            }
        }
    } else if let Some(rest) = clean.strip_prefix("struct ")
        .or_else(|| clean.strip_prefix("enum "))
        .or_else(|| clean.strip_prefix("trait "))
    {
        if let Some(name) = rest.split(['<', '{', ';', '(']).next() {
            let name = name.trim();
            if is_valid_ident(name) {
                out.insert(name.to_string());
            }
        }
    } else if let Some(rest) = clean.strip_prefix("let ") {
        let rest = rest.strip_prefix("mut ").unwrap_or(rest);
        if let Some(lhs) = rest.split([':', '=']).next() {
            let var = lhs.trim();
            if is_valid_ident(var) && !is_code_keyword(var) {
                out.insert(var.to_string());
            }
        }
    }
}

fn extract_js_ts_line(line: &str, out: &mut HashSet<String>) {
    let clean = if let Some(rest) = line.strip_prefix("export ") {
        rest
    } else {
        line
    };

    if let Some(rest) = clean.strip_prefix("function ") {
        if let Some(name) = rest.split(['(', '<']).next() {
            let name = name.trim();
            if is_valid_ident(name) {
                out.insert(name.to_string());
            }
        }
    } else if let Some(rest) = clean.strip_prefix("const ")
        .or_else(|| clean.strip_prefix("let "))
        .or_else(|| clean.strip_prefix("var "))
    {
        if let Some(lhs) = rest.split([':', '=']).next() {
            let var = lhs.trim();
            if is_valid_ident(var) && !is_code_keyword(var) {
                out.insert(var.to_string());
            }
        }
    } else if let Some(rest) = clean.strip_prefix("class ")
        .or_else(|| clean.strip_prefix("interface "))
        .or_else(|| clean.strip_prefix("type "))
    {
        if let Some(name) = rest.split(['<', '{', '=', ' ']).next() {
            let name = name.trim();
            if is_valid_ident(name) {
                out.insert(name.to_string());
            }
        }
    }
}

fn extract_go_line(line: &str, out: &mut HashSet<String>) {
    if let Some(rest) = line.strip_prefix("func ") {
        // func (r *Receiver) Method(...) or func Function(...)
        let target = if rest.starts_with('(') {
            rest.split(')').nth(1).unwrap_or("")
        } else {
            rest
        };
        if let Some(name) = target.trim().split('(').next() {
            let name = name.trim();
            if is_valid_ident(name) {
                out.insert(name.to_string());
            }
        }
    } else if line.contains(":=") {
        if let Some(lhs) = line.split(":=").next() {
            for item in lhs.split(',') {
                let var = item.trim();
                if is_valid_ident(var) && !is_code_keyword(var) {
                    out.insert(var.to_string());
                }
            }
        }
    }
}

fn extract_c_like_line(line: &str, out: &mut HashSet<String>) {
    if line.contains('(') && line.ends_with('{') {
        let before_paren = line.split('(').next().unwrap_or("").trim();
        if let Some(name) = before_paren.split_whitespace().last() {
            let name = name.trim_start_matches('*');
            if is_valid_ident(name) && !is_code_keyword(name) {
                out.insert(name.to_string());
            }
        }
    }
}

fn extract_generic_code_line(line: &str, out: &mut HashSet<String>) {
    if line.contains('=') {
        if let Some(lhs) = line.split('=').next() {
            let candidate = lhs.split_whitespace().last().unwrap_or("");
            if is_valid_ident(candidate) && !is_code_keyword(candidate) {
                out.insert(candidate.to_string());
            }
        }
    }
}

/// Split compound names like `sobel_x` -> `["sobel", "x"]` and `imagePath` -> `["image", "path"]`
pub fn split_identifier(s: &str) -> Vec<String> {
    let mut results = Vec::new();

    // 1. Split on underscores / dashes
    for segment in s.split(|c| c == '_' || c == '-') {
        if segment.is_empty() {
            continue;
        }

        // 2. Split on camelCase transitions
        let mut current = String::new();
        let mut chars = segment.chars().peekable();

        while let Some(ch) = chars.next() {
            current.push(ch);
            if let Some(&next) = chars.peek() {
                if ch.is_lowercase() && next.is_uppercase() {
                    results.push(current.clone());
                    current.clear();
                }
            }
        }
        if !current.is_empty() {
            results.push(current);
        }
    }

    results
}

fn is_valid_ident(s: &str) -> bool {
    if s.is_empty() || s.len() > 64 {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn is_code_keyword(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "def"
            | "fn"
            | "pub"
            | "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "in"
            | "import"
            | "from"
            | "as"
            | "class"
            | "struct"
            | "enum"
            | "let"
            | "mut"
            | "const"
            | "var"
            | "true"
            | "false"
            | "none"
            | "null"
            | "nil"
            | "self"
            | "this"
            | "super"
            | "break"
            | "continue"
            | "pass"
            | "match"
            | "case"
            | "switch"
            | "try"
            | "catch"
            | "except"
            | "finally"
            | "raise"
            | "throw"
            | "yield"
            | "async"
            | "await"
            | "lambda"
            | "type"
            | "interface"
            | "package"
            | "func"
            | "static"
            | "void"
            | "int"
            | "float"
            | "str"
            | "bool"
            | "char"
            | "string"
            | "new"
    )
}

/// Detect semantic domains from code patterns (e.g. calculator, computer vision, web, etc.)
fn detect_code_domain_hints(
    code: &str,
    _symbols: &HashSet<String>,
    tokens: &HashSet<String>,
) -> Vec<String> {
    let mut hints = HashSet::new();

    let all_tokens_lower: HashSet<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
    let code_lower = code.to_lowercase();

    // 1. Calculator / Arithmetic hint
    let arithmetic_names = ["add", "sub", "subtract", "mul", "multiply", "div", "divide", "calc", "calculate", "modulo", "power"];
    let arithmetic_hits = arithmetic_names.iter().filter(|&&name| all_tokens_lower.contains(name)).count();
    let has_arithmetic_ops = (code.contains('+') || code.contains('-')) && (code.contains('*') || code.contains('/'));

    if arithmetic_hits >= 2 || (arithmetic_hits >= 1 && has_arithmetic_ops) {
        hints.insert("calculator".to_string());
        hints.insert("arithmetic".to_string());
        hints.insert("math".to_string());
    }

    // 2. Computer Vision / Image processing hint
    let vision_markers = ["cv2", "sobel", "imread", "imwrite", "cvtcolor", "opencv", "pillow", "pil", "image", "grayscale", "edge"];
    let vision_hits = vision_markers.iter().filter(|&&v| all_tokens_lower.contains(v) || code_lower.contains(v)).count();
    if vision_hits >= 2 {
        hints.insert("computer vision".to_string());
        hints.insert("image processing".to_string());
        hints.insert("opencv".to_string());
    }

    // 3. Machine Learning / Data Science
    let ml_markers = ["torch", "tensorflow", "sklearn", "fit", "predict", "train", "dataset", "epoch", "loss", "tensor", "numpy", "pandas", "dataframe"];
    let ml_hits = ml_markers.iter().filter(|&&m| all_tokens_lower.contains(m) || code_lower.contains(m)).count();
    if ml_hits >= 3 {
        hints.insert("machine learning".to_string());
        hints.insert("data science".to_string());
    }

    // 4. Web / API
    let web_markers = ["http", "request", "response", "get", "post", "endpoint", "router", "route", "server", "express", "fastapi", "flask", "axum"];
    let web_hits = web_markers.iter().filter(|&&w| all_tokens_lower.contains(w) || code_lower.contains(w)).count();
    if web_hits >= 2 {
        hints.insert("web api".to_string());
        hints.insert("backend".to_string());
    }

    let mut res: Vec<String> = hints.into_iter().collect();
    res.sort();
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_python_calculator() {
        let code = r#"
def add(a, b):
    return a + b

def subtract(a, b):
    return a - b

def multiply(a, b):
    return a * b

def divide(a, b):
    if b == 0:
        return None
    return a / b
"#;

        let symbols = extract_code_symbols(code, "py");
        assert!(symbols.symbols.contains(&"add".to_string()));
        assert!(symbols.symbols.contains(&"subtract".to_string()));
        assert!(symbols.symbols.contains(&"multiply".to_string()));
        assert!(symbols.symbols.contains(&"divide".to_string()));
        // Must deduce calculator / arithmetic
        assert!(symbols.domain_hints.contains(&"calculator".to_string()));
        assert!(symbols.domain_hints.contains(&"arithmetic".to_string()));
    }

    #[test]
    fn test_extract_python_vision_10_py() {
        let code = r#"
import cv2
import numpy as np

image_files = ["test.png"]
image_path = image_files[0]
image = cv2.imread(image_path)
gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
sobel_x = cv2.Sobel(gray, cv2.CV_64F, 1, 0, ksize=3)
sobel_y = cv2.Sobel(gray, cv2.CV_64F, 0, 1, ksize=3)
sobel = np.sqrt(sobel_x**2 + sobel_y**2)
cv2.imwrite("sobel_output.jpg", sobel)
"#;

        let symbols = extract_code_symbols(code, "py");
        assert!(symbols.symbols.contains(&"image_path".to_string()));
        assert!(symbols.symbols.contains(&"sobel_x".to_string()));
        assert!(symbols.symbols.contains(&"sobel_y".to_string()));
        assert!(symbols.symbols.contains(&"sobel".to_string()));
        // Sub-token split should include "sobel"
        assert!(symbols.tokens.contains(&"sobel".to_string()));
        // Domain hints should include computer vision
        assert!(symbols.domain_hints.contains(&"computer vision".to_string()) || symbols.domain_hints.contains(&"image processing".to_string()));
    }

    #[test]
    fn test_split_identifier() {
        assert_eq!(split_identifier("sobel_x"), vec!["sobel", "x"]);
        assert_eq!(split_identifier("imagePath"), vec!["image", "Path"]);
        assert_eq!(split_identifier("CalculatorApp"), vec!["Calculator", "App"]);
    }
}
