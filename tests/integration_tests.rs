//! Integration tests for rs_diffcopy CLI
//!
//! These tests verify the end-to-end behavior of the rs_diffcopy command.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Helper struct for test setup
struct TestEnv {
    source: TempDir,
    target: TempDir,
    output: PathBuf,
    _output_parent: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let output_parent = tempfile::tempdir().unwrap();
        let output = output_parent.path().join("output");

        TestEnv {
            source,
            target,
            output,
            _output_parent: output_parent,
        }
    }

    fn source_path(&self) -> &Path {
        self.source.path()
    }

    fn target_path(&self) -> &Path {
        self.target.path()
    }

    fn output_path(&self) -> &Path {
        &self.output
    }
}

/// Create a file with content in the specified directory
fn create_file(base: &Path, rel_path: &str, content: &str) -> PathBuf {
    let path = base.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path
}

/// Run diffcopy command and return output
fn run_diffcopy(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rs_diffcopy"))
        .args(args)
        .output()
        .expect("Failed to execute diffcopy")
}

/// Run diffcopy with source, target, output paths using explicit options
fn run_diffcopy_sto(source: &Path, target: &Path, output: &Path) -> Output {
    run_diffcopy(&[
        "-S", source.to_str().unwrap(),
        "-T", target.to_str().unwrap(),
        "-O", output.to_str().unwrap(),
    ])
}

/// Run diffcopy with source, target, output paths and additional options
fn run_diffcopy_with_opts(source: &Path, target: &Path, output: &Path, opts: &[&str]) -> Output {
    let mut args = vec![
        "-S", source.to_str().unwrap(),
        "-T", target.to_str().unwrap(),
        "-O", output.to_str().unwrap(),
    ];
    args.extend(opts);
    run_diffcopy(&args)
}

/// Get stdout as string
fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Get stderr as string
fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

// ============================================================================
// 3.1 Basic Functionality Tests
// ============================================================================

/// IT-001: Basic diff detection
#[test]
fn test_basic_diff_detection() {
    let env = TestEnv::new();

    create_file(env.source_path(), "src/main.rs", "fn main() {}");
    create_file(env.source_path(), "src/lib.rs", "pub fn hello() {}");
    create_file(env.source_path(), "old_file.txt", "old content");

    create_file(env.target_path(), "src/main.rs", "fn main() { println!(\"Hello\"); }");
    create_file(env.target_path(), "src/lib.rs", "pub fn hello() {}");
    create_file(env.target_path(), "new_file.txt", "new content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(env.output_path().join("src/main.rs").exists());
    assert!(env.output_path().join("new_file.txt").exists());
    assert!(!env.output_path().join("src/lib.rs").exists());
}

/// IT-002: No differences
#[test]
fn test_no_differences() {
    let env = TestEnv::new();

    create_file(env.source_path(), "file.txt", "same content");
    create_file(env.target_path(), "file.txt", "same content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout_str(&output).contains("No differences found."));
}

/// IT-003: Added file detection
#[test]
fn test_added_file_detection() {
    let env = TestEnv::new();

    create_file(env.target_path(), "new_file.txt", "new content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(stdout_str(&output).contains("[added]"));
    assert!(env.output_path().join("new_file.txt").exists());

    let content = fs::read_to_string(env.output_path().join("new_file.txt")).unwrap();
    assert_eq!(content, "new content");
}

/// IT-004: Modified file detection
#[test]
fn test_modified_file_detection() {
    let env = TestEnv::new();

    create_file(env.source_path(), "file.txt", "original content");
    create_file(env.target_path(), "file.txt", "modified content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(stdout_str(&output).contains("[modified]"));
    assert!(env.output_path().join("file.txt").exists());

    let content = fs::read_to_string(env.output_path().join("file.txt")).unwrap();
    assert_eq!(content, "modified content");
}

/// IT-005: Deleted file detection
#[test]
fn test_deleted_file_detection() {
    let env = TestEnv::new();

    create_file(env.source_path(), "old_file.txt", "old content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(stdout_str(&output).contains("[deleted]"));
    assert!(!env.output_path().join("old_file.txt").exists());
}

/// IT-006: Empty directory detection
#[test]
fn test_empty_directory_detection() {
    let env = TestEnv::new();

    fs::create_dir_all(env.target_path().join("new_dir")).unwrap();

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(env.output_path().join("new_dir").exists());
    assert!(env.output_path().join("new_dir").is_dir());
}

// ============================================================================
// 3.2 Option Tests
// ============================================================================

/// IT-101: --help option
#[test]
fn test_help_option() {
    let output = run_diffcopy(&["--help"]);

    assert!(output.status.success());
    let stdout = stdout_str(&output);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--source"));
    assert!(stdout.contains("--target"));
    assert!(stdout.contains("--output"));
    assert!(stdout.contains("--exclude"));
    assert!(stdout.contains("--force"));
    assert!(stdout.contains("--verbose"));
    assert!(stdout.contains("--dry-run"));
    assert!(stdout.contains("--config"));
}

/// IT-102: --version option
#[test]
fn test_version_option() {
    let output = run_diffcopy(&["--version"]);

    assert!(output.status.success());
    assert!(stdout_str(&output).contains("rs_diffcopy 1.0.0"));
}

/// IT-103: --verbose option
#[test]
fn test_verbose_option() {
    let env = TestEnv::new();

    create_file(env.target_path(), "file.txt", "content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--verbose"],
    );

    assert!(output.status.success());
    let stdout = stdout_str(&output);
    // Check for phase output or summary (verbose mode ensures processing happens)
    assert!(stdout.contains("Summary") || stdout.contains("Scanning") || stdout.contains("[added]"));
}

/// IT-104: --dry-run option
#[test]
fn test_dry_run_option() {
    let env = TestEnv::new();

    create_file(env.target_path(), "new_file.txt", "content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--dry-run"],
    );

    assert!(output.status.success());
    assert!(!env.output_path().exists());
}

/// IT-105: --force option
#[test]
fn test_force_option() {
    let env = TestEnv::new();

    fs::create_dir_all(&env.output).unwrap();
    create_file(&env.output, "existing.txt", "existing content");

    create_file(env.target_path(), "new_file.txt", "new content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--force"],
    );

    assert!(output.status.success());
    assert!(!env.output_path().join("existing.txt").exists());
    assert!(env.output_path().join("new_file.txt").exists());
}

/// IT-106: --summary option
#[test]
fn test_summary_option() {
    let env = TestEnv::new();
    let summary_path = env._output_parent.path().join("summary.txt");

    create_file(env.target_path(), "new_file.txt", "content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--summary", summary_path.to_str().unwrap()],
    );

    assert!(output.status.success());
    assert!(summary_path.exists());

    let summary = fs::read_to_string(&summary_path).unwrap();
    assert!(summary.contains("rs_diffcopy Summary"));
    assert!(summary.contains("[added]"));
}

/// IT-107: --exclude option
#[test]
fn test_exclude_option() {
    let env = TestEnv::new();

    create_file(env.target_path(), "main.rs", "fn main() {}");
    create_file(env.target_path(), "debug.log", "log content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--exclude", "*.log"],
    );

    assert!(output.status.success());
    assert!(env.output_path().join("main.rs").exists());
    assert!(!env.output_path().join("debug.log").exists());
}

/// IT-108: Multiple --exclude options
#[test]
fn test_multiple_exclude_options() {
    let env = TestEnv::new();

    create_file(env.target_path(), "main.rs", "fn main() {}");
    create_file(env.target_path(), "debug.log", "log content");
    create_file(env.target_path(), "cache.tmp", "temp content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--exclude", "*.log", "--exclude", "*.tmp"],
    );

    assert!(output.status.success());
    assert!(env.output_path().join("main.rs").exists());
    assert!(!env.output_path().join("debug.log").exists());
    assert!(!env.output_path().join("cache.tmp").exists());
}

