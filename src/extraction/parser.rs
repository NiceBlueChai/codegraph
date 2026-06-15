use crate::types::{ExtractionResult, Node, NodeKind, Language};
use crate::extraction::tree_sitter_parser::TreeSitterParser;
use log::debug;

pub struct CodeParser;

impl CodeParser {
    pub fn new() -> Self { Self }

    pub fn parse(&mut self, file_path: &str, source: &str) -> ExtractionResult {
        let mut result = ExtractionResult::new();
        let line_count = source.lines().count() as u32;
        let lang = Language::from_extension(
            std::path::Path::new(file_path).extension().and_then(|e| e.to_str()).unwrap_or("")
        ).unwrap_or(Language::Unknown);

        result.nodes.push(Node::new(
            format!("{}::[file]", file_path), NodeKind::File, file_path.into(), file_path.into(),
            file_path.into(), lang.clone(), 1, line_count.max(1), 0, 0));

        // Try tree-sitter parsing first for supported languages
        let use_tree_sitter = matches!(lang, Language::TypeScript | Language::JavaScript | Language::Python | Language::Rust);

        if use_tree_sitter {
            let mut ts_parser = TreeSitterParser::new();
            let ts_result = ts_parser.parse(file_path, source);
            if !ts_result.nodes.is_empty() {
                debug!("Tree-sitter parsed {} successfully: {} nodes, {} edges", file_path, ts_result.nodes.len(), ts_result.edges.len());
                result.nodes.extend(ts_result.nodes);
                result.edges.extend(ts_result.edges);
                result.unresolved_refs.extend(ts_result.unresolved_refs);
                return result;
            } else {
                debug!("Tree-sitter parsing returned empty for {}, falling back to text parsing", file_path);
            }
        }

        // Fallback to text-based parsing
        match lang {
            Language::TypeScript | Language::JavaScript => Self::extract_ts(source, file_path, &mut result),
            Language::Python => Self::extract_py(source, file_path, &mut result),
            Language::Rust => Self::extract_rs(source, file_path, &mut result),
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
                let name = t.replace("export ", "").replace("function ", "").split('(').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(format!("{}::{}#{}", file_path, name, c), NodeKind::Function, name.clone(), format!("{}::{}", file_path, name), file_path.into(), Language::TypeScript, i as u32+1, i as u32+1, 0, line.len() as u32));
                }
            }
            if t.starts_with("class ") || t.starts_with("export class ") {
                c += 1;
                let name = t.replace("export ", "").replace("class ", "").split(|ch| ch=='{'||ch==' ').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(format!("{}::{}#{}", file_path, name, c), NodeKind::Class, name.clone(), format!("{}::{}", file_path, name), file_path.into(), Language::TypeScript, i as u32+1, i as u32+1, 0, line.len() as u32));
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
                let name = t.replace("def ", "").split('(').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(format!("{}::{}#{}", file_path, name, c), NodeKind::Function, name.clone(), format!("{}::{}", file_path, name), file_path.into(), Language::Python, i as u32+1, i as u32+1, 0, line.len() as u32));
                }
            }
            if t.starts_with("class ") {
                c += 1;
                let name = t.replace("class ", "").split('(').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(format!("{}::{}#{}", file_path, name, c), NodeKind::Class, name.clone(), format!("{}::{}", file_path, name), file_path.into(), Language::Python, i as u32+1, i as u32+1, 0, line.len() as u32));
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
                let name = t.replace("pub ", "").replace("fn ", "").split('(').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(format!("{}::{}#{}", file_path, name, c), NodeKind::Function, name.clone(), format!("{}::{}", file_path, name), file_path.into(), Language::Rust, i as u32+1, i as u32+1, 0, line.len() as u32));
                }
            }
            if t.starts_with("struct ") || t.starts_with("pub struct ") {
                c += 1;
                let name = t.replace("pub ", "").replace("struct ", "").split(|ch| ch=='{'||ch=='<').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    result.nodes.push(Node::new(format!("{}::{}#{}", file_path, name, c), NodeKind::Struct, name.clone(), format!("{}::{}", file_path, name), file_path.into(), Language::Rust, i as u32+1, i as u32+1, 0, line.len() as u32));
                }
            }
        }
    }
}

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
