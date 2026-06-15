use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::fs;
use tempfile::TempDir;

use codegraph::db::{DatabaseConnection, schema::initialize_schema, QueryBuilder};
use codegraph::core::indexer::Indexer;
use codegraph::extraction::parser::CodeParser;
use codegraph::extraction::tree_sitter_parser::TreeSitterParser;

/// Create a temporary project with sample files for benchmarking
fn create_sample_project(num_files: usize) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create src directory
    fs::create_dir_all(root.join("src")).unwrap();

    for i in 0..num_files {
        let ts_content = format!(
            r#"import {{ useState, useEffect }} from 'react';

interface UserProps {{
    id: number;
    name: string;
    email: string;
}}

export class UserService{0} {{
    private baseUrl: string;

    constructor(baseUrl: string) {{
        this.baseUrl = baseUrl;
    }}

    async getUser(id: number): Promise<UserProps> {{
        const response = await fetch(`${{this.baseUrl}}/users/${{id}}`);
        return response.json();
    }}

    async getUsers(): Promise<UserProps[]> {{
        const response = await fetch(`${{this.baseUrl}}/users`);
        return response.json();
    }}

    createUser(user: Omit<UserProps, 'id'>): Promise<UserProps> {{
        return fetch(`${{this.baseUrl}}/users`, {{
            method: 'POST',
            body: JSON.stringify(user),
        }}).then(r => r.json());
    }}
}}

export function processUsers(users: UserProps[]): UserProps[] {{
    return users.filter(u => u.email.includes('@'));
}}

export type UserRole = 'admin' | 'user' | 'guest';

export enum Status {{
    Active = 'active',
    Inactive = 'inactive',
    Pending = 'pending',
}}
"#,
            i
        );

        let py_content = format!(
            r#"from typing import List, Optional
from dataclasses import dataclass
import asyncio

@dataclass
class Item:
    id: int
    name: str
    value: float

class ItemService{0}:
    def __init__(self, db_path: str):
        self.db_path = db_path
        self._cache: dict = {{}}

    async def get_item(self, item_id: int) -> Optional[Item]:
        if item_id in self._cache:
            return self._cache[item_id]
        # Simulate async DB call
        await asyncio.sleep(0.01)
        return Item(id=item_id, name=f"item_{{item_id}}", value=0.0)

    async def get_all_items(self) -> List[Item]:
        items = []
        for i in range(10):
            item = await self.get_item(i)
            if item:
                items.append(item)
        return items

    def process_items(items: List[Item]) -> List[Item]:
        return [item for item in items if item.value > 0]

def calculate_total(items: List[Item]) -> float:
    return sum(item.value for item in items)
"#,
            i
        );

        fs::write(root.join(format!("src/service_{i}.ts")), ts_content).unwrap();
        fs::write(root.join(format!("src/item_{i}.py")), py_content).unwrap();
    }

    temp_dir
}

fn benchmark_text_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_parser");

    let ts_source = r#"export function hello() {
    console.log("hello");
}

export class Foo {
    bar() {
        return 42;
    }
}"#;

    let py_source = r#"def hello():
    print("hello")

class Foo:
    def bar(self):
        return 42
"#;

    group.bench_function("typescript", |b| {
        b.iter(|| {
            let mut parser = CodeParser::new();
            parser.parse("test.ts", ts_source)
        })
    });

    group.bench_function("python", |b| {
        b.iter(|| {
            let mut parser = CodeParser::new();
            parser.parse("test.py", py_source)
        })
    });

    group.finish();
}

fn benchmark_tree_sitter_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_sitter_parser");

    let ts_source = r#"export function hello() {
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

    let py_source = r#"def hello():
    print("hello")

class Foo:
    def bar(self):
        return 42
"#;

    let rs_source = r#"pub fn hello() {
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

    group.bench_function("typescript", |b| {
        b.iter(|| {
            let mut parser = TreeSitterParser::new();
            parser.parse("test.ts", ts_source)
        })
    });

    group.bench_function("python", |b| {
        b.iter(|| {
            let mut parser = TreeSitterParser::new();
            parser.parse("test.py", py_source)
        })
    });

    group.bench_function("rust", |b| {
        b.iter(|| {
            let mut parser = TreeSitterParser::new();
            parser.parse("test.rs", rs_source)
        })
    });

    group.finish();
}

fn benchmark_parser_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_comparison");

    let ts_source = r#"export function hello() {
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

    group.bench_function("text_parser", |b| {
        b.iter(|| {
            let mut parser = CodeParser::new();
            parser.parse("test.ts", ts_source)
        })
    });

    group.bench_function("tree_sitter_parser", |b| {
        b.iter(|| {
            let mut parser = TreeSitterParser::new();
            parser.parse("test.ts", ts_source)
        })
    });

    group.finish();
}

fn benchmark_indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexing");
    group.sample_size(10); // Reduce sample size for slower benchmarks

    for num_files in [5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel", num_files),
            num_files,
            |b, &num_files| {
                let temp_dir = create_sample_project(num_files);

                b.iter(|| {
                    // Use unique DB path for each iteration
                    let db_path = temp_dir.path().join(format!("bench_{}.db", std::process::id()));
                    let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();
                    initialize_schema(db.get_conn()).unwrap();

                    // Disable foreign keys for benchmark
                    db.get_conn().execute_batch("PRAGMA foreign_keys = OFF;").unwrap();

                    let queries = QueryBuilder::new(db.get_conn());
                    let indexer = Indexer::new(queries, temp_dir.path().to_str().unwrap());
                    let result = indexer.index_all().unwrap();
                    // Clean up
                    drop(db);
                    let _ = std::fs::remove_file(&db_path);
                    result
                });
            },
        );
    }

    group.finish();
}

fn benchmark_database_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("database");

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("bench.db");
    let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();
    initialize_schema(db.get_conn()).unwrap();
    let queries = QueryBuilder::new(db.get_conn());

    // Create sample nodes
    let nodes: Vec<_> = (0..100)
        .map(|i| {
            codegraph::types::Node::new(
                format!("node_{}", i),
                codegraph::types::NodeKind::Function,
                format!("func_{}", i),
                format!("file::func_{}", i),
                "file.ts".to_string(),
                codegraph::types::Language::TypeScript,
                i,
                i + 10,
                0,
                100,
            )
        })
        .collect();

    group.bench_function("insert_100_nodes", |b| {
        b.iter(|| {
            queries.insert_nodes_batch(&nodes).unwrap();
        })
    });

    group.bench_function("get_all_nodes", |b| {
        b.iter(|| {
            queries.get_all_nodes().unwrap();
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_text_parser,
    benchmark_tree_sitter_parser,
    benchmark_parser_comparison,
    benchmark_indexing,
    benchmark_database_operations
);
criterion_main!(benches);
