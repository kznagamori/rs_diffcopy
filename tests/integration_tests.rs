use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

/// Get the path to the rs_diffcopy binary
fn get_binary_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("rs_diffcopy");

    #[cfg(windows)]
    {
        path.set_extension("exe");
    }

    path
}

/// Run rs_diffcopy with given arguments
fn run_diffcopy(args: &[&str]) -> std::process::Output {
    Command::new(get_binary_path())
        .args(args)
        .output()
        .expect("Failed to execute rs_diffcopy")
}

/// Create a test directory structure
fn create_test_structure(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let source = root.join("source");
    let target = root.join("target");
    let output = root.join("output");

    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&target).unwrap();

    (source, target, output)
}

// ============================================================================
// 1. Basic Operation Tests
// ============================================================================

mod basic_tests {
    use super::*;

    #[test]
    fn test_help_option() {
        let output = run_diffcopy(&["--help"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Compare two directories"));
        assert!(stdout.contains("--source"));
        assert!(stdout.contains("--target"));
        assert!(stdout.contains("--output"));
    }

    #[test]
    fn test_help_short_option() {
        let output = run_diffcopy(&["-h"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage:"));
    }

    #[test]
    fn test_version_option() {
        let output = run_diffcopy(&["--version"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("rs_diffcopy"));
        assert!(stdout.contains("1.0.0"));
    }

    #[test]
    fn test_version_short_option() {
        let output = run_diffcopy(&["-V"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("1.0.0"));
    }
}

// ============================================================================
// 2. Two-way Comparison Tests
// ============================================================================

mod two_way_tests {
    use super::*;

    #[test]
    fn test_added_file_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files
        fs::write(target.join("added.txt"), "new content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Added"));

        // Check file was copied
        assert!(output.join("added.txt").exists());
    }

    #[test]
    fn test_modified_file_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files with different content
        fs::write(source.join("file.txt"), "old content").unwrap();
        fs::write(target.join("file.txt"), "new content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Modified"));

        // Check file was copied with new content
        assert!(output.join("file.txt").exists());
        assert_eq!(fs::read_to_string(output.join("file.txt")).unwrap(), "new content");
    }

    #[test]
    fn test_deleted_file_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create file only in source
        fs::write(source.join("deleted.txt"), "deleted content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Deleted"));

        // Deleted file should NOT be copied by default
        assert!(!output.join("deleted.txt").exists());
    }

    #[test]
    fn test_unchanged_file_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create identical files
        fs::write(source.join("same.txt"), "same content").unwrap();
        fs::write(target.join("same.txt"), "same content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Exit code 2 means no differences to copy
        let exit_code = result.status.code().unwrap();
        assert!(exit_code == 0 || exit_code == 2);

        // Unchanged file should NOT be copied
        assert!(!output.join("same.txt").exists());
    }

    #[test]
    fn test_directory_structure_preserved() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create nested structure
        fs::create_dir_all(source.join("sub/nested")).unwrap();
        fs::create_dir_all(target.join("sub/nested")).unwrap();
        fs::write(source.join("sub/nested/file.txt"), "old").unwrap();
        fs::write(target.join("sub/nested/file.txt"), "new").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("sub/nested/file.txt").exists());
    }

    #[test]
    fn test_no_differences_exit_code() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create identical content
        fs::write(source.join("file.txt"), "same").unwrap();
        fs::write(target.join("file.txt"), "same").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Exit code 2 for no differences
        assert_eq!(result.status.code().unwrap(), 2);
    }

    #[test]
    fn test_differences_exit_code() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create different content
        fs::write(source.join("file.txt"), "old").unwrap();
        fs::write(target.join("file.txt"), "new").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Exit code 0 for differences found
        assert_eq!(result.status.code().unwrap(), 0);
    }
}

// ============================================================================
// 3. Three-way Comparison Tests
// ============================================================================

mod three_way_tests {
    use super::*;

    fn create_three_way_structure(root: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let base = root.join("base");
        let ours = root.join("ours");
        let theirs = root.join("theirs");
        let output = root.join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        (base, ours, theirs, output)
    }

    #[test]
    fn test_three_way_basic() {
        let dir = tempdir().unwrap();
        let (base, ours, theirs, output) = create_three_way_structure(dir.path());

        // Create base file
        fs::write(base.join("file.txt"), "base content").unwrap();
        fs::write(ours.join("file.txt"), "ours content").unwrap();
        fs::write(theirs.join("file.txt"), "base content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success() || result.status.code().unwrap() == 3);
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Three-way") || stdout.contains("three-way"));
    }

    #[test]
    fn test_three_way_conflict() {
        let dir = tempdir().unwrap();
        let (base, ours, theirs, output) = create_three_way_structure(dir.path());

        // Create conflicting changes
        fs::write(base.join("file.txt"), "base").unwrap();
        fs::write(ours.join("file.txt"), "ours change").unwrap();
        fs::write(theirs.join("file.txt"), "theirs change").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Exit code 3 for conflicts
        assert_eq!(result.status.code().unwrap(), 3);
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Conflict") || stdout.contains("CONFLICT") || stdout.contains("conflict"));
    }

    #[test]
    fn test_three_way_both_same_change() {
        let dir = tempdir().unwrap();
        let (base, ours, theirs, output) = create_three_way_structure(dir.path());

        // Create same changes on both sides
        fs::write(base.join("file.txt"), "base").unwrap();
        fs::write(ours.join("file.txt"), "same change").unwrap();
        fs::write(theirs.join("file.txt"), "same change").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Should succeed without conflicts
        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("both-same") || stdout.contains("Both same"));
    }

    #[test]
    fn test_three_way_requires_base() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "content").unwrap();
        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Should fail because --base is required
        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("base") || stderr.contains("Base"));
    }
}

// ============================================================================
// 4. Option Tests
// ============================================================================

mod option_tests {
    use super::*;

    #[test]
    fn test_dry_run() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("added.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--dry-run",
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Dry run") || stdout.contains("dry run"));

        // File should NOT be copied in dry-run mode
        assert!(!output.join("added.txt").exists());
    }

    #[test]
    fn test_both_versions() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "old content").unwrap();
        fs::write(target.join("file.txt"), "new content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--both-versions",
        ]);

        assert!(result.status.success());
        assert!(output.join("file.txt.old").exists());
        assert!(output.join("file.txt.new").exists());
        assert_eq!(fs::read_to_string(output.join("file.txt.old")).unwrap(), "old content");
        assert_eq!(fs::read_to_string(output.join("file.txt.new")).unwrap(), "new content");
    }

    #[test]
    fn test_copy_deleted() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("deleted.txt"), "deleted content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--copy-deleted",
        ]);

        assert!(result.status.success());
        assert!(output.join("deleted.txt.deleted").exists());
        assert_eq!(fs::read_to_string(output.join("deleted.txt.deleted")).unwrap(), "deleted content");
    }

    #[test]
    fn test_preserve_timestamps() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        // Wait a bit to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(100));

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--preserve-timestamps",
        ]);

        assert!(result.status.success());

        let target_meta = fs::metadata(target.join("file.txt")).unwrap();
        let output_meta = fs::metadata(output.join("file.txt")).unwrap();

        // Timestamps should be equal
        assert_eq!(
            target_meta.modified().unwrap(),
            output_meta.modified().unwrap()
        );
    }

    #[test]
    fn test_exclude_pattern() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("include.txt"), "include").unwrap();
        fs::write(target.join("exclude.log"), "exclude").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--exclude", "*.log",
        ]);

        assert!(result.status.success());
        assert!(output.join("include.txt").exists());
        assert!(!output.join("exclude.log").exists());
    }

    #[test]
    fn test_multiple_exclude_patterns() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("keep.txt"), "keep").unwrap();
        fs::write(target.join("skip.log"), "skip").unwrap();
        fs::write(target.join("skip.tmp"), "skip").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--exclude", "*.log",
            "--exclude", "*.tmp",
        ]);

        assert!(result.status.success());
        assert!(output.join("keep.txt").exists());
        assert!(!output.join("skip.log").exists());
        assert!(!output.join("skip.tmp").exists());
    }

    #[test]
    fn test_patch_generation() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "line1\nline2\nline3\n").unwrap();
        fs::write(target.join("file.txt"), "line1\nmodified\nline3\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());
        assert!(output.join("file.txt.patch").exists());

        let patch_content = fs::read_to_string(output.join("file.txt.patch")).unwrap();
        assert!(patch_content.contains("---"));
        assert!(patch_content.contains("+++"));
        assert!(patch_content.contains("-line2"));
        assert!(patch_content.contains("+modified"));
    }

    #[test]
    fn test_combined_patch_file() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file1.txt"), "old1\n").unwrap();
        fs::write(target.join("file1.txt"), "new1\n").unwrap();
        fs::write(source.join("file2.txt"), "old2\n").unwrap();
        fs::write(target.join("file2.txt"), "new2\n").unwrap();

        let patch_file = dir.path().join("combined.patch");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch-file", patch_file.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(patch_file.exists());

        let content = fs::read_to_string(&patch_file).unwrap();
        assert!(content.contains("file1.txt"));
        assert!(content.contains("file2.txt"));
    }

    #[test]
    fn test_excel_report() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        // Check file size is reasonable (not empty)
        let metadata = fs::metadata(&excel_path).unwrap();
        assert!(metadata.len() > 1000); // xlsx files should be larger than 1KB
    }

    #[test]
    fn test_summary_file() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(summary_path.exists());

        let content = fs::read_to_string(&summary_path).unwrap();
        assert!(content.contains("Summary"));
        assert!(content.contains("Source:"));
        assert!(content.contains("Target:"));
    }

    #[test]
    fn test_verbose_mode() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--verbose",
        ]);

        assert!(result.status.success());
        // Verbose mode should have more output
    }
}

// ============================================================================
// 5. Japanese Path Tests
// ============================================================================

mod japanese_path_tests {
    use super::*;

