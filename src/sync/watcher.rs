use std::path::Path;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use log::{info, debug, warn};
use notify::{RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher, Event, EventKind};

/// Information about a pending file change
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PendingFileChange {
    first_seen_ms: u128,
    last_seen_ms: u128,
}

/// Message type for the sync thread
enum SyncMessage {
    Stop,
}

/// File watcher for monitoring code changes
#[allow(dead_code)]
pub struct FileWatcher {
    project_root: String,
    recursive_watcher: Option<RecommendedWatcher>,
    dir_watchers: HashMap<String, RecommendedWatcher>,
    pending_files: Arc<Mutex<HashMap<String, PendingFileChange>>>,
    sync_started_ms: u128,
    debounce_ms: u64,
    is_running: bool,
    sync_tx: Option<mpsc::Sender<SyncMessage>>,
    sync_handle: Option<std::thread::JoinHandle<()>>,
}

impl FileWatcher {
    pub fn new(project_root: &str) -> Self {
        let debounce_ms = std::env::var("CODEGRAPH_WATCH_DEBOUNCE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2000);

        Self {
            project_root: project_root.to_string(),
            recursive_watcher: None,
            dir_watchers: HashMap::new(),
            pending_files: Arc::new(Mutex::new(HashMap::new())),
            sync_started_ms: 0,
            debounce_ms,
            is_running: false,
            sync_tx: None,
            sync_handle: None,
        }
    }

    /// Start watching for file changes with a sync callback
    pub fn start<F>(&mut self, sync_callback: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(&[String]) + Send + 'static,
    {
        if let Some(reason) = watch_disabled_reason(&self.project_root) {
            info!("File watcher disabled: {}", reason);
            self.is_running = false;
            return Ok(());
        }

        info!("Starting file watcher for {}", self.project_root);

        let pending_files = Arc::clone(&self.pending_files);
        let project_root = self.project_root.clone();

        // Create channel for sync messages
        let (tx, rx) = mpsc::channel::<SyncMessage>();
        self.sync_tx = Some(tx);

        // Spawn dedicated sync thread
        let debounce_ms = self.debounce_ms;
        let pending_for_sync = Arc::clone(&self.pending_files);

        let handle = std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(debounce_ms));

                // Check for stop message
                match rx.try_recv() {
                    Ok(SyncMessage::Stop) => {
                        info!("Sync thread stopping");
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        info!("Sync channel disconnected");
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }

                // Check pending files
                let files_to_sync: Vec<String>;
                {
                    let mut pending = pending_for_sync.lock().unwrap();
                    if pending.is_empty() {
                        continue;
                    }
                    files_to_sync = pending.keys().cloned().collect();
                    pending.clear();
                }

                if !files_to_sync.is_empty() {
                    info!("Syncing {} files", files_to_sync.len());
                    sync_callback(&files_to_sync);
                }
            }
        });
        self.sync_handle = Some(handle);

        // Create watcher with event handler
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        handle_file_event(&event, &project_root, &pending_files);
                    }
                    Err(e) => {
                        warn!("Watch error: {:?}", e);
                    }
                }
            },
            notify::Config::default()
        )?;

        // Watch recursively if supported
        let path = Path::new(&self.project_root);
        if path.exists() {
            watcher.watch(path, RecursiveMode::Recursive)?;
            info!("Watching {} recursively", self.project_root);
        }

        self.recursive_watcher = Some(watcher);
        self.is_running = true;

        Ok(())
    }

    /// Stop watching
    pub fn stop(&mut self) {
        info!("Stopping file watcher");
        self.recursive_watcher = None;
        self.dir_watchers.clear();
        self.is_running = false;

        // Signal sync thread to stop
        if let Some(tx) = self.sync_tx.take() {
            let _ = tx.send(SyncMessage::Stop);
        }

        // Wait for sync thread to finish
        if let Some(handle) = self.sync_handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if watcher is running
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Get pending files that haven't been synced yet
    pub fn get_pending_files(&self) -> Vec<String> {
        let pending = self.pending_files.lock().unwrap();
        pending.keys().cloned().collect()
    }
}

/// Handle a file system event
fn handle_file_event(
    event: &Event,
    project_root: &str,
    pending_files: &Arc<Mutex<HashMap<String, PendingFileChange>>>,
) {
    // Only care about modify/create/remove events
    match event.kind {
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {}
        _ => return,
    }

    for path in &event.paths {
        if let Some(rel_path) = get_relative_path(path, project_root) {
            // Filter ignored paths
            if is_ignored_path(&rel_path) {
                continue;
            }

            // Only track source files
            if !is_source_file(&rel_path) {
                continue;
            }

            let now = current_time_ms();
            let mut pending = pending_files.lock().unwrap();

            let rel_path_for_debug = rel_path.clone();
            pending.entry(rel_path).and_modify(|info| {
                info.last_seen_ms = now;
            }).or_insert(PendingFileChange {
                first_seen_ms: now,
                last_seen_ms: now,
            });

            debug!("File changed: {}", rel_path_for_debug);
        }
    }
}

/// Get relative path from project root
fn get_relative_path(path: &Path, project_root: &str) -> Option<String> {
    let project_path = Path::new(project_root);
    path.strip_prefix(project_path).ok().map(|p| {
        p.to_string_lossy().to_string()
    })
}