/// IT-109: Exclude pattern matches path components (e.g., __pycache__)
#[test]
fn test_exclude_path_component() {
    let env = TestEnv::new();

    // Create nested __pycache__ directories
    fs::create_dir_all(env.target_path().join("src/__pycache__")).unwrap();
    fs::create_dir_all(env.target_path().join("lib/__pycache__")).unwrap();
    create_file(env.target_path(), "src/__pycache__/module.pyc", "bytecode");
    create_file(env.target_path(), "lib/__pycache__/util.pyc", "bytecode");
    create_file(env.target_path(), "src/main.py", "print('hello')");

    // Also test with source having __pycache__ (should be excluded from deleted)
    fs::create_dir_all(env.source_path().join("old/__pycache__")).unwrap();
    create_file(env.source_path(), "old/__pycache__/old.pyc", "old bytecode");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--exclude", "__pycache__"],
    );

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    // main.py should be added (not excluded)
    assert!(env.output_path().join("src/main.py").exists());

    // __pycache__ directories and contents should be excluded
    assert!(!env.output_path().join("src/__pycache__").exists());
    assert!(!env.output_path().join("lib/__pycache__").exists());

    // __pycache__ should appear in Exclude patterns section
    assert!(
        stdout.contains("Exclude patterns:"),
        "Exclude patterns section should be present"
    );
    assert!(
        stdout.contains("- __pycache__"),
        "Exclude pattern should be listed in summary"
    );

    // __pycache__ should NOT appear in File Tree section (properly excluded)
    let file_tree_section = stdout.split("File Tree").nth(1).unwrap_or("");
    assert!(
        !file_tree_section.contains("__pycache__"),
        "__pycache__ should not appear in File Tree but got:\n{}",
        file_tree_section
    );
}

// ============================================================================
// 3.3 Error Handling Tests
// ============================================================================

