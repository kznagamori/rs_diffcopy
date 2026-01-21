use rayon::prelude::*;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::progress::ProgressPhase;
use crate::types::{ComparisonResult, FileEntry, FileStatus, PatchResult};
use crate::utils::is_binary_file;

/// Patch generator for creating unified diff files
pub struct PatchGenerator<'a> {
    config: &'a Config,
}

impl<'a> PatchGenerator<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Generate patches for modified files
    pub fn generate(
        &self,
        result: &mut ComparisonResult,
        progress: &ProgressPhase,
    ) -> crate::error::Result<()> {
        if self.config.dry_run {
            return Ok(());
        }

        if !self.config.patch && self.config.patch_file.is_none() {
            return Ok(());
        }

        // Collect modified files
        let modified_entries: Vec<&FileEntry> = result
            .entries
            .iter()
            .filter(|e| e.status == FileStatus::Modified && !e.is_directory)
            .collect();

        // Generate patches in parallel
        let patch_results = Arc::new(Mutex::new(Vec::new()));
        let combined_patches = Arc::new(Mutex::new(Vec::new()));

        let chunk_size = (modified_entries.len() / self.config.workers).max(1);

        modified_entries
            .par_chunks(chunk_size)
            .for_each(|entries| {
                for entry in entries {
                    let (patch_result, patch_content) = self.generate_patch(entry);
                    progress.inc();
                    progress.set_message(&entry.relative_path.to_string_lossy());

                    let mut results = patch_results.lock().unwrap();
                    results.push(patch_result);

                    if let Some(content) = patch_content {
                        let mut combined = combined_patches.lock().unwrap();
                        combined.push(content);
                    }
                }
            });

        result.patch_results = Arc::try_unwrap(patch_results).unwrap().into_inner().unwrap();

        // Write combined patch file if requested
        if let Some(ref patch_file) = self.config.patch_file {
            let combined = Arc::try_unwrap(combined_patches)
                .unwrap()
                .into_inner()
                .unwrap();
            self.write_combined_patch(patch_file, &combined)?;
        }

        Ok(())
    }

    fn generate_patch(&self, entry: &FileEntry) -> (PatchResult, Option<String>) {
        let source_path = self.config.source.join(&entry.relative_path);
        let target_path = self.config.target.join(&entry.relative_path);

        // Check if binary
        if let Ok(true) = is_binary_file(&source_path) {
            return (
                PatchResult {
                    path: entry.relative_path.clone(),
                    generated: false,
                    skipped_binary: true,
                    error_message: None,
                },
                None,
            );
        }

        if let Ok(true) = is_binary_file(&target_path) {
            return (
                PatchResult {
                    path: entry.relative_path.clone(),
                    generated: false,
                    skipped_binary: true,
                    error_message: None,
                },
                None,
            );
        }

        // Read file contents
        let source_content = match fs::read_to_string(&source_path) {
            Ok(c) => c,
            Err(e) => {
                return (
                    PatchResult {
                        path: entry.relative_path.clone(),
                        generated: false,
                        skipped_binary: false,
                        error_message: Some(format!("Failed to read source file: {}", e)),
                    },
                    None,
                );
            }
        };

        let target_content = match fs::read_to_string(&target_path) {
            Ok(c) => c,
            Err(e) => {
                return (
                    PatchResult {
                        path: entry.relative_path.clone(),
                        generated: false,
                        skipped_binary: false,
                        error_message: Some(format!("Failed to read target file: {}", e)),
                    },
                    None,
                );
            }
        };

        // Generate unified diff
        let patch_content = self.create_unified_diff(
            &entry.relative_path.to_string_lossy(),
            &source_content,
            &target_content,
        );

        // Write individual patch file if requested
        if self.config.patch {
            let patch_path = self
                .config
                .output
                .join(format!("{}.patch", entry.relative_path.to_string_lossy()));

            if let Err(e) = self.write_patch_file(&patch_path, &patch_content) {
                return (
                    PatchResult {
                        path: entry.relative_path.clone(),
                        generated: false,
                        skipped_binary: false,
                        error_message: Some(format!("Failed to write patch file: {}", e)),
                    },
                    Some(patch_content),
                );
            }
        }

        (
            PatchResult {
                path: entry.relative_path.clone(),
                generated: true,
                skipped_binary: false,
                error_message: None,
            },
            Some(patch_content),
        )
    }

    fn create_unified_diff(&self, path: &str, old_content: &str, new_content: &str) -> String {
        let diff = TextDiff::from_lines(old_content, new_content);
        let mut output = String::new();

        // Header
        output.push_str(&format!("--- a/{}\n", path));
        output.push_str(&format!("+++ b/{}\n", path));

        // Generate hunks
        for group in diff.grouped_ops(3) {
            let first_op = &group[0];
            let last_op = &group[group.len() - 1];

            let old_start = first_op.old_range().start + 1;
            let old_count = last_op.old_range().end - first_op.old_range().start;
            let new_start = first_op.new_range().start + 1;
            let new_count = last_op.new_range().end - first_op.new_range().start;

            output.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                old_start, old_count, new_start, new_count
            ));

            for op in &group {
                for change in diff.iter_changes(op) {
                    let prefix = match change.tag() {
                        ChangeTag::Delete => "-",
                        ChangeTag::Insert => "+",
                        ChangeTag::Equal => " ",
                    };
                    output.push_str(&format!("{}{}", prefix, change.value()));
                    if !change.value().ends_with('\n') {
                        output.push('\n');
                    }
                }
            }
        }

        output
    }

    fn write_patch_file(&self, path: &Path, content: &str) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    fn write_combined_patch(&self, path: &Path, patches: &[String]) -> crate::error::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                crate::error::DiffCopyError::DirectoryCreateError(parent.to_path_buf(), e.to_string())
            })?;
        }

        let mut file = fs::File::create(path).map_err(|e| crate::error::DiffCopyError::FileWriteError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        for patch in patches {
            file.write_all(patch.as_bytes()).map_err(|e| {
                crate::error::DiffCopyError::FileWriteError {
                    path: path.to_path_buf(),
                    message: e.to_string(),
                }
            })?;
            file.write_all(b"\n").ok();
        }

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
            patch: true,
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
    fn test_patch_generator_new() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let _generator = PatchGenerator::new(&config);
    }

    #[test]
    fn test_create_unified_diff_simple() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let generator = PatchGenerator::new(&config);

        let old_content = "line1\nline2\nline3\n";
        let new_content = "line1\nmodified\nline3\n";

        let diff = generator.create_unified_diff("test.txt", old_content, new_content);

        assert!(diff.contains("--- a/test.txt"));
        assert!(diff.contains("+++ b/test.txt"));
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified"));
    }

    #[test]
    fn test_create_unified_diff_addition() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let generator = PatchGenerator::new(&config);

        let old_content = "line1\nline2\n";
        let new_content = "line1\nline2\nline3\n";

        let diff = generator.create_unified_diff("test.txt", old_content, new_content);

        assert!(diff.contains("+line3"));
    }

    #[test]
    fn test_create_unified_diff_deletion() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let generator = PatchGenerator::new(&config);

        let old_content = "line1\nline2\nline3\n";
        let new_content = "line1\nline3\n";

        let diff = generator.create_unified_diff("test.txt", old_content, new_content);

        assert!(diff.contains("-line2"));
    }

    #[test]
    fn test_create_unified_diff_empty_to_content() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let generator = PatchGenerator::new(&config);

        let old_content = "";
        let new_content = "new line\n";

        let diff = generator.create_unified_diff("test.txt", old_content, new_content);

        assert!(diff.contains("+new line"));
    }

    #[test]
    fn test_create_unified_diff_no_changes() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let generator = PatchGenerator::new(&config);

        let content = "line1\nline2\n";

        let diff = generator.create_unified_diff("test.txt", content, content);

        // No hunks should be generated for identical content
        assert!(diff.contains("--- a/test.txt"));
        assert!(diff.contains("+++ b/test.txt"));
        assert!(!diff.contains("@@"));
    }

    #[test]
    fn test_write_patch_file() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let generator = PatchGenerator::new(&config);

        let patch_content = "--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let patch_path = dir.path().join("test.patch");

        let result = generator.write_patch_file(&patch_path, patch_content);
        assert!(result.is_ok());
        assert!(patch_path.exists());
        assert_eq!(fs::read_to_string(&patch_path).unwrap(), patch_content);
    }

    #[test]
    fn test_write_patch_file_with_subdirectory() {
        let dir = tempdir().unwrap();
        let config = create_test_config(
            dir.path().join("source"),
            dir.path().join("target"),
            dir.path().join("output"),
        );
        let generator = PatchGenerator::new(&config);

        let patch_content = "diff content";
        let patch_path = dir.path().join("subdir/nested/test.patch");

        let result = generator.write_patch_file(&patch_path, patch_content);
        assert!(result.is_ok());
        assert!(patch_path.exists());
    }

    #[test]
    fn test_generate_patch_text_file() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&output).unwrap();

        // Create text files with different content
        fs::write(source.join("file.txt"), "old content\n").unwrap();
        fs::write(target.join("file.txt"), "new content\n").unwrap();

        let config = create_test_config(source, target, output.clone());
        let generator = PatchGenerator::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.txt"), FileStatus::Modified, false);
        let (result, patch_content) = generator.generate_patch(&entry);

        assert!(result.generated);
        assert!(!result.skipped_binary);
        assert!(result.error_message.is_none());
        assert!(patch_content.is_some());

        let content = patch_content.unwrap();
        assert!(content.contains("-old content"));
        assert!(content.contains("+new content"));

        // Check that individual patch file was created
        assert!(output.join("file.txt.patch").exists());
    }

    #[test]
    fn test_generate_patch_binary_file() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&output).unwrap();

        // Create binary files
        fs::write(source.join("file.bin"), &[0x00, 0x01, 0x02]).unwrap();
        fs::write(target.join("file.bin"), &[0x00, 0x01, 0x03]).unwrap();

        let config = create_test_config(source, target, output);
        let generator = PatchGenerator::new(&config);

        let entry = FileEntry::new(PathBuf::from("file.bin"), FileStatus::Modified, false);
        let (result, patch_content) = generator.generate_patch(&entry);

        assert!(!result.generated);
        assert!(result.skipped_binary);
        assert!(patch_content.is_none());
    }

    #[test]
    fn test_write_combined_patch() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&output).unwrap();

        let config = create_test_config(source, target, output);
        let generator = PatchGenerator::new(&config);

        let patches = vec![
            "--- a/file1.txt\n+++ b/file1.txt\n@@ -1 +1 @@\n-old1\n+new1\n".to_string(),
            "--- a/file2.txt\n+++ b/file2.txt\n@@ -1 +1 @@\n-old2\n+new2\n".to_string(),
        ];

        let combined_path = dir.path().join("combined.patch");
        let result = generator.write_combined_patch(&combined_path, &patches);

        assert!(result.is_ok());
        assert!(combined_path.exists());

        let content = fs::read_to_string(&combined_path).unwrap();
        assert!(content.contains("file1.txt"));
        assert!(content.contains("file2.txt"));
    }
}
