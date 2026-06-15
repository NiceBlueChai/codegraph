//! Non-interactive agent installer for CodeGraph MCP configuration.
//!
//! The TypeScript installer supports rich prompts; this Rust migration starts
//! with deterministic config printing, install, and uninstall paths.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

const TARGETS: &[&str] = &[
    "claude",
    "cursor",
    "codex",
    "opencode",
    "hermes",
    "gemini",
    "antigravity",
    "kiro",
];

const INSTRUCTIONS_BLOCK: &str = concat!(
    "<!-- CODEGRAPH_START -->\n",
    "## CodeGraph\n\n",
    "In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), ",
    "reach for it BEFORE grep/find or reading files when you need to understand or locate code:\n\n",
    "- **MCP tools** (when available): `codegraph_explore` answers most code questions in one call. ",
    "`codegraph_node` returns one symbol's source + callers, or reads a whole file with line numbers.\n",
    "- **Shell** (always works): `codegraph explore \"<symbol names or question>\"` and ",
    "`codegraph node <symbol-or-file>` print the same output.\n\n",
    "If there is no `.codegraph/` directory, skip CodeGraph entirely.\n",
    "<!-- CODEGRAPH_END -->\n",
);

/// Location for agent configuration writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// User-level configuration.
    Global,
    /// Project-level configuration.
    Local,
}

impl Location {
    /// Parses a CLI location value.
    pub fn parse(value: Option<&str>) -> anyhow::Result<Self> {
        match value.unwrap_or("global") {
            "global" => Ok(Self::Global),
            "local" => Ok(Self::Local),
            other => anyhow::bail!("--location must be \"global\" or \"local\" (got \"{}\")", other),
        }
    }
}

/// Prints a target-specific MCP config snippet without writing files.
pub fn print_config(target: &str, location: Location) -> anyhow::Result<String> {
    ensure_known_target(target)?;
    let path = config_path(target, location)?;
    if target == "codex" && location == Location::Local {
        return Ok("# Codex CLI has no project-local config - use --location=global.\n".to_string());
    }
    Ok(format!("# Add to {}\n\n{}\n", path.display(), snippet(target, location)?))
}

/// Installs CodeGraph into selected agent targets.
pub fn install(
    target_flag: Option<&str>,
    location: Location,
    auto_allow: bool,
) -> anyhow::Result<Vec<String>> {
    let mut reports = Vec::new();
    for target in resolve_targets(target_flag)? {
        if target == "codex" && location == Location::Local {
            reports.push("Skipped codex: Codex CLI has no project-local config".to_string());
            continue;
        }
        install_target(target, location, auto_allow)?;
        reports.push(format!("Installed {}", target));
    }
    Ok(reports)
}

/// Removes CodeGraph entries from selected agent targets.
pub fn uninstall(target_flag: Option<&str>, location: Location) -> anyhow::Result<Vec<String>> {
    let mut reports = Vec::new();
    for target in resolve_targets(target_flag.or(Some("all")))? {
        if target == "codex" && location == Location::Local {
            continue;
        }
        uninstall_target(target, location)?;
        reports.push(format!("Removed {}", target));
    }
    Ok(reports)
}

fn resolve_targets(target_flag: Option<&str>) -> anyhow::Result<Vec<&'static str>> {
    let raw = target_flag.unwrap_or("auto");
    if raw == "none" {
        return Ok(Vec::new());
    }
    if raw == "all" {
        return Ok(TARGETS.to_vec());
    }
    if raw == "auto" {
        return Ok(vec!["claude"]);
    }

    let mut targets = Vec::new();
    for id in raw.split(',').map(str::trim).filter(|id| !id.is_empty()) {
        ensure_known_target(id)?;
        targets.push(TARGETS.iter().copied().find(|known| *known == id).unwrap());
    }
    Ok(targets)
}

fn ensure_known_target(target: &str) -> anyhow::Result<()> {
    if TARGETS.contains(&target) {
        Ok(())
    } else {
        anyhow::bail!(
            "Unknown target \"{}\". Known: {}, plus 'auto' / 'all' / 'none'.",
            target,
            TARGETS.join(", ")
        )
    }
}

fn install_target(target: &str, location: Location, auto_allow: bool) -> anyhow::Result<()> {
    match target {
        "codex" => {
            upsert_codex_config(&config_path(target, location)?)?;
            upsert_marked_section(&instructions_path(target, location)?)?;
        }
        "opencode" => {
            upsert_opencode_config(&config_path(target, location)?)?;
            upsert_marked_section(&instructions_path(target, location)?)?;
        }
        _ => {
            upsert_json_mcp(&config_path(target, location)?, target, location)?;
            if target == "claude" && auto_allow {
                upsert_claude_permissions(&settings_path(location)?)?;
            }
            if let Some(path) = optional_instructions_path(target, location)? {
                upsert_marked_section(&path)?;
            }
        }
    }
    Ok(())
}