/// IT-201: Non-existent source directory
#[test]
fn test_nonexistent_source_directory() {
    let env = TestEnv::new();

    let output = run_diffcopy(&[
        "-S", "/nonexistent/source/path",
        "-T", env.target_path().to_str().unwrap(),
        "-O", env.output_path().to_str().unwrap(),
    ]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_str(&output);
    assert!(stderr.contains("does not exist") || stderr.contains("Error"));
}

/// IT-202: Non-existent target directory
#[test]
fn test_nonexistent_target_directory() {
    let env = TestEnv::new();

    let output = run_diffcopy(&[
        "-S", env.source_path().to_str().unwrap(),
        "-T", "/nonexistent/target/path",
        "-O", env.output_path().to_str().unwrap(),
    ]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_str(&output);
    assert!(stderr.contains("does not exist") || stderr.contains("Error"));
}

/// IT-203: Output directory already exists
#[test]
fn test_output_directory_exists() {
    let env = TestEnv::new();

    fs::create_dir_all(&env.output).unwrap();

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_str(&output);
    assert!(stderr.contains("already exists") || stderr.contains("--force"));
}

/// IT-203b: --force successfully deletes and recreates safe output directory
/// Note: Dangerous path detection is tested via unit tests in main.rs
/// (test_is_dangerous_path_*) to avoid any risk of accidental system damage.
#[test]
fn test_force_with_nested_output() {
    let env = TestEnv::new();

    // Create nested output directory structure
    let nested_output = env.output.join("level1").join("level2");
    fs::create_dir_all(&nested_output).unwrap();
    create_file(&nested_output, "deep_file.txt", "deep content");

    // Create a file in target to trigger diff
    create_file(env.target_path(), "new_file.txt", "new content");

    // Run with --force, should delete nested structure and recreate
    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--force"],
    );

    assert!(output.status.success());
    // Old nested structure should be gone
    assert!(!nested_output.exists());
    // New file should be copied
    assert!(env.output_path().join("new_file.txt").exists());
}

/// IT-204: Invalid exclude pattern
#[test]
fn test_invalid_exclude_pattern() {
    let env = TestEnv::new();

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--exclude", "[invalid"],
    );

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_str(&output);
    assert!(stderr.contains("Invalid") || stderr.contains("pattern") || stderr.contains("Error"));
}

/// IT-205: Missing required arguments
#[test]
fn test_missing_required_arguments() {
    let output = run_diffcopy(&[]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_str(&output);
    assert!(stderr.contains("required") || stderr.contains("--source") || stderr.contains("Error"));
}

// ============================================================================
// 3.4 Exit Code Tests
// ============================================================================

/// IT-301: Normal exit with differences
#[test]
fn test_exit_code_with_differences() {
    let env = TestEnv::new();

    create_file(env.target_path(), "new_file.txt", "content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert_eq!(output.status.code(), Some(0));
}

/// IT-302: Normal exit without differences
#[test]
fn test_exit_code_without_differences() {
    let env = TestEnv::new();

    create_file(env.source_path(), "file.txt", "same");
    create_file(env.target_path(), "file.txt", "same");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert_eq!(output.status.code(), Some(2));
}

/// IT-303: Error exit
#[test]
fn test_exit_code_on_error() {
    let output = run_diffcopy(&[
        "-S", "/nonexistent",
        "-T", "/nonexistent",
        "-O", "/output",
    ]);

    assert_eq!(output.status.code(), Some(1));
}

// ============================================================================
// 3.5 Summary Output Tests
// ============================================================================

/// IT-401: Summary header
#[test]
fn test_summary_header() {
    let env = TestEnv::new();

    create_file(env.target_path(), "file.txt", "content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    let stdout = stdout_str(&output);
    assert!(stdout.contains("rs_diffcopy Summary"));
    assert!(stdout.contains("Source:"));
    assert!(stdout.contains("Target:"));
    assert!(stdout.contains("Date:"));
}

/// IT-402: File tree output
#[test]
fn test_file_tree_output() {
    let env = TestEnv::new();

    create_file(env.target_path(), "src/main.rs", "fn main() {}");
    create_file(env.target_path(), "src/lib.rs", "pub fn hello() {}");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    let stdout = stdout_str(&output);
    assert!(stdout.contains("File Tree"));
    assert!(stdout.contains("src/") || stdout.contains("src"));
}

/// IT-403: Status tags
#[test]
fn test_status_tags() {
    let env = TestEnv::new();

    create_file(env.source_path(), "modified.txt", "original");
    create_file(env.source_path(), "deleted.txt", "to be deleted");
    create_file(env.target_path(), "modified.txt", "changed");
    create_file(env.target_path(), "added.txt", "new file");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    let stdout = stdout_str(&output);
    assert!(stdout.contains("[added]"));
    assert!(stdout.contains("[modified]"));
    assert!(stdout.contains("[deleted]"));
}

/// IT-404: Statistics
#[test]
fn test_statistics() {
    let env = TestEnv::new();

    create_file(env.target_path(), "new1.txt", "content1");
    create_file(env.target_path(), "new2.txt", "content2");
    create_file(env.source_path(), "old.txt", "old content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    let stdout = stdout_str(&output);
    assert!(stdout.contains("Added:"));
    assert!(stdout.contains("Deleted:"));
    assert!(stdout.contains("Total:"));
}

/// IT-405: Options section
#[test]
fn test_options_section() {
    let env = TestEnv::new();

    create_file(env.target_path(), "new.txt", "content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--dry-run", "-e", "*.log"],
    );

    let stdout = stdout_str(&output);
    assert!(stdout.contains("Options:"));
    assert!(stdout.contains("Dry-run"));
    assert!(stdout.contains("Exclude patterns:"));
    assert!(stdout.contains("*.log"));
}

/// IT-406: Added Files section
#[test]
fn test_added_files_section() {
    let env = TestEnv::new();

    create_file(env.target_path(), "new_file.txt", "content");
    fs::create_dir_all(env.target_path().join("new_dir")).unwrap();

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    let stdout = stdout_str(&output);
    assert!(stdout.contains("Added Files"));
    assert!(stdout.contains("Directories:"));
    assert!(stdout.contains("new_dir"));
    assert!(stdout.contains("Files:"));
    assert!(stdout.contains("new_file.txt"));
}

/// IT-407: Modified Files section
#[test]
fn test_modified_files_section() {
    let env = TestEnv::new();

    create_file(env.source_path(), "modified.txt", "old content");
    create_file(env.target_path(), "modified.txt", "new content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    let stdout = stdout_str(&output);
    assert!(stdout.contains("Modified Files"));
    assert!(stdout.contains("modified.txt"));
}

/// IT-408: Deleted Files section
#[test]
fn test_deleted_files_section() {
    let env = TestEnv::new();

    create_file(env.source_path(), "deleted.txt", "old content");
    fs::create_dir_all(env.source_path().join("deleted_dir")).unwrap();
    create_file(env.source_path(), "deleted_dir/file.txt", "content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    let stdout = stdout_str(&output);
    assert!(stdout.contains("Deleted Files"));
    assert!(stdout.contains("Directories:"));
    assert!(stdout.contains("deleted_dir"));
    assert!(stdout.contains("Files:"));
    assert!(stdout.contains("deleted.txt"));
}

// ============================================================================
// 3.6 File Operation Tests
// ============================================================================

/// IT-501: Directory hierarchy preservation
#[test]
fn test_directory_hierarchy_preservation() {
    let env = TestEnv::new();

    create_file(env.target_path(), "a/b/c/d/deep_file.txt", "deep content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(env.output_path().join("a/b/c/d/deep_file.txt").exists());
}

/// IT-502: Binary file copy
#[test]
fn test_binary_file_copy() {
    let env = TestEnv::new();

    let binary_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let path = env.target_path().join("binary.bin");
    fs::write(&path, &binary_data).unwrap();

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    let copied = fs::read(env.output_path().join("binary.bin")).unwrap();
    assert_eq!(copied, binary_data);
}

/// IT-503: Large file handling
#[test]
fn test_large_file_handling() {
    let env = TestEnv::new();

    let large_content: String = "x".repeat(1024 * 1024);
    create_file(env.target_path(), "large_file.txt", &large_content);

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    let copied = fs::read_to_string(env.output_path().join("large_file.txt")).unwrap();
    assert_eq!(copied.len(), large_content.len());
}

/// IT-504: Many files handling
#[test]
fn test_many_files_handling() {
    let env = TestEnv::new();

    for i in 0..100 {
        create_file(
            env.target_path(),
            &format!("file_{:03}.txt", i),
            &format!("content {}", i),
        );
    }

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());

    for i in 0..100 {
        assert!(env
            .output_path()
            .join(format!("file_{:03}.txt", i))
            .exists());
    }
}

// ============================================================================
// 3.7 Symlink Tests (Linux only)
// ============================================================================

#[cfg(unix)]
mod symlink_tests {
    use super::*;
    use std::os::unix::fs::symlink;

    /// IT-601: Symlink added detection
    #[test]
    fn test_symlink_detection() {
        let env = TestEnv::new();

        create_file(env.target_path(), "real_file.txt", "content");
        symlink(
            env.target_path().join("real_file.txt"),
            env.target_path().join("link_file.txt"),
        )
        .unwrap();

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("[symlink: added]"));
        assert!(!env.output_path().join("link_file.txt").exists());
    }

    /// IT-602: Symlink details in summary
    #[test]
    fn test_symlink_details() {
        let env = TestEnv::new();

        create_file(env.target_path(), "target.txt", "content");
        symlink(
            env.target_path().join("target.txt"),
            env.target_path().join("link.txt"),
        )
        .unwrap();

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        let stdout = stdout_str(&output);
        assert!(stdout.contains("Symlink Details"));
        assert!(stdout.contains("Added:"));
    }

    /// IT-603: Symlink deleted detection
    #[test]
    fn test_symlink_deleted_detection() {
        let env = TestEnv::new();

        // Source has symlink, target doesn't
        create_file(env.source_path(), "real_file.txt", "content");
        symlink(
            env.source_path().join("real_file.txt"),
            env.source_path().join("link_file.txt"),
        )
        .unwrap();

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("[symlink: deleted]"));
        assert!(stdout.contains("Deleted:"));
    }

    /// IT-604: Symlink changed detection
    #[test]
    fn test_symlink_changed_detection() {
        let env = TestEnv::new();

        // Source has symlink to one file
        create_file(env.source_path(), "old_target.txt", "old content");
        symlink(
            env.source_path().join("old_target.txt"),
            env.source_path().join("link.txt"),
        )
        .unwrap();

        // Target has symlink to different file
        create_file(env.target_path(), "new_target.txt", "new content");
        symlink(
            env.target_path().join("new_target.txt"),
            env.target_path().join("link.txt"),
        )
        .unwrap();

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("[symlink: changed]"));
        assert!(stdout.contains("Changed:"));
        assert!(stdout.contains("Before:"));
        assert!(stdout.contains("After:"));
    }

    /// IT-605: Broken symlink detection
    #[test]
    fn test_broken_symlink_detection() {
        let env = TestEnv::new();

        // Create symlink to non-existent target
        symlink(
            env.target_path().join("nonexistent.txt"),
            env.target_path().join("broken_link.txt"),
        )
        .unwrap();

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("[symlink: added, broken]"));
        assert!(stdout.contains("BROKEN"));
    }

    /// IT-606: Symlink becomes broken (same target, but target no longer exists)
    #[test]
    fn test_symlink_becomes_broken() {
        let env = TestEnv::new();

        // Source has working symlink
        create_file(env.source_path(), "target.txt", "content");
        symlink(
            Path::new("target.txt"),
            env.source_path().join("link.txt"),
        )
        .unwrap();

        // Target has same symlink but target file doesn't exist (broken)
        symlink(
            Path::new("target.txt"),
            env.target_path().join("link.txt"),
        )
        .unwrap();
        // Don't create target.txt in target dir - symlink will be broken

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(
            stdout.contains("[symlink: broken]"),
            "Expected [symlink: broken] but got:\n{}",
            stdout
        );
    }

    /// IT-607: Regular file becomes symlink
    #[test]
    fn test_file_becomes_symlink() {
        let env = TestEnv::new();

        // Source has regular file
        create_file(env.source_path(), "file.txt", "regular content");

        // Target has symlink with same name
        create_file(env.target_path(), "target.txt", "target content");
        symlink(
            Path::new("target.txt"),
            env.target_path().join("file.txt"),
        )
        .unwrap();

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(
            stdout.contains("[symlink: added]"),
            "Expected [symlink: added] but got:\n{}",
            stdout
        );
    }

    /// IT-608: Symlink becomes regular file
    #[test]
    fn test_symlink_becomes_file() {
        let env = TestEnv::new();

        // Source has symlink
        create_file(env.source_path(), "target.txt", "target content");
        symlink(
            Path::new("target.txt"),
            env.source_path().join("file.txt"),
        )
        .unwrap();

        // Target has regular file with same name
        create_file(env.target_path(), "file.txt", "regular content");

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(
            stdout.contains("[symlink: deleted]"),
            "Expected [symlink: deleted] but got:\n{}",
            stdout
        );
    }

    /// IT-609: Symlink changes target and becomes broken
    #[test]
    fn test_symlink_changed_and_broken() {
        let env = TestEnv::new();

        // Source has working symlink to one target
        create_file(env.source_path(), "old_target.txt", "old content");
        symlink(
            Path::new("old_target.txt"),
            env.source_path().join("link.txt"),
        )
        .unwrap();

        // Target has symlink to different target that doesn't exist
        symlink(
            Path::new("new_target.txt"),
            env.target_path().join("link.txt"),
        )
        .unwrap();
        // Don't create new_target.txt - symlink will be broken

        let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(
            stdout.contains("[symlink: changed, broken]"),
            "Expected [symlink: changed, broken] but got:\n{}",
            stdout
        );
    }
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

/// Test empty directories in both source and target
#[test]
fn test_empty_directories_both_sides() {
    let env = TestEnv::new();

    fs::create_dir_all(env.source_path().join("empty_dir")).unwrap();
    fs::create_dir_all(env.target_path().join("empty_dir")).unwrap();

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert_eq!(output.status.code(), Some(2));
}

/// Test files with same size but different content
#[test]
fn test_same_size_different_content() {
    let env = TestEnv::new();

    create_file(env.source_path(), "file.txt", "AAAA");
    create_file(env.target_path(), "file.txt", "BBBB");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(stdout_str(&output).contains("[modified]"));
}

/// Test special characters in filenames
#[test]
fn test_special_characters_in_filename() {
    let env = TestEnv::new();

    create_file(env.target_path(), "file with spaces.txt", "content");
    create_file(env.target_path(), "file-with-dashes.txt", "content");
    create_file(env.target_path(), "file_with_underscores.txt", "content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(env.output_path().join("file with spaces.txt").exists());
    assert!(env.output_path().join("file-with-dashes.txt").exists());
    assert!(env.output_path().join("file_with_underscores.txt").exists());
}

/// Test Unicode filenames
#[test]
fn test_unicode_filenames() {
    let env = TestEnv::new();

    create_file(env.target_path(), "日本語ファイル.txt", "Japanese content");
    create_file(env.target_path(), "файл.txt", "Russian content");

    let output = run_diffcopy_sto(env.source_path(), env.target_path(), env.output_path());

    assert!(output.status.success());
    assert!(env.output_path().join("日本語ファイル.txt").exists());
    assert!(env.output_path().join("файл.txt").exists());
}

// ============================================================================
// 3.8 Both Versions Option Tests
// ============================================================================

/// IT-701: --both-versions option creates .old and .new files
#[test]
fn test_both_versions_option() {
    let env = TestEnv::new();

    create_file(env.source_path(), "file.txt", "Old content");
    create_file(env.target_path(), "file.txt", "New content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--both-versions"],
    );

    assert!(output.status.success());
    assert!(env.output_path().join("file.txt.old").exists());
    assert!(env.output_path().join("file.txt.new").exists());

    let old_content = fs::read_to_string(env.output_path().join("file.txt.old")).unwrap();
    let new_content = fs::read_to_string(env.output_path().join("file.txt.new")).unwrap();

    assert_eq!(old_content, "Old content");
    assert_eq!(new_content, "New content");
}

/// IT-702: --both-versions with subdirectories
#[test]
fn test_both_versions_with_subdirectories() {
    let env = TestEnv::new();

    create_file(env.source_path(), "src/main.rs", "fn main() {}");
    create_file(env.target_path(), "src/main.rs", "fn main() { println!(\"Hello\"); }");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["-b"],
    );

    assert!(output.status.success());
    assert!(env.output_path().join("src/main.rs.old").exists());
    assert!(env.output_path().join("src/main.rs.new").exists());
}

/// IT-703: --both-versions does not affect added files
#[test]
fn test_both_versions_added_files_unchanged() {
    let env = TestEnv::new();

    create_file(env.target_path(), "new_file.txt", "New content");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--both-versions"],
    );

    assert!(output.status.success());
    assert!(env.output_path().join("new_file.txt").exists());
    assert!(!env.output_path().join("new_file.txt.old").exists());
    assert!(!env.output_path().join("new_file.txt.new").exists());
}

/// IT-704: --both-versions with multiple modified files
#[test]
fn test_both_versions_multiple_files() {
    let env = TestEnv::new();

    create_file(env.source_path(), "file1.txt", "Old content 1");
    create_file(env.source_path(), "file2.txt", "Old content 2");
    create_file(env.target_path(), "file1.txt", "New content 1");
    create_file(env.target_path(), "file2.txt", "New content 2");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--both-versions"],
    );

    assert!(output.status.success());
    assert!(env.output_path().join("file1.txt.old").exists());
    assert!(env.output_path().join("file1.txt.new").exists());
    assert!(env.output_path().join("file2.txt.old").exists());
    assert!(env.output_path().join("file2.txt.new").exists());
}

