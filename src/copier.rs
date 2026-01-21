use filetime::{set_file_mtime, FileTime};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::progress::ProgressPhase;
use crate::types::{ComparisonResult, CopyResult, FileEntry, FileStatus};

/// File copier with various options
pub struct Copier<'a> {
    config: &'a Config,
}

impl<'a> Copier<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Copy files based on comparison result
    pub fn copy(
        &self,
        result: &mut ComparisonResult,
        progress: &ProgressPhase,
    ) -> crate::error::Result<()> {
        if self.config.dry_run {
            return Ok(());
        }

        // Create output directory
        fs::create_dir_all(&self.config.output).map_err(|e| {
            crate::error::DiffCopyError::DirectoryCreateError(
                self.config.output.clone(),
                e.to_string(),
            )
        })?;

        // Collect files to copy
        let entries_to_copy: Vec<&FileEntry> = result
            .entries
            .iter()
            .filter(|e| self.should_copy(e))
            .collect();

        // Copy in parallel
        let copy_results = Arc::new(Mutex::new(Vec::new()));
        let chunk_size = (entries_to_copy.len() / self.config.workers).max(1);

        entries_to_copy
            .par_chunks(chunk_size)
            .for_each(|entries| {
                for entry in entries {
                    let copy_result = self.copy_entry(entry);
                    progress.inc();
                    progress.set_message(&entry.relative_path.to_string_lossy());

                    let mut results = copy_results.lock().unwrap();
                    results.push(copy_result);
                }
            });

        result.copy_results = Arc::try_unwrap(copy_results).unwrap().into_inner().unwrap();

        Ok(())
    }

    fn should_copy(&self, entry: &FileEntry) -> bool {
        // Apply filter if specified
        if !self.config.filter_status.matches(entry.status) {
            return false;
        }

        match entry.status {
            FileStatus::Added => true,
            FileStatus::Modified => true,
            FileStatus::Permission => true,
            // Only copy deleted FILES (not directories) - per spec, --copy-deleted only applies to files
            FileStatus::Deleted => self.config.copy_deleted && !entry.is_directory,
            _ => false,
        }
    }

    fn copy_entry(&self, entry: &FileEntry) -> CopyResult {
        let result = if entry.is_directory {
            self.copy_directory(entry)
        } else {
            match entry.status {
                FileStatus::Added | FileStatus::Permission => self.copy_file_single(entry),
                FileStatus::Modified => {
                    if self.config.both_versions {
                        self.copy_file_both_versions(entry)
                    } else {
                        self.copy_file_single(entry)
                    }
                }
                FileStatus::Deleted if self.config.copy_deleted => self.copy_file_deleted(entry),
                _ => Ok(()),
            }
        };

        CopyResult {
            path: entry.relative_path.clone(),
            success: result.is_ok(),
            error_message: result.err().map(|e| e.to_string()),
        }
    }

    fn copy_directory(&self, entry: &FileEntry) -> std::io::Result<()> {
        let dest = self.config.output.join(&entry.relative_path);
        fs::create_dir_all(&dest)?;
        Ok(())
    }

    fn copy_file_single(&self, entry: &FileEntry) -> std::io::Result<()> {
        let source = self.config.target.join(&entry.relative_path);
        let dest = self.config.output.join(&entry.relative_path);

        // Create parent directory
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy file
        fs::copy(&source, &dest)?;

        // Preserve timestamp if requested
        if self.config.preserve_timestamps {
            self.preserve_timestamp(&source, &dest)?;
        }

        Ok(())
    }

    fn copy_file_both_versions(&self, entry: &FileEntry) -> std::io::Result<()> {
        let source_old = self.config.source.join(&entry.relative_path);
        let source_new = self.config.target.join(&entry.relative_path);

        let filename = entry.relative_path.to_string_lossy();
        let dest_old = self.config.output.join(format!("{}.old", filename));
        let dest_new = self.config.output.join(format!("{}.new", filename));

        // Create parent directory
        if let Some(parent) = dest_old.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy both versions
        fs::copy(&source_old, &dest_old)?;
        fs::copy(&source_new, &dest_new)?;

        // Preserve timestamps if requested
        if self.config.preserve_timestamps {
            self.preserve_timestamp(&source_old, &dest_old)?;
            self.preserve_timestamp(&source_new, &dest_new)?;
        }

        Ok(())
    }

    fn copy_file_deleted(&self, entry: &FileEntry) -> std::io::Result<()> {
        let source = self.config.source.join(&entry.relative_path);
        let filename = entry.relative_path.to_string_lossy();
        let dest = self.config.output.join(format!("{}.deleted", filename));

        // Create parent directory
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy file
        fs::copy(&source, &dest)?;

        // Preserve timestamp if requested
        if self.config.preserve_timestamps {
            self.preserve_timestamp(&source, &dest)?;
        }

        Ok(())
    }

    fn preserve_timestamp(&self, source: &Path, dest: &Path) -> std::io::Result<()> {
        let meta = fs::metadata(source)?;
        let mtime = FileTime::from_last_modification_time(&meta);
        set_file_mtime(dest, mtime)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    use crate::types::{
        CheckPermissionsMode, ColorMode, LogLevel, MergeStyle, StatusFilter,
    };

    fn create_test_config(
        source: PathBuf,
        target: PathBuf,
        output: PathBuf,
    ) -> Config {
        Config {
            source,
            target,
            output,
            exclude: vec![],
            force: false,
            verbose: false,
            dry_run: false,
            both_versions: false,
            summary: None,
            check_permissions: CheckPermissionsMode::None,
            patch: false,
            patch_file: None,
            excel: None,
            excel_fold_level: None,
            show_unchanged: false,
            save_config: None,
            filter_status: StatusFilter::new(),
            stats_only: false,
            no_tree: false,
            no_details: false,
            copy_deleted: false,
            preserve_timestamps: false,
            workers: 1,
            temp_dir: None,
            color: ColorMode::Auto,
            log_level: LogLevel::Warn,
            three_way: false,
            base: None,
            merge_style: MergeStyle::All,
            conflict_only: false,
        }
    }

    #[test]
    fn test_copier_new() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let _copier = Copier::new(&config);
        // Just ensure it compiles and doesn't panic
    }

    #[test]
    fn test_should_copy_added() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Added, false);
        assert!(copier.should_copy(&entry));
    }

    #[test]
    fn test_should_copy_modified() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Modified, false);
        assert!(copier.should_copy(&entry));
    }

    #[test]
    fn test_should_copy_deleted_without_flag() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Deleted, false);
        assert!(!copier.should_copy(&entry));
    }

    #[test]
    fn test_should_copy_deleted_with_flag() {
        let dir = tempdir().unwrap();
        let mut config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        config.copy_deleted = true;
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Deleted, false);
        assert!(copier.should_copy(&entry));
    }

    #[test]
    fn test_should_copy_unchanged() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Unchanged, false);
        assert!(!copier.should_copy(&entry));
    }

    #[test]
    fn test_should_copy_with_filter() {
        let dir = tempdir().unwrap();
        let mut config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        config.filter_status.included.insert("modified".to_string());
        let copier = Copier::new(&config);

        let added = FileEntry::new(PathBuf::from("file1.txt"), FileStatus::Added, false);
        let modified = FileEntry::new(PathBuf::from("file2.txt"), FileStatus::Modified, false);

        // Filter only allows "modified", so "added" should not be copied
        assert!(!copier.should_copy(&added));
        assert!(copier.should_copy(&modified));
    }

    #[test]
    fn test_copy_directory() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        let config = create_test_config(source, target, output.clone());
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("subdir"), FileStatus::Added, true);

        // Create output base directory
        fs::create_dir_all(&output).unwrap();

        let result = copier.copy_directory(&entry);
        assert!(result.is_ok());
        assert!(output.join("subdir").exists());
    }

    #[test]
    fn test_copy_file_single() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&output).unwrap();

        // Create source file in target (for added files)
        fs::write(target.join("file.txt"), "content").unwrap();

        let config = create_test_config(source, target, output.clone());
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Added, false);
        let result = copier.copy_file_single(&entry);

        assert!(result.is_ok());
        assert!(output.join("file.txt").exists());
        assert_eq!(fs::read_to_string(output.join("file.txt")).unwrap(), "content");
    }

    #[test]
    fn test_copy_file_both_versions() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&output).unwrap();

        // Create old version in source, new version in target
        fs::write(source.join("file.txt"), "old content").unwrap();
        fs::write(target.join("file.txt"), "new content").unwrap();

        let mut config = create_test_config(source, target, output.clone());
        config.both_versions = true;
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Modified, false);
        let result = copier.copy_file_both_versions(&entry);

        assert!(result.is_ok());
        assert!(output.join("file.txt.old").exists());
        assert!(output.join("file.txt.new").exists());
        assert_eq!(fs::read_to_string(output.join("file.txt.old")).unwrap(), "old content");
        assert_eq!(fs::read_to_string(output.join("file.txt.new")).unwrap(), "new content");
    }

    #[test]
    fn test_copy_file_deleted() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&output).unwrap();

        // Create deleted file in source
        fs::write(source.join("deleted.txt"), "deleted content").unwrap();

        let mut config = create_test_config(source, target, output.clone());
        config.copy_deleted = true;
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("deleted.txt"), FileStatus::Deleted, false);
        let result = copier.copy_file_deleted(&entry);

        assert!(result.is_ok());
        assert!(output.join("deleted.txt.deleted").exists());
        assert_eq!(fs::read_to_string(output.join("deleted.txt.deleted")).unwrap(), "deleted content");
    }

    #[test]
    fn test_copy_with_subdirectory() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(source.join("subdir")).unwrap();
        fs::create_dir_all(target.join("subdir")).unwrap();
        fs::create_dir_all(&output).unwrap();

        // Create file in subdirectory
        fs::write(target.join("subdir/file.txt"), "content").unwrap();

        let config = create_test_config(source, target, output.clone());
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("subdir/file.txt"), FileStatus::Added, false);
        let result = copier.copy_file_single(&entry);

        assert!(result.is_ok());
        assert!(output.join("subdir/file.txt").exists());
    }

    #[test]
    fn test_preserve_timestamp() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&output).unwrap();

        // Create source file
        fs::write(target.join("file.txt"), "content").unwrap();

        let mut config = create_test_config(source, target.clone(), output.clone());
        config.preserve_timestamps = true;
        let copier = Copier::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Added, false);
        let result = copier.copy_file_single(&entry);

        assert!(result.is_ok());

        // Compare modification times
        let source_meta = fs::metadata(target.join("file.txt")).unwrap();
        let dest_meta = fs::metadata(output.join("file.txt")).unwrap();

        // The modification times should be equal when preserve_timestamps is true
        // Note: There might be slight differences due to filesystem resolution
        assert_eq!(
            source_meta.modified().unwrap(),
            dest_meta.modified().unwrap()
        );
    }
}
