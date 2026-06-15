use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

// ============================================================================
// Node Kinds and Languages
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Module,
    Class,
    Struct,
    Interface,
    Trait,
    Protocol,
    Function,
    Method,
    Property,
    Field,
    Variable,
    Constant,
    Enum,
    EnumMember,
    TypeAlias,
    Namespace,
    Parameter,
    Import,
    Export,
    Route,
    Component,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::File => "file",
            NodeKind::Module => "module",
            NodeKind::Class => "class",
            NodeKind::Struct => "struct",
            NodeKind::Interface => "interface",
            NodeKind::Trait => "trait",
            NodeKind::Protocol => "protocol",
            NodeKind::Function => "function",
            NodeKind::Method => "method",
            NodeKind::Property => "property",
            NodeKind::Field => "field",
            NodeKind::Variable => "variable",
            NodeKind::Constant => "constant",
            NodeKind::Enum => "enum",
            NodeKind::EnumMember => "enum_member",
            NodeKind::TypeAlias => "type_alias",
            NodeKind::Namespace => "namespace",
            NodeKind::Parameter => "parameter",
            NodeKind::Import => "import",
            NodeKind::Export => "export",
            NodeKind::Route => "route",
            NodeKind::Component => "component",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "file" => Some(NodeKind::File),
            "module" => Some(NodeKind::Module),
            "class" => Some(NodeKind::Class),
            "struct" => Some(NodeKind::Struct),
            "interface" => Some(NodeKind::Interface),
            "trait" => Some(NodeKind::Trait),
            "protocol" => Some(NodeKind::Protocol),
            "function" => Some(NodeKind::Function),
            "method" => Some(NodeKind::Method),
            "property" => Some(NodeKind::Property),
            "field" => Some(NodeKind::Field),
            "variable" => Some(NodeKind::Variable),
            "constant" => Some(NodeKind::Constant),
            "enum" => Some(NodeKind::Enum),
            "enum_member" => Some(NodeKind::EnumMember),
            "type_alias" => Some(NodeKind::TypeAlias),
            "namespace" => Some(NodeKind::Namespace),
            "parameter" => Some(NodeKind::Parameter),
            "import" => Some(NodeKind::Import),
            "export" => Some(NodeKind::Export),
            "route" => Some(NodeKind::Route),
            "component" => Some(NodeKind::Component),
            _ => None,
        }
    }
}