// ============================================================================
// 3.9 Config File Tests
// ============================================================================

/// IT-801: Config file basic usage
#[test]
fn test_config_file_basic() {
    let env = TestEnv::new();
    let config_path = env._output_parent.path().join("config.toml");

    create_file(env.target_path(), "file.txt", "content");

    let config_content = format!(
        r#"source = "{}"
target = "{}"
output = "{}"
"#,
        env.source_path().display(),
        env.target_path().display(),
        env.output_path().display()
    );
    fs::write(&config_path, config_content).unwrap();

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(env.output_path().join("file.txt").exists());
}

/// IT-802: Config file with exclude patterns
#[test]
fn test_config_file_with_exclude() {
    let env = TestEnv::new();
    let config_path = env._output_parent.path().join("config.toml");

    create_file(env.target_path(), "main.rs", "fn main() {}");
    create_file(env.target_path(), "debug.log", "log content");

    let config_content = format!(
        r#"source = "{}"
target = "{}"
output = "{}"
exclude = ["*.log"]
"#,
        env.source_path().display(),
        env.target_path().display(),
        env.output_path().display()
    );
    fs::write(&config_path, config_content).unwrap();

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(env.output_path().join("main.rs").exists());
    assert!(!env.output_path().join("debug.log").exists());
}

/// IT-803: Config file with both_versions
#[test]
fn test_config_file_with_both_versions() {
    let env = TestEnv::new();
    let config_path = env._output_parent.path().join("config.toml");

    create_file(env.source_path(), "file.txt", "Old content");
    create_file(env.target_path(), "file.txt", "New content");

    let config_content = format!(
        r#"source = "{}"
target = "{}"
output = "{}"
both_versions = true
"#,
        env.source_path().display(),
        env.target_path().display(),
        env.output_path().display()
    );
    fs::write(&config_path, config_content).unwrap();

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(env.output_path().join("file.txt.old").exists());
    assert!(env.output_path().join("file.txt.new").exists());
}

/// IT-804: CLI args override config file
#[test]
fn test_cli_overrides_config() {
    let env = TestEnv::new();
    let config_path = env._output_parent.path().join("config.toml");
    let alt_output = env._output_parent.path().join("alt_output");

    create_file(env.target_path(), "file.txt", "content");

    let config_content = format!(
        r#"source = "{}"
target = "{}"
output = "{}"
"#,
        env.source_path().display(),
        env.target_path().display(),
        env.output_path().display()
    );
    fs::write(&config_path, config_content).unwrap();

    // CLI output should override config
    let output = run_diffcopy(&[
        "--config", config_path.to_str().unwrap(),
        "-O", alt_output.to_str().unwrap(),
    ]);

    assert!(output.status.success());
    assert!(alt_output.join("file.txt").exists());
    assert!(!env.output_path().exists());
}

