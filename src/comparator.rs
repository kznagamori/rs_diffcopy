use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::progress::ProgressPhase;
use crate::scanner::{get_special_file_type, get_symlink_target, is_symlink_broken, Scanner};
use crate::types::{
    CheckPermissionsMode, ComparisonResult, ComparisonStats, FileEntry, FileStatus,
    PermissionChange, SpecialFileType, SymlinkInfo,
};
use crate::utils::{get_file_mode, hash_file, is_script_file};

/// Two-way directory comparator
pub struct Comparator<'a> {
    config: &'a Config,
    scanner: Scanner,
}

impl<'a> Comparator<'a> {
    pub fn new(config: &'a Config) -> crate::error::Result<Self> {
        let scanner = Scanner::new(&config.exclude)?;
        Ok(Self { config, scanner })
    }

    /// Perform comparison between source and target directories
    pub fn compare(&self, progress: &ProgressPhase) -> crate::error::Result<ComparisonResult> {
        // Scan both directories
        let source_entries = self.scanner.scan(&self.config.source)?;
        let target_entries = self.scanner.scan(&self.config.target)?;

        // Build path sets
        let source_paths: HashSet<PathBuf> = source_entries
            .iter()
            .map(|e| e.relative_path.clone())
            .collect();
        let target_paths: HashSet<PathBuf> = target_entries
            .iter()
            .map(|e| e.relative_path.clone())
            .collect();

        // Get all unique paths
        let all_paths: Vec<PathBuf> = source_paths.union(&target_paths).cloned().collect();

        // Compare files in parallel
        let result = Arc::new(Mutex::new(ComparisonResult::new()));

        let chunk_size = (all_paths.len() / self.config.workers).max(1);

        all_paths
            .par_chunks(chunk_size)
            .for_each(|paths| {
                for path in paths {
                    let entry = self.compare_path(path, &source_paths, &target_paths);
                    if let Some(entry) = entry {
                        let mut res = result.lock().unwrap();
                        res.entries.push(entry);
                    }
                    progress.inc();
                }
            });

        let mut result = Arc::try_unwrap(result).unwrap().into_inner().unwrap();

        // Sort entries by path
        result.entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        // Calculate statistics
        result.stats = self.calculate_stats(&result.entries);

        Ok(result)
    }