impl FromStr for NodeKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file" => Ok(NodeKind::File),
            "module" => Ok(NodeKind::Module),
            "class" => Ok(NodeKind::Class),
            "struct" => Ok(NodeKind::Struct),
            "interface" => Ok(NodeKind::Interface),
            "trait" => Ok(NodeKind::Trait),
            "protocol" => Ok(NodeKind::Protocol),
            "function" => Ok(NodeKind::Function),
            "method" => Ok(NodeKind::Method),
            "property" => Ok(NodeKind::Property),
            "field" => Ok(NodeKind::Field),
            "variable" => Ok(NodeKind::Variable),
            "constant" => Ok(NodeKind::Constant),
            "enum" => Ok(NodeKind::Enum),
            "enum_member" => Ok(NodeKind::EnumMember),
            "type_alias" => Ok(NodeKind::TypeAlias),
            "namespace" => Ok(NodeKind::Namespace),
            "parameter" => Ok(NodeKind::Parameter),
            "import" => Ok(NodeKind::Import),
            "export" => Ok(NodeKind::Export),
            "route" => Ok(NodeKind::Route),
            "component" => Ok(NodeKind::Component),
            _ => Err(format!("Unknown node kind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    TypeScript,
    JavaScript,
    TSX,
    JSX,
    Python,
    Go,
    Rust,
    Java,
    C,
    Cpp,
    CSharp,
    PHP,
    Ruby,
    Swift,
    Kotlin,
    Dart,
    Svelte,
    Vue,
    Astro,
    Liquid,
    Pascal,
    Scala,
    Lua,
    Luau,
    ObjC,
    Unknown,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::TSX => "tsx",
            Language::JSX => "jsx",
            Language::Python => "python",
            Language::Go => "go",
            Language::Rust => "rust",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::CSharp => "csharp",
            Language::PHP => "php",
            Language::Ruby => "ruby",
            Language::Swift => "swift",
            Language::Kotlin => "kotlin",
            Language::Dart => "dart",
            Language::Svelte => "svelte",
            Language::Vue => "vue",
            Language::Astro => "astro",
            Language::Liquid => "liquid",
            Language::Pascal => "pascal",
            Language::Scala => "scala",
            Language::Lua => "lua",
            Language::Luau => "luau",
            Language::ObjC => "objc",
            Language::Unknown => "unknown",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "ts" => Some(Language::TypeScript),
            "tsx" => Some(Language::TSX),
            "js" => Some(Language::JavaScript),
            "jsx" => Some(Language::JSX),
            "mjs" => Some(Language::JavaScript),
            "mts" => Some(Language::TypeScript),
            "cts" => Some(Language::TypeScript),
            "py" => Some(Language::Python),
            "go" => Some(Language::Go),
            "rs" => Some(Language::Rust),
            "java" => Some(Language::Java),
            "c" => Some(Language::C),
            "h" => Some(Language::C),
            "cpp" => Some(Language::Cpp),
            "hpp" => Some(Language::Cpp),
            "cc" => Some(Language::Cpp),
            "cs" => Some(Language::CSharp),
            "php" => Some(Language::PHP),
            "rb" => Some(Language::Ruby),
            "swift" => Some(Language::Swift),
            "kt" => Some(Language::Kotlin),
            "kts" => Some(Language::Kotlin),
            "dart" => Some(Language::Dart),
            "svelte" => Some(Language::Svelte),
            "vue" => Some(Language::Vue),
            "astro" => Some(Language::Astro),
            "liquid" => Some(Language::Liquid),
            "pas" | "dpr" | "dpk" | "lpr" => Some(Language::Pascal),
            "scala" | "sc" => Some(Language::Scala),
            "lua" => Some(Language::Lua),
            "luau" => Some(Language::Luau),
            "m" | "mm" => Some(Language::ObjC),
            _ => None,
        }
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "typescript" => Ok(Language::TypeScript),
            "javascript" | "js" => Ok(Language::JavaScript),
            "tsx" => Ok(Language::TSX),
            "jsx" => Ok(Language::JSX),
            "python" | "py" => Ok(Language::Python),
            "go" => Ok(Language::Go),
            "rust" | "rs" => Ok(Language::Rust),
            "java" => Ok(Language::Java),
            "c" => Ok(Language::C),
            "cpp" | "hpp" | "cc" => Ok(Language::Cpp),
            "csharp" | "cs" => Ok(Language::CSharp),
            "php" => Ok(Language::PHP),
            "ruby" | "rb" => Ok(Language::Ruby),
            "swift" => Ok(Language::Swift),
            "kotlin" | "kt" | "kts" => Ok(Language::Kotlin),
            "dart" => Ok(Language::Dart),
            "svelte" => Ok(Language::Svelte),
            "vue" => Ok(Language::Vue),
            "astro" => Ok(Language::Astro),
            "liquid" => Ok(Language::Liquid),
            "pascal" | "pas" | "dpr" | "dpk" | "lpr" => Ok(Language::Pascal),
            "scala" | "sc" => Ok(Language::Scala),
            "lua" => Ok(Language::Lua),
            "luau" => Ok(Language::Luau),
            "objc" | "m" | "mm" => Ok(Language::ObjC),
            _ => Err(format!("Unknown language: {}", s)),
        }
    }
}

