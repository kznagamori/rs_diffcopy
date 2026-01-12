use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use unicode_width::UnicodeWidthChar;

/// Calculate display width considering CJK characters and box drawing characters
/// Box drawing characters (U+2500-U+257F) are treated as width 2 for Japanese terminal compatibility
pub fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            // Box Drawing characters (U+2500-U+257F) are displayed as width 2 in many CJK terminals
            if ('\u{2500}'..='\u{257F}').contains(&c) {
                2
            } else {
                UnicodeWidthChar::width(c).unwrap_or(0)
            }
        })
        .sum()
}

/// Pad string to specified width considering CJK characters
#[allow(dead_code)]
pub fn pad_to_width(s: &str, width: usize) -> String {
    let current_width = display_width(s);
    if current_width >= width {
        s.to_string()
    } else {
        let padding = width - current_width;
        format!("{}{}", s, " ".repeat(padding))
    }
}

/// Calculate BLAKE3 hash of a file
pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

/// Check if file is binary by looking for NULL bytes in first 8KB
pub fn is_binary_file(path: &Path) -> std::io::Result<bool> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192];

    let bytes_read = reader.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(false);
    }

    // Check for NULL bytes
    if buffer[..bytes_read].contains(&0) {
        return Ok(true);
    }

    // Try to decode as UTF-8
    Ok(std::str::from_utf8(&buffer[..bytes_read]).is_err())
}

/// Check if file extension indicates a script file
pub fn is_script_file(path: &Path) -> bool {
    let script_extensions = [
        "sh", "bash", "zsh", "ksh", "fish", // Shell
        "pl", "pm", // Perl
        "py", "pyw", // Python
        "rb",   // Ruby
        "js", "mjs", // JavaScript
        "ts",   // TypeScript
        "php",  // PHP
        "lua",  // Lua
        "ps1", "psm1", // PowerShell
        "bat", "cmd", // Windows batch
        "exe", "com", // Executables
    ];

    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| script_extensions.contains(&ext.to_lowercase().as_str()))
}

/// Format file size for human readable output
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Count files and directories in a path
pub fn count_directory_contents(path: &Path) -> std::io::Result<(usize, usize, u64)> {
    let mut files = 0;
    let mut dirs = 0;
    let mut total_size = 0u64;

    if !path.exists() {
        return Ok((0, 0, 0));
    }

    for entry in walkdir::WalkDir::new(path).min_depth(1) {
        match entry {
            Ok(e) => {
                if e.file_type().is_file() {
                    files += 1;
                    if let Ok(meta) = e.metadata() {
                        total_size += meta.len();
                    }
                } else if e.file_type().is_dir() {
                    dirs += 1;
                }
            }
            Err(_) => continue,
        }
    }

    Ok((files, dirs, total_size))
}

/// Get file mode as string (Unix)
#[cfg(unix)]
pub fn get_file_mode(path: &Path) -> std::io::Result<String> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path)?;
    let mode = meta.permissions().mode() & 0o777;
    Ok(format!("{:o}", mode))
}

/// Get file mode as string (Windows - read-only attribute)
#[cfg(windows)]
pub fn get_file_mode(path: &Path) -> std::io::Result<String> {
    let meta = std::fs::metadata(path)?;
    if meta.permissions().readonly() {
        Ok("readonly".to_string())
    } else {
        Ok("writable".to_string())
    }
}

/// Check if stdout is connected to a terminal
pub fn is_terminal() -> bool {
    is_terminal::is_terminal(&std::io::stdout())
}

