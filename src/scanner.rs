use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{DiffCopyError, Result};

/// Entry discovered during scanning
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScannedEntry {
    pub relative_path: PathBuf,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub is_special: bool,
}

/// Directory scanner with exclusion patterns
pub struct Scanner {
    exclude_patterns: GlobSet,
}

impl Scanner {
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();

        for pattern in patterns {
            // Add pattern as-is
            let glob = Glob::new(pattern)
                .map_err(|_| DiffCopyError::InvalidGlobPattern(pattern.clone()))?;
            builder.add(glob);

            // Also add pattern with **/ prefix for matching in any directory
            if !pattern.starts_with("**/") && !pattern.contains('/') {
                let prefixed = format!("**/{}", pattern);
                if let Ok(glob) = Glob::new(&prefixed) {
                    builder.add(glob);
                }
            }
        }

        let exclude_patterns = builder
            .build()
            .map_err(|e| DiffCopyError::InvalidGlobPattern(e.to_string()))?;

        Ok(Self { exclude_patterns })
    }

    /// Check if a path should be excluded
    pub fn is_excluded(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check full path
        if self.exclude_patterns.is_match(path) {
            return true;
        }

        // Check each component
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                if self.exclude_patterns.is_match(name) {
                    return true;
                }
            }
        }

        // Check path as string
        self.exclude_patterns.is_match(path_str.as_ref())
    }

    /// Scan a directory and return all entries
    pub fn scan(&self, root: &Path) -> Result<Vec<ScannedEntry>> {
        let mut entries = Vec::new();

        for entry in WalkDir::new(root).min_depth(1) {
            let entry = entry.map_err(|e| {
                DiffCopyError::FileReadError {
                    path: e.path().map(|p| p.to_path_buf()).unwrap_or_default(),
                    message: e.to_string(),
                }
            })?;

            let relative_path = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_path_buf();

            // Check exclusion
            if self.is_excluded(&relative_path) {
                continue;
            }

            let file_type = entry.file_type();
            let is_symlink = file_type.is_symlink();
            let is_special = is_special_file(&entry);

            entries.push(ScannedEntry {
                relative_path,
                is_directory: file_type.is_dir(),
                is_symlink,
                is_special,
            });
        }

        Ok(entries)
    }

    /// Scan multiple directories and return union of all paths
    #[allow(dead_code)]
    pub fn scan_union(&self, dirs: &[&Path]) -> Result<HashSet<PathBuf>> {
        let mut all_paths = HashSet::new();

        for dir in dirs {
            if !dir.exists() {
                continue;
            }

            let entries = self.scan(dir)?;
            for entry in entries {
                all_paths.insert(entry.relative_path);
            }
        }

        Ok(all_paths)
    }
}

/// Check if entry is a special file (Unix only)
#[cfg(unix)]
fn is_special_file(entry: &walkdir::DirEntry) -> bool {
    use std::os::unix::fs::FileTypeExt;

    if let Ok(meta) = entry.metadata() {
        let ft = meta.file_type();
        return ft.is_socket() || ft.is_fifo() || ft.is_block_device() || ft.is_char_device();
    }
    false
}

#[cfg(not(unix))]
fn is_special_file(_entry: &walkdir::DirEntry) -> bool {
    false
}

/// Get special file type (Unix only)
#[cfg(unix)]
pub fn get_special_file_type(path: &Path) -> Option<crate::types::SpecialFileType> {
    use std::os::unix::fs::FileTypeExt;

    if let Ok(meta) = std::fs::symlink_metadata(path) {
        let ft = meta.file_type();
        if ft.is_socket() {
            return Some(crate::types::SpecialFileType::Socket);
        }
        if ft.is_fifo() {
            return Some(crate::types::SpecialFileType::Fifo);
        }
        if ft.is_block_device() {
            return Some(crate::types::SpecialFileType::BlockDevice);
        }
        if ft.is_char_device() {
            return Some(crate::types::SpecialFileType::CharDevice);
        }
    }
    None
}

#[cfg(not(unix))]
pub fn get_special_file_type(_path: &Path) -> Option<crate::types::SpecialFileType> {
    None
}

/// Get symlink target
pub fn get_symlink_target(path: &Path) -> Option<PathBuf> {
    std::fs::read_link(path).ok()
}