/// IT-805: Config file missing required field
#[test]
fn test_config_file_missing_required() {
    let env = TestEnv::new();
    let config_path = env._output_parent.path().join("config.toml");

    // Missing output
    let config_content = format!(
        r#"source = "{}"
target = "{}"
"#,
        env.source_path().display(),
        env.target_path().display()
    );
    fs::write(&config_path, config_content).unwrap();

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_str(&output);
    assert!(stderr.contains("required") || stderr.contains("output") || stderr.contains("Output"));
}

/// IT-806: Config file with multiple exclude patterns
#[test]
fn test_config_file_multiple_excludes() {
    let env = TestEnv::new();
    let config_path = env._output_parent.path().join("config.toml");

    create_file(env.target_path(), "main.rs", "fn main() {}");
    create_file(env.target_path(), "debug.log", "log");
    create_file(env.target_path(), "cache.tmp", "tmp");
    create_file(env.target_path(), "node_modules/pkg.js", "js");

    let config_content = format!(
        r#"source = "{}"
target = "{}"
output = "{}"
exclude = ["*.log", "*.tmp", "node_modules/**"]
"#,
        env.source_path().display(),
        env.target_path().display(),
        env.output_path().display()
    );
    fs::write(&config_path, config_content).unwrap();

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(env.output_path().join("main.rs").exists());
    assert!(!env.output_path().join("debug.log").exists());
    assert!(!env.output_path().join("cache.tmp").exists());
    assert!(!env.output_path().join("node_modules/pkg.js").exists());
}

// ============================================================================
// 3.10 Permission Check Tests
// ============================================================================

#[cfg(unix)]
mod permission_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// IT-901: Permission check with scripts mode
    #[test]
    fn test_permission_check_scripts_mode() {
        let env = TestEnv::new();

        // Create files with same content but different permissions
        let source_file = create_file(env.source_path(), "script.sh", "#!/bin/bash\necho hello");
        let target_file = create_file(env.target_path(), "script.sh", "#!/bin/bash\necho hello");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        let output = run_diffcopy_with_opts(
            env.source_path(),
            env.target_path(),
            env.output_path(),
            &["-P", "scripts"],
        );

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("Permission Changes"));
        assert!(stdout.contains("755") && stdout.contains("644"));
    }

    /// IT-902: Permission check with all mode
    #[test]
    fn test_permission_check_all_mode() {
        let env = TestEnv::new();

        // Create non-script file with same content but different permissions
        let source_file = create_file(env.source_path(), "file.txt", "content");
        let target_file = create_file(env.target_path(), "file.txt", "content");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o644);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o600);
        fs::set_permissions(&target_file, perms_new).unwrap();

        let output = run_diffcopy_with_opts(
            env.source_path(),
            env.target_path(),
            env.output_path(),
            &["-P", "all"],
        );

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("Permission Changes"));
        assert!(stdout.contains("644") && stdout.contains("600"));
    }

    /// IT-903: Permission check disabled by default
    #[test]
    fn test_permission_check_default_disabled() {
        let env = TestEnv::new();

        // Create files with same content but different permissions
        let source_file = create_file(env.source_path(), "script.sh", "#!/bin/bash\necho hello");
        let target_file = create_file(env.target_path(), "script.sh", "#!/bin/bash\necho hello");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        // Without -P flag, permissions should not be checked
        let output = run_diffcopy_sto(
            env.source_path(),
            env.target_path(),
            env.output_path(),
        );

        // Exit code 2 means no differences
        assert_eq!(output.status.code(), Some(2));
        let stdout = stdout_str(&output);
        assert!(stdout.contains("No differences found"));
    }

    /// IT-904: Permission check scripts mode ignores non-script files
    #[test]
    fn test_permission_check_scripts_ignores_non_script() {
        let env = TestEnv::new();

        // Create non-script file with same content but different permissions
        let source_file = create_file(env.source_path(), "file.txt", "content");
        let target_file = create_file(env.target_path(), "file.txt", "content");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o644);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o600);
        fs::set_permissions(&target_file, perms_new).unwrap();

        let output = run_diffcopy_with_opts(
            env.source_path(),
            env.target_path(),
            env.output_path(),
            &["-P", "scripts"],
        );

        // Exit code 2 means no differences (txt files are not scripts)
        assert_eq!(output.status.code(), Some(2));
        let stdout = stdout_str(&output);
        assert!(stdout.contains("No differences found"));
    }

    /// IT-905: Permission check via config file
    #[test]
    fn test_permission_check_config_file() {
        let env = TestEnv::new();
        let config_path = env._output_parent.path().join("config.toml");

        // Create files with same content but different permissions
        let source_file = create_file(env.source_path(), "script.sh", "#!/bin/bash");
        let target_file = create_file(env.target_path(), "script.sh", "#!/bin/bash");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        let config_content = format!(
            r#"source = "{}"
target = "{}"
output = "{}"
check_permissions = "scripts"
"#,
            env.source_path().display(),
            env.target_path().display(),
            env.output_path().display()
        );
        fs::write(&config_path, config_content).unwrap();

        let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("Permission Changes"));
    }

    /// IT-906: Permission check summary statistics
    #[test]
    fn test_permission_check_statistics() {
        let env = TestEnv::new();

        // Create multiple files with permission changes
        let source1 = create_file(env.source_path(), "script1.sh", "#!/bin/bash");
        let target1 = create_file(env.target_path(), "script1.sh", "#!/bin/bash");
        let source2 = create_file(env.source_path(), "script2.py", "#!/usr/bin/env python3");
        let target2 = create_file(env.target_path(), "script2.py", "#!/usr/bin/env python3");

        // Set different permissions
        for source in [&source1, &source2] {
            let mut perms = fs::metadata(source).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(source, perms).unwrap();
        }
        for target in [&target1, &target2] {
            let mut perms = fs::metadata(target).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(target, perms).unwrap();
        }

        let output = run_diffcopy_with_opts(
            env.source_path(),
            env.target_path(),
            env.output_path(),
            &["-P", "scripts"],
        );

        assert!(output.status.success());
        let stdout = stdout_str(&output);
        assert!(stdout.contains("Permissions: 2 files"));
    }
}

// ============================================================================
// Patch Generation Tests
// ============================================================================

/// IT-501: Individual patch file generation
#[test]
fn test_patch_individual_files() {
    let env = TestEnv::new();

    // Create modified file
    create_file(env.source_path(), "file.txt", "line1\nline2\nline3\n");
    create_file(env.target_path(), "file.txt", "line1\nmodified\nline3\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--patch"],
    );

    assert!(output.status.success());

    // Check that .patch file was created
    assert!(env.output_path().join("file.txt.patch").exists());

    // Check patch content
    let patch_content = fs::read_to_string(env.output_path().join("file.txt.patch")).unwrap();
    assert!(patch_content.contains("--- a/file.txt"));
    assert!(patch_content.contains("+++ b/file.txt"));
    assert!(patch_content.contains("-line2"));
    assert!(patch_content.contains("+modified"));
}

/// IT-502: Combined patch file generation
#[test]
fn test_patch_combined_file() {
    let env = TestEnv::new();
    let patch_file = env.output_path().parent().unwrap().join("combined.patch");

    // Create multiple modified files
    create_file(env.source_path(), "file1.txt", "old content 1\n");
    create_file(env.target_path(), "file1.txt", "new content 1\n");
    create_file(env.source_path(), "file2.txt", "old content 2\n");
    create_file(env.target_path(), "file2.txt", "new content 2\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--patch-file", patch_file.to_str().unwrap()],
    );

    assert!(output.status.success());

    // Check that combined patch file was created
    assert!(patch_file.exists());

    // Check patch content contains both files
    let patch_content = fs::read_to_string(&patch_file).unwrap();
    assert!(patch_content.contains("--- a/file1.txt"));
    assert!(patch_content.contains("--- a/file2.txt"));
}