/// Check if running in a pipe or redirect context
pub fn is_piped() -> bool {
    !is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_display_width() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("日本語"), 6); // Each CJK char is 2-width
        assert_eq!(display_width("hello日本語"), 11);
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width("한글"), 4); // Korean characters are also 2-width
    }

    #[test]
    fn test_display_width_halfwidth_katakana() {
        // Half-width katakana should be 1-width each
        assert_eq!(display_width("ｱｲｳｴｵ"), 5); // Half-width katakana
        assert_eq!(display_width("アイウエオ"), 10); // Full-width katakana (2-width each)
        assert_eq!(display_width("ｱｲｳ日本語"), 9); // Mixed: 3 half-width + 3 full-width
    }

    #[test]
    fn test_display_width_tree_connectors() {
        // Tree connectors (Box Drawing characters U+2500-U+257F) are width 2 for CJK terminal compatibility
        // ├, └, │, ─ are all width 2
        assert_eq!(display_width("├── "), 7);  // ├(2) + ─(2) + ─(2) + space(1) = 7
        assert_eq!(display_width("└── "), 7);  // └(2) + ─(2) + ─(2) + space(1) = 7
        assert_eq!(display_width("│   "), 5);  // │(2) + space(1) + space(1) + space(1) = 5
        assert_eq!(display_width("├── file.txt"), 15); // 7 + 8 = 15
        // 日本語 = 6 width (each CJK is 2), .txt = 4 width
        assert_eq!(display_width("├── 日本語.txt"), 17); // 7 + 6 + 4 = 17
    }

    #[test]
    fn test_pad_to_width() {
        assert_eq!(pad_to_width("abc", 5), "abc  ");
        assert_eq!(pad_to_width("日本", 6), "日本  ");
        assert_eq!(pad_to_width("toolong", 3), "toolong"); // Longer than width returns as-is
        assert_eq!(pad_to_width("exact", 5), "exact");
        assert_eq!(pad_to_width("", 3), "   ");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
        assert_eq!(format_size(2147483648), "2.0 GB");
    }

    #[test]
    fn test_is_script_file() {
        // Shell scripts
        assert!(is_script_file(Path::new("test.sh")));
        assert!(is_script_file(Path::new("test.bash")));
        assert!(is_script_file(Path::new("test.zsh")));

        // Python
        assert!(is_script_file(Path::new("test.py")));
        assert!(is_script_file(Path::new("test.pyw")));

        // JavaScript/TypeScript
        assert!(is_script_file(Path::new("test.js")));
        assert!(is_script_file(Path::new("test.mjs")));
        assert!(is_script_file(Path::new("test.ts")));

        // Other scripts
        assert!(is_script_file(Path::new("test.rb")));
        assert!(is_script_file(Path::new("test.php")));
        assert!(is_script_file(Path::new("test.lua")));
        assert!(is_script_file(Path::new("test.pl")));
        assert!(is_script_file(Path::new("test.ps1")));
        assert!(is_script_file(Path::new("test.bat")));
        assert!(is_script_file(Path::new("test.cmd")));

        // Non-scripts
        assert!(!is_script_file(Path::new("test.txt")));
        assert!(!is_script_file(Path::new("test.rs")));
        assert!(!is_script_file(Path::new("test.c")));
        assert!(!is_script_file(Path::new("test.h")));
        assert!(!is_script_file(Path::new("noextension")));

        // Case insensitive
        assert!(is_script_file(Path::new("test.SH")));
        assert!(is_script_file(Path::new("test.PY")));
    }

    #[test]
    fn test_hash_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "hello world").unwrap();

        let hash = hash_file(&file).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // BLAKE3 produces 64 hex chars

        // Same content should produce same hash
        let file2 = dir.path().join("test2.txt");
        fs::write(&file2, "hello world").unwrap();
        let hash2 = hash_file(&file2).unwrap();
        assert_eq!(hash, hash2);

        // Different content should produce different hash
        let file3 = dir.path().join("test3.txt");
        fs::write(&file3, "different content").unwrap();
        let hash3 = hash_file(&file3).unwrap();
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_hash_file_empty() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty.txt");
        fs::write(&file, "").unwrap();

        let hash = hash_file(&file).unwrap();
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_hash_file_nonexistent() {
        let result = hash_file(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_is_binary_file_text() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("text.txt");
        fs::write(&file, "This is a text file with UTF-8 content.\n日本語も含む。").unwrap();

        assert!(!is_binary_file(&file).unwrap());
    }

    #[test]
    fn test_is_binary_file_binary() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("binary.bin");
        // Write some bytes including NULL
        fs::write(&file, &[0x00, 0x01, 0x02, 0xFF]).unwrap();

        assert!(is_binary_file(&file).unwrap());
    }

    #[test]
    fn test_is_binary_file_empty() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty.txt");
        fs::write(&file, "").unwrap();

        // Empty file is considered text
        assert!(!is_binary_file(&file).unwrap());
    }

    #[test]
    fn test_count_directory_contents() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create test structure
        fs::create_dir_all(root.join("subdir1")).unwrap();
        fs::create_dir_all(root.join("subdir2/nested")).unwrap();
        fs::write(root.join("file1.txt"), "content").unwrap();
        fs::write(root.join("subdir1/file2.txt"), "content").unwrap();
        fs::write(root.join("subdir2/nested/file3.txt"), "content").unwrap();

        let (files, dirs, size) = count_directory_contents(root).unwrap();
        assert_eq!(files, 3);
        assert_eq!(dirs, 3); // subdir1, subdir2, nested
        assert!(size > 0);
    }

    #[test]
    fn test_count_directory_contents_empty() {
        let dir = tempdir().unwrap();
        let (files, dirs, size) = count_directory_contents(dir.path()).unwrap();
        assert_eq!(files, 0);
        assert_eq!(dirs, 0);
        assert_eq!(size, 0);
    }

    #[test]
    fn test_count_directory_contents_nonexistent() {
        let (files, dirs, size) = count_directory_contents(Path::new("/nonexistent/path")).unwrap();
        assert_eq!(files, 0);
        assert_eq!(dirs, 0);
        assert_eq!(size, 0);
    }

    #[test]
    fn test_get_file_mode() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "content").unwrap();

        let mode = get_file_mode(&file).unwrap();
        assert!(!mode.is_empty());
    }
}