// ============================================================================
// Edge Kinds
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    Contains,
    Calls,
    Imports,
    Exports,
    Extends,
    Implements,
    References,
    TypeOf,
    Returns,
    Instantiates,
    Overrides,
    Decorates,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::Contains => "contains",
            EdgeKind::Calls => "calls",
            EdgeKind::Imports => "imports",
            EdgeKind::Exports => "exports",
            EdgeKind::Extends => "extends",
            EdgeKind::Implements => "implements",
            EdgeKind::References => "references",
            EdgeKind::TypeOf => "type_of",
            EdgeKind::Returns => "returns",
            EdgeKind::Instantiates => "instantiates",
            EdgeKind::Overrides => "overrides",
            EdgeKind::Decorates => "decorates",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "contains" => Some(EdgeKind::Contains),
            "calls" => Some(EdgeKind::Calls),
            "imports" => Some(EdgeKind::Imports),
            "exports" => Some(EdgeKind::Exports),
            "extends" => Some(EdgeKind::Extends),
            "implements" => Some(EdgeKind::Implements),
            "references" => Some(EdgeKind::References),
            "type_of" => Some(EdgeKind::TypeOf),
            "returns" => Some(EdgeKind::Returns),
            "instantiates" => Some(EdgeKind::Instantiates),
            "overrides" => Some(EdgeKind::Overrides),
            "decorates" => Some(EdgeKind::Decorates),
            _ => None,
        }
    }
}

impl FromStr for EdgeKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "contains" => Ok(EdgeKind::Contains),
            "calls" => Ok(EdgeKind::Calls),
            "imports" => Ok(EdgeKind::Imports),
            "exports" => Ok(EdgeKind::Exports),
            "extends" => Ok(EdgeKind::Extends),
            "implements" => Ok(EdgeKind::Implements),
            "references" => Ok(EdgeKind::References),
            "type_of" => Ok(EdgeKind::TypeOf),
            "returns" => Ok(EdgeKind::Returns),
            "instantiates" => Ok(EdgeKind::Instantiates),
            "overrides" => Ok(EdgeKind::Overrides),
            "decorates" => Ok(EdgeKind::Decorates),
            _ => Err(format!("Unknown edge kind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provenance {
    TreeSitter,
    Scip,
    Heuristic,
}

impl Provenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provenance::TreeSitter => "tree-sitter",
            Provenance::Scip => "scip",
            Provenance::Heuristic => "heuristic",
        }
    }
}

// ============================================================================
// Core Graph Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub language: Language,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub docstring: Option<String>,
    pub signature: Option<String>,
    pub visibility: Option<String>,
    pub is_exported: bool,
    pub is_async: bool,
    pub is_static: bool,
    pub is_abstract: bool,
    pub decorators: Vec<String>,
    pub type_parameters: Vec<String>,
    pub return_type: Option<String>,
    pub updated_at: i64,
}