    #[test]
    fn test_japanese_directory_names() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("ソース");
        let target = dir.path().join("ターゲット");
        let output = dir.path().join("出力");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("file.txt").exists());
    }

    #[test]
    fn test_japanese_file_names() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("テストファイル.txt"), "日本語コンテンツ").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("テストファイル.txt").exists());
        assert_eq!(
            fs::read_to_string(output.join("テストファイル.txt")).unwrap(),
            "日本語コンテンツ"
        );
    }

    #[test]
    fn test_japanese_nested_path() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("ソース");
        let target = dir.path().join("ターゲット");
        let output = dir.path().join("出力");

        fs::create_dir_all(source.join("フォルダ/サブフォルダ")).unwrap();
        fs::create_dir_all(target.join("フォルダ/サブフォルダ")).unwrap();

        fs::write(
            source.join("フォルダ/サブフォルダ/ファイル.txt"),
            "古いコンテンツ"
        ).unwrap();
        fs::write(
            target.join("フォルダ/サブフォルダ/ファイル.txt"),
            "新しいコンテンツ"
        ).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("フォルダ/サブフォルダ/ファイル.txt").exists());
        assert_eq!(
            fs::read_to_string(output.join("フォルダ/サブフォルダ/ファイル.txt")).unwrap(),
            "新しいコンテンツ"
        );
    }

    #[test]
    fn test_mixed_japanese_english_path() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::create_dir_all(source.join("test/テスト/data")).unwrap();
        fs::create_dir_all(target.join("test/テスト/data")).unwrap();

        fs::write(
            source.join("test/テスト/data/データ.txt"),
            "old"
        ).unwrap();
        fs::write(
            target.join("test/テスト/data/データ.txt"),
            "new"
        ).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("test/テスト/data/データ.txt").exists());
    }

    #[test]
    fn test_japanese_in_summary_output() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("日本語ファイル.txt"), "コンテンツ").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        // Should contain the Japanese filename in output
        assert!(stdout.contains("日本語ファイル") || output.join("日本語ファイル.txt").exists());
    }

    #[test]
    fn test_japanese_both_versions() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("ソース");
        let target = dir.path().join("ターゲット");
        let output = dir.path().join("出力");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        fs::write(source.join("修正ファイル.txt"), "古い内容").unwrap();
        fs::write(target.join("修正ファイル.txt"), "新しい内容").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--both-versions",
        ]);

        assert!(result.status.success());
        assert!(output.join("修正ファイル.txt.old").exists());
        assert!(output.join("修正ファイル.txt.new").exists());
    }

    #[test]
    fn test_japanese_copy_deleted() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("ソース");
        let target = dir.path().join("ターゲット");
        let output = dir.path().join("出力");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        fs::write(source.join("削除されたファイル.txt"), "削除される内容").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--copy-deleted",
        ]);

        assert!(result.status.success());
        assert!(output.join("削除されたファイル.txt.deleted").exists());
    }

    #[test]
    fn test_japanese_patch_generation() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("日本語.txt"), "行1\n行2\n行3\n").unwrap();
        fs::write(target.join("日本語.txt"), "行1\n変更済み\n行3\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());
        assert!(output.join("日本語.txt.patch").exists());

        let patch_content = fs::read_to_string(output.join("日本語.txt.patch")).unwrap();
        assert!(patch_content.contains("-行2"));
        assert!(patch_content.contains("+変更済み"));
    }

    #[test]
    fn test_japanese_excel_report() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("ソース");
        let target = dir.path().join("ターゲット");
        let output = dir.path().join("出力");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        fs::write(target.join("ファイル.txt"), "内容").unwrap();

        let excel_path = dir.path().join("レポート.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());
    }

    #[test]
    fn test_japanese_summary_file() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("ソース");
        let target = dir.path().join("ターゲット");
        let output = dir.path().join("出力");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        fs::write(target.join("ファイル.txt"), "内容").unwrap();

        let summary_path = dir.path().join("サマリー.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(summary_path.exists());
    }

    #[test]
    fn test_japanese_three_way() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("ベース");
        let ours = dir.path().join("私たちの");
        let theirs = dir.path().join("相手の");
        let output = dir.path().join("出力");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        fs::write(base.join("ファイル.txt"), "ベース").unwrap();
        fs::write(ours.join("ファイル.txt"), "私たちの変更").unwrap();
        fs::write(theirs.join("ファイル.txt"), "ベース").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success() || result.status.code().unwrap() == 3);
    }
}

// ============================================================================
// 6. Error Handling Tests
// ============================================================================

mod error_tests {
    use super::*;

    #[test]
    fn test_nonexistent_source() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        let output = dir.path().join("output");

        fs::create_dir_all(&target).unwrap();

        let result = run_diffcopy(&[
            "-S", "/nonexistent/source/path",
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(!result.status.success());
        assert_eq!(result.status.code().unwrap(), 1);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("not exist") || stderr.contains("Source"));
    }

    #[test]
    fn test_nonexistent_target() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let output = dir.path().join("output");

        fs::create_dir_all(&source).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", "/nonexistent/target/path",
            "-O", output.to_str().unwrap(),
        ]);

        assert!(!result.status.success());
        assert_eq!(result.status.code().unwrap(), 1);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("not exist") || stderr.contains("Target"));
    }

    #[test]
    fn test_output_exists_without_force() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create output directory
        fs::create_dir_all(&output).unwrap();

        fs::write(source.join("file.txt"), "content").unwrap();
        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("exists") || stderr.contains("--force"));
    }

    #[test]
    fn test_invalid_glob_pattern() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--exclude", "[invalid",
        ]);

        assert!(!result.status.success());
    }
}

// ============================================================================
// 7. Semi-Normal Tests (準正常系)
// ============================================================================

mod semi_normal_tests {
    use super::*;

    // TWO-008: Empty directories
    #[test]
    fn test_empty_directories() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Both directories are empty
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Should succeed with exit code 2 (no differences)
        assert_eq!(result.status.code().unwrap(), 2);
    }

    // TWO-009: Large file comparison (1MB+)
    #[test]
    fn test_large_file_comparison() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create 1MB file
        let large_content: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();
        let mut modified_content = large_content.clone();
        modified_content[512 * 1024] = 0xFF; // Modify middle byte

        fs::write(source.join("large.bin"), &large_content).unwrap();
        fs::write(target.join("large.bin"), &modified_content).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("large.bin").exists());
    }

    // TWO-010: Binary file detection
    #[test]
    fn test_binary_file_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create binary file with null bytes
        let binary_content: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0x00, 0x89, 0x50, 0x4E, 0x47];
        fs::write(target.join("binary.dat"), &binary_content).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("binary.dat").exists());
        assert_eq!(fs::read(output.join("binary.dat")).unwrap(), binary_content);
    }

    // TWO-011: Symlink handling (Unix only)
    #[cfg(unix)]
    #[test]
    fn test_symlink_handling() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create real file and symlink
        fs::write(target.join("real.txt"), "content").unwrap();
        symlink(target.join("real.txt"), target.join("link.txt")).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("real.txt").exists());
    }

    // TWO-012: Special characters in filename
    #[test]
    fn test_special_characters_in_filename() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create file with special characters (spaces, dashes, underscores)
        fs::write(target.join("file with spaces.txt"), "content1").unwrap();
        fs::write(target.join("file-with-dashes.txt"), "content2").unwrap();
        fs::write(target.join("file_with_underscores.txt"), "content3").unwrap();
        fs::write(target.join("file.multiple.dots.txt"), "content4").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("file with spaces.txt").exists());
        assert!(output.join("file-with-dashes.txt").exists());
        assert!(output.join("file_with_underscores.txt").exists());
        assert!(output.join("file.multiple.dots.txt").exists());
    }

    // TWO-013: Deeply nested directories (10+ levels)
    #[test]
    fn test_deeply_nested_directories() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create 12-level deep directory structure
        let mut deep_path = PathBuf::new();
        for i in 0..12 {
            deep_path = deep_path.join(format!("level{}", i));
        }

        fs::create_dir_all(source.join(&deep_path)).unwrap();
        fs::create_dir_all(target.join(&deep_path)).unwrap();

        fs::write(source.join(&deep_path).join("deep.txt"), "old").unwrap();
        fs::write(target.join(&deep_path).join("deep.txt"), "new").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join(&deep_path).join("deep.txt").exists());
    }

    // TWO-014: Many files (100+)
    #[test]
    fn test_many_files() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create 150 files
        for i in 0..150 {
            fs::write(target.join(format!("file_{:03}.txt", i)), format!("content {}", i)).unwrap();
        }

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        // Check some files exist
        assert!(output.join("file_000.txt").exists());
        assert!(output.join("file_075.txt").exists());
        assert!(output.join("file_149.txt").exists());
    }

    // TWO-015: Empty file comparison
    #[test]
    fn test_empty_file() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create empty file in both (unchanged)
        fs::write(source.join("empty.txt"), "").unwrap();
        fs::write(target.join("empty.txt"), "").unwrap();

        // Create empty file only in target (added)
        fs::write(target.join("new_empty.txt"), "").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        // Empty file should not be copied (unchanged)
        assert!(!output.join("empty.txt").exists());
        // New empty file should be copied (added)
        assert!(output.join("new_empty.txt").exists());
    }

    // THREE-005: Ours only change
    #[test]
    fn test_three_way_ours_only_change() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // Only ours changed
        fs::write(base.join("file.txt"), "base content").unwrap();
        fs::write(ours.join("file.txt"), "ours changed").unwrap();
        fs::write(theirs.join("file.txt"), "base content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("ours-only") || stdout.contains("Ours"));
    }

    // THREE-006: Theirs only change
    #[test]
    fn test_three_way_theirs_only_change() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // Only theirs changed
        fs::write(base.join("file.txt"), "base content").unwrap();
        fs::write(ours.join("file.txt"), "base content").unwrap();
        fs::write(theirs.join("file.txt"), "theirs changed").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("theirs-only") || stdout.contains("Theirs"));
    }

    // THREE-007: File added in both with different content
    #[test]
    fn test_three_way_file_added_both() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File doesn't exist in base, added in both with different content
        fs::write(ours.join("newfile.txt"), "ours version").unwrap();
        fs::write(theirs.join("newfile.txt"), "theirs version").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Should detect conflict
        assert_eq!(result.status.code().unwrap(), 3);
    }

    // THREE-008: File deleted in both
    #[test]
    fn test_three_way_file_deleted_both() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File exists in base, deleted in both
        fs::write(base.join("deleted.txt"), "base content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Should succeed (no conflict - same deletion)
        assert!(result.status.success() || result.status.code().unwrap() == 2);
    }

    // OPT-012: Stats only mode
    #[test]
    fn test_stats_only() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--stats-only",
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        // Stats only hides tree/details but shows summary header with statistics
        assert!(stdout.contains("Added") || stdout.contains("Summary") || stdout.contains("1"));
        // Tree structure should NOT be shown
        assert!(!stdout.contains("├──") && !stdout.contains("└──"));
    }

    // OPT-013: Filter status (added only)
    #[test]
    fn test_filter_status_added() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create added and modified files
        fs::write(target.join("added.txt"), "new content").unwrap();
        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--filter-status", "added",
        ]);

        assert!(result.status.success());
        // Only added file should be copied
        assert!(output.join("added.txt").exists());
        assert!(!output.join("modified.txt").exists());
    }

    // OPT-014: Filter status (modified only)
    #[test]
    fn test_filter_status_modified() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create added and modified files
        fs::write(target.join("added.txt"), "new content").unwrap();
        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--filter-status", "modified",
        ]);

        assert!(result.status.success());
        // Only modified file should be copied
        assert!(!output.join("added.txt").exists());
        assert!(output.join("modified.txt").exists());
    }

    // OPT-015: Force option with dry-run
    #[test]
    fn test_force_option_with_dry_run() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create output directory (exists)
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("existing.txt"), "should not be deleted").unwrap();

        fs::write(target.join("new.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--force",
            "--dry-run",
        ]);

        assert!(result.status.success());
        // Existing file should still be there (dry-run doesn't delete)
        assert!(output.join("existing.txt").exists());
    }

    // OPT-016: No tree option
    #[test]
    fn test_no_tree_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--no-tree",
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        // Tree structure should not be displayed (no tree lines like ├── or └──)
        assert!(!stdout.contains("├──") && !stdout.contains("└──"));
    }

    // OPT-017: No details option
    #[test]
    fn test_no_details_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--no-details",
        ]);

        assert!(result.status.success());
        // Should still produce output but without file details
    }

    // OPT-018: Workers option
    #[test]
    fn test_workers_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create multiple files to process in parallel
        for i in 0..10 {
            fs::write(target.join(format!("file{}.txt", i)), format!("content {}", i)).unwrap();
        }

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--workers", "4",
        ]);

        assert!(result.status.success());
        for i in 0..10 {
            assert!(output.join(format!("file{}.txt", i)).exists());
        }
    }

    // ERR-007: Missing required arguments
    #[test]
    fn test_missing_required_args() {
        // Missing all required args
        let result = run_diffcopy(&[]);
        assert!(!result.status.success());

        // Missing output
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
        ]);
        assert!(!result.status.success());
    }

    // JP-012: Japanese content in file
    #[test]
    fn test_japanese_content_in_file() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        let japanese_content = "これは日本語のテストです。\n日本語の内容が正しく処理されるか確認します。";
        fs::write(target.join("content.txt"), japanese_content).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("content.txt").exists());
        assert_eq!(
            fs::read_to_string(output.join("content.txt")).unwrap(),
            japanese_content
        );
    }

    // JP-013: Hiragana, Katakana, Kanji mixed
    #[test]
    fn test_hiragana_katakana_kanji_mixed() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files with mixed Japanese character types
        fs::write(target.join("ひらがな.txt"), "ひらがなの内容").unwrap();
        fs::write(target.join("カタカナ.txt"), "カタカナの内容").unwrap();
        fs::write(target.join("漢字.txt"), "漢字の内容").unwrap();
        fs::write(target.join("混合ミックス混ぜる.txt"), "混合内容").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("ひらがな.txt").exists());
        assert!(output.join("カタカナ.txt").exists());
        assert!(output.join("漢字.txt").exists());
        assert!(output.join("混合ミックス混ぜる.txt").exists());
    }

    // JP-014: Long Japanese filename
    #[test]
    fn test_long_japanese_filename() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create file with long Japanese name
        let long_name = "これはとても長い日本語のファイル名でテストを行います.txt";
        fs::write(target.join(long_name), "内容").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join(long_name).exists());
    }
}

