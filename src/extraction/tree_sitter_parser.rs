//! Tree-sitter AST extraction for languages with native grammar support.

use crate::types::*;
use log::debug;
use tree_sitter::{Node as TSTreeSitterNode, Parser};

/// Tree-sitter based code parser providing accurate AST extraction
pub struct TreeSitterParser {
    parser: Parser,
}

impl TreeSitterParser {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
        }
    }

    /// Parse source code using tree-sitter for the given language
    pub fn parse(&mut self, file_path: &str, source: &str) -> ExtractionResult {
        let mut result = ExtractionResult::new();
        let lang = Language::from_extension(
            std::path::Path::new(file_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""),
        )
        .unwrap_or(Language::Unknown);

        let ts_lang = match lang {
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::JavaScript => tree_sitter_typescript::LANGUAGE_TSX.into(), // Use TSX for JS
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            _ => {
                debug!("No tree-sitter grammar for {:?}, skipping AST parse", lang);
                return result;
            }
        };

        if self.parser.set_language(&ts_lang).is_err() {
            debug!("Failed to set tree-sitter language for {:?}", lang);
            return result;
        }

        let tree = match self.parser.parse(source, None) {
            Some(t) => t,
            None => {
                debug!("Failed to parse {}", file_path);
                return result;
            }
        };

        let line_count = source.lines().count() as u32;

        // Add file node
        result.nodes.push(Node::new(
            format!("{}::[file]", file_path),
            NodeKind::File,
            file_path.to_string(),
            file_path.to_string(),
            file_path.to_string(),
            lang.clone(),
            1,
            line_count.max(1),
            0,
            0,
        ));

        let root = tree.root_node();
        self.walk_node(root, source, file_path, &lang, &mut result);

        debug!(
            "Tree-sitter parsed {}: {} nodes, {} edges",
            file_path,
            result.nodes.len(),
            result.edges.len()
        );
        result
    }

    /// Recursively walk AST nodes and extract code elements
    fn walk_node(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let node_type = node.kind();

        match node_type {
            // Functions
            "function_declaration" | "function_signature" => {
                self.extract_function(node, source, file_path, lang, result);
            }

            // Arrow functions and function expressions assigned to variables
            "lexical_declaration" | "variable_declaration" => {
                self.extract_variable_decl(node, source, file_path, lang, result);
            }

            // Classes
            "class_declaration" | "abstract_class_declaration" => {
                self.extract_class(node, source, file_path, lang, result);
            }

            // Interfaces (TypeScript)
            "interface_declaration" => {
                self.extract_interface(node, source, file_path, lang, result);
            }

            // Type aliases (TypeScript)
            "type_alias_declaration" => {
                self.extract_type_alias(node, source, file_path, lang, result);
            }

            // Enums
            "enum_declaration" => {
                self.extract_enum(node, source, file_path, lang, result);
            }

            // Imports
            "import_statement" => {
                self.extract_import(node, source, file_path, lang, result);
            }
            "import_from_statement" => {
                if *lang == Language::Python {
                    self.extract_python_import_from(node, source, file_path, result);
                }
            }
            "export_statement" => {
                if node.child_by_field_name("source").is_some() {
                    self.extract_import(node, source, file_path, lang, result);
                }
            }

            // Method definitions inside classes
            "method_definition" => {
                self.extract_method(node, source, file_path, lang, result);
            }

            // Python-specific
            "function_definition" | "decorated_definition" => {
                if *lang == Language::Python {
                    self.extract_python_function(node, source, file_path, result);
                }
            }
            "class_definition" => {
                if *lang == Language::Python {
                    self.extract_python_class(node, source, file_path, result);
                }
            }

            // Rust-specific
            "function_item" => {
                if *lang == Language::Rust {
                    if find_parent_kind(node, "impl_item").is_some() {
                        self.extract_rust_impl_method(node, source, file_path, result);
                    } else {
                        self.extract_rust_function(node, source, file_path, result);
                    }
                }
            }
            "struct_item" => {
                if *lang == Language::Rust {
                    self.extract_rust_struct(node, source, file_path, result);
                }
            }
            "trait_item" => {
                if *lang == Language::Rust {
                    self.extract_rust_trait(node, source, file_path, result);
                }
            }
            "impl_item" => {
                if *lang == Language::Rust {
                    self.extract_rust_impl(node, source, file_path, result);
                }
            }
            "use_declaration" => {
                if *lang == Language::Rust {
                    self.extract_rust_use(node, source, file_path, result);
                }
            }
            "struct_expression" => {
                if *lang == Language::Rust {
                    self.extract_rust_struct_expression(node, source, file_path, result);
                }
            }

            // Call expressions (for call edges)
            "call_expression" => {
                self.extract_call(node, source, file_path, lang, result);
            }
            "call" => {
                if *lang == Language::Python {
                    self.extract_call(node, source, file_path, lang, result);
                }
            }

            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.walk_node(child, source, file_path, lang, result);
        }
    }

    fn extract_function(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let mut n = Node::new(
            id,
            NodeKind::Function,
            name,
            qualified_name,
            file_path.to_string(),
            lang.clone(),
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        n.signature = Some(self.get_node_text(node, source));
        n.is_async = self.has_modifier(node, "async");
        n.is_exported = self.is_exported(node);
        result.nodes.push(n);
    }

    fn extract_variable_decl(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        // Check if any declarator has an arrow function or function expression
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                if let Some(value) = child.child_by_field_name("value") {
                    if value.kind() == "arrow_function" || value.kind() == "function_expression" {
                        let name = self
                            .get_child_text(child, "name", source)
                            .unwrap_or_default();
                        if name.is_empty() {
                            continue;
                        }

                        let start = node.start_position();
                        let end = node.end_position();
                        let qualified_name = format!("{}::{}", file_path, name);
                        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

                        let n = Node::new(
                            id,
                            NodeKind::Function,
                            name,
                            qualified_name,
                            file_path.to_string(),
                            lang.clone(),
                            (start.row + 1) as u32,
                            (end.row + 1) as u32,
                            start.column as u32,
                            end.column as u32,
                        );
                        result.nodes.push(n);
                    }
                }
            }
        }
    }

    fn extract_class(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let mut n = Node::new(
            id.clone(),
            NodeKind::Class,
            name.clone(),
            qualified_name,
            file_path.to_string(),
            lang.clone(),
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        n.is_abstract = node.kind() == "abstract_class_declaration";
        result.nodes.push(n);

        // Extract extends relationship
        if let Some(super_clause) = self.find_child(node, "class_heritage") {
            if let Some(parent_name) = self.get_node_text_parts(super_clause, source).first() {
                // Create unresolved reference for resolution
                let uref = UnresolvedReference {
                    id: Some(0),
                    from_node_id: id.clone(),
                    reference_name: parent_name.clone(),
                    reference_kind: "extends".to_string(),
                    line: (start.row + 1) as u32,
                    col: start.column as u32,
                    candidates: None,
                    file_path: file_path.to_string(),
                    language: "unknown".to_string(),
                };
                result.unresolved_refs.push(uref);
            }
        }
    }

    fn extract_interface(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let n = Node::new(
            id,
            NodeKind::Interface,
            name,
            qualified_name,
            file_path.to_string(),
            lang.clone(),
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        result.nodes.push(n);
    }

    fn extract_type_alias(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let n = Node::new(
            id,
            NodeKind::TypeAlias,
            name,
            qualified_name,
            file_path.to_string(),
            lang.clone(),
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        result.nodes.push(n);
    }

    fn extract_enum(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let n = Node::new(
            id,
            NodeKind::Enum,
            name,
            qualified_name,
            file_path.to_string(),
            lang.clone(),
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        result.nodes.push(n);
    }

    fn extract_import(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        // Extract import source
        let source_text = self.get_child_text(node, "source", source);
        if let Some(src) = source_text {
            let clean_src = src.trim_matches(|c| c == '\'' || c == '"');
            let uref = UnresolvedReference {
                id: Some(0),
                from_node_id: format!("{}::[file]", file_path),
                reference_name: clean_src.to_string(),
                reference_kind: "import".to_string(),
                line: (node.start_position().row + 1) as u32,
                col: node.start_position().column as u32,
                candidates: None,
                file_path: file_path.to_string(),
                language: lang.as_str().to_string(),
            };
            result.unresolved_refs.push(uref);
        }
    }

    fn extract_python_import_from(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let text = self.get_node_text(node, source);
        let Some(import_index) = text.find(" import ") else {
            return;
        };
        let module = text
            .trim_start_matches("from ")
            .get(..import_index.saturating_sub("from ".len()))
            .unwrap_or("")
            .trim();
        let names = text[import_index + " import ".len()..].trim();
        if names == "*" {
            return;
        }

        for raw_name in names.split(',') {
            let local_name = raw_name
                .split(" as ")
                .last()
                .unwrap_or(raw_name)
                .trim()
                .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_');
            if local_name.is_empty() {
                continue;
            }
            let reference_name = if module == "." {
                format!(".{}", local_name)
            } else {
                local_name.to_string()
            };
            result.unresolved_refs.push(UnresolvedReference {
                id: Some(0),
                from_node_id: format!("{}::[file]", file_path),
                reference_name,
                reference_kind: "imports".to_string(),
                line: (node.start_position().row + 1) as u32,
                col: node.start_position().column as u32,
                candidates: None,
                file_path: file_path.to_string(),
                language: Language::Python.as_str().to_string(),
            });
        }
    }

    fn extract_method(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let mut n = Node::new(
            id,
            NodeKind::Method,
            name,
            qualified_name,
            file_path.to_string(),
            lang.clone(),
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        n.is_async = self.has_modifier(node, "async");
        n.is_static = self.has_modifier(node, "static");
        result.nodes.push(n);
    }

    fn extract_python_function(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let actual = if node.kind() == "decorated_definition" {
            match self.find_child(node, "function_definition") {
                Some(child) => child,
                None => node,
            }
        } else {
            node
        };

        let name = self.get_child_text(actual, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let mut n = Node::new(
            id,
            NodeKind::Function,
            name,
            qualified_name,
            file_path.to_string(),
            Language::Python,
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        n.is_async = self.has_child(actual, "async");
        result.nodes.push(n);
    }

    fn extract_python_class(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let actual = if node.kind() == "decorated_definition" {
            match self.find_child(node, "class_definition") {
                Some(child) => child,
                None => node,
            }
        } else {
            node
        };

        let name = self.get_child_text(actual, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let n = Node::new(
            id,
            NodeKind::Class,
            name,
            qualified_name,
            file_path.to_string(),
            Language::Python,
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        result.nodes.push(n);
    }

    fn extract_rust_function(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let mut n = Node::new(
            id,
            NodeKind::Function,
            name,
            qualified_name,
            file_path.to_string(),
            Language::Rust,
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        n.is_async = self.has_child_text(node, "async");
        n.visibility = Some(if self.has_child_text(node, "pub") {
            "public".to_string()
        } else {
            "private".to_string()
        });
        result.nodes.push(n);
    }

    fn extract_rust_struct(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let qualified_name = format!("{}::{}", file_path, name);
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);

        let n = Node::new(
            id,
            NodeKind::Struct,
            name,
            qualified_name,
            file_path.to_string(),
            Language::Rust,
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        result.nodes.push(n);
    }

    fn extract_rust_impl(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let Some((trait_name, type_name)) = rust_impl_trait_and_type(&self.get_node_text(node, source)) else {
            return;
        };
        let Some(type_node) = result.nodes.iter().find(|candidate| {
            candidate.language == Language::Rust
                && matches!(candidate.kind, NodeKind::Struct | NodeKind::Enum | NodeKind::Class)
                && candidate.name == type_name
        }) else {
            return;
        };

        result.unresolved_refs.push(UnresolvedReference {
            id: Some(0),
            from_node_id: type_node.id.clone(),
            reference_name: trait_name,
            reference_kind: "implements".to_string(),
            line: (node.start_position().row + 1) as u32,
            col: node.start_position().column as u32,
            candidates: None,
            file_path: file_path.to_string(),
            language: Language::Rust.as_str().to_string(),
        });
    }

    fn extract_rust_trait(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let id = format!("{}::{}#{}", file_path, name, start.row + 1);
        let mut trait_node = Node::new(
            id.clone(),
            NodeKind::Trait,
            name.clone(),
            format!("{}::{}", file_path, name),
            file_path.to_string(),
            Language::Rust,
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        trait_node.visibility = Some(if self.has_child_text(node, "pub") {
            "public".to_string()
        } else {
            "private".to_string()
        });
        trait_node.is_exported = trait_node.visibility.as_deref() == Some("public");
        result.nodes.push(trait_node);

        if let Some(body) = self.find_child(node, "declaration_list") {
            let mut cursor = body.walk();
            for child in body.named_children(&mut cursor) {
                if child.kind() == "function_signature_item" {
                    self.extract_rust_trait_method(child, source, file_path, result, &id, &name);
                }
            }
        }
    }

    fn extract_rust_trait_method(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
        trait_id: &str,
        trait_name: &str,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }

        let start = node.start_position();
        let end = node.end_position();
        let method_id = format!("{}::{}.{}#{}", file_path, trait_name, name, start.row + 1);
        let mut method = Node::new(
            method_id.clone(),
            NodeKind::Method,
            name.clone(),
            format!("{}::{}.{}", file_path, trait_name, name),
            file_path.to_string(),
            Language::Rust,
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        method.signature = Some(self.get_node_text(node, source));
        result.nodes.push(method);
        result.edges.push(Edge::new(
            trait_id.to_string(),
            method_id,
            EdgeKind::Contains,
        ));
    }

    fn extract_rust_impl_method(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let name = self.get_child_text(node, "name", source).unwrap_or_default();
        if name.is_empty() {
            return;
        }
        let Some(impl_node) = find_parent_kind(node, "impl_item") else {
            return;
        };
        let owner = rust_impl_self_type(&self.get_node_text(impl_node, source))
            .unwrap_or_else(|| "impl".to_string());
        let start = node.start_position();
        let end = node.end_position();
        let method_id = format!("{}::{}#{}", file_path, name, start.row + 1);
        let mut method = Node::new(
            method_id.clone(),
            NodeKind::Method,
            name.clone(),
            format!("{}::{}.{}", file_path, owner, name),
            file_path.to_string(),
            Language::Rust,
            (start.row + 1) as u32,
            (end.row + 1) as u32,
            start.column as u32,
            end.column as u32,
        );
        method.signature = Some(self.get_node_text(node, source));
        result.nodes.push(method);
        if let Some(owner_node) = result.nodes.iter().find(|candidate| {
            candidate.language == Language::Rust
                && matches!(candidate.kind, NodeKind::Struct | NodeKind::Enum | NodeKind::Class)
                && candidate.name == owner
        }) {
            result.edges.push(Edge::new(
                owner_node.id.clone(),
                method_id,
                EdgeKind::Contains,
            ));
        }
    }

    fn extract_rust_use(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let text = self
            .get_node_text(node, source)
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_string();
        let Some(path) = text
            .strip_prefix("pub use ")
            .or_else(|| text.strip_prefix("use "))
            .map(str::trim)
        else {
            return;
        };
        if path.ends_with("::*") || path.is_empty() {
            return;
        }

        result.unresolved_refs.push(UnresolvedReference {
            id: Some(0),
            from_node_id: format!("{}::[file]", file_path),
            reference_name: path.to_string(),
            reference_kind: "imports".to_string(),
            line: (node.start_position().row + 1) as u32,
            col: node.start_position().column as u32,
            candidates: None,
            file_path: file_path.to_string(),
            language: Language::Rust.as_str().to_string(),
        });
    }

    fn extract_rust_struct_expression(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        result: &mut ExtractionResult,
    ) {
        let Some(name_node) = node.child_by_field_name("name").or_else(|| node.named_child(0)) else {
            return;
        };
        let reference_name = rust_type_leaf_name(&self.get_node_text(name_node, source));
        if reference_name.is_empty() {
            return;
        }
        let Some(from_node_id) = self.find_enclosing_function_id(node, source, file_path) else {
            return;
        };

        result.unresolved_refs.push(UnresolvedReference {
            id: Some(0),
            from_node_id,
            reference_name,
            reference_kind: "instantiation".to_string(),
            line: (node.start_position().row + 1) as u32,
            col: node.start_position().column as u32,
            candidates: None,
            file_path: file_path.to_string(),
            language: Language::Rust.as_str().to_string(),
        });
    }

    fn extract_call(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
        lang: &Language,
        result: &mut ExtractionResult,
    ) {
        let func_node = node.child_by_field_name("function");
        if let Some(func) = func_node {
            let callee = self.get_node_text(func, source);
            if !callee.is_empty() {
                // Try to find parent function
                let caller_id = self.find_enclosing_function_id(node, source, file_path);
                if let Some(caller) = caller_id {
                    // Create unresolved ref for resolution
                    let uref = UnresolvedReference {
                        id: Some(0),
                        from_node_id: caller,
                        reference_name: callee,
                        reference_kind: "call".to_string(),
                        line: (node.start_position().row + 1) as u32,
                        col: node.start_position().column as u32,
                        candidates: None,
                        file_path: file_path.to_string(),
                        language: lang.as_str().to_string(),
                    };
                    result.unresolved_refs.push(uref);
                }
            }
        }
    }

    // Helper: find enclosing function node and return its ID
    fn find_enclosing_function_id(
        &self,
        node: TSTreeSitterNode,
        source: &str,
        file_path: &str,
    ) -> Option<String> {
        let mut current = node.parent()?;
        loop {
            match current.kind() {
                "function_declaration" | "function_expression" | "arrow_function"
                | "method_definition" | "function_item" | "function_definition" => {
                    let name = self
                        .get_child_text(current, "name", source)
                        .unwrap_or_else(|| "anonymous".to_string());
                    let start = current.start_position();
                    return Some(format!("{}::{}#{}", file_path, name, start.row + 1));
                }
                _ => {
                    current = current.parent()?;
                }
            }
        }
    }

    // Helper: get child node text by field name
    fn get_child_text(&self, node: TSTreeSitterNode, field: &str, source: &str) -> Option<String> {
        if source.is_empty() {
            return None;
        }
        node.child_by_field_name(field)
            .and_then(|c| {
                let start = c.start_byte();
                let end = c.end_byte();
                if start >= source.len() || end > source.len() || start > end {
                    return None;
                }
                c.utf8_text(source.as_bytes()).ok()
            })
            .map(|s| s.to_string())
    }

    // Helper: get full node text
    fn get_node_text(&self, node: TSTreeSitterNode, source: &str) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        if start >= source.len() || end > source.len() || start > end {
            return String::new();
        }
        node.utf8_text(source.as_bytes())
            .unwrap_or("")
            .to_string()
    }

    // Helper: get text parts of a node (split by whitespace)
    fn get_node_text_parts(&self, node: TSTreeSitterNode, source: &str) -> Vec<String> {
        self.get_node_text(node, source)
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    }

    // Helper: find first child of a given type
    fn find_child<'a>(&self, node: TSTreeSitterNode<'a>, kind: &str) -> Option<TSTreeSitterNode<'a>> {
        let mut cursor = node.walk();
        let result = node.named_children(&mut cursor).find(|c| c.kind() == kind);
        result
    }

    // Helper: check if node has a child of given type
    fn has_child(&self, node: TSTreeSitterNode, kind: &str) -> bool {
        self.find_child(node, kind).is_some()
    }

    // Helper: check if node has a child with specific text
    fn has_child_text(&self, node: TSTreeSitterNode, text: &str) -> bool {
        let mut cursor = node.walk();
        let result = node.children(&mut cursor)
            .any(|c| c.kind() == text);
        result
    }

    // Helper: check if node has a modifier
    fn has_modifier(&self, node: TSTreeSitterNode, modifier: &str) -> bool {
        let mut cursor = node.walk();
        let result = node.children(&mut cursor).any(|c| c.kind() == modifier);
        result
    }

    // Helper: check if node is exported
    fn is_exported(&self, node: TSTreeSitterNode) -> bool {
        // Check for "export" keyword before the node
        if let Some(prev) = node.prev_named_sibling() {
            prev.kind() == "export_statement"
        } else {
            // Check parent
            if let Some(parent) = node.parent() {
                parent.kind() == "export_statement"
            } else {
                false
            }
        }
    }
}

fn rust_type_leaf_name(raw_name: &str) -> String {
    raw_name
        .split("::")
        .last()
        .unwrap_or(raw_name)
        .split('<')
        .next()
        .unwrap_or(raw_name)
        .trim()
        .to_string()
}

fn find_parent_kind<'a>(
    mut node: TSTreeSitterNode<'a>,
    kind: &str,
) -> Option<TSTreeSitterNode<'a>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn rust_impl_trait_and_type(impl_text: &str) -> Option<(String, String)> {
    let header = impl_text.split('{').next()?.trim();
    let after_impl = header.strip_prefix("impl")?.trim();
    let (trait_part, type_part) = after_impl.split_once(" for ")?;
    let trait_name = rust_type_leaf_name(trait_part);
    let type_name = rust_type_leaf_name(type_part.split_whitespace().next().unwrap_or(type_part));
    if trait_name.is_empty() || type_name.is_empty() {
        return None;
    }
    Some((trait_name, type_name))
}

fn rust_impl_self_type(impl_text: &str) -> Option<String> {
    let header = impl_text.split('{').next()?.trim();
    let after_impl = header.strip_prefix("impl")?.trim();
    let type_part = after_impl
        .split_once(" for ")
        .map(|(_, implemented_type)| implemented_type)
        .unwrap_or(after_impl);
    let type_name = rust_type_leaf_name(type_part.split_whitespace().next().unwrap_or(type_part));
    (!type_name.is_empty()).then_some(type_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_typescript() {
        let mut parser = TreeSitterParser::new();
        let source = r#"export function hello() {
    console.log("hello");
}

export class Foo {
    bar() {
        return 42;
    }
}

interface Baz {
    name: string;
}

type Qux = string | number;

enum Color {
    Red,
    Green,
}
"#;
        let result = parser.parse("test.ts", source);

        // Should have file + hello + Foo + bar + Baz + Qux + Color
        let node_names: Vec<&str> = result.nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(node_names.contains(&"hello"), "Should find function hello, got {:?}", node_names);
        assert!(node_names.contains(&"Foo"), "Should find class Foo, got {:?}", node_names);
        assert!(node_names.contains(&"Baz"), "Should find interface Baz, got {:?}", node_names);
        assert!(node_names.contains(&"Qux"), "Should find type Qux, got {:?}", node_names);
        assert!(node_names.contains(&"Color"), "Should find enum Color, got {:?}", node_names);
    }

    #[test]
    fn test_parse_python() {
        let mut parser = TreeSitterParser::new();
        let source = r#"
def hello():
    print("hello")

class Foo:
    def bar(self):
        return 42
"#;
        let result = parser.parse("test.py", source);
        let node_names: Vec<&str> = result.nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(node_names.contains(&"hello"), "Should find function hello, got {:?}", node_names);
        assert!(node_names.contains(&"Foo"), "Should find class Foo, got {:?}", node_names);
    }

    #[test]
    fn test_parse_rust() {
        let mut parser = TreeSitterParser::new();
        let source = r#"
pub fn hello() {
    println!("hello");
}

struct Foo {
    name: String,
}

impl Foo {
    fn bar(&self) -> i32 {
        42
    }
}
"#;
        let result = parser.parse("test.rs", source);
        let node_names: Vec<&str> = result.nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(node_names.contains(&"hello"), "Should find function hello, got {:?}", node_names);
        assert!(node_names.contains(&"Foo"), "Should find struct Foo, got {:?}", node_names);
    }
}
