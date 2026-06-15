//! Project discovery and database-opening helpers for CodeGraph commands.
//!
//! CLI and MCP entry points use this module so project-root behavior stays
//! consistent across subdirectories and Windows path spellings.

use std::path::{Path, PathBuf};

use crate::db::{get_database_path, is_initialized, DatabaseConnection};

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub db_path: PathBuf,
}

impl ProjectContext {
    pub fn open_database(&self) -> anyhow::Result<DatabaseConnection> {
        DatabaseConnection::open(self.db_path.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Database path is not valid UTF-8: {}",
                self.db_path.display()
            )
        })?)
        .map_err(|e| anyhow::anyhow!("Failed to open database {}: {}", self.db_path.display(), e))
    }

    pub fn root_str(&self) -> anyhow::Result<&str> {
        self.root.to_str().ok_or_else(|| {
            anyhow::anyhow!("Project path is not valid UTF-8: {}", self.root.display())
        })
    }
}

pub fn resolve_project(path_arg: Option<&str>) -> anyhow::Result<ProjectContext> {
    let start = path_arg.unwrap_or(".");
    let start_path = Path::new(start);
    let absolute = if start_path.is_absolute() {
        start_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(start_path)
    };
    let absolute = absolute
        .canonicalize()
        .map(normalize_windows_verbatim)
        .unwrap_or_else(|_| normalize_without_existing(&absolute));

    let mut current = if absolute.is_file() {
        absolute.parent().unwrap_or(&absolute).to_path_buf()
    } else {
        absolute
    };

    loop {
        if is_initialized(path_to_str(&current)?) {
            let db_path = PathBuf::from(get_database_path(path_to_str(&current)?));
            return Ok(ProjectContext {
                root: current,
                db_path,
            });
        }
        if !current.pop() {
            break;
        }
    }

    Err(anyhow::anyhow!(
        "CodeGraph not initialized for {}. Run 'codegraph init' first.",
        start
    ))
}

pub fn is_project_initialized(path_arg: Option<&str>) -> bool {
    resolve_project(path_arg).is_ok()
}

fn path_to_str(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Path is not valid UTF-8: {}", path.display()))
}

fn normalize_without_existing(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        normalized.push(component.as_os_str());
    }
    normalize_windows_verbatim(normalized)
}

#[cfg(windows)]
fn normalize_windows_verbatim(path: PathBuf) -> PathBuf {
    let raw = path.to_string_lossy().into_owned();
    if let Some(stripped) = raw.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{}", stripped));
    }
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        return PathBuf::from(stripped);
    }
    path
}

#[cfg(not(windows))]
fn normalize_windows_verbatim(path: PathBuf) -> PathBuf {
    path
}