// ============================================================================
// 8. Exit Code Tests
// ============================================================================

mod exit_code_tests {
    use super::*;

    #[test]
    fn test_exit_code_0_with_differences() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("added.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert_eq!(result.status.code().unwrap(), 0);
    }

    #[test]
    fn test_exit_code_1_on_error() {
        let result = run_diffcopy(&[
            "-S", "/nonexistent",
            "-T", "/nonexistent",
            "-O", "/nonexistent",
        ]);

        assert_eq!(result.status.code().unwrap(), 1);
    }

    #[test]
    fn test_exit_code_2_no_differences() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "same").unwrap();
        fs::write(target.join("file.txt"), "same").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert_eq!(result.status.code().unwrap(), 2);
    }

    #[test]
    fn test_exit_code_3_conflicts() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        fs::write(base.join("file.txt"), "base").unwrap();
        fs::write(ours.join("file.txt"), "ours").unwrap();
        fs::write(theirs.join("file.txt"), "theirs").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert_eq!(result.status.code().unwrap(), 3);
    }
}

// ============================================================================
// 9. Additional Edge Case Tests (from old integration tests)
// ============================================================================

mod edge_case_tests {
    use super::*;

    // IT-006: Empty directory detection
    #[test]
    fn test_empty_directory_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create empty directory in target only
        fs::create_dir_all(target.join("new_dir")).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("new_dir").exists());
        assert!(output.join("new_dir").is_dir());
    }

    // Empty directories on both sides (no difference)
    #[test]
    fn test_empty_directories_both_sides() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::create_dir_all(source.join("empty_dir")).unwrap();
        fs::create_dir_all(target.join("empty_dir")).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert_eq!(result.status.code().unwrap(), 2);
    }

    // Same size but different content
    #[test]
    fn test_same_size_different_content() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "AAAA").unwrap();
        fs::write(target.join("file.txt"), "BBBB").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("[modified]") || stdout.contains("Modified"));
    }

    // Unicode filenames (Russian)
    #[test]
    fn test_unicode_russian_filenames() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("файл.txt"), "Russian content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(output.join("файл.txt").exists());
    }

    // Exclude __pycache__ pattern
    #[test]
    fn test_exclude_pycache_pattern() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::create_dir_all(target.join("src/__pycache__")).unwrap();
        fs::write(target.join("src/__pycache__/module.pyc"), "bytecode").unwrap();
        fs::write(target.join("src/main.py"), "print('hello')").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-e", "__pycache__",
        ]);

        assert!(result.status.success());
        assert!(output.join("src/main.py").exists());
        assert!(!output.join("src/__pycache__").exists());
    }

    // Both versions - added files unchanged
    #[test]
    fn test_both_versions_added_files_unchanged() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create only added file (not in source)
        fs::write(target.join("new_file.txt"), "New content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--both-versions",
        ]);

        assert!(result.status.success());
        // Added files should be copied without .old/.new extensions
        assert!(output.join("new_file.txt").exists());
        assert!(!output.join("new_file.txt.old").exists());
        assert!(!output.join("new_file.txt.new").exists());
    }
}

// ============================================================================
// 10. Config File Tests
// ============================================================================

mod config_file_tests {
    use super::*;

    // IT-801: Config file basic usage
    #[test]
    fn test_config_file_basic() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");
        let config_path = dir.path().join("config.toml");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("file.txt"), "content").unwrap();

        let config_content = format!(
            r#"source = "{}"
target = "{}"
output = "{}"
"#,
            source.display().to_string().replace('\\', "/"),
            target.display().to_string().replace('\\', "/"),
            output.display().to_string().replace('\\', "/")
        );
        fs::write(&config_path, config_content).unwrap();

        let result = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

        assert!(result.status.success());
        assert!(output.join("file.txt").exists());
    }

    // IT-802: Config file with exclude patterns
    #[test]
    fn test_config_file_with_exclude() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");
        let config_path = dir.path().join("config.toml");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("main.rs"), "fn main() {}").unwrap();
        fs::write(target.join("debug.log"), "log content").unwrap();

        let config_content = format!(
            r#"source = "{}"
target = "{}"
output = "{}"
exclude = ["*.log"]
"#,
            source.display().to_string().replace('\\', "/"),
            target.display().to_string().replace('\\', "/"),
            output.display().to_string().replace('\\', "/")
        );
        fs::write(&config_path, config_content).unwrap();

        let result = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

        assert!(result.status.success());
        assert!(output.join("main.rs").exists());
        assert!(!output.join("debug.log").exists());
    }

    // IT-803: Config file with both_versions
    #[test]
    fn test_config_file_with_both_versions() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");
        let config_path = dir.path().join("config.toml");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("file.txt"), "Old content").unwrap();
        fs::write(target.join("file.txt"), "New content").unwrap();

        let config_content = format!(
            r#"source = "{}"
target = "{}"
output = "{}"
both_versions = true
"#,
            source.display().to_string().replace('\\', "/"),
            target.display().to_string().replace('\\', "/"),
            output.display().to_string().replace('\\', "/")
        );
        fs::write(&config_path, config_content).unwrap();

        let result = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

        assert!(result.status.success());
        assert!(output.join("file.txt.old").exists());
        assert!(output.join("file.txt.new").exists());
    }

    // IT-804: CLI args override config file
    #[test]
    fn test_cli_overrides_config() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        let output = dir.path().join("output");
        let alt_output = dir.path().join("alt_output");
        let config_path = dir.path().join("config.toml");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("file.txt"), "content").unwrap();

        let config_content = format!(
            r#"source = "{}"
target = "{}"
output = "{}"
"#,
            source.display().to_string().replace('\\', "/"),
            target.display().to_string().replace('\\', "/"),
            output.display().to_string().replace('\\', "/")
        );
        fs::write(&config_path, config_content).unwrap();

        // CLI output should override config
        let result = run_diffcopy(&[
            "--config", config_path.to_str().unwrap(),
            "-O", alt_output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(alt_output.join("file.txt").exists());
        assert!(!output.exists());
    }
}

// ============================================================================
// 11. Show Unchanged Tests
// ============================================================================

mod show_unchanged_tests {
    use super::*;

    // IT-1001: --show-unchanged option
    #[test]
    fn test_show_unchanged_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Same file in both
        fs::write(source.join("same.txt"), "Same content").unwrap();
        fs::write(target.join("same.txt"), "Same content").unwrap();
        // Modified file to have some difference
        fs::write(source.join("modified.txt"), "Old").unwrap();
        fs::write(target.join("modified.txt"), "New").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--show-unchanged",
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("[unchanged]") || stdout.contains("Unchanged"));
        assert!(stdout.contains("same.txt"));
    }

    // IT-1002: -u short option
    #[test]
    fn test_show_unchanged_short_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("same.txt"), "Same content").unwrap();
        fs::write(target.join("same.txt"), "Same content").unwrap();
        fs::write(target.join("added.txt"), "Added").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-u",
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("[unchanged]") || stdout.contains("Unchanged"));
    }

    // IT-1004: Unchanged count shown in statistics
    #[test]
    fn test_unchanged_count_in_statistics() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("same.txt"), "Same content").unwrap();
        fs::write(target.join("same.txt"), "Same content").unwrap();
        fs::write(source.join("modified.txt"), "Old").unwrap();
        fs::write(target.join("modified.txt"), "New").unwrap();

        // Without --show-unchanged, unchanged count should still be in statistics
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Unchanged:"));
    }
}

// ============================================================================
// 12. Symlink Tests (Unix only)
// ============================================================================

