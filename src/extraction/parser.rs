//! Source-file parsing and lightweight fallback extractors for supported languages.

use crate::extraction::tree_sitter_parser::TreeSitterParser;
use crate::types::{ExtractionResult, Language, Node, NodeKind, UnresolvedReference};
use log::debug;
use once_cell::sync::Lazy;
use regex::Regex;

static C_FAMILY_FUNCTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        r#"(?m)^[ \t]*(?:template[^\n{;]*[ \t]*)?(?:[A-Za-z_][A-Za-z0-9_:<>,~*&\[\]\s]*\s+)?"#,
        r#"(?P<name>[A-Za-z_~][A-Za-z0-9_:~]*)\s*\([^;{}]*\)\s*"#,
        r#"(?:const\s*)?(?:noexcept\s*)?(?:override\s*)?(?:final\s*)?(?:->\s*[^ {]+)?\s*\{"#,
    ))
    .expect("valid C-family function regex")
});

static C_FAMILY_CALL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?P<callee>[A-Za-z_~][A-Za-z0-9_:~]*(?:\s*(?:\.|->|::)\s*[A-Za-z_~][A-Za-z0-9_:~]*)*)\s*\("#,
    )
    .expect("valid C-family call regex")
});

static GO_FUNCTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        r#"(?m)^[ \t]*func\s+"#,
        r#"(?P<receiver>\([^)]*\)\s*)?"#,
        r#"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*"#,
        r#"(?:\[[^\]]*\]\s*)?\([^;{}]*\)\s*[^{;]*\{"#,
    ))
    .expect("valid Go function regex")
});

static GO_CALL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?P<callee>[A-Za-z_][A-Za-z0-9_]*(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*)*)\s*\("#)
        .expect("valid Go call regex")
});

#[derive(Debug, Clone)]
struct CFamilyFunctionSpan {
    id: String,
    body_start: usize,
    body_end: usize,
}

/// Parses source files into CodeGraph nodes, edges, and unresolved references.
pub struct CodeParser;

impl CodeParser {
    /// Creates a parser instance with no retained per-file state.
    pub fn new() -> Self {
        Self
    }

    /// Parses one source file and returns extracted graph data.
    pub fn parse(&mut self, file_path: &str, source: &str) -> ExtractionResult {
        let mut result = ExtractionResult::new();
        let line_count = source.lines().count() as u32;
        let lang = Language::from_extension(
            std::path::Path::new(file_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""),
        )
        .unwrap_or(Language::Unknown);

        result.nodes.push(Node::new(
            format!("{}::[file]", file_path),
            NodeKind::File,
            file_path.into(),
            file_path.into(),
            file_path.into(),
            lang.clone(),
            1,
            line_count.max(1),
            0,
            0,
        ));

        // Try tree-sitter parsing first for supported languages
        let use_tree_sitter = matches!(
            lang,
            Language::TypeScript | Language::JavaScript | Language::Python | Language::Rust
        );

        if use_tree_sitter {
            let mut ts_parser = TreeSitterParser::new();
            let ts_result = ts_parser.parse(file_path, source);
            if !ts_result.nodes.is_empty() {
                debug!(
                    "Tree-sitter parsed {} successfully: {} nodes, {} edges",
                    file_path,
                    ts_result.nodes.len(),
                    ts_result.edges.len()
                );
                result.nodes.extend(ts_result.nodes);
                result.edges.extend(ts_result.edges);
                result.unresolved_refs.extend(ts_result.unresolved_refs);
                return result;
            } else {
                debug!(
                    "Tree-sitter parsing returned empty for {}, falling back to text parsing",
                    file_path
                );
            }
        }

        // Fallback to text-based parsing
        match lang {
            Language::TypeScript | Language::JavaScript => {
                Self::extract_ts(source, file_path, &mut result)
            }
            Language::Python => Self::extract_py(source, file_path, &mut result),
            Language::Rust => Self::extract_rs(source, file_path, &mut result),
            Language::C | Language::Cpp | Language::Java => {
                Self::extract_c_family(source, file_path, lang, &mut result)
            }
            Language::Go => Self::extract_go(source, file_path, &mut result),
            _ => {}
        }
        result
    }

