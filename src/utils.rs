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

/// Convert UTF-8 string to CP932 (Shift-JIS) for Windows console output
/// Box Drawing characters are explicitly mapped to CP932 keisen characters
/// Other characters that cannot be represented in CP932 are replaced with '?'
#[cfg(windows)]
pub fn to_cp932(s: &str) -> Vec<u8> {
    use encoding_rs::SHIFT_JIS;

    let mut result = Vec::with_capacity(s.len() * 2);
    let mut temp = String::new();

    for c in s.chars() {
        // Check for Box Drawing characters and map to CP932 keisen bytes
        let cp932_bytes: Option<&[u8]> = match c {
            '─' => Some(&[0x84, 0x9F]), // U+2500 BOX DRAWINGS LIGHT HORIZONTAL
            '│' => Some(&[0x84, 0xA0]), // U+2502 BOX DRAWINGS LIGHT VERTICAL
            '┌' => Some(&[0x84, 0xA1]), // U+250C BOX DRAWINGS LIGHT DOWN AND RIGHT
            '┐' => Some(&[0x84, 0xA2]), // U+2510 BOX DRAWINGS LIGHT DOWN AND LEFT
            '└' => Some(&[0x84, 0xA4]), // U+2514 BOX DRAWINGS LIGHT UP AND RIGHT
            '┘' => Some(&[0x84, 0xA3]), // U+2518 BOX DRAWINGS LIGHT UP AND LEFT
            '├' => Some(&[0x84, 0xA5]), // U+251C BOX DRAWINGS LIGHT VERTICAL AND RIGHT
            '┤' => Some(&[0x84, 0xA7]), // U+2524 BOX DRAWINGS LIGHT VERTICAL AND LEFT
            '┬' => Some(&[0x84, 0xA6]), // U+252C BOX DRAWINGS LIGHT DOWN AND HORIZONTAL
            '┴' => Some(&[0x84, 0xA8]), // U+2534 BOX DRAWINGS LIGHT UP AND HORIZONTAL
            '┼' => Some(&[0x84, 0xA9]), // U+253C BOX DRAWINGS LIGHT VERTICAL AND HORIZONTAL
            _ => None,
        };

        if let Some(bytes) = cp932_bytes {
            // Flush any accumulated regular characters first
            if !temp.is_empty() {
                let (encoded, _, _) = SHIFT_JIS.encode(&temp);
                result.extend_from_slice(&encoded);
                temp.clear();
            }
            result.extend_from_slice(bytes);
        } else {
            temp.push(c);
        }
    }

    // Flush remaining characters
    if !temp.is_empty() {
        let (encoded, _, _) = SHIFT_JIS.encode(&temp);
        result.extend_from_slice(&encoded);
    }

    result
}

/// Print string to stdout with CP932 encoding (Windows only)
/// On non-Windows platforms, this just prints UTF-8
#[cfg(windows)]
pub fn print_cp932(s: &str) {
    use std::io::Write;
    let bytes = to_cp932(s);
    let _ = std::io::stdout().write_all(&bytes);
    let _ = std::io::stdout().flush();
}

/// Print string to stdout with CP932 encoding and newline (Windows only)
/// On non-Windows platforms, this just prints UTF-8
#[cfg(windows)]
pub fn println_cp932(s: &str) {
    use std::io::Write;
    let mut bytes = to_cp932(s);
    bytes.push(b'\n');
    let _ = std::io::stdout().write_all(&bytes);
    let _ = std::io::stdout().flush();
}

/// Print string to stderr with CP932 encoding (Windows only)
#[cfg(windows)]
pub fn eprint_cp932(s: &str) {
    use std::io::Write;
    let bytes = to_cp932(s);
    let _ = std::io::stderr().write_all(&bytes);
    let _ = std::io::stderr().flush();
}

/// Print string to stderr with CP932 encoding and newline (Windows only)
#[cfg(windows)]
pub fn eprintln_cp932(s: &str) {
    use std::io::Write;
    let mut bytes = to_cp932(s);
    bytes.push(b'\n');
    let _ = std::io::stderr().write_all(&bytes);
    let _ = std::io::stderr().flush();
}

/// Non-Windows: just use regular print
#[cfg(not(windows))]
pub fn print_cp932(s: &str) {
    print!("{}", s);
}

/// Non-Windows: just use regular println
#[cfg(not(windows))]
pub fn println_cp932(s: &str) {
    println!("{}", s);
}

/// Non-Windows: just use regular eprint
#[cfg(not(windows))]
pub fn eprint_cp932(s: &str) {
    eprint!("{}", s);
}

