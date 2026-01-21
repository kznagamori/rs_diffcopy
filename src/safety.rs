use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::{DiffCopyError, Result};
use crate::utils::{count_directory_contents, format_size};

/// Protected paths that cannot be deleted with --force
#[cfg(unix)]
fn get_protected_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/"),
        PathBuf::from("/bin"),
        PathBuf::from("/boot"),
        PathBuf::from("/dev"),
        PathBuf::from("/etc"),
        PathBuf::from("/home"),
        PathBuf::from("/lib"),
        PathBuf::from("/lib32"),
        PathBuf::from("/lib64"),
        PathBuf::from("/libx32"),
        PathBuf::from("/media"),
        PathBuf::from("/mnt"),
        PathBuf::from("/opt"),
        PathBuf::from("/proc"),
        PathBuf::from("/root"),
        PathBuf::from("/run"),
        PathBuf::from("/sbin"),
        PathBuf::from("/srv"),
        PathBuf::from("/sys"),
        PathBuf::from("/tmp"),
        PathBuf::from("/usr"),
        PathBuf::from("/var"),
    ];

    // Add user home directory
    if let Some(home) = home_dir() {
        paths.push(home);
    }

    paths
}

#[cfg(windows)]
fn get_protected_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Drive roots
    for letter in 'A'..='Z' {
        paths.push(PathBuf::from(format!("{}:\\", letter)));
    }

    // System directories (typically on C:)
    let system_dirs = [
        "C:\\Windows",
        "C:\\Program Files",
        "C:\\Program Files (x86)",
        "C:\\ProgramData",
        "C:\\Users",
    ];

    for dir in system_dirs {
        paths.push(PathBuf::from(dir));
    }

    // Current user's directories
    if let Some(home) = home_dir() {
        paths.push(home.clone());

        // Protect immediate subdirectories (AppData, Desktop, Documents, etc.)
        if let Ok(entries) = std::fs::read_dir(&home) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                    paths.push(entry.path());
                }
            }
        }
    }

    paths
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
}

/// Check if a path is protected (cannot be deleted)
pub fn is_protected_path(path: &Path) -> bool {
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };

    let protected = get_protected_paths();

    for protected_path in &protected {
        let protected_canonical = protected_path.canonicalize().unwrap_or(protected_path.clone());
        if canonical == protected_canonical {
            return true;
        }
    }

    false
}

/// Prompt user for confirmation before deleting directory
pub fn confirm_delete(path: &Path) -> Result<bool> {
    let (files, dirs, total_size) = count_directory_contents(path).unwrap_or((0, 0, 0));

    eprintln!("Output directory '{}' already exists.", path.display());
    eprintln!("  Contains: {} files, {} directories", files, dirs);
    eprintln!("  Total size: {}", format_size(total_size));
    eprintln!();
    eprint!("Delete and continue? [y/N]: ");
    io::stderr().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| {
        DiffCopyError::Other(format!("Failed to read input: {}", e))
    })?;

    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}

/// Check output directory and handle --force option
pub fn check_output_directory(path: &Path, force: bool, dry_run: bool) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    if !force {
        return Err(DiffCopyError::OutputExists(path.to_path_buf()));
    }

    // In dry-run mode, don't actually delete
    if dry_run {
        return Ok(());
    }

    // Check if path is protected
    if is_protected_path(path) {
        return Err(DiffCopyError::ProtectedPath(path.to_path_buf()));
    }

    // Confirm with user
    if !confirm_delete(path)? {
        return Err(DiffCopyError::UserCancelled);
    }

    // Delete the directory
    std::fs::remove_dir_all(path).map_err(|e| {
        DiffCopyError::DirectoryDeleteError(path.to_path_buf(), e.to_string())
    })?;

    Ok(())
}

