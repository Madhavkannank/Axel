use crate::config::resolve_workspace_path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub mod metadata;
pub mod virtual_tree;
pub use metadata::FileMetadataTag;
pub use virtual_tree::{
    build_virtual_tree, compute_virtual_paths, DuplicateCluster, VirtualFolder,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexItem {
    pub file: String,
    pub category: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
    pub path: String,
    pub hash: String,
    #[serde(default)]
    pub indexed_at: Option<String>,
    #[serde(default)]
    pub virtual_paths: Vec<String>,
    #[serde(default)]
    pub physical_paths: Vec<String>,
    #[serde(default)]
    pub is_duplicate: bool,
}

impl IndexItem {
    pub fn empty() -> Self {
        Self {
            file: String::new(),
            category: String::new(),
            keywords: Vec::new(),
            symbols: Vec::new(),
            path: String::new(),
            hash: String::new(),
            indexed_at: Some(chrono::Utc::now().to_rfc3339()),
            virtual_paths: Vec::new(),
            physical_paths: Vec::new(),
            is_duplicate: false,
        }
    }

    pub fn new(
        file: String,
        category: String,
        keywords: Vec<String>,
        symbols: Vec<String>,
        path: String,
        hash: String,
        virtual_paths: Vec<String>,
    ) -> Self {
        let ext = Path::new(&file)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let computed_virtual_paths = compute_virtual_paths(&file, &category, &virtual_paths, ext);
        let physical_paths = vec![path.clone()];
        Self {
            file,
            category,
            keywords,
            symbols,
            path,
            hash,
            indexed_at: Some(chrono::Utc::now().to_rfc3339()),
            virtual_paths: computed_virtual_paths,
            physical_paths,
            is_duplicate: false,
        }
    }
}

impl AsRef<IndexItem> for IndexItem {
    fn as_ref(&self) -> &IndexItem {
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub item: IndexItem,
    pub score: f64,
    pub match_type: String, // "exact", "partial", "suggested"
}

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

pub struct IndexFileLock {
    #[allow(dead_code)]
    file: std::fs::File,
}

impl IndexFileLock {
    pub fn acquire(lock_path: &Path) -> Result<Self, std::io::Error> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        #[cfg(unix)]
        {
            let fd = file.as_raw_fd();
            let ret = unsafe { libc::flock(fd, libc::LOCK_EX) };
            if ret != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(Self { file })
    }
}

#[cfg(unix)]
impl Drop for IndexFileLock {
    fn drop(&mut self) {
        let fd = self.file.as_raw_fd();
        unsafe { libc::flock(fd, libc::LOCK_UN) };
    }
}

#[derive(Debug, Clone)]
pub struct DocumentIndex {
    file_path: PathBuf,
    items: Vec<IndexItem>,
    dirty: bool,
    initial_items_hashes: std::collections::HashSet<String>,
    deleted_hashes: std::collections::HashSet<String>,
    removed_physical_paths: std::collections::HashSet<String>,
    modified_hashes: std::collections::HashSet<String>,
    pending_xattr_migrations: Vec<(PathBuf, String, Vec<String>, Vec<String>, String)>,
    recovered_from_backup: bool,
}

impl Default for DocumentIndex {
    fn default() -> Self {
        Self::new()
    }
}

fn paths_match(indexed_str: &str, target: &Path, target_resolved: &Path) -> bool {
    let indexed_path = Path::new(indexed_str);
    if indexed_str == target.to_string_lossy() {
        return true;
    }
    let indexed_resolved = resolve_workspace_path(indexed_path);
    if indexed_resolved == target_resolved {
        return true;
    }
    // Match workspace-relative paths
    if indexed_path.is_relative() && target_resolved.ends_with(indexed_path) {
        return true;
    }
    if target.is_relative() && indexed_resolved.ends_with(target) {
        return true;
    }
    false
}

impl DocumentIndex {
    pub fn load<P: AsRef<Path>>(index_path: P) -> Self {
        let file_path = index_path.as_ref().to_path_buf();
        let bak_path = format!("{}.bak", file_path.display());
        let mut recovered_from_backup = false;

        let raw_items: Vec<IndexItem> = if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => match serde_json::from_str::<Vec<IndexItem>>(&content) {
                    Ok(items) => items,
                    Err(err) => {
                        eprintln!(
                            "[Index] Warning: Failed to parse index JSON at {}: {err}. Preserving corrupt file and attempting recovery from backup...",
                            file_path.display()
                        );
                        // A09: Preserve the corrupt file to .corrupt.<timestamp> for post-mortem analysis
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let corrupt_path = format!("{}.corrupt.{}", file_path.display(), now);
                        let _ = std::fs::copy(&file_path, &corrupt_path);

                        if let Ok(bak_content) = std::fs::read_to_string(&bak_path) {
                            match serde_json::from_str::<Vec<IndexItem>>(&bak_content) {
                                Ok(items) => {
                                    eprintln!(
                                        "[Index] Successfully recovered {} items from backup: {}",
                                        items.len(),
                                        bak_path
                                    );
                                    recovered_from_backup = true;
                                    items
                                }
                                Err(bak_err) => {
                                    eprintln!("[Index] CRITICAL: Backup file also invalid: {bak_err}. Unrecoverable corruption.");
                                    Vec::new()
                                }
                            }
                        } else {
                            eprintln!("[Index] CRITICAL: Backup file not found. Unrecoverable corruption.");
                            Vec::new()
                        }
                    }
                },
                Err(err) => {
                    eprintln!(
                        "[Index] Warning: Failed to read index file at {}: {err}. Attempting recovery from backup...",
                        file_path.display()
                    );
                    if let Ok(bak_content) = std::fs::read_to_string(&bak_path) {
                        match serde_json::from_str::<Vec<IndexItem>>(&bak_content) {
                            Ok(items) => {
                                eprintln!(
                                    "[Index] Successfully recovered {} items from backup: {}",
                                    items.len(),
                                    bak_path
                                );
                                recovered_from_backup = true;
                                items
                            }
                            Err(_) => Vec::new(),
                        }
                    } else {
                        Vec::new()
                    }
                }
            }
        } else if std::path::Path::new(&bak_path).exists() {
            // Primary missing on disk; recover from valid backup
            if let Ok(bak_content) = std::fs::read_to_string(&bak_path) {
                match serde_json::from_str::<Vec<IndexItem>>(&bak_content) {
                    Ok(items) => {
                        eprintln!(
                            "[Index] Primary file missing; recovered {} items from backup: {}",
                            items.len(),
                            bak_path
                        );
                        recovered_from_backup = true;
                        items
                    }
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Filter out any legacy or invalid "General" items so they are properly re-indexed with dynamic tags
        let mut items: Vec<IndexItem> = raw_items
            .into_iter()
            .filter(|item| {
                let cat = item.category.trim();
                !cat.is_empty()
                    && !cat.eq_ignore_ascii_case("general")
                    && !cat.eq_ignore_ascii_case("documents/general")
            })
            .collect();

        // A04: Versioned hash migration covering both JSON index and extended attributes.
        // Files tagged before full SHA-256 lacked user.axel.hash_algo = "sha256".
        // Recompute full SHA-256 and update in-memory index record.
        // Extended attributes will be durably stamped when the index JSON is committed to disk (A04).
        let mut migrated_any = false;
        let mut pending_xattr_migrations: Vec<(PathBuf, String, Vec<String>, Vec<String>, String)> = Vec::new();
        for item in &mut items {
            let p = Path::new(&item.path);
            if p.is_file() {
                if let Ok(meta) = std::fs::metadata(p) {
                    if meta.len() > 16 * 1024 * 1024 {
                        let algo_tag = xattr::get(p, crate::index::metadata::XATTR_HASH_ALGO).ok().flatten();
                        let has_sha256 = algo_tag.as_deref() == Some(crate::index::metadata::CURRENT_HASH_ALGO);
                        let version_tag = xattr::get(p, crate::index::metadata::XATTR_VERSION).ok().flatten();
                        let is_v3 = version_tag.as_deref() == Some(crate::index::metadata::CURRENT_XATTR_VERSION);

                        if !has_sha256 || !is_v3 {
                            if let Some(new_hash) = Self::compute_file_hash(p) {
                                if new_hash != item.hash {
                                    item.hash = new_hash.clone();
                                    migrated_any = true;
                                }
                                pending_xattr_migrations.push((
                                    p.to_path_buf(),
                                    item.category.clone(),
                                    item.keywords.clone(),
                                    item.virtual_paths.clone(),
                                    item.hash.clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        let initial_items_hashes = items.iter().map(|i| i.hash.clone()).collect();

        Self {
            file_path,
            items,
            dirty: recovered_from_backup || migrated_any,
            initial_items_hashes,
            deleted_hashes: std::collections::HashSet::new(),
            removed_physical_paths: std::collections::HashSet::new(),
            modified_hashes: std::collections::HashSet::new(),
            pending_xattr_migrations,
            recovered_from_backup,
        }
    }

    pub fn from_items(items: Vec<IndexItem>) -> Self {
        let initial_items_hashes = items.iter().map(|i| i.hash.clone()).collect();
        Self {
            file_path: PathBuf::new(),
            items,
            dirty: false,
            initial_items_hashes,
            deleted_hashes: std::collections::HashSet::new(),
            removed_physical_paths: std::collections::HashSet::new(),
            modified_hashes: std::collections::HashSet::new(),
            pending_xattr_migrations: Vec::new(),
            recovered_from_backup: false,
        }
    }

    pub fn new() -> Self {
        Self::from_items(Vec::new())
    }

    pub fn compute_file_hash<P: AsRef<Path>>(path: P) -> Option<String> {
        let mut file = File::open(&path).ok()?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let bytes_read = file.read(&mut buffer).ok()?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Some(format!("{:x}", hasher.finalize()))
    }

    pub fn items(&self) -> &[IndexItem] {
        &self.items
    }

    pub fn find_by_hash(&self, hash: &str) -> Option<&IndexItem> {
        self.items.iter().find(|item| item.hash == hash)
    }

    #[allow(dead_code)]
    pub fn find_by_path(&self, path: &str) -> Option<&IndexItem> {
        let target = resolve_workspace_path(Path::new(path));
        self.items
            .iter()
            .find(|item| item.path == path || resolve_workspace_path(Path::new(&item.path)) == target)
    }

    pub fn upsert(&mut self, mut item: IndexItem) {
        let ext = Path::new(&item.file)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        if item.virtual_paths.is_empty() {
            item.virtual_paths = compute_virtual_paths(&item.file, &item.category, &[], ext);
        }

        if item.physical_paths.is_empty() {
            item.physical_paths.push(item.path.clone());
        }

        // Canonical/resolved paths claimed by this item
        let claimed_paths: Vec<PathBuf> = item
            .physical_paths
            .iter()
            .map(|p| resolve_workspace_path(Path::new(p)))
            .collect();

        // H08: Maintain unique path ownership across ALL records.
        // If any OTHER record claims any of these paths, relinquish ownership atomically.
        let mut idx = 0;
        while idx < self.items.len() {
            if self.items[idx].hash != item.hash {
                let other = &mut self.items[idx];
                let before_len = other.physical_paths.len();
                let other_hash = other.hash.clone();
                let removed_from_other: Vec<String> = other
                    .physical_paths
                    .iter()
                    .filter(|p| claimed_paths.contains(&resolve_workspace_path(Path::new(p))))
                    .cloned()
                    .collect();

                other.physical_paths.retain(|p| {
                    let r = resolve_workspace_path(Path::new(p));
                    !claimed_paths.contains(&r)
                });

                if other.physical_paths.len() < before_len {
                    for rp in removed_from_other {
                        self.removed_physical_paths.insert(
                            resolve_workspace_path(Path::new(&rp))
                                .to_string_lossy()
                                .to_string(),
                        );
                    }
                    self.modified_hashes.insert(other_hash.clone());

                    if other.physical_paths.is_empty() {
                        // All physical paths moved to the new record; drop the empty record
                        self.items.remove(idx);
                        self.deleted_hashes.insert(other_hash);
                        continue;
                    } else {
                        // Promote remaining alias if primary was claimed
                        let primary_resolved = resolve_workspace_path(Path::new(&other.path));
                        if claimed_paths.contains(&primary_resolved) {
                            if let Some(next_p) = other.physical_paths.first().cloned() {
                                if let Some(fname) =
                                    Path::new(&next_p).file_name().and_then(|s| s.to_str())
                                {
                                    other.file = fname.to_string();
                                }
                                other.path = next_p;
                            }
                        }
                        other.is_duplicate = other.physical_paths.len() > 1;
                    }
                }
            }
            idx += 1;
        }

        self.modified_hashes.insert(item.hash.clone());
        self.deleted_hashes.remove(&item.hash);

        // Now update or insert the item under its hash
        if let Some(pos) = self.items.iter().position(|i| i.hash == item.hash) {
            let existing = &mut self.items[pos];

            // M04: Separate metadata replacement from duplicate registration.
            // Check if this upsert is a refresh of the existing primary document or an existing copy.
            let is_refresh = existing.path == item.path
                || resolve_workspace_path(Path::new(&existing.path))
                    == resolve_workspace_path(Path::new(&item.path))
                || existing.physical_paths.iter().any(|ep| {
                    resolve_workspace_path(Path::new(ep))
                        == resolve_workspace_path(Path::new(&item.path))
                })
                || existing.physical_paths.is_empty();

            if is_refresh {
                // Metadata refresh: replace category, keywords, symbols, and virtual paths
                if !item.category.trim().is_empty() {
                    existing.category = item.category;
                }
                if !item.keywords.is_empty() {
                    existing.keywords = item.keywords;
                }
                if !item.symbols.is_empty() {
                    existing.symbols = item.symbols;
                }
                if !item.virtual_paths.is_empty() {
                    existing.virtual_paths = item.virtual_paths;
                }
            }

            // Register any new physical paths
            for p in item.physical_paths {
                let p_res = resolve_workspace_path(Path::new(&p));
                if !existing.physical_paths.iter().any(|ep| resolve_workspace_path(Path::new(ep)) == p_res) {
                    existing.physical_paths.push(p);
                }
            }
            existing.is_duplicate = existing.physical_paths.len() > 1;
        } else {
            self.items.push(item);
        }

        self.dirty = true;
    }

    pub fn remove_by_hash(&mut self, hash: &str) -> Option<IndexItem> {
        if let Some(pos) = self.items.iter().position(|i| i.hash == hash) {
            let removed = self.items.remove(pos);
            for p in &removed.physical_paths {
                self.removed_physical_paths.insert(
                    resolve_workspace_path(Path::new(p)).to_string_lossy().to_string(),
                );
            }
            self.deleted_hashes.insert(hash.to_string());
            self.modified_hashes.remove(hash);
            self.dirty = true;
            Some(removed)
        } else {
            None
        }
    }

    pub fn remove_physical_copy(&mut self, hash: &str, path_to_remove: &str) -> bool {
        self.remove_physical_copy_with_promotion(hash, path_to_remove, None)
    }

    pub fn remove_physical_copy_with_promotion(
        &mut self,
        hash: &str,
        path_to_remove: &str,
        promoted_path: Option<&str>,
    ) -> bool {
        let path_resolved = resolve_workspace_path(Path::new(path_to_remove));
        if let Some(pos) = self.items.iter().position(|i| i.hash == hash) {
            let before = self.items[pos].physical_paths.len();
            self.items[pos].physical_paths.retain(|p| {
                p != path_to_remove && resolve_workspace_path(Path::new(p)) != path_resolved
            });
            if self.items[pos].physical_paths.len() < before {
                self.removed_physical_paths.insert(path_resolved.to_string_lossy().to_string());
                self.modified_hashes.insert(hash.to_string());

                if self.items[pos].physical_paths.is_empty() {
                    self.items.remove(pos);
                    self.deleted_hashes.insert(hash.to_string());
                } else {
                    self.items[pos].is_duplicate = self.items[pos].physical_paths.len() > 1;
                    let current_primary_resolved =
                        resolve_workspace_path(Path::new(&self.items[pos].path));
                    if self.items[pos].path == path_to_remove
                        || current_primary_resolved == path_resolved
                    {
                        let chosen_path = promoted_path
                            .and_then(|pp| {
                                let pp_res = resolve_workspace_path(Path::new(pp));
                                self.items[pos]
                                    .physical_paths
                                    .iter()
                                    .find(|p| *p == pp || resolve_workspace_path(Path::new(p)) == pp_res)
                                    .cloned()
                            })
                            .or_else(|| self.items[pos].physical_paths.first().cloned());

                        if let Some(next_p) = chosen_path {
                            if let Some(fname) =
                                Path::new(&next_p).file_name().and_then(|s| s.to_str())
                            {
                                self.items[pos].file = fname.to_string();
                            }
                            self.items[pos].path = next_p;
                        }
                    }
                }
                self.dirty = true;
                return true;
            }
        }
        false
    }

    pub fn restore_item(&mut self, item: IndexItem) {
        let hash = item.hash.clone();
        for p in &item.physical_paths {
            self.removed_physical_paths.remove(&resolve_workspace_path(Path::new(p)).to_string_lossy().to_string());
        }
        if let Some(pos) = self.items.iter().position(|i| i.hash == hash) {
            self.items[pos] = item;
        } else {
            self.items.push(item);
        }
        self.deleted_hashes.remove(&hash);
        self.modified_hashes.insert(hash);
        self.dirty = true;
    }

    /// Reconciles deletion of a file at `path` across all records.
    /// If surviving physical aliases exist, promotes one to primary path.
    /// If no aliases remain, removes the document record entirely from the index (M05).
    pub fn remove_path<P: AsRef<Path>>(&mut self, path: P) -> bool {
        let p = path.as_ref();
        let p_resolved = resolve_workspace_path(p);
        let p_resolved_str = p_resolved.to_string_lossy().to_string();
        let mut modified = false;

        let mut idx = 0;
        while idx < self.items.len() {
            let matches_primary = paths_match(&self.items[idx].path, p, &p_resolved);
            let has_in_physical = self.items[idx].physical_paths.iter().any(|pp| {
                paths_match(pp, p, &p_resolved)
            });

            if matches_primary || has_in_physical {
                let hash = self.items[idx].hash.clone();
                self.items[idx].physical_paths.retain(|pp| {
                    !paths_match(pp, p, &p_resolved)
                });

                self.removed_physical_paths.insert(p_resolved_str.clone());
                self.modified_hashes.insert(hash.clone());

                if self.items[idx].physical_paths.is_empty() {
                    self.items.remove(idx);
                    self.deleted_hashes.insert(hash);
                    modified = true;
                    continue;
                } else {
                    self.items[idx].is_duplicate = self.items[idx].physical_paths.len() > 1;
                    if matches_primary {
                        if let Some(next_p) = self.items[idx].physical_paths.first().cloned() {
                            if let Some(fname) = Path::new(&next_p).file_name().and_then(|s| s.to_str()) {
                                self.items[idx].file = fname.to_string();
                            }
                            self.items[idx].path = next_p;
                        }
                    }
                    modified = true;
                }
            }
            idx += 1;
        }

        if modified {
            self.dirty = true;
        }
        modified
    }

    /// Registers a duplicate physical path as an alias in an existing document record (M05).
    pub fn add_physical_alias(&mut self, hash: &str, path: &str) -> bool {
        let p_resolved = resolve_workspace_path(Path::new(path));
        if let Some(item) = self.items.iter_mut().find(|i| i.hash == hash) {
            if !item.physical_paths.iter().any(|p| resolve_workspace_path(Path::new(p)) == p_resolved) {
                item.physical_paths.push(path.to_string());
                item.is_duplicate = item.physical_paths.len() > 1;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    pub fn items_mut(&mut self) -> &mut Vec<IndexItem> {
        self.dirty = true;
        &mut self.items
    }

    /// Remove entries whose files no longer exist on disk or live outside `workspace`.
    pub fn prune_missing_in_workspace(&mut self, workspace: &Path) -> usize {
        let base = resolve_workspace_path(workspace);
        let before = self.items.len();
        self.items.retain(|item| {
            let p = resolve_workspace_path(Path::new(&item.path));
            p.exists() && p.is_file() && p.starts_with(&base)
        });
        let removed = before.saturating_sub(self.items.len());
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if !self.dirty {
            return Ok(());
        }

        if self.file_path.as_os_str().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot save index: file_path is empty",
            ));
        }

        if let Some(parent) = self.file_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // A03: Interprocess advisory lock to serialize read-merge-write-fsync-rename cycles
        let lock_path = format!("{}.lock", self.file_path.display());
        let _lock = IndexFileLock::acquire(Path::new(&lock_path))?;

        // A02: Reconcile with on-disk state using explicit path and metadata mutation deltas
        if self.file_path.exists() {
            if let Ok(disk_content) = std::fs::read_to_string(&self.file_path) {
                if let Ok(disk_items) = serde_json::from_str::<Vec<IndexItem>>(&disk_content) {
                    for disk_item in disk_items {
                        // If we explicitly deleted this hash in self, never resurrect it
                        if self.deleted_hashes.contains(&disk_item.hash) {
                            continue;
                        }

                        if let Some(pos) = self.items.iter().position(|i| i.hash == disk_item.hash) {
                            let local = &mut self.items[pos];
                            // Merge physical paths that another writer added, filtering out paths explicitly removed in self
                            for dp in disk_item.physical_paths {
                                let dp_res = resolve_workspace_path(Path::new(&dp));
                                let dp_res_str = dp_res.to_string_lossy().to_string();
                                if self.removed_physical_paths.contains(&dp_res_str) {
                                    continue;
                                }
                                if !local.physical_paths.iter().any(|lp| resolve_workspace_path(Path::new(lp)) == dp_res) {
                                    local.physical_paths.push(dp);
                                }
                            }
                            local.is_duplicate = local.physical_paths.len() > 1;

                            // If self did NOT modify this item, adopt any refreshed metadata from disk
                            if !self.modified_hashes.contains(&disk_item.hash) {
                                local.category = disk_item.category;
                                local.virtual_paths = disk_item.virtual_paths;
                                local.keywords = disk_item.keywords;
                            }
                        } else if !self.initial_items_hashes.contains(&disk_item.hash) {
                            // An independent handle/writer inserted this item while self was open!
                            // Preserve without dropping, but exclude any paths explicitly removed in self
                            let mut item_to_add = disk_item;
                            item_to_add.physical_paths.retain(|p| {
                                let res = resolve_workspace_path(Path::new(p)).to_string_lossy().to_string();
                                !self.removed_physical_paths.contains(&res)
                            });
                            if !item_to_add.physical_paths.is_empty() {
                                self.items.push(item_to_add);
                            }
                        }
                    }
                }
            }
        }

        let json = serde_json::to_string_pretty(&self.items)
            .map_err(std::io::Error::other)?;

        // Atomic persistence with fsync and crash-recovery backup
        static SAVE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let count = SAVE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let parent_dir = self.file_path.parent().unwrap_or_else(|| Path::new("."));
        let temp_name = format!(
            ".tmp_index_{}_{}_{}.tmp",
            std::process::id(),
            now,
            count
        );
        let temp_path = parent_dir.join(temp_name);

        {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }

        // A09: Preserve validated backup generations.
        // Validate against Vec<IndexItem> schema, NEVER arbitrary serde_json::Value!
        if self.file_path.exists() {
            let primary_is_valid = match std::fs::read_to_string(&self.file_path) {
                Ok(content) => serde_json::from_str::<Vec<IndexItem>>(&content).is_ok(),
                Err(_) => false,
            };
            if primary_is_valid {
                let bak_path = format!("{}.bak", self.file_path.display());
                let _ = std::fs::copy(&self.file_path, &bak_path);
            }
        }

        if let Err(e) = std::fs::rename(&temp_path, &self.file_path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(e);
        }

        // A04: Commit pending xattr migrations now that index JSON is durably written to disk
        for (p, cat, kws, vpaths, h) in self.pending_xattr_migrations.drain(..) {
            let _ = crate::index::metadata::FileMetadataTag::write_to_file(&p, &cat, &kws, &vpaths, &h);
        }

        self.initial_items_hashes = self.items.iter().map(|i| i.hash.clone()).collect();
        self.deleted_hashes.clear();
        self.removed_physical_paths.clear();
        self.modified_hashes.clear();
        self.dirty = false;
        self.recovered_from_backup = false;
        Ok(())
    }

    pub fn all(&self) -> &[IndexItem] {
        &self.items
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_clean = query.trim().to_lowercase();
        if query_clean.is_empty() {
            return Vec::new();
        }

        let query_terms: Vec<&str> = query_clean.split_whitespace().collect();
        let mut filter_dup = false;
        let mut filter_symbols = Vec::new();
        let mut filter_keywords = Vec::new();
        let mut filter_cats = Vec::new();
        let mut plain_terms = Vec::new();

        for term in &query_terms {
            if *term == "is:dup" || *term == "dup:" || *term == "•" {
                filter_dup = true;
            } else if let Some(cat) = term.strip_prefix("@cat:") {
                if !cat.is_empty() {
                    filter_cats.push(cat);
                }
            } else if let Some(sym) = term
                .strip_prefix("@fn:")
                .or_else(|| term.strip_prefix("@var:"))
                .or_else(|| term.strip_prefix('@'))
            {
                if !sym.is_empty() {
                    filter_symbols.push(sym);
                }
            } else if let Some(kw) = term.strip_prefix('#') {
                if !kw.is_empty() {
                    filter_keywords.push(kw);
                }
            } else {
                plain_terms.push(*term);
            }
        }

        let mut exact_matches = Vec::new();
        let mut fuzzy_matches = Vec::new();

        for item in &self.items {
            let cat_lower = item.category.to_lowercase();
            let file_lower = item.file.to_lowercase();
            let path_lower = item.path.to_lowercase();
            let kws_lower: Vec<String> = item.keywords.iter().map(|k| k.to_lowercase()).collect();
            let symbols_lower: Vec<String> = item.symbols.iter().map(|s| s.to_lowercase()).collect();

            // 1. Strict filter checks if specific prefixes were specified
            if filter_dup && (!item.is_duplicate && item.physical_paths.len() <= 1) {
                continue;
            }
            if !filter_cats.is_empty() && !filter_cats.iter().any(|c| cat_lower.contains(c)) {
                continue;
            }
            if !filter_symbols.is_empty()
                && !filter_symbols
                    .iter()
                    .any(|s| symbols_lower.iter().any(|sym| sym.contains(s)))
            {
                continue;
            }
            if !filter_keywords.is_empty()
                && !filter_keywords
                    .iter()
                    .any(|k| kws_lower.iter().any(|kw| kw.contains(k)))
            {
                continue;
            }

            let mut score: f64 = 0.0;
            let mut match_type = "partial";

            // If query consists entirely of valid prefix filters that passed
            if plain_terms.is_empty()
                && (filter_dup
                    || !filter_symbols.is_empty()
                    || !filter_keywords.is_empty()
                    || !filter_cats.is_empty())
            {
                score = 0.95;
                match_type = "filter";
            }

            // Keyword and symbol boosts
            if !filter_symbols.is_empty() {
                if filter_symbols.iter().any(|s| symbols_lower.iter().any(|sym| sym == s)) {
                    score = score.max(0.98);
                    match_type = "exact_symbol";
                } else {
                    score = score.max(0.92);
                }
            }
            if !filter_keywords.is_empty() {
                if filter_keywords.iter().any(|k| kws_lower.iter().any(|kw| kw == k)) {
                    score = score.max(0.97);
                    match_type = "exact_kw";
                } else {
                    score = score.max(0.91);
                }
            }

            // Plain term matches
            if !plain_terms.is_empty() {
                let joined_plain = plain_terms.join(" ");
                if cat_lower.contains(&joined_plain) {
                    score = score.max(1.0);
                    match_type = "exact";
                }
                if file_lower.contains(&joined_plain) {
                    score = score.max(0.95);
                    match_type = "exact";
                }
                if symbols_lower.iter().any(|s| s == &joined_plain) {
                    score = score.max(0.96);
                    match_type = "exact";
                } else if symbols_lower.iter().any(|s| s.contains(&joined_plain)) {
                    score = score.max(0.91);
                    match_type = "exact";
                }
                if kws_lower.iter().any(|k| k.contains(&joined_plain)) {
                    score = score.max(0.9);
                    match_type = "exact";
                }
                let is_path_query = joined_plain.contains('/') || joined_plain.contains('\\');
                if (is_path_query && path_lower.contains(&joined_plain))
                    || (!is_path_query
                        && joined_plain.len() >= 4
                        && Path::new(&item.path)
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_lowercase())
                            .as_deref()
                            == Some(&joined_plain))
                {
                    score = score.max(0.85);
                }

                let term_hits = plain_terms
                    .iter()
                    .filter(|&t| {
                        cat_lower.contains(t)
                            || file_lower.contains(t)
                            || kws_lower.iter().any(|k| k.contains(t))
                            || symbols_lower.iter().any(|s| s.contains(t))
                    })
                    .count();

                if term_hits > 0 {
                    let term_score = 0.7 + (term_hits as f64 / plain_terms.len() as f64) * 0.25;
                    score = score.max(term_score);
                }
            }

            if score > 0.0 {
                exact_matches.push(SearchResult {
                    item: item.clone(),
                    score,
                    match_type: match_type.to_string(),
                });
            } else if !plain_terms.is_empty() {
                let joined_plain = plain_terms.join(" ");
                let best_sim = kws_lower
                    .iter()
                    .map(|k| strsim::jaro_winkler(&joined_plain, k))
                    .chain(symbols_lower.iter().map(|s| strsim::jaro_winkler(&joined_plain, s)))
                    .chain(std::iter::once(strsim::jaro_winkler(&joined_plain, &file_lower)))
                    .chain(std::iter::once(strsim::jaro_winkler(&joined_plain, &cat_lower)))
                    .fold(0.0f64, |acc, v| acc.max(v));

                if best_sim >= 0.70 {
                    fuzzy_matches.push(SearchResult {
                        item: item.clone(),
                        score: best_sim * 0.7,
                        match_type: "suggested".to_string(),
                    });
                }
            }
        }

        let mut all_results = Vec::new();
        all_results.extend(exact_matches);
        all_results.extend(fuzzy_matches);

        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for r in all_results {
            if seen.insert(r.item.hash.clone()) {
                deduped.push(r);
            }
        }

        deduped.truncate(limit);
        deduped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_by_variable_or_function_symbol() {
        let items = vec![
            IndexItem {
                file: "10.py".to_string(),
                category: "Code/Vision".to_string(),
                keywords: vec!["computer vision".to_string(), "opencv".to_string()],
                symbols: vec!["sobel_x".to_string(), "sobel_y".to_string(), "image_path".to_string()],
                path: "/workspace/projects/10.py".to_string(),
                hash: "h10".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/workspace/projects/10.py".to_string()],
                is_duplicate: false,
            },
            IndexItem {
                file: "1.py".to_string(),
                category: "Code/Math".to_string(),
                keywords: vec!["calculator".to_string(), "arithmetic".to_string(), "math".to_string()],
                symbols: vec!["add".to_string(), "subtract".to_string(), "multiply".to_string()],
                path: "/workspace/projects/1.py".to_string(),
                hash: "h1".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/workspace/projects/1.py".to_string()],
                is_duplicate: false,
            },
        ];

        let index = DocumentIndex::from_items(items);

        // 1. Search by exact variable name in 10.py
        let res_var = index.search("sobel_x", 5);
        assert_eq!(res_var.len(), 1);
        assert_eq!(res_var[0].item.file, "10.py");

        // 2. Search by concept keyword "calculator" in 1.py
        let res_concept = index.search("calculator", 5);
        assert_eq!(res_concept.len(), 1);
        assert_eq!(res_concept[0].item.file, "1.py");

        // 3. Search by function name "add" in 1.py
        let res_func = index.search("add", 5);
        assert_eq!(res_func.len(), 1);
        assert_eq!(res_func[0].item.file, "1.py");
    }

    #[test]
    fn test_duplicate_download_tracking_and_virtual_pointers() {
        let mut index = DocumentIndex::from_items(Vec::new());

        // File downloaded first time as notes.pdf
        let item1 = IndexItem {
            file: "calculus_notes.pdf".to_string(),
            category: "Academic".to_string(),
            keywords: vec!["calculus".to_string(), "derivatives".to_string(), "notes".to_string()],
            symbols: Vec::new(),
            path: "/downloads/calculus_notes.pdf".to_string(),
            hash: "same_sha256_hash_123".to_string(),
            indexed_at: None,
            virtual_paths: vec!["Study Materials/Calculus/Notes".to_string()],
            physical_paths: Vec::new(),
            is_duplicate: false,
        };

        index.upsert(item1);
        assert_eq!(index.count(), 1);
        assert!(!index.all()[0].is_duplicate);
        assert_eq!(index.all()[0].physical_paths.len(), 1);

        // Same file downloaded second time as calculus_notes (1).pdf
        let item2 = IndexItem {
            file: "calculus_notes (1).pdf".to_string(),
            category: "Academic".to_string(),
            keywords: vec!["calculus".to_string()],
            symbols: Vec::new(),
            path: "/downloads/calculus_notes (1).pdf".to_string(),
            hash: "same_sha256_hash_123".to_string(),
            indexed_at: None,
            virtual_paths: Vec::new(),
            physical_paths: Vec::new(),
            is_duplicate: false,
        };

        index.upsert(item2);

        // Count must still be 1 (unified, no duplicate clutter!)
        assert_eq!(index.count(), 1);
        let canonical = &index.all()[0];

        // Must be marked as duplicate with 2 physical paths tracked
        assert!(canonical.is_duplicate);
        assert_eq!(canonical.physical_paths.len(), 2);
        assert!(canonical.physical_paths.contains(&"/downloads/calculus_notes.pdf".to_string()));
        assert!(canonical.physical_paths.contains(&"/downloads/calculus_notes (1).pdf".to_string()));

        // Virtual pointers auto-computed
        assert!(!canonical.virtual_paths.is_empty());
        assert!(canonical.virtual_paths[0].starts_with("Study Materials/Calculus/"));
    }

    #[test]
    fn test_structured_query_grammar() {
        let items = vec![
            IndexItem {
                file: "10.py".to_string(),
                category: "Code/Vision".to_string(),
                keywords: vec!["computer vision".to_string(), "opencv".to_string()],
                symbols: vec!["sobel_x".to_string(), "sobel_y".to_string(), "image_path".to_string()],
                path: "/workspace/projects/10.py".to_string(),
                hash: "h10".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/workspace/projects/10.py".to_string()],
                is_duplicate: false,
            },
            IndexItem {
                file: "1.py".to_string(),
                category: "Code/Math".to_string(),
                keywords: vec!["calculator".to_string(), "arithmetic".to_string()],
                symbols: vec!["add".to_string(), "subtract".to_string()],
                path: "/workspace/projects/1.py".to_string(),
                hash: "h1".to_string(),
                indexed_at: None,
                virtual_paths: Vec::new(),
                physical_paths: vec!["/workspace/projects/1.py".to_string(), "/downloads/1.py".to_string()],
                is_duplicate: true,
            },
        ];

        let index = DocumentIndex::from_items(items);

        // 1. Filter by duplicate badge/tag
        let dups = index.search("is:dup", 10);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].item.file, "1.py");

        // 2. Filter by AST function symbol
        let sym_search = index.search("@fn:sobel_x", 10);
        assert_eq!(sym_search.len(), 1);
        assert_eq!(sym_search[0].item.file, "10.py");

        // 3. Filter by keyword chip
        let kw_search = index.search("#opencv", 10);
        assert_eq!(kw_search.len(), 1);
        assert_eq!(kw_search[0].item.file, "10.py");

        // 4. Combined query: category filter + plain term
        let cat_search = index.search("@cat:vision sobel", 10);
        assert_eq!(cat_search.len(), 1);
        assert_eq!(cat_search[0].item.file, "10.py");
    }
}