/// IT-503: Both individual and combined patches
#[test]
fn test_patch_both_modes() {
    let env = TestEnv::new();
    let patch_file = env.output_path().parent().unwrap().join("all.patch");

    create_file(env.source_path(), "test.txt", "before\n");
    create_file(env.target_path(), "test.txt", "after\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--patch", "--patch-file", patch_file.to_str().unwrap()],
    );

    assert!(output.status.success());

    // Both should exist
    assert!(env.output_path().join("test.txt.patch").exists());
    assert!(patch_file.exists());
}

/// IT-504: Binary file skipped in patch
#[test]
fn test_patch_binary_skipped() {
    let env = TestEnv::new();

    // Create binary files (different content)
    let binary_data_old: Vec<u8> = vec![0x00, 0x01, 0x02, 0x03];
    let binary_data_new: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF];
    fs::write(env.source_path().join("binary.bin"), &binary_data_old).unwrap();
    fs::write(env.target_path().join("binary.bin"), &binary_data_new).unwrap();

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--patch"],
    );

    assert!(output.status.success());

    // No .patch file should be created for binary
    assert!(!env.output_path().join("binary.bin.patch").exists());

    // Summary should mention skipped
    let stdout = stdout_str(&output);
    assert!(stdout.contains("[skip]") || stdout.contains("Skipped"));
}

/// IT-505: Patch with subdirectories
#[test]
fn test_patch_with_subdirectories() {
    let env = TestEnv::new();

    create_file(env.source_path(), "src/main.rs", "fn main() {}\n");
    create_file(env.target_path(), "src/main.rs", "fn main() { println!(\"hello\"); }\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--patch"],
    );

    assert!(output.status.success());

    // Patch file should be in same directory structure
    assert!(env.output_path().join("src/main.rs.patch").exists());
}

/// IT-506: Patch summary section
#[test]
fn test_patch_summary_section() {
    let env = TestEnv::new();

    create_file(env.source_path(), "text.txt", "old\n");
    create_file(env.target_path(), "text.txt", "new\n");

    // Create binary file too
    fs::write(env.source_path().join("bin.dat"), vec![0x00, 0x01]).unwrap();
    fs::write(env.target_path().join("bin.dat"), vec![0xFF, 0xFE]).unwrap();

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--patch"],
    );

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(stdout.contains("Patch Details"));
    assert!(stdout.contains("Generated:"));
    assert!(stdout.contains("Skipped:"));
}

/// IT-507: Patch options in summary
#[test]
fn test_patch_options_in_summary() {
    let env = TestEnv::new();
    let patch_file = env.output_path().parent().unwrap().join("out.patch");

    create_file(env.source_path(), "file.txt", "a\n");
    create_file(env.target_path(), "file.txt", "b\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--patch", "--patch-file", patch_file.to_str().unwrap()],
    );

    assert!(output.status.success());
    let stdout = stdout_str(&output);

    assert!(stdout.contains("Patch mode: Individual files (.patch)"));
    assert!(stdout.contains("Combined patch file:"));
}

/// IT-508: Patch with config file
#[test]
fn test_patch_with_config() {
    let env = TestEnv::new();
    let config_path = env.source_path().parent().unwrap().join("config.toml");

    let config_content = format!(
        r#"
source = "{}"
target = "{}"
output = "{}"
patch = true
"#,
        env.source_path().display(),
        env.target_path().display(),
        env.output_path().display()
    );
    fs::write(&config_path, config_content).unwrap();

    create_file(env.source_path(), "test.txt", "old\n");
    create_file(env.target_path(), "test.txt", "new\n");

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(env.output_path().join("test.txt.patch").exists());
}

// ============================================================================
// 3.10 Excel Report Tests (using calamine for content verification)
// ============================================================================

use calamine::{Reader, Xlsx, Data};

/// Helper function to open an Excel file and return a Xlsx reader
fn open_excel(path: &Path) -> Xlsx<std::io::BufReader<std::fs::File>> {
    calamine::open_workbook(path).expect("Failed to open Excel file")
}

/// Helper function to get cell value as string from a worksheet
fn get_cell_string(range: &calamine::Range<Data>, row: u32, col: u32) -> String {
    range.get_value((row, col))
        .map(|v| match v {
            Data::String(s) => s.clone(),
            Data::Float(f) => f.to_string(),
            Data::Int(i) => i.to_string(),
            Data::Bool(b) => b.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

/// Helper function to check if a value exists anywhere in the worksheet
fn sheet_contains(range: &calamine::Range<Data>, needle: &str) -> bool {
    for row in range.rows() {
        for cell in row {
            if let Data::String(s) = cell {
                if s.contains(needle) {
                    return true;
                }
            }
        }
    }
    false
}

/// IT-901: Excel file generation with differences - verify sheet structure
#[test]
fn test_excel_file_generation() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    create_file(env.source_path(), "old.txt", "old content\n");
    create_file(env.target_path(), "old.txt", "new content\n");
    create_file(env.target_path(), "new.txt", "new file\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());
    assert!(excel_path.exists(), "Excel file should be created");

    // Verify Excel structure using calamine
    let workbook = open_excel(&excel_path);
    let sheet_names = workbook.sheet_names();

    assert!(sheet_names.contains(&"Summary".to_string()), "Should have Summary sheet");
    assert!(sheet_names.contains(&"File Tree".to_string()), "Should have File Tree sheet");
    assert!(sheet_names.contains(&"Details".to_string()), "Should have Details sheet");
}

/// IT-902: Excel file structure - verify Summary sheet content
#[test]
fn test_excel_file_is_valid_xlsx() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    create_file(env.source_path(), "file.txt", "old\n");
    create_file(env.target_path(), "file.txt", "new\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());

    // Verify Summary sheet content using calamine
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Summary").expect("Should have Summary sheet");

    // Check title
    let title = get_cell_string(&range, 0, 0);
    assert!(title.contains("rs_diffcopy"), "Title should contain 'rs_diffcopy'");

    // Check that Source, Target labels exist
    assert!(sheet_contains(&range, "Source:"), "Should have Source label");
    assert!(sheet_contains(&range, "Target:"), "Should have Target label");
    assert!(sheet_contains(&range, "Date:"), "Should have Date label");
}

/// IT-903: Excel with summary text file
#[test]
fn test_excel_with_summary() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");
    let summary_path = env.output_path().parent().unwrap().join("summary.txt");

    create_file(env.source_path(), "file.txt", "old\n");
    create_file(env.target_path(), "file.txt", "new\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &[
            "--excel", excel_path.to_str().unwrap(),
            "-s", summary_path.to_str().unwrap(),
        ],
    );

    assert!(output.status.success());
    assert!(excel_path.exists(), "Excel file should be created");
    assert!(summary_path.exists(), "Summary file should be created");

    // Verify both files have consistent content
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Summary").expect("Should have Summary sheet");
    assert!(sheet_contains(&range, "Modified"), "Excel should show Modified");

    let summary_content = fs::read_to_string(&summary_path).unwrap();
    assert!(summary_content.contains("Modified"), "Summary should show Modified");
}

/// IT-904: Excel with various options - verify options are recorded
#[test]
fn test_excel_with_options() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    create_file(env.source_path(), "file.txt", "old\n");
    create_file(env.target_path(), "file.txt", "new\n");
    create_file(env.target_path(), "skip.log", "log content\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &[
            "--excel", excel_path.to_str().unwrap(),
            "-e", "*.log",
            "--dry-run",
        ],
    );

    assert!(output.status.success());
    assert!(excel_path.exists(), "Excel file should be created even in dry-run");

    // Verify options are recorded in Summary sheet
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Summary").expect("Should have Summary sheet");

    assert!(sheet_contains(&range, "Dry-run"), "Should show Dry-run option");
    assert!(sheet_contains(&range, "*.log"), "Should show exclude pattern");
}