/// Check if symlink is broken
pub fn is_symlink_broken(path: &Path) -> bool {
    if let Ok(target) = std::fs::read_link(path) {
        let resolved = if target.is_absolute() {
            target
        } else {
            path.parent().unwrap_or(Path::new("")).join(&target)
        };
        !resolved.exists()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_scanner_new_empty_patterns() {
        let scanner = Scanner::new(&[]).unwrap();
        assert!(!scanner.is_excluded(Path::new("anything.txt")));
    }

    #[test]
    fn test_scanner_new_invalid_pattern() {
        let result = Scanner::new(&["[invalid".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_scanner_exclusion_glob_patterns() {
        let scanner = Scanner::new(&[
            "*.log".to_string(),
            "node_modules".to_string(),
            ".git/**".to_string(),
        ])
        .unwrap();

        // Test glob patterns
        assert!(scanner.is_excluded(Path::new("debug.log")));
        assert!(scanner.is_excluded(Path::new("src/app.log")));
        assert!(scanner.is_excluded(Path::new("node_modules")));
        assert!(scanner.is_excluded(Path::new("node_modules/pkg")));
        assert!(scanner.is_excluded(Path::new(".git/config")));
        assert!(!scanner.is_excluded(Path::new("src/main.rs")));
        assert!(!scanner.is_excluded(Path::new("mylog.txt")));
    }

    #[test]
    fn test_scanner_exclusion_multiple_extensions() {
        let scanner = Scanner::new(&[
            "*.tmp".to_string(),
            "*.bak".to_string(),
            "*.swp".to_string(),
        ])
        .unwrap();

        assert!(scanner.is_excluded(Path::new("file.tmp")));
        assert!(scanner.is_excluded(Path::new("file.bak")));
        assert!(scanner.is_excluded(Path::new("file.swp")));
        assert!(!scanner.is_excluded(Path::new("file.txt")));
    }

    #[test]
    fn test_scanner_exclusion_directory_pattern() {
        let scanner = Scanner::new(&[
            "target/**".to_string(),
            "__pycache__".to_string(),
        ])
        .unwrap();

        assert!(scanner.is_excluded(Path::new("target/debug/main")));
        assert!(scanner.is_excluded(Path::new("__pycache__")));
        assert!(scanner.is_excluded(Path::new("src/__pycache__")));
        assert!(!scanner.is_excluded(Path::new("src/lib.rs")));
    }

    #[test]
    fn test_scanner_scan_basic() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create test structure
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("README.md"), "# Test").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "").unwrap();

        let scanner = Scanner::new(&["node_modules".to_string()]).unwrap();
        let entries = scanner.scan(root).unwrap();

        let paths: Vec<_> = entries.iter().map(|e| e.relative_path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("src")));
        assert!(paths.contains(&PathBuf::from("src/main.rs")));
        assert!(paths.contains(&PathBuf::from("README.md")));
        assert!(!paths.iter().any(|p| p.starts_with("node_modules")));
    }

    #[test]
    fn test_scanner_scan_empty_directory() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let scanner = Scanner::new(&[]).unwrap();
        let entries = scanner.scan(root).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_scanner_scan_nested_directories() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("a/b/c")).unwrap();
        fs::write(root.join("a/b/c/deep.txt"), "content").unwrap();

        let scanner = Scanner::new(&[]).unwrap();
        let entries = scanner.scan(root).unwrap();

        let paths: Vec<_> = entries.iter().map(|e| e.relative_path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("a")));
        assert!(paths.contains(&PathBuf::from("a/b")));
        assert!(paths.contains(&PathBuf::from("a/b/c")));
        assert!(paths.contains(&PathBuf::from("a/b/c/deep.txt")));
    }

    #[test]
    fn test_scanner_scan_identifies_directories() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir(root.join("subdir")).unwrap();
        fs::write(root.join("file.txt"), "content").unwrap();

        let scanner = Scanner::new(&[]).unwrap();
        let entries = scanner.scan(root).unwrap();

        let dir_entry = entries.iter().find(|e| e.relative_path == PathBuf::from("subdir")).unwrap();
        assert!(dir_entry.is_directory);

        let file_entry = entries.iter().find(|e| e.relative_path == PathBuf::from("file.txt")).unwrap();
        assert!(!file_entry.is_directory);
    }

    #[test]
    fn test_scanner_scan_union_basic() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();

        fs::write(dir1.path().join("file1.txt"), "content1").unwrap();
        fs::write(dir2.path().join("file2.txt"), "content2").unwrap();

        let scanner = Scanner::new(&[]).unwrap();
        let paths = scanner.scan_union(&[dir1.path(), dir2.path()]).unwrap();

        assert!(paths.contains(&PathBuf::from("file1.txt")));
        assert!(paths.contains(&PathBuf::from("file2.txt")));
    }

    #[test]
    fn test_scanner_scan_union_with_nonexistent() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();

        let nonexistent = Path::new("/nonexistent/path/12345");

        let scanner = Scanner::new(&[]).unwrap();
        let paths = scanner.scan_union(&[dir.path(), nonexistent]).unwrap();

        assert!(paths.contains(&PathBuf::from("file.txt")));
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn test_get_symlink_target() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "content").unwrap();

        // Test with regular file (not a symlink)
        let target = get_symlink_target(&file);
        assert!(target.is_none());
    }

    #[test]
    fn test_is_symlink_broken_regular_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "content").unwrap();

        // Regular file should return false
        assert!(!is_symlink_broken(&file));
    }

    #[cfg(unix)]
    #[test]
    fn test_get_symlink_target_unix() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.txt");
        let link = dir.path().join("link.txt");

        fs::write(&target, "content").unwrap();
        symlink(&target, &link).unwrap();

        let result = get_symlink_target(&link);
        assert!(result.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn test_is_symlink_broken_unix() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("target.txt");
        let valid_link = dir.path().join("valid_link.txt");
        let broken_link = dir.path().join("broken_link.txt");

        fs::write(&target, "content").unwrap();
        symlink(&target, &valid_link).unwrap();
        symlink(Path::new("/nonexistent/file"), &broken_link).unwrap();

        assert!(!is_symlink_broken(&valid_link));
        assert!(is_symlink_broken(&broken_link));
    }
}