/// Check if path should be ignored
fn is_ignored_path(rel_path: &str) -> bool {
    // Always ignore these patterns
    let ignore_patterns = [
        ".git/",
        "node_modules/",
        "target/",
        ".codegraph/",
        "dist/",
        "build/",
        ".next/",
        "out/",
    ];

    for pattern in &ignore_patterns {
        if rel_path.contains(pattern) {
            return true;
        }
    }

    // Ignore binary/non-source extensions
    let binary_extensions = [
        ".exe", ".dll", ".so", ".dylib", ".a", ".lib",
        ".png", ".jpg", ".jpeg", ".gif", ".ico", ".svg",
        ".pdf", ".doc", ".docx", ".xls", ".xlsx",
        ".zip", ".tar", ".gz", ".rar", ".7z",
        ".mp3", ".mp4", ".avi", ".mov",
        ".pyc", ".pyo", ".pyd", "__pycache__",
    ];

    for ext in &binary_extensions {
        if rel_path.ends_with(ext) {
            return true;
        }
    }

    false
}

/// Check if file is a source file
fn is_source_file(rel_path: &str) -> bool {
    let source_extensions = [
        ".ts", ".tsx", ".js", ".jsx", ".mjs", ".mts", ".cts",
        ".py", ".pyi",
        ".go",
        ".rs",
        ".java",
        ".c", ".cpp", ".h", ".hpp", ".cc",
        ".cs",
        ".php",
        ".rb",
        ".swift",
        ".kt", ".kts",
        ".dart",
        ".vue", ".svelte", ".astro",
        ".liquid",
        ".scala", ".sc",
        ".lua", ".luau",
        ".m", ".mm",
        ".toml", ".yaml", ".yml", ".json",
    ];

    for ext in &source_extensions {
        if rel_path.ends_with(ext) {
            return true;
        }
    }

    false
}

/// Get current time in milliseconds
fn current_time_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Returns a human-readable reason when live watching should be disabled.
pub fn watch_disabled_reason(project_root: &str) -> Option<String> {
    let no_watch = truthy_env("CODEGRAPH_NO_WATCH");
    let force_watch = truthy_env("CODEGRAPH_FORCE_WATCH");
    watch_disabled_reason_with(project_root, no_watch, force_watch, detect_wsl())
}

fn watch_disabled_reason_with(
    project_root: &str,
    no_watch: bool,
    force_watch: bool,
    is_wsl: bool,
) -> Option<String> {
    if no_watch {
        return Some("CODEGRAPH_NO_WATCH is set".to_string());
    }
    if force_watch {
        return None;
    }
    if is_wsl && is_windows_drive_mount(project_root) {
        return Some(
            "project is on a WSL /mnt drive where recursive watching is slow".to_string(),
        );
    }
    None
}

fn truthy_env(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            let value = value.trim();
            !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
        }
        Err(_) => false,
    }
}

fn is_windows_drive_mount(project_root: &str) -> bool {
    let normalized = project_root.replace('\\', "/").to_ascii_lowercase();
    let rest = match normalized.strip_prefix("/mnt/") {
        Some(rest) => rest,
        None => return false,
    };
    let mut chars = rest.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_lowercase())
        && matches!(chars.next(), Some('/') | None)
}

fn detect_wsl() -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        false
    }

    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WSL_DISTRO_NAME").is_some()
            || std::env::var_os("WSL_INTEROP").is_some()
        {
            return true;
        }
        std::fs::read_to_string("/proc/version")
            .map(|version| {
                let version = version.to_ascii_lowercase();
                version.contains("microsoft") || version.contains("wsl")
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn test_is_source_file() {
        assert!(is_source_file("src/main.ts"));
        assert!(is_source_file("lib/utils.py"));
        assert!(is_source_file("app.go"));
        assert!(is_source_file("Cargo.toml"));
        assert!(is_source_file("data.json")); // JSON is considered source for config files
        assert!(!is_source_file("image.png"));
        assert!(!is_source_file("photo.jpg"));
    }

    #[test]
    fn test_is_ignored_path() {
        assert!(is_ignored_path(".git/config"));
        assert!(is_ignored_path("node_modules/package/index.js"));
        assert!(is_ignored_path("target/debug/app.exe"));
        assert!(is_ignored_path("image.png"));
        assert!(!is_ignored_path("src/main.ts"));
    }

    #[test]
    fn test_get_relative_path() {
        let project_root = "/home/user/project";
        let full_path = "/home/user/project/src/main.ts";

        let rel = get_relative_path(Path::new(full_path), project_root);
        assert_eq!(rel, Some("src/main.ts".to_string()));
    }

    #[test]
    fn watch_policy_no_watch_wins() {
        let reason = watch_disabled_reason_with("/home/me/project", true, true, false);

        assert!(reason.unwrap().contains("CODEGRAPH_NO_WATCH"));
    }

    #[test]
    fn watch_policy_disables_wsl_windows_drive_mounts() {
        let reason = watch_disabled_reason_with("/mnt/d/code/project", false, false, true);

        assert!(reason.unwrap().contains("/mnt"));
        assert!(watch_disabled_reason_with("/mnt/wsl/project", false, false, true).is_none());
        assert!(watch_disabled_reason_with("/mnt/d/code/project", false, false, false).is_none());
    }

    #[test]
    fn watch_policy_force_watch_overrides_wsl_mount() {
        let reason = watch_disabled_reason_with("/mnt/d/code/project", false, true, true);

        assert!(reason.is_none());
    }

    #[test]
    fn watch_policy_env_no_watch_keeps_watcher_inactive() {
        let _guard = env_lock();
        let previous = std::env::var_os("CODEGRAPH_NO_WATCH");
        std::env::set_var("CODEGRAPH_NO_WATCH", "1");
        let dir = tempfile::tempdir().unwrap();
        let mut watcher = FileWatcher::new(dir.path().to_str().unwrap());

        watcher.start(|_| {}).unwrap();

        assert!(!watcher.is_running());
        if let Some(previous) = previous {
            std::env::set_var("CODEGRAPH_NO_WATCH", previous);
        } else {
            std::env::remove_var("CODEGRAPH_NO_WATCH");
        }
    }
}