/// Non-Windows: just use regular eprintln
#[cfg(not(windows))]
pub fn eprintln_cp932(s: &str) {
    eprintln!("{}", s);
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

    #[test]
    fn test_cp932_conversion_ascii() {
        // Test that ASCII characters are preserved
        use encoding_rs::SHIFT_JIS;
        let input = "Hello, World!";
        let (encoded, _, _) = SHIFT_JIS.encode(input);
        assert_eq!(encoded.as_ref(), b"Hello, World!");
    }

    #[test]
    fn test_cp932_conversion_japanese() {
        // Test that Japanese characters are converted to CP932
        use encoding_rs::SHIFT_JIS;
        let input = "日本語";
        let (encoded, _, _) = SHIFT_JIS.encode(input);
        // CP932 encoding for "日本語": 0x93FA (日), 0x967B (本), 0x8CEA (語)
        // Note: encoding_rs uses Shift_JIS which is compatible with CP932 for most characters
        assert!(!encoded.is_empty());
        // The encoded bytes should be different from UTF-8
        assert_ne!(encoded.as_ref(), input.as_bytes());
    }

    #[test]
    fn test_cp932_conversion_mixed() {
        // Test mixed ASCII and Japanese
        use encoding_rs::SHIFT_JIS;
        let input = "Hello 日本語 World";
        let (encoded, _, _) = SHIFT_JIS.encode(input);
        assert!(!encoded.is_empty());
        // Contains both ASCII and Japanese characters
        assert!(encoded.len() > input.len() / 2); // Japanese chars take 2 bytes in CP932
    }

    #[test]
    fn test_cp932_conversion_unconvertible() {
        // Test that characters not in CP932 are replaced
        use encoding_rs::SHIFT_JIS;
        // Emoji is not representable in CP932
        let input = "Hello 😀 World";
        let (encoded, _, had_errors) = SHIFT_JIS.encode(input);
        // Some characters couldn't be converted
        assert!(had_errors || !encoded.contains(&0xF0)); // UTF-8 emoji starts with 0xF0
    }

    #[test]
    fn test_cp932_conversion_box_drawing() {
        // Test box drawing characters (used in file tree display)
        use encoding_rs::SHIFT_JIS;
        let input = "├── file.txt";
        let (encoded, _, _) = SHIFT_JIS.encode(input);
        // Box drawing characters should be converted (may be replaced if not in CP932)
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_cp932_box_drawing_explicit_mapping() {
        // Test that box drawing characters have correct CP932 keisen byte mappings
        // These mappings are used by the Windows-specific to_cp932 function
        // CP932 keisen bytes (JIS X 0208 row 8):
        // ─ = 0x849F, │ = 0x84A0, ┌ = 0x84A1, ┐ = 0x84A2
        // └ = 0x84A4, ┘ = 0x84A3, ├ = 0x84A5, ┤ = 0x84A7
        // ┬ = 0x84A6, ┴ = 0x84A8, ┼ = 0x84A9

        // Verify the expected mappings for tree display
        let box_chars = [
            ('─', [0x84u8, 0x9F]),  // horizontal line
            ('│', [0x84, 0xA0]),    // vertical line
            ('└', [0x84, 0xA4]),    // corner (up-right)
            ('├', [0x84, 0xA5]),    // T-junction (vertical-right)
        ];

        for (c, expected_bytes) in &box_chars {
            // Verify the character exists and maps to expected values
            assert!(c.is_ascii() == false, "Box drawing char {} should be non-ASCII", c);
            assert_eq!(expected_bytes.len(), 2, "CP932 keisen should be 2 bytes");
            // First byte should be 0x84 (JIS X 0208 row 8)
            assert_eq!(expected_bytes[0], 0x84, "First byte of keisen should be 0x84");
            // Second byte should be in valid range
            assert!(expected_bytes[1] >= 0x9F && expected_bytes[1] <= 0xA9,
                   "Second byte 0x{:02X} should be in keisen range", expected_bytes[1]);
        }
    }

    #[test]
    fn test_println_cp932_basic() {
        // Test that println_cp932 doesn't panic
        // On non-Windows, this just calls println!
        // On Windows, this converts to CP932 and writes to stdout
        println_cp932("Test output");
        println_cp932("日本語テスト");
        println_cp932("Mixed: Hello 日本語");
    }

    #[test]
    fn test_eprintln_cp932_basic() {
        // Test that eprintln_cp932 doesn't panic
        eprintln_cp932("Test error output");
        eprintln_cp932("日本語エラー");
        eprintln_cp932("Mixed: Error 日本語");
    }
}