/// IT-905: Excel statistics - verify statistics in Summary sheet
#[test]
fn test_excel_statistics() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Create test scenario: 1 added, 1 modified, 1 deleted
    create_file(env.source_path(), "deleted.txt", "will be deleted\n");
    create_file(env.source_path(), "modified.txt", "old content\n");
    create_file(env.target_path(), "modified.txt", "new content\n");
    create_file(env.target_path(), "added.txt", "new file\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());
    assert!(excel_path.exists());

    // Verify statistics in Summary sheet
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Summary").expect("Should have Summary sheet");

    assert!(sheet_contains(&range, "Added"), "Should have Added category");
    assert!(sheet_contains(&range, "Modified"), "Should have Modified category");
    assert!(sheet_contains(&range, "Deleted"), "Should have Deleted category");
    assert!(sheet_contains(&range, "Total"), "Should have Total row");
}

/// IT-906: Excel file tree - verify tree structure with nested directories
#[test]
fn test_excel_file_tree() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    create_file(env.source_path(), "root.txt", "old\n");
    create_file(env.target_path(), "root.txt", "new\n");
    create_file(env.target_path(), "dir1/file1.txt", "content\n");
    create_file(env.target_path(), "dir1/dir2/file2.txt", "nested\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());
    assert!(excel_path.exists());

    // Verify File Tree sheet content
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("File Tree").expect("Should have File Tree sheet");

    // Check that tree contains directories and files
    assert!(sheet_contains(&range, "dir1"), "Should contain dir1");
    assert!(sheet_contains(&range, "dir2"), "Should contain dir2");
    assert!(sheet_contains(&range, "file2.txt"), "Should contain file2.txt");
    assert!(sheet_contains(&range, "root.txt"), "Should contain root.txt");

    // Check tree connectors exist
    let has_tree_connector = range.rows().any(|row| {
        row.iter().any(|cell| {
            if let Data::String(s) = cell {
                s.contains("├─") || s.contains("└─")
            } else {
                false
            }
        })
    });
    assert!(has_tree_connector, "Should have tree connectors");
}

/// IT-907: Excel details - verify Details sheet content
#[test]
fn test_excel_details() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    create_file(env.source_path(), "src/main.rs", "fn main() {}\n");
    create_file(env.target_path(), "src/main.rs", "fn main() { println!(\"hello\"); }\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());
    assert!(excel_path.exists());

    // Verify Details sheet content
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Details").expect("Should have Details sheet");

    assert!(sheet_contains(&range, "Modified Files"), "Should have Modified Files section");
    assert!(sheet_contains(&range, "main.rs"), "Should contain main.rs");
}

/// IT-908: Excel with config file
#[test]
fn test_excel_with_config() {
    let env = TestEnv::new();
    let config_path = env.source_path().join("config.toml");
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Use forward slashes for TOML compatibility on all platforms
    let source_str = env.source_path().display().to_string().replace("\\", "/");
    let target_str = env.target_path().display().to_string().replace("\\", "/");
    let output_str = env.output_path().display().to_string().replace("\\", "/");
    let excel_str = excel_path.display().to_string().replace("\\", "/");

    let config_content = format!(
        r#"
source = "{}"
target = "{}"
output = "{}"
excel = "{}"
"#,
        source_str,
        target_str,
        output_str,
        excel_str
    );
    fs::write(&config_path, config_content).unwrap();

    create_file(env.source_path(), "test.txt", "old\n");
    create_file(env.target_path(), "test.txt", "new\n");

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(excel_path.exists(), "Excel file should be created via config");

    // Verify Excel content
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Summary").expect("Should have Summary sheet");
    assert!(sheet_contains(&range, "Modified"), "Should show Modified");
}

/// IT-909: Excel with no differences
#[test]
fn test_excel_no_differences() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Same content in both
    create_file(env.source_path(), "same.txt", "same content\n");
    create_file(env.target_path(), "same.txt", "same content\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    // Exit code 2 for no differences
    assert_eq!(output.status.code(), Some(2));
    // Excel file should still be created
    assert!(excel_path.exists(), "Excel file should be created even with no differences");

    // Verify Excel shows no differences message or empty statistics
    let workbook = open_excel(&excel_path);
    let sheet_names = workbook.sheet_names();
    assert!(sheet_names.contains(&"Summary".to_string()), "Should have Summary sheet");
}

/// IT-910: Excel file tree hierarchy verification
#[test]
fn test_excel_file_tree_hierarchy() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Create nested structure: downloads/.dummyfile, repos/deby/kas/board/kas.yml
    create_file(env.source_path(), "downloads/.dummyfile", "dummy\n");
    create_file(env.target_path(), "repos/deby/kas/board/kas.yml", "config\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());

    // Verify File Tree sheet has proper hierarchy
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("File Tree").expect("Should have File Tree sheet");

    // Collect all rows to verify hierarchy
    let mut found_downloads = false;
    let mut found_dummyfile = false;
    let mut found_repos = false;
    let mut found_deby = false;
    let mut found_kas_yml = false;

    for row in range.rows() {
        let row_text: String = row.iter()
            .filter_map(|cell| {
                if let Data::String(s) = cell {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        if row_text.contains("downloads") {
            found_downloads = true;
        }
        if row_text.contains(".dummyfile") {
            found_dummyfile = true;
        }
        if row_text.contains("repos") {
            found_repos = true;
        }
        if row_text.contains("deby") {
            found_deby = true;
        }
        if row_text.contains("kas.yml") {
            found_kas_yml = true;
        }
    }

    assert!(found_downloads, "Should find downloads directory");
    assert!(found_dummyfile, "Should find .dummyfile");
    assert!(found_repos, "Should find repos directory");
    assert!(found_deby, "Should find deby directory");
    assert!(found_kas_yml, "Should find kas.yml file");
}

/// IT-911: Excel Details sheet with all change types
#[test]
fn test_excel_details_all_types() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Create all types of changes
    create_file(env.source_path(), "deleted.txt", "will be deleted\n");
    create_file(env.source_path(), "modified.txt", "old\n");
    create_file(env.target_path(), "modified.txt", "new\n");
    create_file(env.target_path(), "added.txt", "new file\n");
    fs::create_dir_all(env.target_path().join("new_dir")).unwrap();

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());

    // Verify Details sheet has all sections
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Details").expect("Should have Details sheet");

    assert!(sheet_contains(&range, "Added Files"), "Should have Added Files section");
    assert!(sheet_contains(&range, "Modified Files"), "Should have Modified Files section");
    assert!(sheet_contains(&range, "Deleted Files"), "Should have Deleted Files section");

    // Verify specific files are listed
    assert!(sheet_contains(&range, "added.txt"), "Should list added.txt");
    assert!(sheet_contains(&range, "modified.txt"), "Should list modified.txt");
    assert!(sheet_contains(&range, "deleted.txt"), "Should list deleted.txt");
}

/// IT-912: Excel fold level option
#[test]
fn test_excel_fold_level() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Create nested directory structure
    create_file(env.target_path(), "level1/level2/level3/deep.txt", "deep file\n");
    create_file(env.target_path(), "level1/shallow.txt", "shallow file\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap(), "-L", "2"],
    );

    assert!(output.status.success());
    assert!(excel_path.exists());

    // Verify Excel was created and has File Tree sheet
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("File Tree").expect("Should have File Tree sheet");
    assert!(sheet_contains(&range, "level1"), "Should contain level1 directory");
}