fn uninstall_target(target: &str, location: Location) -> anyhow::Result<()> {
    match target {
        "codex" => {
            remove_codex_config(&config_path(target, location)?)?;
            remove_marked_section(&instructions_path(target, location)?)?;
        }
        "opencode" => {
            remove_opencode_config(&config_path(target, location)?)?;
            remove_marked_section(&instructions_path(target, location)?)?;
        }
        _ => {
            remove_json_mcp(&config_path(target, location)?)?;
            if target == "claude" {
                remove_claude_permissions(&settings_path(location)?)?;
            }
            if let Some(path) = optional_instructions_path(target, location)? {
                remove_marked_section(&path)?;
            }
        }
    }
    Ok(())
}

fn snippet(target: &str, location: Location) -> anyhow::Result<String> {
    match target {
        "codex" => Ok(codex_block()),
        "opencode" => Ok(serde_json::to_string_pretty(&json!({
            "$schema": "https://opencode.ai/config.json",
            "mcp": {"codegraph": opencode_server()}
        }))?),
        _ => Ok(serde_json::to_string_pretty(&json!({
            "mcpServers": {"codegraph": json_server(target, location)?}
        }))?),
    }
}

fn json_server(target: &str, location: Location) -> anyhow::Result<Value> {
    let mut args = vec!["serve".to_string(), "--mcp".to_string()];
    if target == "cursor" {
        args.push("--path".to_string());
        args.push(match location {
            Location::Global => "${workspaceFolder}".to_string(),
            Location::Local => std::env::current_dir()?.display().to_string(),
        });
    }
    Ok(json!({
        "type": "stdio",
        "command": "codegraph",
        "args": args,
    }))
}

fn opencode_server() -> Value {
    json!({
        "type": "local",
        "command": ["codegraph", "serve", "--mcp"],
        "enabled": true,
    })
}

fn codex_block() -> String {
    "[mcp_servers.codegraph]\ncommand = \"codegraph\"\nargs = [\"serve\", \"--mcp\"]\n".to_string()
}

fn upsert_json_mcp(path: &Path, target: &str, location: Location) -> anyhow::Result<()> {
    let mut value = read_json_object(path)?;
    if !value.get("mcpServers").is_some_and(Value::is_object) {
        value["mcpServers"] = json!({});
    }
    value["mcpServers"]["codegraph"] = json_server(target, location)?;
    write_json(path, &value)
}

fn remove_json_mcp(path: &Path) -> anyhow::Result<()> {
    let mut value = read_json_object(path)?;
    if let Some(servers) = value.get_mut("mcpServers").and_then(Value::as_object_mut) {
        servers.remove("codegraph");
        if servers.is_empty() {
            value.as_object_mut().unwrap().remove("mcpServers");
        }
    }
    write_json(path, &value)
}

fn upsert_opencode_config(path: &Path) -> anyhow::Result<()> {
    let mut value = read_json_object(path)?;
    value["$schema"] = json!("https://opencode.ai/config.json");
    if !value.get("mcp").is_some_and(Value::is_object) {
        value["mcp"] = json!({});
    }
    value["mcp"]["codegraph"] = opencode_server();
    write_json(path, &value)
}

fn remove_opencode_config(path: &Path) -> anyhow::Result<()> {
    let mut value = read_json_object(path)?;
    if let Some(mcp) = value.get_mut("mcp").and_then(Value::as_object_mut) {
        mcp.remove("codegraph");
        if mcp.is_empty() {
            value.as_object_mut().unwrap().remove("mcp");
        }
    }
    write_json(path, &value)
}

fn upsert_codex_config(path: &Path) -> anyhow::Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let stripped = remove_toml_table(&existing, "mcp_servers.codegraph");
    let next = format!("{}{}\n", stripped.trim_end(), if stripped.trim().is_empty() { "" } else { "\n\n" });
    write_text(path, &(next + &codex_block()))
}

fn remove_codex_config(path: &Path) -> anyhow::Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    write_text(path, &(remove_toml_table(&existing, "mcp_servers.codegraph").trim_end().to_string() + "\n"))
}

fn remove_toml_table(content: &str, table: &str) -> String {
    let header = format!("[{}]", table);
    let mut out = Vec::new();
    let mut skipping = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == header {
            skipping = true;
            continue;
        }
        if skipping && trimmed.starts_with('[') && trimmed.ends_with(']') {
            skipping = false;
        }
        if !skipping {
            out.push(line);
        }
    }
    out.join("\n")
}

fn upsert_claude_permissions(path: &Path) -> anyhow::Result<()> {
    let mut value = read_json_object(path)?;
    if !value.get("permissions").is_some_and(Value::is_object) {
        value["permissions"] = json!({});
    }
    if !value["permissions"].get("allow").is_some_and(Value::is_array) {
        value["permissions"]["allow"] = json!([]);
    }
    let allow = value["permissions"]["allow"].as_array_mut().unwrap();
    for permission in [
        "mcp__codegraph__codegraph_explore",
        "mcp__codegraph__codegraph_search",
        "mcp__codegraph__codegraph_node",
        "mcp__codegraph__codegraph_callers",
        "mcp__codegraph__codegraph_callees",
        "mcp__codegraph__codegraph_impact",
        "mcp__codegraph__codegraph_files",
        "mcp__codegraph__codegraph_status",
    ] {
        if !allow.iter().any(|item| item == permission) {
            allow.push(json!(permission));
        }
    }
    write_json(path, &value)
}

