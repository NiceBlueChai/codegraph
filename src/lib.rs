pub mod types;
pub mod context_formatter;
pub mod db;
pub mod core;
pub mod extraction;
pub mod installer;
pub mod sync;
pub mod mcp;
pub mod project;
pub mod query_service;
pub mod util;

// Re-export main types
pub use types::*;
pub use db::{DatabaseConnection, QueryBuilder};

/// CodeGraph library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