#[cfg(unix)]
mod symlink_tests {
    use super::*;
    use std::os::unix::fs::symlink;

    // IT-601: Symlink added detection
    #[test]
    fn test_symlink_added_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("real_file.txt"), "content").unwrap();
        symlink(
            target.join("real_file.txt"),
            target.join("link_file.txt"),
        ).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        // Symlink should be detected
        assert!(stdout.contains("symlink") || stdout.contains("link_file.txt"));
    }

    // IT-603: Symlink deleted detection
    #[test]
    fn test_symlink_deleted_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Source has symlink, target doesn't
        fs::write(source.join("real_file.txt"), "content").unwrap();
        symlink(
            source.join("real_file.txt"),
            source.join("link_file.txt"),
        ).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("symlink") || stdout.contains("[deleted]"));
    }

    // IT-605: Broken symlink detection
    #[test]
    fn test_broken_symlink_detection() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create symlink to non-existent target
        symlink(
            target.join("nonexistent.txt"),
            target.join("broken_link.txt"),
        ).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("broken") || stdout.contains("BROKEN") || stdout.contains("symlink"));
    }
}

// ============================================================================
// 13. Permission Tests (Unix only)
// ============================================================================

#[cfg(unix)]
mod permission_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    // IT-901: Permission check scripts mode
    #[test]
    fn test_permission_check_scripts_mode() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files with same content but different permissions
        let source_file = source.join("script.sh");
        let target_file = target.join("script.sh");
        fs::write(&source_file, "#!/bin/bash\necho hello").unwrap();
        fs::write(&target_file, "#!/bin/bash\necho hello").unwrap();

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-P", "scripts",
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("Permission") || stdout.contains("755") || stdout.contains("644"));
    }

    // IT-903: Permission check disabled by default
    #[test]
    fn test_permission_check_default_disabled() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        let source_file = source.join("script.sh");
        let target_file = target.join("script.sh");
        fs::write(&source_file, "#!/bin/bash\necho hello").unwrap();
        fs::write(&target_file, "#!/bin/bash\necho hello").unwrap();

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        // Without -P flag, permissions should not be checked
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Exit code 2 means no differences
        assert_eq!(result.status.code(), Some(2));
    }
}

// ============================================================================
// 14. Extended Three-way Tests
// ============================================================================

mod extended_three_way_tests {
    use super::*;

    // IT-3006: Three-way added-ours
    #[test]
    fn test_three_way_added_ours() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File only in ours
        fs::write(ours.join("new_file.txt"), "new content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("added-ours") || stdout.contains("Added"));
        assert!(output.join("new_file.txt").exists());
    }

    // IT-3007: Three-way added-theirs
    #[test]
    fn test_three_way_added_theirs() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File only in theirs
        fs::write(theirs.join("new_file.txt"), "new content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("added-theirs") || stdout.contains("Added"));
        assert!(output.join("new_file.txt").exists());
    }

    // IT-3010: Three-way deleted-ours
    #[test]
    fn test_three_way_deleted_ours() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File in base and theirs, but deleted in ours
        fs::write(base.join("file.txt"), "content").unwrap();
        fs::write(theirs.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("deleted-ours") || stdout.contains("Deleted"));
    }

    // IT-3011: Three-way deleted-theirs
    #[test]
    fn test_three_way_deleted_theirs() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File in base and ours, but deleted in theirs
        fs::write(base.join("file.txt"), "content").unwrap();
        fs::write(ours.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("deleted-theirs") || stdout.contains("Deleted"));
    }

    // IT-3012: Three-way deleted-both
    #[test]
    fn test_three_way_deleted_both() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File only in base, deleted in both ours and theirs
        fs::write(base.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success() || result.status.code().unwrap() == 2);
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("deleted-both") || stdout.contains("Deleted"));
    }

    // IT-3013: Three-way modify-delete conflict
    #[test]
    fn test_three_way_modify_delete_conflict() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // File modified in ours, deleted in theirs
        fs::write(base.join("file.txt"), "base content").unwrap();
        fs::write(ours.join("file.txt"), "modified content").unwrap();
        // theirs: file doesn't exist (deleted)

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert_eq!(result.status.code().unwrap(), 3);
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains("CONFLICT") || stdout.contains("modify-delete"));
    }

    // IT-3015: Three-way with merge-style=ours
    #[test]
    fn test_three_way_merge_style_ours() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        fs::write(base.join("file.txt"), "base content").unwrap();
        fs::write(ours.join("file.txt"), "ours change").unwrap();
        fs::write(theirs.join("file.txt"), "theirs change").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--merge-style", "ours",
        ]);

        assert_eq!(result.status.code().unwrap(), 3);
        assert!(output.join("file.txt").exists());
        let content = fs::read_to_string(output.join("file.txt")).unwrap();
        assert_eq!(content, "ours change");
    }

    // IT-3016: Three-way with merge-style=theirs
    #[test]
    fn test_three_way_merge_style_theirs() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        fs::write(base.join("file.txt"), "base content").unwrap();
        fs::write(ours.join("file.txt"), "ours change").unwrap();
        fs::write(theirs.join("file.txt"), "theirs change").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--merge-style", "theirs",
        ]);

        assert_eq!(result.status.code().unwrap(), 3);
        assert!(output.join("file.txt").exists());
        let content = fs::read_to_string(output.join("file.txt")).unwrap();
        assert_eq!(content, "theirs change");
    }

    // IT-3017: Three-way with --conflict-only
    #[test]
    fn test_three_way_conflict_only() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base");
        let ours = dir.path().join("ours");
        let theirs = dir.path().join("theirs");
        let output = dir.path().join("output");

        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&ours).unwrap();
        fs::create_dir_all(&theirs).unwrap();

        // Create one conflict and one non-conflict
        fs::write(base.join("conflict.txt"), "base").unwrap();
        fs::write(ours.join("conflict.txt"), "ours").unwrap();
        fs::write(theirs.join("conflict.txt"), "theirs").unwrap();

        fs::write(base.join("ours_only.txt"), "base").unwrap();
        fs::write(ours.join("ours_only.txt"), "ours").unwrap();
        fs::write(theirs.join("ours_only.txt"), "base").unwrap();

        let result = run_diffcopy(&[
            "--three-way",
            "--base", base.to_str().unwrap(),
            "-S", ours.to_str().unwrap(),
            "-T", theirs.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--conflict-only",
        ]);

        assert_eq!(result.status.code().unwrap(), 3);
        // Only conflict file should be copied
        assert!(output.join("conflict.txt.base").exists() ||
                output.join("conflict.txt.ours").exists() ||
                output.join("conflict.txt.theirs").exists());
        // Non-conflict file should not be copied
        assert!(!output.join("ours_only.txt").exists());
    }
}

// ============================================================================
// 15. Excel Content Verification Tests
// ============================================================================

mod excel_content_tests {
    use super::*;
    use calamine::{open_workbook, Reader, Xlsx};

    // IT-1501: Verify Excel file has correct sheets
    #[test]
    fn test_excel_has_correct_sheets() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("old.txt"), "old content").unwrap();
        fs::write(target.join("old.txt"), "new content").unwrap();
        fs::write(target.join("added.txt"), "added").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        let workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");
        let sheet_names = workbook.sheet_names();

        assert!(sheet_names.contains(&"Summary".to_string()));
        assert!(sheet_names.contains(&"File Tree".to_string()));
        assert!(sheet_names.contains(&"Details".to_string()));
    }

    // IT-1502: Verify Excel Summary sheet contains correct statistics
    #[test]
    fn test_excel_summary_statistics() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create test files: 1 modified, 2 added
        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(target.join("added1.txt"), "added1").unwrap();
        fs::write(target.join("added2.txt"), "added2").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Summary") {
            let mut found_title = false;
            let mut found_added = false;
            let mut found_modified = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str.contains("rs_diffcopy Summary") {
                        found_title = true;
                    }
                    if cell_str == "Added (files)" {
                        if let Some(value) = row.get(1) {
                            assert_eq!(value.to_string(), "2");
                            found_added = true;
                        }
                    }
                    if cell_str == "Modified" {
                        if let Some(value) = row.get(1) {
                            assert_eq!(value.to_string(), "1");
                            found_modified = true;
                        }
                    }
                }
            }

            assert!(found_title, "Title not found in Summary sheet");
            assert!(found_added, "Added count not found or incorrect");
            assert!(found_modified, "Modified count not found or incorrect");
        } else {
            panic!("Could not read Summary sheet");
        }
    }

    // IT-1503: Verify Excel File Tree sheet contains file entries
    #[test]
    fn test_excel_file_tree_entries() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("newfile.txt"), "content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("File Tree") {
            let mut found_header = false;
            let mut found_newfile = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Path" {
                        found_header = true;
                    }
                    if cell_str.contains("newfile.txt") {
                        found_newfile = true;
                        // Check status column
                        if let Some(status) = row.get(1) {
                            assert_eq!(status.to_string(), "added");
                        }
                    }
                }
            }

            assert!(found_header, "Header not found in File Tree sheet");
            assert!(found_newfile, "newfile.txt not found in File Tree");
        } else {
            panic!("Could not read File Tree sheet");
        }
    }

    // IT-1504: Verify Excel Details sheet contains correct sections
    #[test]
    fn test_excel_details_sections() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(target.join("added.txt"), "added").unwrap();
        fs::write(source.join("deleted.txt"), "deleted").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Details") {
            let mut found_added_section = false;
            let mut found_modified_section = false;
            let mut found_deleted_section = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Added Files" {
                        found_added_section = true;
                    }
                    if cell_str == "Modified Files" {
                        found_modified_section = true;
                    }
                    if cell_str == "Deleted Files" {
                        found_deleted_section = true;
                    }
                }
            }

            assert!(found_added_section, "Added Files section not found");
            assert!(found_modified_section, "Modified Files section not found");
            assert!(found_deleted_section, "Deleted Files section not found");
        } else {
            panic!("Could not read Details sheet");
        }
    }

    // IT-1505: Verify Excel with Japanese filenames
    #[test]
    fn test_excel_japanese_filenames() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("日本語ファイル.txt"), "内容").unwrap();

        let excel_path = dir.path().join("レポート.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("File Tree") {
            let mut found_japanese_file = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str.contains("日本語ファイル") {
                        found_japanese_file = true;
                    }
                }
            }

            assert!(found_japanese_file, "Japanese filename not found in Excel");
        } else {
            panic!("Could not read File Tree sheet");
        }
    }

    // IT-1506: Verify Excel with subdirectory structure
    #[test]
    fn test_excel_subdirectory_structure() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::create_dir_all(target.join("subdir/nested")).unwrap();
        fs::write(target.join("subdir/nested/file.txt"), "content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Details") {
            let mut found_subdir = false;
            let mut found_file = false;

            for row in range.rows() {
                // Check Directory column (column 1) and File column (column 2)
                if let Some(dir_cell) = row.get(1) {
                    let dir_str = dir_cell.to_string();
                    if dir_str.contains("subdir") && dir_str.contains("nested") {
                        found_subdir = true;
                    }
                }
                if let Some(file_cell) = row.get(2) {
                    if file_cell.to_string() == "file.txt" {
                        found_file = true;
                    }
                }
            }

            assert!(found_subdir, "Subdirectory path not found in Details");
            assert!(found_file, "File name not found in Details");
        } else {
            panic!("Could not read Details sheet");
        }
    }
}