fn remove_claude_permissions(path: &Path) -> anyhow::Result<()> {
    let mut value = read_json_object(path)?;
    if let Some(allow) = value
        .get_mut("permissions")
        .and_then(|permissions| permissions.get_mut("allow"))
        .and_then(Value::as_array_mut)
    {
        allow.retain(|item| {
            !item
                .as_str()
                .is_some_and(|permission| permission.starts_with("mcp__codegraph__"))
        });
    }
    write_json(path, &value)
}

fn upsert_marked_section(path: &Path) -> anyhow::Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let cleaned = remove_marked_block(&existing);
    let separator = if cleaned.trim().is_empty() { "" } else { "\n\n" };
    write_text(path, &format!("{}{}{}", cleaned.trim_end(), separator, INSTRUCTIONS_BLOCK))
}

fn remove_marked_section(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    let cleaned = remove_marked_block(&existing);
    if cleaned.trim().is_empty() {
        let _ = fs::remove_file(path);
        Ok(())
    } else {
        write_text(path, &(cleaned.trim_end().to_string() + "\n"))
    }
}

fn remove_marked_block(content: &str) -> String {
    let Some(start) = content.find("<!-- CODEGRAPH_START -->") else {
        return content.to_string();
    };
    let Some(relative_end) = content[start..].find("<!-- CODEGRAPH_END -->") else {
        return content.to_string();
    };
    let end = start + relative_end + "<!-- CODEGRAPH_END -->".len();
    format!("{}{}", content[..start].trim_end(), content[end..].trim_start())
}

fn read_json_object(path: &Path) -> anyhow::Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(json!({}));
    }
    Ok(serde_json::from_str(&content).unwrap_or_else(|_| json!({})))
}

fn write_json(path: &Path, value: &Value) -> anyhow::Result<()> {
    write_text(path, &(serde_json::to_string_pretty(value)? + "\n"))
}

fn write_text(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn config_path(target: &str, location: Location) -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(match (target, location) {
        ("claude", Location::Global) => home_dir()?.join(".claude.json"),
        ("claude", Location::Local) => cwd.join(".mcp.json"),
        ("cursor", Location::Global) => home_dir()?.join(".cursor").join("mcp.json"),
        ("cursor", Location::Local) => cwd.join(".cursor").join("mcp.json"),
        ("codex", Location::Global) => home_dir()?.join(".codex").join("config.toml"),
        ("opencode", Location::Global) => config_home()?.join("opencode").join("opencode.jsonc"),
        ("opencode", Location::Local) => cwd.join("opencode.jsonc"),
        ("hermes", Location::Global) => home_dir()?.join(".hermes").join("mcp.json"),
        ("hermes", Location::Local) => cwd.join(".hermes").join("mcp.json"),
        ("gemini", Location::Global) => home_dir()?.join(".gemini").join("settings.json"),
        ("gemini", Location::Local) => cwd.join(".gemini").join("settings.json"),
        ("antigravity", Location::Global) => home_dir()?.join(".gemini").join("antigravity").join("mcp_config.json"),
        ("antigravity", Location::Local) => cwd.join(".gemini").join("antigravity").join("mcp_config.json"),
        ("kiro", Location::Global) => home_dir()?.join(".kiro").join("mcp.json"),
        ("kiro", Location::Local) => cwd.join(".kiro").join("mcp.json"),
        _ => anyhow::bail!("Unsupported target/location combination: {} {:?}", target, location),
    })
}

fn optional_instructions_path(target: &str, location: Location) -> anyhow::Result<Option<PathBuf>> {
    Ok(match (target, location) {
        ("claude", Location::Global) => Some(home_dir()?.join(".claude").join("CLAUDE.md")),
        ("claude", Location::Local) => Some(std::env::current_dir()?.join(".claude").join("CLAUDE.md")),
        ("codex", Location::Global) => Some(home_dir()?.join(".codex").join("AGENTS.md")),
        ("opencode", Location::Global) => Some(config_home()?.join("opencode").join("AGENTS.md")),
        ("opencode", Location::Local) => Some(std::env::current_dir()?.join("AGENTS.md")),
        ("gemini", Location::Global) => Some(home_dir()?.join(".gemini").join("GEMINI.md")),
        ("gemini", Location::Local) => Some(std::env::current_dir()?.join("GEMINI.md")),
        _ => None,
    })
}

fn instructions_path(target: &str, location: Location) -> anyhow::Result<PathBuf> {
    optional_instructions_path(target, location)?
        .ok_or_else(|| anyhow::anyhow!("Target {} does not have an instructions file", target))
}

fn settings_path(location: Location) -> anyhow::Result<PathBuf> {
    Ok(match location {
        Location::Global => home_dir()?.join(".claude").join("settings.json"),
        Location::Local => std::env::current_dir()?.join(".claude").join("settings.json"),
    })
}

fn home_dir() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
}

fn config_home() -> anyhow::Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata));
    }
    Ok(home_dir()?.join(".config"))
}