    fn compare_path(
        &self,
        relative_path: &Path,
        source_paths: &HashSet<PathBuf>,
        target_paths: &HashSet<PathBuf>,
    ) -> Option<FileEntry> {
        let source_path = self.config.source.join(relative_path);
        let target_path = self.config.target.join(relative_path);

        let in_source = source_paths.contains(relative_path);
        let in_target = target_paths.contains(relative_path);

        // Check for symlinks
        if source_path.is_symlink() || target_path.is_symlink() {
            return self.handle_symlink(relative_path, &source_path, &target_path, in_source, in_target);
        }

        // Check for special files (Unix)
        if let Some(special_type) = get_special_file_type(&source_path)
            .or_else(|| get_special_file_type(&target_path))
        {
            return self.handle_special_file(relative_path, special_type);
        }

        // Determine status
        let is_dir = source_path.is_dir() || target_path.is_dir();

        match (in_source, in_target) {
            (false, true) => {
                // Added in target
                let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Added, is_dir);
                if !is_dir {
                    entry.target_size = std::fs::metadata(&target_path).ok().map(|m| m.len());
                    entry.target_hash = hash_file(&target_path).ok();
                }
                Some(entry)
            }
            (true, false) => {
                // Deleted from target
                let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Deleted, is_dir);
                if !is_dir {
                    entry.source_size = std::fs::metadata(&source_path).ok().map(|m| m.len());
                    entry.source_hash = hash_file(&source_path).ok();
                }
                Some(entry)
            }
            (true, true) => {
                if is_dir {
                    // Directories that exist in both - skip unless checking permissions
                    if self.should_check_permissions(relative_path) {
                        self.check_directory_permissions(relative_path, &source_path, &target_path)
                    } else {
                        None
                    }
                } else {
                    // File exists in both - compare content
                    self.compare_file(relative_path, &source_path, &target_path)
                }
            }
            (false, false) => None,
        }
    }

    fn compare_file(
        &self,
        relative_path: &Path,
        source_path: &Path,
        target_path: &Path,
    ) -> Option<FileEntry> {
        let source_meta = match std::fs::metadata(source_path) {
            Ok(m) => m,
            Err(e) => {
                let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Error, false);
                entry.error_message = Some(e.to_string());
                return Some(entry);
            }
        };

        let target_meta = match std::fs::metadata(target_path) {
            Ok(m) => m,
            Err(e) => {
                let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Error, false);
                entry.error_message = Some(e.to_string());
                return Some(entry);
            }
        };

        let source_size = source_meta.len();
        let target_size = target_meta.len();

        // Quick check: different sizes means different content
        let content_changed = if source_size != target_size {
            true
        } else {
            // Same size - compare hashes
            let source_hash = hash_file(source_path).ok();
            let target_hash = hash_file(target_path).ok();
            source_hash != target_hash
        };

        // Check permissions if enabled
        let permission_changed = self.check_permission_change(relative_path, source_path, target_path);

        if content_changed {
            let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Modified, false);
            entry.source_size = Some(source_size);
            entry.target_size = Some(target_size);
            entry.source_hash = hash_file(source_path).ok();
            entry.target_hash = hash_file(target_path).ok();
            entry.permission_change = permission_changed;
            Some(entry)
        } else if let Some(perm_change) = permission_changed {
            // Only permission changed
            let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Permission, false);
            entry.source_size = Some(source_size);
            entry.target_size = Some(target_size);
            entry.permission_change = Some(perm_change);
            Some(entry)
        } else if self.config.show_unchanged {
            // No changes but showing unchanged
            let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Unchanged, false);
            entry.source_size = Some(source_size);
            entry.target_size = Some(target_size);
            Some(entry)
        } else {
            None
        }
    }

    fn handle_symlink(
        &self,
        relative_path: &Path,
        source_path: &Path,
        target_path: &Path,
        in_source: bool,
        in_target: bool,
    ) -> Option<FileEntry> {
        let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Symlink, false);

        match (in_source && source_path.is_symlink(), in_target && target_path.is_symlink()) {
            (false, true) => {
                // Added symlink
                let target = get_symlink_target(target_path)
                    .unwrap_or_else(|| PathBuf::from("(unknown target)"));
                entry.symlink_info = Some(SymlinkInfo {
                    path: relative_path.to_path_buf(),
                    target,
                    is_directory: target_path.is_dir(),
                    is_broken: is_symlink_broken(target_path),
                });
            }
            (true, false) => {
                // Deleted symlink
                let target = get_symlink_target(source_path)
                    .unwrap_or_else(|| PathBuf::from("(unknown target)"));
                entry.symlink_info = Some(SymlinkInfo {
                    path: relative_path.to_path_buf(),
                    target,
                    is_directory: source_path.is_dir(),
                    is_broken: is_symlink_broken(source_path),
                });
            }
            (true, true) => {
                // Both are symlinks - check if target changed
                let source_target = get_symlink_target(source_path);
                let target_target = get_symlink_target(target_path);
                if source_target != target_target {
                    // Symlink target changed
                    let target = target_target
                        .unwrap_or_else(|| PathBuf::from("(unknown target)"));
                    entry.symlink_info = Some(SymlinkInfo {
                        path: relative_path.to_path_buf(),
                        target,
                        is_directory: target_path.is_dir(),
                        is_broken: is_symlink_broken(target_path),
                    });
                } else {
                    // Symlink unchanged - don't report
                    return None;
                }
            }
            _ => return None,
        }

        Some(entry)
    }

    fn handle_special_file(
        &self,
        relative_path: &Path,
        special_type: SpecialFileType,
    ) -> Option<FileEntry> {
        let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Special, false);
        entry.special_type = Some(special_type);
        Some(entry)
    }

    fn should_check_permissions(&self, relative_path: &Path) -> bool {
        match self.config.check_permissions {
            CheckPermissionsMode::None => false,
            CheckPermissionsMode::Scripts => is_script_file(relative_path),
            CheckPermissionsMode::All => true,
        }
    }

    fn check_permission_change(
        &self,
        relative_path: &Path,
        source_path: &Path,
        target_path: &Path,
    ) -> Option<PermissionChange> {
        if !self.should_check_permissions(relative_path) {
            return None;
        }

        let old_mode = get_file_mode(source_path).ok()?;
        let new_mode = get_file_mode(target_path).ok()?;

        if old_mode != new_mode {
            Some(PermissionChange {
                path: relative_path.to_path_buf(),
                old_mode,
                new_mode,
            })
        } else {
            None
        }
    }

    fn check_directory_permissions(
        &self,
        relative_path: &Path,
        source_path: &Path,
        target_path: &Path,
    ) -> Option<FileEntry> {
        if let Some(perm_change) = self.check_permission_change(relative_path, source_path, target_path) {
            let mut entry = FileEntry::new(relative_path.to_path_buf(), FileStatus::Permission, true);
            entry.permission_change = Some(perm_change);
            Some(entry)
        } else {
            None
        }
    }

    fn calculate_stats(&self, entries: &[FileEntry]) -> ComparisonStats {
        let mut stats = ComparisonStats::default();

        for entry in entries {
            match entry.status {
                FileStatus::Added => {
                    if entry.is_directory {
                        stats.added_dirs += 1;
                    } else {
                        stats.added_files += 1;
                    }
                }
                FileStatus::Modified => stats.modified_files += 1,
                FileStatus::Deleted => {
                    if entry.is_directory {
                        stats.deleted_dirs += 1;
                    } else {
                        stats.deleted_files += 1;
                    }
                }
                FileStatus::Unchanged => stats.unchanged_files += 1,
                FileStatus::Symlink => stats.symlink_files += 1,
                FileStatus::Special => stats.special_files += 1,
                FileStatus::Permission => stats.permission_files += 1,
                FileStatus::Error => stats.error_files += 1,
            }
        }

        // Calculate total unique items
        stats.total_items = entries.len();

        stats
    }
}