    fn extract_ts(source: &str, file_path: &str, result: &mut ExtractionResult) {
        let mut c = 0;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim().to_string();
            if t.starts_with("function ") || t.starts_with("export function ") {
                c += 1;
                let name = t
                    .replace("export ", "")
                    .replace("function ", "")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(
                        format!("{}::{}#{}", file_path, name, c),
                        NodeKind::Function,
                        name.clone(),
                        format!("{}::{}", file_path, name),
                        file_path.into(),
                        Language::TypeScript,
                        i as u32 + 1,
                        i as u32 + 1,
                        0,
                        line.len() as u32,
                    ));
                }
            }
            if t.starts_with("class ") || t.starts_with("export class ") {
                c += 1;
                let name = t
                    .replace("export ", "")
                    .replace("class ", "")
                    .split(|ch| ch == '{' || ch == ' ')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(
                        format!("{}::{}#{}", file_path, name, c),
                        NodeKind::Class,
                        name.clone(),
                        format!("{}::{}", file_path, name),
                        file_path.into(),
                        Language::TypeScript,
                        i as u32 + 1,
                        i as u32 + 1,
                        0,
                        line.len() as u32,
                    ));
                }
            }
        }
    }

    fn extract_py(source: &str, file_path: &str, result: &mut ExtractionResult) {
        let mut c = 0;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim().to_string();
            if t.starts_with("def ") {
                c += 1;
                let name = t
                    .replace("def ", "")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(
                        format!("{}::{}#{}", file_path, name, c),
                        NodeKind::Function,
                        name.clone(),
                        format!("{}::{}", file_path, name),
                        file_path.into(),
                        Language::Python,
                        i as u32 + 1,
                        i as u32 + 1,
                        0,
                        line.len() as u32,
                    ));
                }
            }
            if t.starts_with("class ") {
                c += 1;
                let name = t
                    .replace("class ", "")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(
                        format!("{}::{}#{}", file_path, name, c),
                        NodeKind::Class,
                        name.clone(),
                        format!("{}::{}", file_path, name),
                        file_path.into(),
                        Language::Python,
                        i as u32 + 1,
                        i as u32 + 1,
                        0,
                        line.len() as u32,
                    ));
                }
            }
        }
    }

    fn extract_rs(source: &str, file_path: &str, result: &mut ExtractionResult) {
        let mut c = 0;
        for (i, line) in source.lines().enumerate() {
            let t = line.trim().to_string();
            if t.starts_with("fn ") || t.starts_with("pub fn ") {
                c += 1;
                let name = t
                    .replace("pub ", "")
                    .replace("fn ", "")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(
                        format!("{}::{}#{}", file_path, name, c),
                        NodeKind::Function,
                        name.clone(),
                        format!("{}::{}", file_path, name),
                        file_path.into(),
                        Language::Rust,
                        i as u32 + 1,
                        i as u32 + 1,
                        0,
                        line.len() as u32,
                    ));
                }
            }
            if t.starts_with("struct ") || t.starts_with("pub struct ") {
                c += 1;
                let name = t
                    .replace("pub ", "")
                    .replace("struct ", "")
                    .split(|ch| ch == '{' || ch == '<')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(
                        format!("{}::{}#{}", file_path, name, c),
                        NodeKind::Struct,
                        name.clone(),
                        format!("{}::{}", file_path, name),
                        file_path.into(),
                        Language::Rust,
                        i as u32 + 1,
                        i as u32 + 1,
                        0,
                        line.len() as u32,
                    ));
                }
            }
        }
    }

    fn extract_go(source: &str, file_path: &str, result: &mut ExtractionResult) {
        let masked_source = mask_c_family_noise(source);
        let mut functions = Vec::new();

        for captures in GO_FUNCTION_RE.captures_iter(&masked_source) {
            let Some(full_match) = captures.get(0) else {
                continue;
            };
            let Some(name_match) = captures.name("name") else {
                continue;
            };
            let name = name_match.as_str().trim().to_string();
            if name.is_empty() || is_go_keyword(&name) {
                continue;
            }

            let Some(open_brace) = masked_source[full_match.start()..full_match.end()]
                .rfind('{')
                .map(|offset| full_match.start() + offset)
            else {
                continue;
            };
            let Some(close_brace) = find_matching_brace(&masked_source, open_brace) else {
                continue;
            };

            let receiver = captures.name("receiver").and_then(|m| go_receiver_type(m.as_str()));
            let (start_line, start_col) = byte_position(source, name_match.start());
            let (end_line, end_col) = byte_position(source, close_brace);
            let id = format!("{}::{}#{}", file_path, name, start_line);
            let qualified_name = receiver
                .as_ref()
                .map(|receiver| format!("{}::{}.{}", file_path, receiver, name))
                .unwrap_or_else(|| format!("{}::{}", file_path, name));
            let mut node = Node::new(
                id.clone(),
                if receiver.is_some() {
                    NodeKind::Method
                } else {
                    NodeKind::Function
                },
                name.clone(),
                qualified_name,
                file_path.to_string(),
                Language::Go,
                start_line,
                end_line,
                start_col,
                end_col,
            );
            node.signature = Some(source[full_match.start()..open_brace].trim().to_string());
            result.nodes.push(node);
            functions.push(CFamilyFunctionSpan {
                id,
                body_start: open_brace + 1,
                body_end: close_brace,
            });
        }

        for function in functions {
            if function.body_start >= function.body_end || function.body_end > masked_source.len() {
                continue;
            }
            let body = &masked_source[function.body_start..function.body_end];
            for captures in GO_CALL_RE.captures_iter(body) {
                let Some(callee_match) = captures.name("callee") else {
                    continue;
                };
                let callee = normalize_go_callee(callee_match.as_str());
                if callee.is_empty() || is_go_keyword(&callee) {
                    continue;
                }
                let call_start = function.body_start + callee_match.start();
                let (line, col) = byte_position(source, call_start);
                result.unresolved_refs.push(UnresolvedReference {
                    id: Some(0),
                    from_node_id: function.id.clone(),
                    reference_name: callee,
                    reference_kind: "call".to_string(),
                    line,
                    col,
                    candidates: None,
                    file_path: file_path.to_string(),
                    language: Language::Go.as_str().to_string(),
                });
            }
        }
    }

    fn extract_c_family(
        source: &str,
        file_path: &str,
        lang: Language,
        result: &mut ExtractionResult,
    ) {
        let masked_source = mask_c_family_noise(source);
        let mut functions = Vec::new();

        for captures in C_FAMILY_FUNCTION_RE.captures_iter(&masked_source) {
            let Some(full_match) = captures.get(0) else {
                continue;
            };
            let Some(name_match) = captures.name("name") else {
                continue;
            };
            let raw_name = name_match.as_str().trim();
            let name = normalize_c_family_symbol_name(raw_name);
            if name.is_empty() || is_c_family_keyword(&name) {
                continue;
            }

            let Some(open_brace) = masked_source[full_match.start()..full_match.end()]
                .rfind('{')
                .map(|offset| full_match.start() + offset)
            else {
                continue;
            };
            let Some(close_brace) = find_matching_brace(&masked_source, open_brace) else {
                continue;
            };

            let (start_line, start_col) = byte_position(source, name_match.start());
            let (end_line, end_col) = byte_position(source, close_brace);
            let id = format!("{}::{}#{}", file_path, name, start_line);
            let mut node = Node::new(
                id.clone(),
                if raw_name.contains("::") {
                    NodeKind::Method
                } else {
                    NodeKind::Function
                },
                name.clone(),
                format!("{}::{}", file_path, raw_name),
                file_path.to_string(),
                lang.clone(),
                start_line,
                end_line,
                start_col,
                end_col,
            );
            node.signature = Some(source[full_match.start()..open_brace].trim().to_string());
            result.nodes.push(node);
            functions.push(CFamilyFunctionSpan {
                id,
                body_start: open_brace + 1,
                body_end: close_brace,
            });
        }

        for function in functions {
            if function.body_start >= function.body_end || function.body_end > masked_source.len() {
                continue;
            }
            let body = &masked_source[function.body_start..function.body_end];
            for captures in C_FAMILY_CALL_RE.captures_iter(body) {
                let Some(callee_match) = captures.name("callee") else {
                    continue;
                };
                let raw_callee = callee_match.as_str();
                let callee = normalize_c_family_callee(raw_callee);
                if callee.is_empty() || is_c_family_keyword(&callee) {
                    continue;
                }
                let call_start = function.body_start + callee_match.start();
                let (line, col) = byte_position(source, call_start);
                result.unresolved_refs.push(UnresolvedReference {
                    id: Some(0),
                    from_node_id: function.id.clone(),
                    reference_name: callee,
                    reference_kind: "call".to_string(),
                    line,
                    col,
                    candidates: None,
                    file_path: file_path.to_string(),
                    language: lang.as_str().to_string(),
                });
            }
        }
    }
}