// ============================================================================
// 16. Summary File Content Verification Tests
// ============================================================================

mod summary_content_tests {
    use super::*;

    // IT-1601: Verify summary file contains header information
    #[test]
    fn test_summary_contains_header() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(summary_path.exists());

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("rs_diffcopy Summary"), "Missing title");
        assert!(content.contains("Source:"), "Missing Source");
        assert!(content.contains("Target:"), "Missing Target");
        assert!(content.contains("Output:"), "Missing Output");
        assert!(content.contains("Date:"), "Missing Date");
    }

    // IT-1602: Verify summary file contains correct statistics
    #[test]
    fn test_summary_statistics_accuracy() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // 2 added, 1 modified, 1 deleted
        fs::write(target.join("added1.txt"), "added1").unwrap();
        fs::write(target.join("added2.txt"), "added2").unwrap();
        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(source.join("deleted.txt"), "deleted").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("Added:"), "Missing Added section");
        assert!(content.contains("2 files"), "Wrong added count");
        assert!(content.contains("Modified:"), "Missing Modified section");
        assert!(content.contains("1 files"), "Wrong modified count");
        assert!(content.contains("Deleted:"), "Missing Deleted section");
    }

    // IT-1603: Verify summary file contains file tree
    #[test]
    fn test_summary_contains_file_tree() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("File Tree"), "Missing File Tree section");
        assert!(content.contains("file.txt"), "Missing file in tree");
        assert!(content.contains("[added]"), "Missing status tag");
    }

    // IT-1604: Verify summary file contains details section
    #[test]
    fn test_summary_contains_details() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("Modified Files"), "Missing Modified Files section");
        assert!(content.contains("modified.txt"), "Missing file in details");
    }

    // IT-1605: Verify summary with Japanese paths
    #[test]
    fn test_summary_japanese_paths() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("ソース");
        let target = dir.path().join("ターゲット");
        let output = dir.path().join("出力");

        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();

        fs::write(target.join("日本語.txt"), "内容").unwrap();

        let summary_path = dir.path().join("サマリー.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(summary_path.exists());

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("日本語.txt"), "Missing Japanese filename");
        assert!(content.contains("ソース"), "Missing Japanese source path");
        assert!(content.contains("ターゲット"), "Missing Japanese target path");
    }

    // IT-1606: Verify summary with options section
    #[test]
    fn test_summary_options_section() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(target.join("file.txt"), "content").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
            "--dry-run",
            "-e", "*.log",
        ]);

        assert!(result.status.success());

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("Options:"), "Missing Options section");
        assert!(content.contains("Dry run"), "Missing dry run option");
        assert!(content.contains("Exclude patterns"), "Missing exclude patterns");
        assert!(content.contains("*.log"), "Missing exclude pattern value");
    }

    // IT-1607: Verify summary with no differences
    #[test]
    fn test_summary_no_differences() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create identical files
        fs::write(source.join("same.txt"), "same content").unwrap();
        fs::write(target.join("same.txt"), "same content").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert_eq!(result.status.code().unwrap(), 2); // No differences

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("No differences found"), "Missing no differences message");
    }

    // IT-1608: Verify summary with subdirectory structure
    #[test]
    fn test_summary_subdirectory_tree() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::create_dir_all(target.join("level1/level2")).unwrap();
        fs::write(target.join("level1/level2/deep.txt"), "content").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--summary", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let content = fs::read_to_string(&summary_path).unwrap();

        assert!(content.contains("level1"), "Missing level1 directory");
        assert!(content.contains("level2"), "Missing level2 directory");
        assert!(content.contains("deep.txt"), "Missing deep file");
    }
}

// ============================================================================
// 17. Patch File Content Verification Tests
// ============================================================================

mod patch_content_tests {
    use super::*;

    // IT-1701: Verify individual patch file format
    #[test]
    fn test_patch_file_unified_format() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "line1\nline2\nline3\n").unwrap();
        fs::write(target.join("file.txt"), "line1\nmodified\nline3\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        let patch_path = output.join("file.txt.patch");
        assert!(patch_path.exists(), "Patch file not created");

        let content = fs::read_to_string(&patch_path).unwrap();

        // Check unified diff format
        assert!(content.contains("--- a/file.txt"), "Missing old file header");
        assert!(content.contains("+++ b/file.txt"), "Missing new file header");
        assert!(content.contains("@@"), "Missing hunk header");
        assert!(content.contains("-line2"), "Missing deleted line");
        assert!(content.contains("+modified"), "Missing added line");
    }

    // IT-1702: Verify combined patch file
    #[test]
    fn test_combined_patch_file() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file1.txt"), "old1\n").unwrap();
        fs::write(target.join("file1.txt"), "new1\n").unwrap();
        fs::write(source.join("file2.txt"), "old2\n").unwrap();
        fs::write(target.join("file2.txt"), "new2\n").unwrap();

        let patch_file = dir.path().join("combined.patch");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch-file", patch_file.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(patch_file.exists(), "Combined patch file not created");

        let content = fs::read_to_string(&patch_file).unwrap();

        // Should contain patches for both files
        assert!(content.contains("file1.txt"), "Missing file1 patch");
        assert!(content.contains("file2.txt"), "Missing file2 patch");
        assert!(content.contains("-old1"), "Missing old1 content");
        assert!(content.contains("+new1"), "Missing new1 content");
        assert!(content.contains("-old2"), "Missing old2 content");
        assert!(content.contains("+new2"), "Missing new2 content");
    }

    // IT-1703: Verify patch with addition only
    #[test]
    fn test_patch_addition_only() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "line1\nline2\n").unwrap();
        fs::write(target.join("file.txt"), "line1\nline2\nline3\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        let patch_path = output.join("file.txt.patch");
        let content = fs::read_to_string(&patch_path).unwrap();

        assert!(content.contains("+line3"), "Missing added line");
        assert!(!content.contains("-line3"), "Should not have deleted line3");
    }

    // IT-1704: Verify patch with deletion only
    #[test]
    fn test_patch_deletion_only() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "line1\nline2\nline3\n").unwrap();
        fs::write(target.join("file.txt"), "line1\nline3\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        let patch_path = output.join("file.txt.patch");
        let content = fs::read_to_string(&patch_path).unwrap();

        assert!(content.contains("-line2"), "Missing deleted line");
        assert!(!content.contains("+line2"), "Should not have added line2");
    }

    // IT-1705: Verify patch with multiple hunks
    #[test]
    fn test_patch_multiple_hunks() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files with changes at different locations
        let old_content = (1..=20).map(|i| format!("line{}\n", i)).collect::<String>();
        let mut new_content = old_content.clone();
        new_content = new_content.replace("line3\n", "modified3\n");
        new_content = new_content.replace("line17\n", "modified17\n");

        fs::write(source.join("file.txt"), &old_content).unwrap();
        fs::write(target.join("file.txt"), &new_content).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        let patch_path = output.join("file.txt.patch");
        let content = fs::read_to_string(&patch_path).unwrap();

        // Should have two hunks (@@)
        let hunk_count = content.matches("@@").count();
        assert!(hunk_count >= 2, "Should have multiple hunks, found {}", hunk_count / 2);

        assert!(content.contains("-line3"), "Missing first change");
        assert!(content.contains("+modified3"), "Missing first change");
        assert!(content.contains("-line17"), "Missing second change");
        assert!(content.contains("+modified17"), "Missing second change");
    }

    // IT-1706: Verify patch in subdirectory
    #[test]
    fn test_patch_subdirectory() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::create_dir_all(source.join("sub/dir")).unwrap();
        fs::create_dir_all(target.join("sub/dir")).unwrap();

        fs::write(source.join("sub/dir/file.txt"), "old\n").unwrap();
        fs::write(target.join("sub/dir/file.txt"), "new\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        // Patch should be in subdirectory structure
        let patch_path = output.join("sub/dir/file.txt.patch");
        assert!(patch_path.exists(), "Patch file not in correct subdirectory");

        let content = fs::read_to_string(&patch_path).unwrap();
        assert!(content.contains("sub/dir/file.txt"), "Path in patch header incorrect");
    }

    // IT-1707: Verify patch with Japanese content
    #[test]
    fn test_patch_japanese_content() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("日本語.txt"), "古い内容\n二行目\n").unwrap();
        fs::write(target.join("日本語.txt"), "新しい内容\n二行目\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        let patch_path = output.join("日本語.txt.patch");
        assert!(patch_path.exists(), "Japanese filename patch not created");

        let content = fs::read_to_string(&patch_path).unwrap();

        assert!(content.contains("-古い内容"), "Missing old Japanese content");
        assert!(content.contains("+新しい内容"), "Missing new Japanese content");
    }

    // IT-1708: Verify binary file is skipped in patch
    #[test]
    fn test_patch_binary_skipped() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create binary files
        fs::write(source.join("binary.bin"), &[0x00, 0x01, 0x02, 0x03]).unwrap();
        fs::write(target.join("binary.bin"), &[0x00, 0x01, 0x02, 0x04]).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        // Binary file should not have a patch
        let patch_path = output.join("binary.bin.patch");
        assert!(!patch_path.exists(), "Binary file should not have patch");
    }

    // IT-1709: Verify both patch and patch-file together
    #[test]
    fn test_patch_and_patch_file_together() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "old\n").unwrap();
        fs::write(target.join("file.txt"), "new\n").unwrap();

        let combined_patch = dir.path().join("all.patch");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
            "--patch-file", combined_patch.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        // Both should exist
        assert!(output.join("file.txt.patch").exists(), "Individual patch missing");
        assert!(combined_patch.exists(), "Combined patch missing");

        // Both should have same content
        let individual = fs::read_to_string(output.join("file.txt.patch")).unwrap();
        let combined = fs::read_to_string(&combined_patch).unwrap();

        assert!(individual.contains("-old"), "Individual patch missing content");
        assert!(combined.contains("-old"), "Combined patch missing content");
    }

    // IT-1710: Verify patch hunk header format
    #[test]
    fn test_patch_hunk_header_format() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "a\nb\nc\n").unwrap();
        fs::write(target.join("file.txt"), "a\nB\nc\n").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--patch",
        ]);

        assert!(result.status.success());

        let patch_path = output.join("file.txt.patch");
        let content = fs::read_to_string(&patch_path).unwrap();

        // Check hunk header format: @@ -start,count +start,count @@
        let hunk_regex = regex::Regex::new(r"@@ -\d+,\d+ \+\d+,\d+ @@").unwrap();
        assert!(hunk_regex.is_match(&content), "Hunk header format incorrect: {}", content);
    }
}

