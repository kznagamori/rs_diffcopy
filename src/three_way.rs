use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::progress::ProgressPhase;
use crate::scanner::Scanner;
use crate::types::{CopyResult, MergeStyle, ThreeWayEntry, ThreeWayResult, ThreeWayStats, ThreeWayStatus};
use crate::utils::hash_file;

/// Three-way directory comparator
pub struct ThreeWayComparator<'a> {
    config: &'a Config,
    scanner: Scanner,
}

impl<'a> ThreeWayComparator<'a> {
    pub fn new(config: &'a Config) -> crate::error::Result<Self> {
        let scanner = Scanner::new(&config.exclude)?;
        Ok(Self { config, scanner })
    }

    /// Perform three-way comparison
    pub fn compare(&self, progress: &ProgressPhase) -> crate::error::Result<ThreeWayResult> {
        let base_path = self.config.base.as_ref().ok_or_else(|| {
            crate::error::DiffCopyError::ThreeWayRequiresBase
        })?;

        // Scan all three directories
        let base_entries = self.scanner.scan(base_path)?;
        let ours_entries = self.scanner.scan(&self.config.source)?;
        let theirs_entries = self.scanner.scan(&self.config.target)?;

        // Build path sets
        let base_paths: HashSet<PathBuf> = base_entries
            .iter()
            .map(|e| e.relative_path.clone())
            .collect();
        let ours_paths: HashSet<PathBuf> = ours_entries
            .iter()
            .map(|e| e.relative_path.clone())
            .collect();
        let theirs_paths: HashSet<PathBuf> = theirs_entries
            .iter()
            .map(|e| e.relative_path.clone())
            .collect();

        // Get all unique paths
        let mut all_paths: HashSet<PathBuf> = HashSet::new();
        all_paths.extend(base_paths.iter().cloned());
        all_paths.extend(ours_paths.iter().cloned());
        all_paths.extend(theirs_paths.iter().cloned());

        let all_paths: Vec<PathBuf> = all_paths.into_iter().collect();

        // Compare in parallel
        let result = Arc::new(Mutex::new(ThreeWayResult::new()));
        let chunk_size = (all_paths.len() / self.config.workers).max(1);

        all_paths
            .par_chunks(chunk_size)
            .for_each(|paths| {
                for path in paths {
                    let entry = self.compare_path(
                        path,
                        base_path,
                        &base_paths,
                        &ours_paths,
                        &theirs_paths,
                    );
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
        base_path: &Path,
        base_paths: &HashSet<PathBuf>,
        ours_paths: &HashSet<PathBuf>,
        theirs_paths: &HashSet<PathBuf>,
    ) -> Option<ThreeWayEntry> {
        let in_base = base_paths.contains(relative_path);
        let in_ours = ours_paths.contains(relative_path);
        let in_theirs = theirs_paths.contains(relative_path);

        let base_full = base_path.join(relative_path);
        let ours_full = self.config.source.join(relative_path);
        let theirs_full = self.config.target.join(relative_path);

        // Determine if it's a directory
        let is_dir = base_full.is_dir() || ours_full.is_dir() || theirs_full.is_dir();

        // Skip directories for now (focus on files)
        if is_dir {
            return None;
        }

        // Get hashes
        let base_hash = if in_base { hash_file(&base_full).ok() } else { None };
        let ours_hash = if in_ours { hash_file(&ours_full).ok() } else { None };
        let theirs_hash = if in_theirs { hash_file(&theirs_full).ok() } else { None };

        // Get sizes
        let base_size = if in_base { fs::metadata(&base_full).ok().map(|m| m.len()) } else { None };
        let ours_size = if in_ours { fs::metadata(&ours_full).ok().map(|m| m.len()) } else { None };
        let theirs_size = if in_theirs { fs::metadata(&theirs_full).ok().map(|m| m.len()) } else { None };

        // Determine status
        let status = self.determine_status(
            in_base, in_ours, in_theirs,
            &base_hash, &ours_hash, &theirs_hash,
        );

        // Skip unchanged unless showing them
        if status == ThreeWayStatus::Unchanged && !self.config.show_unchanged {
            return None;
        }

        // Skip non-conflicts if conflict_only mode
        if self.config.conflict_only && !status.is_conflict() {
            return None;
        }

        let mut entry = ThreeWayEntry::new(relative_path.to_path_buf(), status, is_dir);
        entry.base_exists = in_base;
        entry.ours_exists = in_ours;
        entry.theirs_exists = in_theirs;
        entry.base_hash = base_hash;
        entry.ours_hash = ours_hash;
        entry.theirs_hash = theirs_hash;
        entry.base_size = base_size;
        entry.ours_size = ours_size;
        entry.theirs_size = theirs_size;

        Some(entry)
    }

    fn determine_status(
        &self,
        in_base: bool,
        in_ours: bool,
        in_theirs: bool,
        base_hash: &Option<String>,
        ours_hash: &Option<String>,
        theirs_hash: &Option<String>,
    ) -> ThreeWayStatus {
        match (in_base, in_ours, in_theirs) {
            // All three exist
            (true, true, true) => {
                let base_eq_ours = base_hash == ours_hash;
                let base_eq_theirs = base_hash == theirs_hash;
                let ours_eq_theirs = ours_hash == theirs_hash;

                match (base_eq_ours, base_eq_theirs, ours_eq_theirs) {
                    (true, true, true) => ThreeWayStatus::Unchanged,
                    (false, true, false) => ThreeWayStatus::OursOnly,
                    (true, false, false) => ThreeWayStatus::TheirsOnly,
                    (false, false, true) => ThreeWayStatus::BothSame,
                    (false, false, false) => ThreeWayStatus::Conflict,
                    _ => ThreeWayStatus::Unchanged,
                }
            }
            // Base exists, ours exists, theirs deleted
            (true, true, false) => {
                if base_hash == ours_hash {
                    ThreeWayStatus::DeletedTheirs
                } else {
                    ThreeWayStatus::ModifyDelete
                }
            }
            // Base exists, ours deleted, theirs exists
            (true, false, true) => {
                if base_hash == theirs_hash {
                    ThreeWayStatus::DeletedOurs
                } else {
                    ThreeWayStatus::DeleteModify
                }
            }
            // Base exists, both deleted
            (true, false, false) => ThreeWayStatus::DeletedBoth,
            // Base doesn't exist, ours added
            (false, true, false) => ThreeWayStatus::AddedOurs,
            // Base doesn't exist, theirs added
            (false, false, true) => ThreeWayStatus::AddedTheirs,
            // Base doesn't exist, both added
            (false, true, true) => {
                if ours_hash == theirs_hash {
                    ThreeWayStatus::AddedBothSame
                } else {
                    ThreeWayStatus::AddedBothDiff
                }
            }
            // None exist (shouldn't happen)
            (false, false, false) => ThreeWayStatus::Unchanged,
        }
    }

    fn calculate_stats(&self, entries: &[ThreeWayEntry]) -> ThreeWayStats {
        let mut stats = ThreeWayStats::default();

        for entry in entries {
            match entry.status {
                ThreeWayStatus::Unchanged => stats.unchanged += 1,
                ThreeWayStatus::OursOnly => stats.ours_only += 1,
                ThreeWayStatus::TheirsOnly => stats.theirs_only += 1,
                ThreeWayStatus::BothSame => stats.both_same += 1,
                ThreeWayStatus::Conflict => stats.conflict += 1,
                ThreeWayStatus::AddedOurs => stats.added_ours += 1,
                ThreeWayStatus::AddedTheirs => stats.added_theirs += 1,
                ThreeWayStatus::AddedBothSame => stats.added_both_same += 1,
                ThreeWayStatus::AddedBothDiff => stats.added_both_diff += 1,
                ThreeWayStatus::DeletedOurs => stats.deleted_ours += 1,
                ThreeWayStatus::DeletedTheirs => stats.deleted_theirs += 1,
                ThreeWayStatus::DeletedBoth => stats.deleted_both += 1,
                ThreeWayStatus::ModifyDelete => stats.modify_delete += 1,
                ThreeWayStatus::DeleteModify => stats.delete_modify += 1,
            }
        }

        stats.total_items = entries.len();

        stats
    }

    /// Copy files based on three-way comparison result
    pub fn copy(
        &self,
        result: &mut ThreeWayResult,
        progress: &ProgressPhase,
    ) -> crate::error::Result<()> {
        if self.config.dry_run {
            return Ok(());
        }

        let base_path = self.config.base.as_ref().ok_or_else(|| {
            crate::error::DiffCopyError::ThreeWayRequiresBase
        })?;

        // Create output directory
        fs::create_dir_all(&self.config.output).map_err(|e| {
            crate::error::DiffCopyError::DirectoryCreateError(
                self.config.output.clone(),
                e.to_string(),
            )
        })?;

        let copy_results = Arc::new(Mutex::new(Vec::new()));
        let chunk_size = (result.entries.len() / self.config.workers).max(1);

        result.entries
            .par_chunks(chunk_size)
            .for_each(|entries| {
                for entry in entries {
                    let copy_result = self.copy_entry(entry, base_path);
                    progress.inc();
                    progress.set_message(&entry.relative_path.to_string_lossy());

                    let mut results = copy_results.lock().unwrap();
                    results.push(copy_result);
                }
            });

        result.copy_results = Arc::try_unwrap(copy_results).unwrap().into_inner().unwrap();

        Ok(())
    }

    fn copy_entry(&self, entry: &ThreeWayEntry, base_path: &Path) -> CopyResult {
        let result = match entry.status {
            ThreeWayStatus::Unchanged => Ok(()),
            ThreeWayStatus::OursOnly => self.copy_single(entry, &self.config.source),
            ThreeWayStatus::TheirsOnly => self.copy_single(entry, &self.config.target),
            ThreeWayStatus::BothSame => self.copy_single(entry, &self.config.source),
            ThreeWayStatus::AddedOurs => self.copy_single(entry, &self.config.source),
            ThreeWayStatus::AddedTheirs => self.copy_single(entry, &self.config.target),
            ThreeWayStatus::AddedBothSame => self.copy_single(entry, &self.config.source),
            ThreeWayStatus::DeletedOurs | ThreeWayStatus::DeletedTheirs | ThreeWayStatus::DeletedBoth => Ok(()),
            ThreeWayStatus::Conflict | ThreeWayStatus::AddedBothDiff => {
                self.copy_conflict(entry, base_path)
            }
            ThreeWayStatus::ModifyDelete => {
                self.copy_modify_delete(entry, base_path)
            }
            ThreeWayStatus::DeleteModify => {
                self.copy_delete_modify(entry, base_path)
            }
        };

        CopyResult {
            path: entry.relative_path.clone(),
            success: result.is_ok(),
            error_message: result.err().map(|e| e.to_string()),
        }
    }

    fn copy_single(&self, entry: &ThreeWayEntry, source_dir: &Path) -> std::io::Result<()> {
        let source = source_dir.join(&entry.relative_path);
        let dest = self.config.output.join(&entry.relative_path);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(&source, &dest)?;
        Ok(())
    }

    fn copy_conflict(&self, entry: &ThreeWayEntry, base_path: &Path) -> std::io::Result<()> {
        match self.config.merge_style {
            MergeStyle::All => {
                // Copy all three versions with suffixes
                if entry.base_exists {
                    let source = base_path.join(&entry.relative_path);
                    let dest = self.config.output.join(format!(
                        "{}.base",
                        entry.relative_path.to_string_lossy()
                    ));
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&source, &dest)?;
                }

                if entry.ours_exists {
                    let source = self.config.source.join(&entry.relative_path);
                    let dest = self.config.output.join(format!(
                        "{}.ours",
                        entry.relative_path.to_string_lossy()
                    ));
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&source, &dest)?;
                }

                if entry.theirs_exists {
                    let source = self.config.target.join(&entry.relative_path);
                    let dest = self.config.output.join(format!(
                        "{}.theirs",
                        entry.relative_path.to_string_lossy()
                    ));
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&source, &dest)?;
                }

                Ok(())
            }
            MergeStyle::Ours => self.copy_single(entry, &self.config.source),
            MergeStyle::Theirs => self.copy_single(entry, &self.config.target),
        }
    }