/// IT-913: Excel fold level with config file
#[test]
fn test_excel_fold_level_with_config() {
    let env = TestEnv::new();
    let config_path = env.source_path().join("config.toml");
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Use forward slashes for TOML compatibility on all platforms
    let source_str = env.source_path().display().to_string().replace("\\", "/");
    let target_str = env.target_path().display().to_string().replace("\\", "/");
    let output_str = env.output_path().display().to_string().replace("\\", "/");
    let excel_str = excel_path.display().to_string().replace("\\", "/");

    let config_content = format!(
        r#"
source = "{}"
target = "{}"
output = "{}"
excel = "{}"
excel_fold_level = 1
"#,
        source_str,
        target_str,
        output_str,
        excel_str
    );
    fs::write(&config_path, config_content).unwrap();

    create_file(env.target_path(), "dir1/dir2/file.txt", "content\n");

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    assert!(excel_path.exists(), "Excel file should be created via config with fold_level");
}

/// IT-914: Excel Details sheet column structure (Directory and File separated)
#[test]
fn test_excel_details_columns() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    // Create file with directory path
    create_file(env.target_path(), "src/components/Button.tsx", "export const Button = () => {};\n");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());
    assert!(excel_path.exists());

    // Verify Details sheet has separated Directory and File columns
    let mut workbook = open_excel(&excel_path);
    let range = workbook.worksheet_range("Details").expect("Should have Details sheet");

    // Check column headers
    assert!(sheet_contains(&range, "Directory"), "Should have Directory column");
    assert!(sheet_contains(&range, "File"), "Should have File column header");

    // Check that file name is separated from directory
    assert!(sheet_contains(&range, "Button.tsx"), "Should contain file name");
    assert!(sheet_contains(&range, "src/components") || sheet_contains(&range, "src\\components"),
            "Should contain directory path");
}

// ==================== show_unchanged tests ====================

/// IT-1001: --show-unchanged option shows unchanged files in summary
#[test]
fn test_show_unchanged_option() {
    let env = TestEnv::new();

    // Create same file in both source and target
    create_file(env.source_path(), "same.txt", "Same content");
    create_file(env.target_path(), "same.txt", "Same content");
    // Create a modified file to have some differences
    create_file(env.source_path(), "modified.txt", "Old");
    create_file(env.target_path(), "modified.txt", "New");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--show-unchanged"],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[unchanged]"), "Should show [unchanged] tag");
    assert!(stdout.contains("same.txt"), "Should show unchanged file name");
    assert!(stdout.contains("Unchanged Files"), "Should have Unchanged Files section");
}

/// IT-1002: -u short option for show-unchanged
#[test]
fn test_show_unchanged_short_option() {
    let env = TestEnv::new();

    create_file(env.source_path(), "same.txt", "Same content");
    create_file(env.target_path(), "same.txt", "Same content");
    create_file(env.target_path(), "added.txt", "Added");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["-u"],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[unchanged]"), "Should show [unchanged] tag with -u option");
}

/// IT-1003: show_unchanged in config file
#[test]
fn test_show_unchanged_config() {
    let env = TestEnv::new();
    let config_path = env.output_path().parent().unwrap().join("config.toml");

    let source_str = env.source_path().to_str().unwrap().replace('\\', "/");
    let target_str = env.target_path().to_str().unwrap().replace('\\', "/");
    let output_str = env.output_path().to_str().unwrap().replace('\\', "/");

    let config_content = format!(
        r#"
source = "{}"
target = "{}"
output = "{}"
show_unchanged = true
"#,
        source_str, target_str, output_str
    );
    fs::write(&config_path, config_content).unwrap();

    create_file(env.source_path(), "same.txt", "Same content");
    create_file(env.target_path(), "same.txt", "Same content");
    create_file(env.target_path(), "added.txt", "Added");

    let output = run_diffcopy(&["--config", config_path.to_str().unwrap()]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[unchanged]"), "Should show [unchanged] with config file");
    assert!(stdout.contains("Show unchanged: Yes"), "Options should show 'Show unchanged: Yes'");
}

/// IT-1004: Unchanged count always shown in statistics
#[test]
fn test_unchanged_count_always_shown() {
    let env = TestEnv::new();

    // Create same file in both source and target
    create_file(env.source_path(), "same.txt", "Same content");
    create_file(env.target_path(), "same.txt", "Same content");
    // Create a modified file to have some differences
    create_file(env.source_path(), "modified.txt", "Old");
    create_file(env.target_path(), "modified.txt", "New");

    // Without --show-unchanged, unchanged count should still be in statistics
    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &[],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Unchanged:"), "Should always show Unchanged count in statistics");
    assert!(stdout.contains("1 files"), "Should show 1 unchanged file");
}

/// IT-1005: Total uses unique path count formula
#[test]
fn test_total_unique_paths() {
    let env = TestEnv::new();

    // source: 3 files (same.txt, source_only.txt, modified.txt)
    // target: 3 files (same.txt, target_only.txt, modified.txt)
    // common: 2 (same.txt, modified.txt)
    // Total unique = 3 + 3 - 2 = 4
    create_file(env.source_path(), "same.txt", "Same content");
    create_file(env.target_path(), "same.txt", "Same content");
    create_file(env.source_path(), "source_only.txt", "Source only");
    create_file(env.target_path(), "target_only.txt", "Target only");
    create_file(env.source_path(), "modified.txt", "Old");
    create_file(env.target_path(), "modified.txt", "New");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &[],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Total should be 4 (3 + 3 - 2)
    assert!(stdout.contains("Total:"), "Should have Total line");
    assert!(stdout.contains("4 items"), "Total should be 4 items (unique paths)");
}

/// IT-1006: Without --show-unchanged, unchanged files not in details
#[test]
fn test_unchanged_not_in_details_without_option() {
    let env = TestEnv::new();

    create_file(env.source_path(), "same.txt", "Same content");
    create_file(env.target_path(), "same.txt", "Same content");
    create_file(env.target_path(), "added.txt", "Added");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &[],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Without --show-unchanged, [unchanged] tag should not appear
    assert!(!stdout.contains("[unchanged]"), "Should not show [unchanged] tag without option");
    // But unchanged count should still be in statistics
    assert!(stdout.contains("Unchanged:"), "Unchanged count should still be shown");
}

/// IT-1007: --show-unchanged with Excel output
#[test]
fn test_show_unchanged_with_excel() {
    let env = TestEnv::new();
    let excel_path = env.output_path().parent().unwrap().join("report.xlsx");

    create_file(env.source_path(), "same.txt", "Same content");
    create_file(env.target_path(), "same.txt", "Same content");
    create_file(env.target_path(), "added.txt", "Added");

    let output = run_diffcopy_with_opts(
        env.source_path(),
        env.target_path(),
        env.output_path(),
        &["--show-unchanged", "--excel", excel_path.to_str().unwrap()],
    );

    assert!(output.status.success());
    assert!(excel_path.exists());

    let mut workbook = open_excel(&excel_path);

    // Check Summary sheet has Unchanged
    let summary_range = workbook.worksheet_range("Summary").expect("Should have Summary sheet");
    assert!(sheet_contains(&summary_range, "Unchanged"), "Summary should contain Unchanged");

    // Check Details sheet has unchanged entry (section header is "Unchanged Files")
    let details_range = workbook.worksheet_range("Details").expect("Should have Details sheet");
    assert!(sheet_contains(&details_range, "Unchanged Files"), "Details should contain Unchanged Files section");
}