fn mask_c_family_noise(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let start = i;
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            blank_non_newline_bytes(&mut out, start, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            blank_non_newline_bytes(&mut out, start, i);
            continue;
        }
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            blank_non_newline_bytes(&mut out, start, i);
            continue;
        }
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| source.to_string())
}

fn blank_non_newline_bytes(bytes: &mut [u8], start: usize, end: usize) {
    for byte in &mut bytes[start..end] {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

fn find_matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in source[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_brace + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn byte_position(source: &str, byte_index: usize) -> (u32, u32) {
    let capped = byte_index.min(source.len());
    let prefix = &source[..capped];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32 + 1;
    let col = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail.len())
        .unwrap_or(prefix.len()) as u32;
    (line, col)
}

fn normalize_c_family_symbol_name(raw_name: &str) -> String {
    raw_name
        .split("::")
        .filter(|part| !part.is_empty())
        .last()
        .unwrap_or(raw_name)
        .trim()
        .to_string()
}

fn normalize_c_family_callee(raw_callee: &str) -> String {
    let normalized = raw_callee
        .split_whitespace()
        .collect::<String>()
        .trim_start_matches('&')
        .trim()
        .replace("->", ".");
    for receiver in ["this.", "self.", "cls.", "super."] {
        if let Some(member) = normalized.strip_prefix(receiver) {
            return member.to_string();
        }
    }
    normalized
}

fn go_receiver_type(receiver: &str) -> Option<String> {
    let receiver = receiver.trim().trim_start_matches('(').trim_end_matches(')').trim();
    let raw_type = receiver.split_whitespace().last()?.trim();
    let name = raw_type
        .trim_start_matches('*')
        .trim_start_matches("[]")
        .rsplit('.')
        .next()
        .unwrap_or(raw_type)
        .trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn normalize_go_callee(raw_callee: &str) -> String {
    raw_callee
        .split_whitespace()
        .collect::<String>()
        .rsplit('.')
        .next()
        .unwrap_or(raw_callee)
        .trim()
        .to_string()
}

fn is_go_keyword(name: &str) -> bool {
    matches!(
        name,
        "break" | "case" | "chan" | "const" | "continue" | "default" | "defer" | "else"
            | "fallthrough" | "for" | "func" | "go" | "goto" | "if" | "import"
            | "interface" | "map" | "package" | "range" | "return" | "select" | "struct"
            | "switch" | "type" | "var"
    )
}

fn is_c_family_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "for" | "while" | "switch" | "catch" | "return" | "sizeof" | "alignof"
            | "static_cast" | "reinterpret_cast" | "const_cast" | "dynamic_cast" | "new"
            | "delete" | "defined" | "operator"
    )
}

/// Convenience wrapper for parsing a single source string.
pub fn parse_file(file_path: &str, source: &str) -> ExtractionResult {
    let mut parser = CodeParser::new();
    parser.parse(file_path, source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse() {
        let r = parse_file("test.ts", "export class Foo {}\nfunction bar() {}");
        assert!(r.nodes.len() >= 3);
    }
}