    fn copy_modify_delete(&self, entry: &ThreeWayEntry, base_path: &Path) -> std::io::Result<()> {
        match self.config.merge_style {
            MergeStyle::All => {
                // Copy base and ours
                let base_source = base_path.join(&entry.relative_path);
                let base_dest = self.config.output.join(format!(
                    "{}.base",
                    entry.relative_path.to_string_lossy()
                ));
                if let Some(parent) = base_dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&base_source, &base_dest)?;

                let ours_source = self.config.source.join(&entry.relative_path);
                let ours_dest = self.config.output.join(format!(
                    "{}.ours",
                    entry.relative_path.to_string_lossy()
                ));
                fs::copy(&ours_source, &ours_dest)?;

                Ok(())
            }
            MergeStyle::Ours => self.copy_single(entry, &self.config.source),
            MergeStyle::Theirs => Ok(()), // Theirs deleted it
        }
    }

    fn copy_delete_modify(&self, entry: &ThreeWayEntry, base_path: &Path) -> std::io::Result<()> {
        match self.config.merge_style {
            MergeStyle::All => {
                // Copy base and theirs
                let base_source = base_path.join(&entry.relative_path);
                let base_dest = self.config.output.join(format!(
                    "{}.base",
                    entry.relative_path.to_string_lossy()
                ));
                if let Some(parent) = base_dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&base_source, &base_dest)?;

                let theirs_source = self.config.target.join(&entry.relative_path);
                let theirs_dest = self.config.output.join(format!(
                    "{}.theirs",
                    entry.relative_path.to_string_lossy()
                ));
                fs::copy(&theirs_source, &theirs_dest)?;

                Ok(())
            }
            MergeStyle::Ours => Ok(()), // Ours deleted it
            MergeStyle::Theirs => self.copy_single(entry, &self.config.target),
        }
    }
}