/// Validate source and target directories exist
pub fn validate_directories(source: &Path, target: &Path, base: Option<&Path>) -> Result<()> {
    if !source.exists() {
        return Err(DiffCopyError::SourceNotFound(source.to_path_buf()));
    }

    if !target.exists() {
        return Err(DiffCopyError::TargetNotFound(target.to_path_buf()));
    }

    if let Some(base_path) = base {
        if !base_path.exists() {
            return Err(DiffCopyError::BaseNotFound(base_path.to_path_buf()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    fn test_is_protected_path_unix_root() {
        assert!(is_protected_path(Path::new("/")));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_protected_path_unix_system_dirs() {
        assert!(is_protected_path(Path::new("/etc")));
        assert!(is_protected_path(Path::new("/home")));
        assert!(is_protected_path(Path::new("/usr")));
        assert!(is_protected_path(Path::new("/var")));
        assert!(is_protected_path(Path::new("/bin")));
        assert!(is_protected_path(Path::new("/boot")));
    }

    #[cfg(unix)]
    #[test]
    fn test_is_protected_path_unix_safe_paths() {
        // Temporary and user-created directories should not be protected
        let dir = tempdir().unwrap();
        assert!(!is_protected_path(dir.path()));
    }

    #[cfg(windows)]
    #[test]
    fn test_is_protected_path_windows_drives() {
        assert!(is_protected_path(Path::new("C:\\")));
        assert!(is_protected_path(Path::new("D:\\")));
    }

    #[cfg(windows)]
    #[test]
    fn test_is_protected_path_windows_system() {
        assert!(is_protected_path(Path::new("C:\\Windows")));
        assert!(is_protected_path(Path::new("C:\\Program Files")));
    }

    #[test]
    fn test_validate_directories_all_exist() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();

        let result = validate_directories(dir1.path(), dir2.path(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_directories_with_base() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        let base = tempdir().unwrap();

        let result = validate_directories(source.path(), target.path(), Some(base.path()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_directories_source_missing() {
        let target = tempdir().unwrap();
        let nonexistent = Path::new("/nonexistent/source/dir");

        let result = validate_directories(nonexistent, target.path(), None);
        assert!(result.is_err());
        match result {
            Err(DiffCopyError::SourceNotFound(_)) => {}
            _ => panic!("Expected SourceNotFound error"),
        }
    }

    #[test]
    fn test_validate_directories_target_missing() {
        let source = tempdir().unwrap();
        let nonexistent = Path::new("/nonexistent/target/dir");

        let result = validate_directories(source.path(), nonexistent, None);
        assert!(result.is_err());
        match result {
            Err(DiffCopyError::TargetNotFound(_)) => {}
            _ => panic!("Expected TargetNotFound error"),
        }
    }

    #[test]
    fn test_validate_directories_base_missing() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        let nonexistent = Path::new("/nonexistent/base/dir");

        let result = validate_directories(source.path(), target.path(), Some(nonexistent));
        assert!(result.is_err());
        match result {
            Err(DiffCopyError::BaseNotFound(_)) => {}
            _ => panic!("Expected BaseNotFound error"),
        }
    }

    #[test]
    fn test_check_output_directory_not_exists() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("new_output");

        // Non-existent directory should be OK
        let result = check_output_directory(&output, false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_output_directory_exists_no_force() {
        let output = tempdir().unwrap();

        // Existing directory without --force should error
        let result = check_output_directory(output.path(), false, false);
        assert!(result.is_err());
        match result {
            Err(DiffCopyError::OutputExists(_)) => {}
            _ => panic!("Expected OutputExists error"),
        }
    }

    #[test]
    fn test_check_output_directory_exists_dry_run() {
        let output = tempdir().unwrap();

        // Existing directory with --force and --dry-run should be OK (no actual delete)
        let result = check_output_directory(output.path(), true, true);
        assert!(result.is_ok());
        // Directory should still exist
        assert!(output.path().exists());
    }

    #[test]
    fn test_check_output_protected_path() {
        // Protected paths should be rejected even with --force
        #[cfg(unix)]
        {
            // Note: This test doesn't actually run the full check_output_directory
            // because /etc doesn't "exist" as an output we'd create, but it tests
            // the is_protected_path function
            assert!(is_protected_path(Path::new("/etc")));
        }
    }

    #[test]
    fn test_home_dir() {
        // home_dir should return something on most systems
        let home = home_dir();
        // This may be None in some CI environments, so we just check it doesn't panic
        if let Some(path) = home {
            assert!(path.is_absolute());
        }
    }

    #[test]
    fn test_get_protected_paths_not_empty() {
        let paths = get_protected_paths();
        assert!(!paths.is_empty());
    }
}