impl Node {
    pub fn new(
        id: String,
        kind: NodeKind,
        name: String,
        qualified_name: String,
        file_path: String,
        language: Language,
        start_line: u32,
        end_line: u32,
        start_column: u32,
        end_column: u32,
    ) -> Self {
        Self {
            id,
            kind,
            name,
            qualified_name,
            file_path,
            language,
            start_line,
            end_line,
            start_column,
            end_column,
            docstring: None,
            signature: None,
            visibility: None,
            is_exported: false,
            is_async: false,
            is_static: false,
            is_abstract: false,
            decorators: Vec::new(),
            type_parameters: Vec::new(),
            return_type: None,
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub provenance: Option<Provenance>,
}

impl Edge {
    pub fn new(source: String, target: String, kind: EdgeKind) -> Self {
        Self {
            source,
            target,
            kind,
            metadata: None,
            line: None,
            column: None,
            provenance: Some(Provenance::TreeSitter),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub content_hash: String,
    pub language: Language,
    pub size: u64,
    pub modified_at: i64,
    pub indexed_at: i64,
    pub node_count: u32,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedReference {
    pub id: Option<i64>,
    pub from_node_id: String,
    pub reference_name: String,
    pub reference_kind: String,
    pub line: u32,
    pub col: u32,
    pub candidates: Option<Vec<String>>,
    pub file_path: String,
    pub language: String,
}

// ============================================================================
// Reference Resolution Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    FilePath,
    QualifiedName,
    MethodCall,
    ExactName,
    Fuzzy,
    ImportPath,
}

impl ResolutionStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionStrategy::FilePath => "file_path",
            ResolutionStrategy::QualifiedName => "qualified_name",
            ResolutionStrategy::MethodCall => "method_call",
            ResolutionStrategy::ExactName => "exact_name",
            ResolutionStrategy::Fuzzy => "fuzzy",
            ResolutionStrategy::ImportPath => "import_path",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedReference {
    pub from_node_id: String,
    pub target_node_id: String,
    pub edge_kind: EdgeKind,
    pub confidence: f64,
    pub strategy: ResolutionStrategy,
    pub line: u32,
    pub col: u32,
}

// Language families for cross-language isolation
pub fn get_language_family(lang: &Language) -> Option<&'static str> {
    match lang {
        Language::Java | Language::Kotlin | Language::Scala => Some("jvm"),
        Language::Swift | Language::ObjC => Some("apple"),
        Language::TypeScript | Language::JavaScript | Language::TSX | Language::JSX => Some("web"),
        Language::C | Language::Cpp => Some("c"),
        Language::CSharp => Some("dotnet"),
        _ => None,
    }
}

// ============================================================================
// Extraction Results
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub unresolved_refs: Vec<UnresolvedReference>,
    pub errors: Vec<String>,
}

impl ExtractionResult {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            unresolved_refs: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty() && self.errors.is_empty()
    }
}

// ============================================================================
// Graph Query Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subgraph {
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

impl Subgraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraversalOptions {
    pub max_depth: usize,
    pub edge_kinds: Option<Vec<EdgeKind>>,
    pub node_kinds: Option<Vec<NodeKind>>,
    pub direction: TraversalDirection,
    pub limit: Option<usize>,
}

impl Default for TraversalOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            edge_kinds: None,
            node_kinds: None,
            direction: TraversalDirection::Outgoing,
            limit: Some(100),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraversalDirection {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub node: Node,
    pub score: f64,
    pub matched_fields: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: usize,
    pub kinds: Option<Vec<NodeKind>>,
    pub file_pattern: Option<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 20,
            kinds: None,
            file_pattern: None,
        }
    }
}

// ============================================================================
// Context Building
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub node: Node,
    pub ancestors: Vec<Node>,
    pub children: Vec<Node>,
    pub incoming_references: Vec<(Node, Edge)>,
    pub outgoing_references: Vec<(Node, Edge)>,
    pub callers: Vec<(Node, Edge)>,
    pub callees: Vec<(Node, Edge)>,
    pub types: Vec<Node>,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BuildContextOptions {
    pub max_nodes: usize,
    pub include_code: bool,
    pub format: OutputFormat,
    pub depth: usize,
}

impl Default for BuildContextOptions {
    fn default() -> Self {
        Self {
            max_nodes: 50,
            include_code: true,
            format: OutputFormat::Markdown,
            depth: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Markdown,
    Json,
    Plain,
}

pub type TaskInput = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub query: String,
    pub entry_points: Vec<Node>,
    pub related_nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub code_snippets: HashMap<String, String>,
    pub stats: ContextStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub files_involved: usize,
    pub code_lines: usize,
}

// ============================================================================
// Statistics
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: u64,
    pub edge_count: u64,
    pub file_count: u64,
    pub unresolved_ref_count: u64,
    pub db_size_bytes: u64,
    pub last_indexed_at: Option<i64>,
}

// ============================================================================
// Progress Reporting
// ============================================================================

#[derive(Debug, Clone)]
pub struct IndexProgress {
    pub phase: String,
    pub current: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResult {
    pub success: bool,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_errored: usize,
    pub nodes_created: usize,
    pub edges_created: usize,
    pub errors: Vec<IndexError>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub files_checked: usize,
    pub files_added: usize,
    pub files_modified: usize,
    pub files_removed: usize,
    pub nodes_updated: usize,
    pub duration_ms: u64,
    pub changed_file_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexError {
    pub message: String,
    pub severity: String,
    pub file_path: Option<String>,
}
