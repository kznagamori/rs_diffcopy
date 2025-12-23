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
