use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

pub struct FolderWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
    #[allow(dead_code)]
    watched_dir: PathBuf,
}

impl FolderWatcher {
    pub fn new<P: AsRef<Path>>(dir: P) -> Result<Self, String> {
        let watched_dir = dir.as_ref().to_path_buf();
        if !watched_dir.exists() {
            std::fs::create_dir_all(&watched_dir)
                .map_err(|e| format!("Failed to create watched directory {}: {e}", watched_dir.display()))?;
        }

        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default().with_poll_interval(Duration::from_secs(1)))
            .map_err(|e| format!("Failed to initialize file watcher: {e}"))?;

        watcher
            .watch(&watched_dir, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch directory {}: {e}", watched_dir.display()))?;

        Ok(Self {
            _watcher: watcher,
            rx,
            watched_dir,
        })
    }

    #[allow(dead_code)]
    pub fn watched_dir(&self) -> &Path {
        &self.watched_dir
    }

    pub fn poll_next_file(&self) -> Option<PathBuf> {
        while let Ok(event_result) = self.rx.try_recv() {
            if let Ok(event) = event_result {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in event.paths {
                            if Self::is_processable_file(&path) {
                                return Some(path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    fn is_processable_file(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
            if file_name.starts_with('.') || file_name.starts_with("~") || file_name.starts_with("updated_") {
                return false;
            }
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lower = ext.to_lowercase();
            matches!(
                ext_lower.as_str(),
                "pdf" | "docx" | "jpg" | "jpeg" | "png" | "txt" | "md"
            )
        } else {
            false
        }
    }
}