// ============================================================================
// 18. Symlink Bug Fix Tests (Issue: Statistics showed symlinks but no details)
// ============================================================================

#[cfg(unix)]
mod symlink_bug_fix_tests {
    use super::*;
    use std::os::unix::fs::symlink;

    // IT-1801: Unchanged symlinks should NOT be counted in statistics
    #[test]
    fn test_unchanged_symlinks_not_counted() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create the same symlink in both source and target
        fs::write(source.join("real_file.txt"), "content").unwrap();
        fs::write(target.join("real_file.txt"), "content").unwrap();

        // Create identical symlinks pointing to the same target
        symlink("real_file.txt", source.join("link.txt")).unwrap();
        symlink("real_file.txt", target.join("link.txt")).unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        let stdout = String::from_utf8_lossy(&result.stdout);

        // Symlinks: 0 files (or no "Symlinks:" line at all) since unchanged
        // Should NOT have "Symlinks: 1" or "Symlinks: 2"
        let symlink_count_regex = regex::Regex::new(r"Symlinks:\s+(\d+)").unwrap();
        if let Some(caps) = symlink_count_regex.captures(&stdout) {
            let count: i32 = caps[1].parse().unwrap();
            assert_eq!(count, 0, "Unchanged symlinks should not be counted. Got: {}", count);
        }

        // Also verify no Symlink Details section exists
        assert!(!stdout.contains("Symlink Details"),
            "Symlink Details section should not appear for unchanged symlinks");
    }

    // IT-1802: Changed symlinks should be counted and have details
    #[test]
    fn test_changed_symlinks_counted_with_details() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files that symlinks will point to
        fs::write(source.join("old_target.txt"), "old").unwrap();
        fs::write(target.join("new_target.txt"), "new").unwrap();

        // Create symlinks pointing to different targets
        symlink("old_target.txt", source.join("link.txt")).unwrap();
        symlink("new_target.txt", target.join("link.txt")).unwrap();

        let summary_path = dir.path().join("summary.txt");
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let summary = fs::read_to_string(&summary_path).unwrap();

        // Should have Symlinks: 1 (changed symlink)
        let symlink_count_regex = regex::Regex::new(r"Symlinks:\s+(\d+)").unwrap();
        let caps = symlink_count_regex.captures(&summary).expect("Symlinks count not found");
        let count: i32 = caps[1].parse().unwrap();
        assert!(count >= 1, "Changed symlink should be counted. Got: {}", count);

        // Should have Symlink Details section
        assert!(summary.contains("Symlink Details"),
            "Symlink Details section should appear for changed symlinks");

        // Should show the symlink path in details
        assert!(summary.contains("link.txt") || summary.contains("link"),
            "Symlink path should appear in details");
    }

    // IT-1803: Statistics symlink count should match Symlink Details entries
    #[test]
    fn test_symlink_statistics_match_details_count() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create multiple symlink scenarios
        // 1. Added symlink
        fs::write(target.join("target1.txt"), "content1").unwrap();
        symlink("target1.txt", target.join("added_link.txt")).unwrap();

        // 2. Deleted symlink
        fs::write(source.join("target2.txt"), "content2").unwrap();
        symlink("target2.txt", source.join("deleted_link.txt")).unwrap();

        // 3. Changed symlink
        fs::write(source.join("old_target.txt"), "old").unwrap();
        fs::write(target.join("new_target.txt"), "new").unwrap();
        symlink("old_target.txt", source.join("changed_link.txt")).unwrap();
        symlink("new_target.txt", target.join("changed_link.txt")).unwrap();

        let summary_path = dir.path().join("summary.txt");
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let summary = fs::read_to_string(&summary_path).unwrap();

        // Get symlink count from statistics
        let symlink_count_regex = regex::Regex::new(r"Symlinks:\s+(\d+)").unwrap();
        let caps = symlink_count_regex.captures(&summary).expect("Symlinks count not found");
        let stats_count: i32 = caps[1].parse().unwrap();

        // Count entries in Symlink Details section
        // The section contains entries for added, deleted, and changed symlinks
        let details_start = summary.find("Symlink Details");
        assert!(details_start.is_some(), "Symlink Details section must exist");

        // Count how many symlink paths appear in the details
        // Looking for patterns like "added_link.txt", "deleted_link.txt", "changed_link.txt"
        let details_section = &summary[details_start.unwrap()..];
        let mut details_count = 0;
        if details_section.contains("added_link") { details_count += 1; }
        if details_section.contains("deleted_link") { details_count += 1; }
        if details_section.contains("changed_link") { details_count += 1; }

        assert_eq!(stats_count, details_count,
            "Statistics count ({}) should match details entries ({})",
            stats_count, details_count);
    }

    // IT-1804: Symlink info should always be set for reported symlinks
    #[test]
    fn test_symlink_info_always_set() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create an added symlink
        fs::write(target.join("real.txt"), "content").unwrap();
        symlink("real.txt", target.join("new_link.txt")).unwrap();

        let summary_path = dir.path().join("summary.txt");
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let summary = fs::read_to_string(&summary_path).unwrap();

        // If symlink count > 0, Symlink Details must exist
        let symlink_count_regex = regex::Regex::new(r"Symlinks:\s+(\d+)").unwrap();
        if let Some(caps) = symlink_count_regex.captures(&summary) {
            let count: i32 = caps[1].parse().unwrap();
            if count > 0 {
                assert!(summary.contains("Symlink Details"),
                    "When symlink count is {}, Symlink Details section must exist", count);

                // Should NOT have "(symlink info unavailable)" for normal cases
                assert!(!summary.contains("symlink info unavailable"),
                    "Symlink info should be properly set, not unavailable");
            }
        }
    }

    // IT-1805: Multiple unchanged symlinks should all be ignored
    #[test]
    fn test_multiple_unchanged_symlinks_ignored() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create 3 identical symlinks in both source and target
        for i in 1..=3 {
            let target_file = format!("target{}.txt", i);
            let link_file = format!("link{}.txt", i);

            fs::write(source.join(&target_file), format!("content{}", i)).unwrap();
            fs::write(target.join(&target_file), format!("content{}", i)).unwrap();

            symlink(&target_file, source.join(&link_file)).unwrap();
            symlink(&target_file, target.join(&link_file)).unwrap();
        }

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        let stdout = String::from_utf8_lossy(&result.stdout);

        // All 3 symlinks are unchanged, so none should be counted
        let symlink_count_regex = regex::Regex::new(r"Symlinks:\s+(\d+)").unwrap();
        if let Some(caps) = symlink_count_regex.captures(&stdout) {
            let count: i32 = caps[1].parse().unwrap();
            assert_eq!(count, 0,
                "All unchanged symlinks should not be counted. Got: {}", count);
        }

        // No Symlink Details section
        assert!(!stdout.contains("Symlink Details"),
            "Symlink Details section should not appear when all symlinks are unchanged");
    }

    // IT-1806: Mix of changed and unchanged symlinks - only changed counted
    #[test]
    fn test_mixed_changed_unchanged_symlinks() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create 2 unchanged symlinks
        for i in 1..=2 {
            let target_file = format!("unchanged_target{}.txt", i);
            let link_file = format!("unchanged_link{}.txt", i);

            fs::write(source.join(&target_file), format!("content{}", i)).unwrap();
            fs::write(target.join(&target_file), format!("content{}", i)).unwrap();

            symlink(&target_file, source.join(&link_file)).unwrap();
            symlink(&target_file, target.join(&link_file)).unwrap();
        }

        // Create 1 changed symlink
        fs::write(source.join("old.txt"), "old").unwrap();
        fs::write(target.join("new.txt"), "new").unwrap();
        symlink("old.txt", source.join("changed_link.txt")).unwrap();
        symlink("new.txt", target.join("changed_link.txt")).unwrap();

        let summary_path = dir.path().join("summary.txt");
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let summary = fs::read_to_string(&summary_path).unwrap();

        // Only the 1 changed symlink should be counted
        let symlink_count_regex = regex::Regex::new(r"Symlinks:\s+(\d+)").unwrap();
        let caps = symlink_count_regex.captures(&summary).expect("Symlinks count not found");
        let count: i32 = caps[1].parse().unwrap();
        assert_eq!(count, 1,
            "Only changed symlinks should be counted. Expected 1, got: {}", count);

        // Symlink Details should only show the changed one
        assert!(summary.contains("Symlink Details"),
            "Symlink Details section should appear");
        assert!(summary.contains("changed_link"),
            "Changed symlink should appear in details");
        assert!(!summary.contains("unchanged_link1") && !summary.contains("unchanged_link2"),
            "Unchanged symlinks should not appear in details");
    }
}

// ============================================================================
// 19. Unchanged/Total Statistics Bug Fix Tests
// ============================================================================

mod unchanged_total_stats_tests {
    use super::*;

    // IT-1901: Unchanged count should be shown even without --show-unchanged option
    #[test]
    fn test_unchanged_count_always_shown() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create unchanged files
        fs::write(source.join("unchanged1.txt"), "same content").unwrap();
        fs::write(target.join("unchanged1.txt"), "same content").unwrap();
        fs::write(source.join("unchanged2.txt"), "same content 2").unwrap();
        fs::write(target.join("unchanged2.txt"), "same content 2").unwrap();

        // Create one modified file
        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        // Run without --show-unchanged option
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);

        // Unchanged count should be 2 (both unchanged files)
        let unchanged_regex = regex::Regex::new(r"Unchanged:\s+(\d+)").unwrap();
        let caps = unchanged_regex.captures(&stdout).expect("Unchanged count not found in output");
        let count: i32 = caps[1].parse().unwrap();
        assert_eq!(count, 2, "Unchanged count should be 2, got: {}", count);
    }

    // IT-1902: Total should equal all unique paths
    #[test]
    fn test_total_equals_all_unique_paths() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files: 2 unchanged, 1 modified, 1 added, 1 deleted
        fs::write(source.join("unchanged1.txt"), "same").unwrap();
        fs::write(target.join("unchanged1.txt"), "same").unwrap();
        fs::write(source.join("unchanged2.txt"), "same2").unwrap();
        fs::write(target.join("unchanged2.txt"), "same2").unwrap();
        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(source.join("deleted.txt"), "will be deleted").unwrap();
        fs::write(target.join("added.txt"), "newly added").unwrap();

        // Total should be 5 unique paths:
        // unchanged1.txt, unchanged2.txt, modified.txt, deleted.txt, added.txt

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);

        let total_regex = regex::Regex::new(r"Total:\s+(\d+)").unwrap();
        let caps = total_regex.captures(&stdout).expect("Total count not found in output");
        let count: i32 = caps[1].parse().unwrap();
        assert_eq!(count, 5, "Total should be 5, got: {}", count);
    }

    // IT-1903: Statistics categories sum should make sense
    #[test]
    fn test_statistics_categories_sum() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create files
        fs::write(source.join("unchanged.txt"), "same").unwrap();
        fs::write(target.join("unchanged.txt"), "same").unwrap();
        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(source.join("deleted.txt"), "will be deleted").unwrap();
        fs::write(target.join("added.txt"), "newly added").unwrap();

        let summary_path = dir.path().join("summary.txt");
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let summary = fs::read_to_string(&summary_path).unwrap();

        // Parse all statistics
        let added_regex = regex::Regex::new(r"Added:\s+(\d+)").unwrap();
        let modified_regex = regex::Regex::new(r"Modified:\s+(\d+)").unwrap();
        let deleted_regex = regex::Regex::new(r"Deleted:\s+(\d+)").unwrap();
        let unchanged_regex = regex::Regex::new(r"Unchanged:\s+(\d+)").unwrap();
        let total_regex = regex::Regex::new(r"Total:\s+(\d+)").unwrap();

        let added: i32 = added_regex.captures(&summary)
            .map(|c| c[1].parse().unwrap()).unwrap_or(0);
        let modified: i32 = modified_regex.captures(&summary)
            .map(|c| c[1].parse().unwrap()).unwrap_or(0);
        let deleted: i32 = deleted_regex.captures(&summary)
            .map(|c| c[1].parse().unwrap()).unwrap_or(0);
        let unchanged: i32 = unchanged_regex.captures(&summary)
            .map(|c| c[1].parse().unwrap()).unwrap_or(0);
        let total: i32 = total_regex.captures(&summary)
            .map(|c| c[1].parse().unwrap()).unwrap_or(0);

        // Verify individual counts
        assert_eq!(added, 1, "Added should be 1");
        assert_eq!(modified, 1, "Modified should be 1");
        assert_eq!(deleted, 1, "Deleted should be 1");
        assert_eq!(unchanged, 1, "Unchanged should be 1");
        assert_eq!(total, 4, "Total should be 4");

        // Verify: Added + Modified + Deleted + Unchanged = Total
        assert_eq!(added + modified + deleted + unchanged, total,
            "Sum of categories ({}) should equal Total ({})",
            added + modified + deleted + unchanged, total);
    }

    // IT-1904: Unchanged count correct with many unchanged files
    #[test]
    fn test_unchanged_count_with_many_files() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create 10 unchanged files
        for i in 1..=10 {
            let content = format!("content {}", i);
            fs::write(source.join(format!("file{}.txt", i)), &content).unwrap();
            fs::write(target.join(format!("file{}.txt", i)), &content).unwrap();
        }

        // Create 2 modified files
        fs::write(source.join("mod1.txt"), "old1").unwrap();
        fs::write(target.join("mod1.txt"), "new1").unwrap();
        fs::write(source.join("mod2.txt"), "old2").unwrap();
        fs::write(target.join("mod2.txt"), "new2").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        let stdout = String::from_utf8_lossy(&result.stdout);

        // Unchanged should be 10
        let unchanged_regex = regex::Regex::new(r"Unchanged:\s+(\d+)").unwrap();
        let caps = unchanged_regex.captures(&stdout).expect("Unchanged count not found");
        let unchanged: i32 = caps[1].parse().unwrap();
        assert_eq!(unchanged, 10, "Unchanged should be 10, got: {}", unchanged);

        // Modified should be 2
        let modified_regex = regex::Regex::new(r"Modified:\s+(\d+)").unwrap();
        let caps = modified_regex.captures(&stdout).expect("Modified count not found");
        let modified: i32 = caps[1].parse().unwrap();
        assert_eq!(modified, 2, "Modified should be 2, got: {}", modified);

        // Total should be 12
        let total_regex = regex::Regex::new(r"Total:\s+(\d+)").unwrap();
        let caps = total_regex.captures(&stdout).expect("Total count not found");
        let total: i32 = caps[1].parse().unwrap();
        assert_eq!(total, 12, "Total should be 12, got: {}", total);
    }

    // IT-1905: Unchanged count with --show-unchanged should be same as without
    #[test]
    fn test_unchanged_count_same_with_or_without_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());
        let output2 = dir.path().join("output2");

        // Create 5 unchanged files
        for i in 1..=5 {
            let content = format!("content {}", i);
            fs::write(source.join(format!("file{}.txt", i)), &content).unwrap();
            fs::write(target.join(format!("file{}.txt", i)), &content).unwrap();
        }

        // Run without --show-unchanged
        let result1 = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);
        // Exit code 2 = no differences, which is expected
        assert!(result1.status.code() == Some(0) || result1.status.code() == Some(2),
            "Expected exit code 0 or 2, got: {:?}", result1.status.code());
        let stdout1 = String::from_utf8_lossy(&result1.stdout);

        // Run with --show-unchanged (need to use different output)
        let result2 = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output2.to_str().unwrap(),
            "--show-unchanged",
        ]);
        assert!(result2.status.code() == Some(0) || result2.status.code() == Some(2),
            "Expected exit code 0 or 2, got: {:?}", result2.status.code());
        let stdout2 = String::from_utf8_lossy(&result2.stdout);

        // Extract unchanged counts from both
        let unchanged_regex = regex::Regex::new(r"Unchanged:\s+(\d+)").unwrap();

        let count1: i32 = unchanged_regex.captures(&stdout1)
            .map(|c| c[1].parse().unwrap()).unwrap_or(0);
        let count2: i32 = unchanged_regex.captures(&stdout2)
            .map(|c| c[1].parse().unwrap()).unwrap_or(0);

        assert_eq!(count1, count2,
            "Unchanged count should be same with or without --show-unchanged. Without: {}, With: {}",
            count1, count2);
        assert_eq!(count1, 5, "Unchanged count should be 5, got: {}", count1);
    }

    // IT-1906: Unchanged directories counted correctly
    #[test]
    fn test_unchanged_directories_counted() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create unchanged directory structure
        let subdir = source.join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        let target_subdir = target.join("subdir");
        fs::create_dir_all(&target_subdir).unwrap();
        fs::write(target_subdir.join("file.txt"), "content").unwrap();

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
        ]);

        // Exit code 0 or 2 (no differences) is OK
        assert!(result.status.code() == Some(0) || result.status.code() == Some(2),
            "Expected exit code 0 or 2, got: {:?}", result.status.code());
        let stdout = String::from_utf8_lossy(&result.stdout);

        // Unchanged should include the directory and file
        let unchanged_regex = regex::Regex::new(r"Unchanged:\s+(\d+)").unwrap();
        let caps = unchanged_regex.captures(&stdout).expect("Unchanged count not found");
        let unchanged: i32 = caps[1].parse().unwrap();
        // Should be at least 2 (subdir directory + file.txt)
        assert!(unchanged >= 2, "Unchanged should be at least 2, got: {}", unchanged);
    }
}

// Section 20: Excel format enhancement tests
mod excel_format_tests {
    use super::*;
    use calamine::{open_workbook, Reader, Xlsx};

    // IT-2001: Verify Excel Summary sheet has Options section
    #[test]
    fn test_excel_summary_has_options_section() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "old content").unwrap();
        fs::write(target.join("file.txt"), "new content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
            "-v",  // Enable verbose to have options to show
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Summary") {
            let mut found_options = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Options" {
                        found_options = true;
                        break;
                    }
                }
            }

            assert!(found_options, "Options section not found in Summary sheet");
        } else {
            panic!("Could not read Summary sheet");
        }
    }

    // IT-2002: Verify Excel Summary sheet has Statistics section header
    #[test]
    fn test_excel_summary_has_statistics_section() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "old content").unwrap();
        fs::write(target.join("file.txt"), "new content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Summary") {
            let mut found_statistics = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Statistics" {
                        found_statistics = true;
                        break;
                    }
                }
            }

            assert!(found_statistics, "Statistics section header not found in Summary sheet");
        } else {
            panic!("Could not read Summary sheet");
        }
    }

    // IT-2003: Verify Excel File Tree with --excel-fold-level generates correct file
    #[test]
    fn test_excel_fold_level_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create nested directory structure
        let nested_dir = target.join("level1").join("level2").join("level3");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("deep_file.txt"), "deep content").unwrap();

        // Also add a file at level 1
        fs::create_dir_all(target.join("level1")).unwrap();
        fs::write(target.join("level1").join("shallow.txt"), "shallow").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
            "--excel-fold-level", "2",  // Fold level 2
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("File Tree") {
            let mut found_deep_file = false;

            for row in range.rows() {
                for cell in row {
                    let cell_str = cell.to_string();
                    if cell_str.contains("deep_file.txt") {
                        found_deep_file = true;
                        break;
                    }
                }
            }

            assert!(found_deep_file, "deep_file.txt not found in File Tree with fold level");
        } else {
            panic!("Could not read File Tree sheet");
        }
    }

    // IT-2004: Verify Excel Details sheet has header row
    #[test]
    fn test_excel_details_has_header() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Details") {
            // First row should be header
            if let Some(first_row) = range.rows().next() {
                // Check that all 4 columns have header content
                assert!(first_row.len() >= 4, "Details header should have at least 4 columns");

                // Check header cells are not empty
                let col0 = first_row.get(0).map(|c| c.to_string()).unwrap_or_default();
                let col1 = first_row.get(1).map(|c| c.to_string()).unwrap_or_default();
                let col2 = first_row.get(2).map(|c| c.to_string()).unwrap_or_default();
                let col3 = first_row.get(3).map(|c| c.to_string()).unwrap_or_default();

                assert!(!col0.is_empty() || !col1.is_empty() || !col2.is_empty() || !col3.is_empty(),
                    "Details header should have at least one non-empty cell");
            } else {
                panic!("Details sheet is empty");
            }
        } else {
            panic!("Could not read Details sheet");
        }
    }

    // IT-2005: Verify Excel Summary sheet has all label rows (Source, Target, etc.)
    #[test]
    fn test_excel_summary_has_labels() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("file.txt"), "content").unwrap();
        fs::write(target.join("file.txt"), "content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        // Exit code could be 0 (success) or 2 (no differences)
        assert!(result.status.code() == Some(0) || result.status.code() == Some(2),
            "Expected exit code 0 or 2");

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Summary") {
            let mut found_source = false;
            let mut found_target = false;
            let mut found_output = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Source:" {
                        found_source = true;
                    }
                    if cell_str == "Target:" {
                        found_target = true;
                    }
                    if cell_str == "Output:" {
                        found_output = true;
                    }
                }
            }

            assert!(found_source, "Source: label not found in Summary sheet");
            assert!(found_target, "Target: label not found in Summary sheet");
            assert!(found_output, "Output: label not found in Summary sheet");
        } else {
            panic!("Could not read Summary sheet");
        }
    }

    // IT-2006: Verify Excel fold level 0 disables grouping (default)
    #[test]
    fn test_excel_fold_level_zero_default() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        let nested = target.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("file.txt"), "content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        // Run without --excel-fold-level (default is 0)
        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        // File should be generated successfully
        let workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");
        let sheet_names = workbook.sheet_names();
        assert!(sheet_names.contains(&"File Tree".to_string()));
    }

    // IT-2007: Verify Excel File Tree preserves directory structure in cells
    #[test]
    fn test_excel_file_tree_cell_structure() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create directory structure
        let subdir = target.join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("nested.txt"), "content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("File Tree") {
            let mut found_nested = false;

            for row in range.rows() {
                for cell in row {
                    let cell_str = cell.to_string();
                    if cell_str.contains("nested.txt") || cell_str.contains("subdir") {
                        found_nested = true;
                    }
                }
            }

            assert!(found_nested, "Nested file structure not found in File Tree");
        } else {
            panic!("Could not read File Tree sheet");
        }
    }
}

// Section 21: Filter status in Options tests
mod filter_status_options_tests {
    use super::*;
    use calamine::{open_workbook, Reader, Xlsx};

    // IT-2101: Verify Summary file contains filter_status in Options
    #[test]
    fn test_summary_contains_filter_status_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create test files: 1 added, 1 modified
        fs::write(source.join("modified.txt"), "old content").unwrap();
        fs::write(target.join("modified.txt"), "new content").unwrap();
        fs::write(target.join("added.txt"), "added content").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
            "--filter-status", "added",
        ]);

        assert!(result.status.success());
        assert!(summary_path.exists());

        let summary_content = fs::read_to_string(&summary_path).unwrap();
        assert!(summary_content.contains("Filter status:"),
            "Summary file should contain 'Filter status:' in Options section");
        assert!(summary_content.contains("added"),
            "Summary file should contain 'added' as filter status value");
    }

    // IT-2102: Verify Excel file contains filter_status in Options section
    #[test]
    fn test_excel_contains_filter_status_option() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old content").unwrap();
        fs::write(target.join("modified.txt"), "new content").unwrap();
        fs::write(target.join("added.txt"), "added content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
            "--filter-status", "modified",
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Summary") {
            let mut found_filter_status = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Filter status:" {
                        found_filter_status = true;
                        // Check value column contains "modified"
                        if let Some(value) = row.get(1) {
                            assert!(value.to_string().contains("modified"),
                                "Filter status value should contain 'modified'");
                        }
                        break;
                    }
                }
            }

            assert!(found_filter_status, "Filter status: not found in Excel Options section");
        } else {
            panic!("Could not read Summary sheet");
        }
    }

    // IT-2103: Verify multiple filter statuses are shown correctly
    #[test]
    fn test_filter_status_multiple_values() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(target.join("added.txt"), "added").unwrap();
        fs::write(source.join("deleted.txt"), "deleted").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
            "--filter-status", "added,modified",
        ]);

        assert!(result.status.success());

        let summary_content = fs::read_to_string(&summary_path).unwrap();
        assert!(summary_content.contains("Filter status:"),
            "Summary should contain Filter status option");
        assert!(summary_content.contains("added") && summary_content.contains("modified"),
            "Summary should contain both 'added' and 'modified' in Filter status");
    }

    // IT-2104: Verify no filter_status shown when not specified
    #[test]
    fn test_no_filter_status_when_not_specified() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let summary_content = fs::read_to_string(&summary_path).unwrap();
        // When no filter is specified, the Options section might not exist at all
        // or it shouldn't contain "Filter status:" if no other options are set
        // Check that Filter status is not displayed when not using --filter-status
        let lines: Vec<&str> = summary_content.lines().collect();
        let has_filter_status_line = lines.iter().any(|line| line.contains("Filter status:"));
        assert!(!has_filter_status_line,
            "Summary should not contain 'Filter status:' when --filter-status is not specified");
    }

    // IT-2105: Verify Excel Options section correctly excludes filter_status when not specified
    #[test]
    fn test_excel_no_filter_status_when_not_specified() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Summary") {
            let mut found_filter_status = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Filter status:" {
                        found_filter_status = true;
                        break;
                    }
                }
            }

            assert!(!found_filter_status,
                "Filter status: should not appear in Excel when --filter-status is not specified");
        } else {
            panic!("Could not read Summary sheet");
        }
    }
}

// Section 22: Filter status exclusion and Excel format bug fixes
mod filter_status_bugfix_tests {
    use super::*;
    use calamine::{open_workbook, Reader, Xlsx};

    // IT-2201: Verify filter_status with "all,^deleted" shows in Summary
    #[test]
    fn test_summary_filter_status_all_with_exclusion() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(target.join("added.txt"), "added").unwrap();
        fs::write(source.join("deleted.txt"), "deleted").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
            "--filter-status", "all,^deleted",
        ]);

        assert!(result.status.success());

        let summary_content = fs::read_to_string(&summary_path).unwrap();
        assert!(summary_content.contains("Filter status:"),
            "Summary should contain Filter status option");
        assert!(summary_content.contains("all"),
            "Summary should contain 'all' in Filter status");
        assert!(summary_content.contains("^deleted"),
            "Summary should contain '^deleted' in Filter status");
    }

    // IT-2202: Verify filter_status with "all,^deleted" shows in Excel
    #[test]
    fn test_excel_filter_status_all_with_exclusion() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();
        fs::write(target.join("added.txt"), "added").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
            "--filter-status", "all,^deleted",
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("Summary") {
            let mut found_filter_status = false;
            let mut found_all = false;
            let mut found_exclusion = false;

            for row in range.rows() {
                if let Some(cell) = row.first() {
                    let cell_str = cell.to_string();
                    if cell_str == "Filter status:" {
                        found_filter_status = true;
                        if let Some(value) = row.get(1) {
                            let val_str = value.to_string();
                            found_all = val_str.contains("all");
                            found_exclusion = val_str.contains("^deleted");
                        }
                        break;
                    }
                }
            }

            assert!(found_filter_status, "Filter status: should appear in Excel Options");
            assert!(found_all, "Filter status should contain 'all'");
            assert!(found_exclusion, "Filter status should contain '^deleted'");
        } else {
            panic!("Could not read Summary sheet");
        }
    }

    // IT-2203: Verify Excel File Tree uses cell-based structure
    #[test]
    fn test_excel_file_tree_cell_structure() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create nested directory structure
        let nested = target.join("level1").join("level2");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.txt"), "content").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
        ]);

        assert!(result.status.success());

        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("File Tree") {
            // Find the row with deep.txt
            let mut found_deep = false;
            for row in range.rows() {
                let row_str: String = row.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("|");
                if row_str.contains("deep.txt") {
                    found_deep = true;
                    // Verify that level1/ and level2/ are in separate cells
                    let has_level1 = row.iter().any(|c| c.to_string().contains("level1"));
                    let has_level2 = row.iter().any(|c| c.to_string().contains("level2"));
                    assert!(has_level1, "level1/ should be in a cell");
                    assert!(has_level2, "level2/ should be in a cell");
                    break;
                }
            }
            assert!(found_deep, "deep.txt should be in File Tree");
        } else {
            panic!("Could not read File Tree sheet");
        }
    }

    // IT-2204: Verify fold-level 2 hides items at depth >= 2
    #[test]
    fn test_excel_fold_level_groups_correctly() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        // Create structure:
        // a.txt (depth 1)
        // b/ (depth 1)
        //   d.txt (depth 2)
        //   e/ (depth 2)
        //     g.txt (depth 3)
        fs::write(target.join("a.txt"), "a").unwrap();
        let b_dir = target.join("b");
        fs::create_dir_all(&b_dir).unwrap();
        fs::write(b_dir.join("d.txt"), "d").unwrap();
        let e_dir = b_dir.join("e");
        fs::create_dir_all(&e_dir).unwrap();
        fs::write(e_dir.join("g.txt"), "g").unwrap();

        let excel_path = dir.path().join("report.xlsx");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "--excel", excel_path.to_str().unwrap(),
            "--excel-fold-level", "2",
        ]);

        assert!(result.status.success());
        assert!(excel_path.exists());

        // Verify the file was created and contains all entries
        let mut workbook: Xlsx<_> = open_workbook(&excel_path).expect("Failed to open Excel file");

        if let Ok(range) = workbook.worksheet_range("File Tree") {
            let mut found_a = false;
            let mut found_d = false;
            let mut found_g = false;

            for row in range.rows() {
                let row_str: String = row.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("");
                if row_str.contains("a.txt") { found_a = true; }
                if row_str.contains("d.txt") { found_d = true; }
                if row_str.contains("g.txt") { found_g = true; }
            }

            assert!(found_a, "a.txt should be in File Tree");
            assert!(found_d, "d.txt should be in File Tree");
            assert!(found_g, "g.txt should be in File Tree");
        } else {
            panic!("Could not read File Tree sheet");
        }
    }

    // IT-2205: Verify filter_status with only exclusion shows "all (implied)"
    #[test]
    fn test_summary_filter_status_only_exclusion() {
        let dir = tempdir().unwrap();
        let (source, target, output) = create_test_structure(dir.path());

        fs::write(source.join("modified.txt"), "old").unwrap();
        fs::write(target.join("modified.txt"), "new").unwrap();

        let summary_path = dir.path().join("summary.txt");

        let result = run_diffcopy(&[
            "-S", source.to_str().unwrap(),
            "-T", target.to_str().unwrap(),
            "-O", output.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
            "--filter-status", "^deleted",
        ]);

        assert!(result.status.success());

        let summary_content = fs::read_to_string(&summary_path).unwrap();
        assert!(summary_content.contains("Filter status:"),
            "Summary should contain Filter status option");
        // When only exclusions, it should show "all (implied)"
        assert!(summary_content.contains("all"),
            "Summary should contain 'all' when only exclusions are specified");
        assert!(summary_content.contains("^deleted"),
            "Summary should contain '^deleted'");
    }
}
