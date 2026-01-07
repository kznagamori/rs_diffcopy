use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::{Parser, ValueEnum};
use filetime::{FileTime, set_file_mtime};
use glob::Pattern;
use rayon::prelude::*;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
#[cfg(windows)]
use std::io::IsTerminal;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use walkdir::WalkDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Permission check mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionCheckMode {
    /// Do not check permissions (default)
    #[default]
    None,
    /// Check only script files
    Scripts,
    /// Check all files
    All,
}

/// Merge style for three-way comparison conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeStyle {
    /// Copy all versions (.base/.ours/.theirs) for conflicts
    #[default]
    All,
    /// Prefer ours version for conflicts
    Ours,
    /// Prefer theirs version for conflicts
    Theirs,
}

/// Script file extensions for permission checking
const SCRIPT_EXTENSIONS: &[&str] = &[
    // Shell scripts
    "sh", "bash", "zsh", "ksh", "fish",
    // Perl
    "pl", "pm",
    // Python
    "py", "pyw",
    // Ruby
    "rb",
    // JavaScript/Node.js
    "js", "mjs",
    // TypeScript
    "ts",
    // PHP
    "php",
    // Lua
    "lua",
    // PowerShell
    "ps1", "psm1",
    // Windows batch
    "bat", "cmd",
    // Executables
    "exe", "com",
];

/// Dangerous paths that should never be deleted (Linux/Unix)
#[cfg(unix)]
const DANGEROUS_PATHS_UNIX: &[&str] = &[
    "/",
    "/bin",
    "/boot",
    "/dev",
    "/etc",
    "/home",
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/media",
    "/mnt",
    "/opt",
    "/proc",
    "/root",
    "/run",
    "/sbin",
    "/srv",
    "/sys",
    "/tmp",
    "/usr",
    "/var",
];

/// Dangerous paths that should never be deleted (Windows)
#[cfg(windows)]
const DANGEROUS_PATHS_WINDOWS: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "system volume information",
    "$recycle.bin",
];

/// Check if stdout is a terminal
fn is_terminal() -> bool {
    #[cfg(windows)]
    {
        std::io::stdout().is_terminal()
    }
    #[cfg(not(windows))]
    {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal()
    }
}

/// Print string to stdout, converting to CP932 on Windows console
fn print_to_stdout(s: &str) {
    #[cfg(windows)]
    {
        if std::io::stdout().is_terminal() {
            // Windows console: convert UTF-8 to CP932 (Shift-JIS)
            let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(s);
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            let _ = handle.write_all(&encoded);
            let _ = handle.flush();
        } else {
            // Pipe or file: output as UTF-8
            print!("{}", s);
            let _ = std::io::stdout().flush();
        }
    }
    #[cfg(not(windows))]
    {
        print!("{}", s);
        let _ = std::io::stdout().flush();
    }
}

/// Print progress bar with phase info (thread-safe version using atomic counter)
fn print_progress_atomic(counter: &AtomicUsize, total: usize, phase: usize, total_phases: usize, task: &str) {
    if !is_terminal() || total == 0 {
        return;
    }

    let current = counter.load(Ordering::Relaxed);
    let percentage = (current * 100) / total;
    let bar_width = 30;
    let filled = (current * bar_width) / total;
    let empty = bar_width - filled;

    let bar: String = "=".repeat(filled) + if filled < bar_width { ">" } else { "" } + &" ".repeat(if empty > 0 { empty - 1 } else { 0 });

    print_to_stdout(&format!(
        "\r[{}/{}] {}: [{}] {}% ({}/{})",
        phase, total_phases, task, &bar[..bar_width.min(bar.len())], percentage, current, total
    ));
}

/// Clear the progress line
fn clear_progress_line() {
    if is_terminal() {
        print_to_stdout("\r\x1b[K"); // Clear line
    }
}

/// Print phase header
fn print_phase(phase: usize, total_phases: usize, message: &str) {
    if is_terminal() {
        println_to_stdout(&format!("[{}/{}] {}", phase, total_phases, message));
    }
}

/// Print string with newline to stdout, converting to CP932 on Windows console
fn println_to_stdout(s: &str) {
    #[cfg(windows)]
    {
        if std::io::stdout().is_terminal() {
            // Windows console: convert UTF-8 to CP932 (Shift-JIS)
            let with_newline = format!("{}\n", s);
            let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(&with_newline);
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            let _ = handle.write_all(&encoded);
            let _ = handle.flush();
        } else {
            // Pipe or file: output as UTF-8
            println!("{}", s);
        }
    }
    #[cfg(not(windows))]
    {
        println!("{}", s);
    }
}

#[derive(Parser, Debug)]
#[command(name = "rs_diffcopy")]
#[command(version = "1.0.0")]
#[command(about = "Compare two directories and extract files with differences")]
#[command(long_about = "A tool to compare two directories (source and target) and extract only the files with differences while maintaining the directory structure.")]
struct Args {
    /// Source directory (before)
    #[arg(short = 'S', long, value_name = "PATH")]
    source: Option<PathBuf>,

    /// Target directory (after)
    #[arg(short = 'T', long, value_name = "PATH")]
    target: Option<PathBuf>,

    /// Output directory for diff files
    #[arg(short = 'O', long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Configuration file (TOML format)
    #[arg(short = 'c', long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Exclude patterns (glob format, can be specified multiple times)
    #[arg(short, long, value_name = "PATTERN")]
    exclude: Vec<String>,

    /// Force: delete output directory if it exists and re-run
    #[arg(short, long)]
    force: bool,

    /// Output summary to file (default: stdout)
    #[arg(short, long, value_name = "PATH")]
    summary: Option<PathBuf>,

    /// Verbose mode: show processing file names
    #[arg(short, long)]
    verbose: bool,

    /// Dry-run: show target files without copying
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Copy both old and new versions of modified files (adds .old/.new extensions)
    #[arg(short = 'b', long)]
    both_versions: bool,

    /// Check file permissions/attributes for changes (none/scripts/all)
    #[arg(short = 'P', long, value_enum, default_value = "none")]
    check_permissions: PermissionCheckMode,

    /// Generate individual patch files for modified files (output alongside copied files)
    #[arg(short = 'p', long)]
    patch: bool,

    /// Generate combined patch file (all patches in one file)
    #[arg(short = 'F', long, value_name = "PATH")]
    patch_file: Option<PathBuf>,

    /// Output summary to Excel file (.xlsx)
    #[arg(short = 'E', long, value_name = "PATH")]
    excel: Option<PathBuf>,

    /// Fold level for Excel file tree (rows deeper than this level will be collapsed)
    #[arg(short = 'L', long = "excel-fold-level", value_name = "LEVEL")]
    excel_fold_level: Option<u16>,

    /// Show unchanged files in summary output
    #[arg(short = 'u', long)]
    show_unchanged: bool,

    /// Save current options to a config file (TOML format)
    #[arg(short = 'C', long, value_name = "PATH")]
    save_config: Option<PathBuf>,

    /// Enable three-way comparison mode (requires --base)
    #[arg(short = '3', long)]
    three_way: bool,

    /// Base directory (common ancestor) for three-way comparison
    #[arg(short = 'B', long, value_name = "PATH")]
    base: Option<PathBuf>,

    /// Merge style for conflicts in three-way mode (all/ours/theirs)
    #[arg(short = 'M', long, value_enum, default_value = "all")]
    merge_style: MergeStyle,

    /// Only output conflict files in three-way mode
    #[arg(long)]
    conflict_only: bool,

    /// Filter output by status (can be specified multiple times)
    /// Two-way: added, modified, deleted, unchanged, symlink, special, error, permission
    /// Three-way: unchanged, ours-only, theirs-only, both-same, conflict, added-ours, added-theirs, added-both, deleted-ours, deleted-theirs, deleted-both
    #[arg(long = "filter-status", value_name = "STATUS")]
    filter_status: Vec<String>,

    /// Show only statistics (hide File Tree and detail sections)
    #[arg(long)]
    stats_only: bool,

    /// Hide File Tree section in output
    #[arg(long)]
    no_tree: bool,

    /// Hide detail sections (Added/Modified/Deleted Files etc.) in output
    #[arg(long)]
    no_details: bool,

    /// Copy deleted files (files only in source) to output directory
    #[arg(long)]
    copy_deleted: bool,

    /// Preserve file timestamps when copying
    #[arg(long)]
    preserve_timestamps: bool,
}

/// Configuration file structure (TOML format)
#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    source: Option<String>,
    target: Option<String>,
    output: Option<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    force: bool,
    #[serde(default)]
    verbose: bool,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    both_versions: bool,
    summary: Option<String>,
    #[serde(default)]
    check_permissions: PermissionCheckMode,
    #[serde(default)]
    patch: bool,
    patch_file: Option<String>,
    excel: Option<String>,
    excel_fold_level: Option<u16>,
    #[serde(default)]
    show_unchanged: bool,
    // Three-way comparison options
    #[serde(default)]
    three_way: bool,
    base: Option<String>,
    #[serde(default)]
    merge_style: MergeStyle,
    #[serde(default)]
    conflict_only: bool,
    // Output filter options
    #[serde(default)]
    filter_status: Vec<String>,
    #[serde(default)]
    stats_only: bool,
    #[serde(default)]
    no_tree: bool,
    #[serde(default)]
    no_details: bool,
    // Copy options
    #[serde(default)]
    copy_deleted: bool,
    #[serde(default)]
    preserve_timestamps: bool,
}

/// Resolved configuration after merging CLI args and config file
struct ResolvedConfig {
    source_dir: PathBuf,
    target_dir: PathBuf,
    output_dir: PathBuf,
    exclude: Vec<String>,
    force: bool,
    verbose: bool,
    dry_run: bool,
    both_versions: bool,
    summary: Option<PathBuf>,
    check_permissions: PermissionCheckMode,
    config_file: Option<PathBuf>,
    patch: bool,
    patch_file: Option<PathBuf>,
    excel: Option<PathBuf>,
    excel_fold_level: Option<u16>,
    show_unchanged: bool,
    save_config: Option<PathBuf>,
    // Three-way comparison options
    three_way: bool,
    base_dir: Option<PathBuf>,
    merge_style: MergeStyle,
    conflict_only: bool,
    // Output filter options
    filter_status: Vec<String>,
    stats_only: bool,
    no_tree: bool,
    no_details: bool,
    // Copy options
    copy_deleted: bool,
    preserve_timestamps: bool,
}

impl ResolvedConfig {
    /// Create resolved config from CLI args and optional config file
    fn from_args(args: Args) -> Result<Self> {
        // Load config file if specified
        let config_file = if let Some(config_path) = &args.config {
            let content = fs::read_to_string(config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            toml::from_str::<ConfigFile>(&content)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?
        } else {
            ConfigFile::default()
        };

        // Merge CLI args with config file (CLI takes precedence)
        let source_dir = args.source
            .or_else(|| config_file.source.map(PathBuf::from))
            .ok_or_else(|| anyhow::anyhow!("Source directory is required. Use --source or specify in config file."))?;

        let target_dir = args.target
            .or_else(|| config_file.target.map(PathBuf::from))
            .ok_or_else(|| anyhow::anyhow!("Target directory is required. Use --target or specify in config file."))?;

        let output_dir = args.output
            .or_else(|| config_file.output.map(PathBuf::from))
            .ok_or_else(|| anyhow::anyhow!("Output directory is required. Use --output or specify in config file."))?;

        // Merge exclude patterns (combine both)
        let mut exclude = args.exclude;
        exclude.extend(config_file.exclude);

        // Boolean flags: CLI true overrides, otherwise use config
        let force = args.force || config_file.force;
        let verbose = args.verbose || config_file.verbose;
        let dry_run = args.dry_run || config_file.dry_run;
        let both_versions = args.both_versions || config_file.both_versions;

        // Summary: CLI takes precedence
        let summary = args.summary
            .or_else(|| config_file.summary.map(PathBuf::from));

        // Check permissions: CLI takes precedence if not None
        let check_permissions = if args.check_permissions != PermissionCheckMode::None {
            args.check_permissions
        } else {
            config_file.check_permissions
        };

        // Patch options
        let patch = args.patch || config_file.patch;
        let patch_file = args.patch_file
            .or_else(|| config_file.patch_file.map(PathBuf::from));

        // Excel output
        let excel = args.excel
            .or_else(|| config_file.excel.map(PathBuf::from));

        // Excel fold level (CLI takes precedence)
        let excel_fold_level = args.excel_fold_level
            .or(config_file.excel_fold_level);

        // Show unchanged files
        let show_unchanged = args.show_unchanged || config_file.show_unchanged;

        // Three-way comparison options
        let three_way = args.three_way || config_file.three_way;
        let base_dir = args.base
            .or_else(|| config_file.base.map(PathBuf::from));
        let merge_style = if args.merge_style != MergeStyle::All {
            args.merge_style
        } else {
            config_file.merge_style
        };
        let conflict_only = args.conflict_only || config_file.conflict_only;

        // Output filter options
        let mut filter_status = args.filter_status;
        if filter_status.is_empty() {
            filter_status = config_file.filter_status;
        }
        let stats_only = args.stats_only || config_file.stats_only;
        let no_tree = args.no_tree || config_file.no_tree;
        let no_details = args.no_details || config_file.no_details;

        // Copy options
        let copy_deleted = args.copy_deleted || config_file.copy_deleted;
        let preserve_timestamps = args.preserve_timestamps || config_file.preserve_timestamps;

        // Validate three-way mode requirements
        if three_way && base_dir.is_none() {
            bail!("Base directory is required for three-way mode. Use --base or specify in config file.");
        }

        Ok(Self {
            source_dir,
            target_dir,
            output_dir,
            exclude,
            force,
            verbose,
            dry_run,
            both_versions,
            summary,
            check_permissions,
            config_file: args.config,
            patch,
            patch_file,
            excel,
            excel_fold_level,
            show_unchanged,
            save_config: args.save_config,
            three_way,
            base_dir,
            merge_style,
            conflict_only,
            filter_status,
            stats_only,
            no_tree,
            no_details,
            copy_deleted,
            preserve_timestamps,
        })
    }

    /// Generate TOML config file content
    fn generate_config_content(&self) -> String {
        let mut content = String::new();

        // Header comment
        content.push_str("# rs_diffcopy 設定ファイル\n");
        content.push_str("# このファイルは --save-config オプションにより自動生成されました\n");
        content.push_str("# 設定を変更して再利用することができます\n");
        content.push_str("\n");

        // Required settings
        content.push_str("# 必須設定\n");
        content.push_str(&format!(
            "source = \"{}\"  # 比較元ディレクトリ（変更する場合はパスを修正してください）\n",
            self.source_dir.display()
        ));
        content.push_str(&format!(
            "target = \"{}\"  # 比較先ディレクトリ（変更する場合はパスを修正してください）\n",
            self.target_dir.display()
        ));
        content.push_str(&format!(
            "output = \"{}\"  # 出力ディレクトリ（変更する場合はパスを修正してください）\n",
            self.output_dir.display()
        ));
        content.push_str("\n");

        // Optional settings
        content.push_str("# オプション設定\n");
        content.push_str(&format!("force = {}\n", self.force));
        content.push_str(&format!("verbose = {}\n", self.verbose));

        // dry_run is always commented out
        content.push_str("# 注: dry_run はこの設定ファイルでは無効になっています\n");
        content.push_str("# 必要に応じてコメントを外してください\n");
        content.push_str(&format!("# dry_run = {}\n", self.dry_run));

        content.push_str(&format!("both_versions = {}\n", self.both_versions));

        // Summary file
        if let Some(ref summary_path) = self.summary {
            content.push_str(&format!("summary = \"{}\"\n", summary_path.display()));
        } else {
            content.push_str("# summary = \"./summary.txt\"  # サマリー出力ファイル\n");
        }

        // Permission check mode
        let check_perm_str = match self.check_permissions {
            PermissionCheckMode::None => "none",
            PermissionCheckMode::Scripts => "scripts",
            PermissionCheckMode::All => "all",
        };
        content.push_str(&format!("check_permissions = \"{}\"  # none / scripts / all\n", check_perm_str));

        // Patch options
        content.push_str(&format!("patch = {}  # 個別パッチファイル生成\n", self.patch));
        if let Some(ref patch_file) = self.patch_file {
            content.push_str(&format!("patch_file = \"{}\"  # 統合パッチファイル\n", patch_file.display()));
        } else {
            content.push_str("# patch_file = \"\"  # 統合パッチファイル（空欄で無効）\n");
        }

        // Excel output
        if let Some(ref excel_path) = self.excel {
            content.push_str(&format!("excel = \"{}\"\n", excel_path.display()));
        } else {
            content.push_str("# excel = \"\"  # Excel出力ファイル（空欄で無効）\n");
        }
        if let Some(level) = self.excel_fold_level {
            content.push_str(&format!("excel_fold_level = {}  # Excelファイルツリーの折りたたみレベル\n", level));
        } else {
            content.push_str("# excel_fold_level = 2  # Excelファイルツリーの折りたたみレベル（省略時は折りたたみなし）\n");
        }

        // Show unchanged
        content.push_str(&format!("show_unchanged = {}  # 変更なしファイルをサマリーに表示\n", self.show_unchanged));
        content.push_str("\n");

        // Three-way comparison options
        content.push_str("# 三者間比較オプション\n");
        content.push_str(&format!("three_way = {}  # 三者間比較モード\n", self.three_way));
        if let Some(ref base_path) = self.base_dir {
            content.push_str(&format!("base = \"{}\"  # 共通祖先ディレクトリ\n", base_path.display()));
        } else {
            content.push_str("# base = \"./base_version\"  # 共通祖先ディレクトリ\n");
        }
        let merge_style_str = match self.merge_style {
            MergeStyle::All => "all",
            MergeStyle::Ours => "ours",
            MergeStyle::Theirs => "theirs",
        };
        content.push_str(&format!("merge_style = \"{}\"  # all / ours / theirs\n", merge_style_str));
        content.push_str(&format!("conflict_only = {}  # コンフリクトのみ出力\n", self.conflict_only));
        content.push_str("\n");

        // Output filter options
        content.push_str("# 出力フィルター設定\n");
        if self.filter_status.is_empty() {
            content.push_str("# filter_status = [\"added\", \"modified\"]  # 表示するステータス（省略時は全て表示）\n");
        } else {
            content.push_str("filter_status = [\n");
            for status in &self.filter_status {
                content.push_str(&format!("    \"{}\",\n", status));
            }
            content.push_str("]\n");
        }
        content.push_str(&format!("stats_only = {}  # 統計情報のみ表示\n", self.stats_only));
        content.push_str(&format!("no_tree = {}  # File Treeセクション非表示\n", self.no_tree));
        content.push_str(&format!("no_details = {}  # 詳細セクション非表示\n", self.no_details));
        content.push_str("\n");

        // Copy options
        content.push_str("# コピーオプション\n");
        content.push_str(&format!("copy_deleted = {}  # 削除ファイルもコピー\n", self.copy_deleted));
        content.push_str(&format!("preserve_timestamps = {}  # タイムスタンプを保持\n", self.preserve_timestamps));
        content.push_str("\n");

        // Exclude patterns
        content.push_str("# 除外パターン（glob形式、複数指定可）\n");
        if self.exclude.is_empty() {
            content.push_str("# exclude = [\n");
            content.push_str("#     \"*.log\",\n");
            content.push_str("#     \"*.tmp\",\n");
            content.push_str("#     \"node_modules\",\n");
            content.push_str("#     \".git\",\n");
            content.push_str("# ]\n");
        } else {
            content.push_str("exclude = [\n");
            for pattern in &self.exclude {
                content.push_str(&format!("    \"{}\",\n", pattern));
            }
            content.push_str("]\n");
        }

        content
    }

    /// Save config to file
    fn save_config_file(&self, path: &Path) -> Result<()> {
        let content = self.generate_config_content();
        fs::write(path, &content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;
        Ok(())
    }
}

/// Symlink change type
#[derive(Debug, Clone, PartialEq, Eq)]
enum SymlinkChangeType {
    Added,
    Deleted,
    Changed,
}

/// Symlink information
#[derive(Debug, Clone, PartialEq, Eq)]
struct SymlinkInfo {
    target: PathBuf,
    exists: bool,
    is_dir: bool,
}

/// Special file type (socket, fifo, device, etc.)
#[derive(Debug, Clone, PartialEq, Eq)]
enum SpecialFileType {
    Socket,
    Fifo,
    BlockDevice,
    CharDevice,
    Unknown,
}

impl std::fmt::Display for SpecialFileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecialFileType::Socket => write!(f, "socket"),
            SpecialFileType::Fifo => write!(f, "fifo"),
            SpecialFileType::BlockDevice => write!(f, "block device"),
            SpecialFileType::CharDevice => write!(f, "char device"),
            SpecialFileType::Unknown => write!(f, "special file"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileStatus {
    Added,
    Modified,
    Deleted,
    Unchanged,
    Symlink {
        change_type: SymlinkChangeType,
        current: Option<SymlinkInfo>,  // None if deleted
        previous: Option<SymlinkInfo>, // None if added, Some if changed/deleted
    },
    SpecialFile {
        file_type: SpecialFileType,
    },
    PermissionDenied { error: String },
}

#[derive(Debug, Clone)]
struct DiffEntry {
    relative_path: PathBuf,
    is_dir: bool,
    status: FileStatus,
}

/// Permission change entry
#[derive(Debug, Clone)]
struct PermissionChange {
    relative_path: PathBuf,
    old_mode: String,
    new_mode: String,
}

struct DiffResult {
    entries: Vec<DiffEntry>,
    permission_changes: Vec<PermissionChange>,
    source_dir: PathBuf,
    target_dir: PathBuf,
    /// Total files/dirs in source (excluding excluded patterns)
    source_count: usize,
    /// Total files/dirs in target (excluding excluded patterns)
    target_count: usize,
    /// Files/dirs that exist in both source and target
    common_count: usize,
}

// ============================================================================
// Three-way comparison data structures
// ============================================================================

/// Three-way comparison file status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreeWayStatus {
    /// All three versions are identical
    Unchanged,
    /// Only ours differs from base (theirs == base)
    OursOnly,
    /// Only theirs differs from base (ours == base)
    TheirsOnly,
    /// Both changed the same way (ours == theirs, both != base)
    BothSame,
    /// Both changed differently (ours != theirs, both != base) - CONFLICT
    Conflict,
    /// File added only in ours
    AddedOurs,
    /// File added only in theirs
    AddedTheirs,
    /// File added in both with same content
    AddedBothSame,
    /// File added in both with different content - CONFLICT
    AddedBothDiff,
    /// File deleted only in ours
    DeletedOurs,
    /// File deleted only in theirs
    DeletedTheirs,
    /// File deleted in both
    DeletedBoth,
    /// File modified in ours, deleted in theirs - CONFLICT
    ModifyDelete,
    /// File deleted in ours, modified in theirs - CONFLICT
    DeleteModify,
}

impl ThreeWayStatus {
    /// Returns true if this status represents a conflict
    pub fn is_conflict(&self) -> bool {
        matches!(
            self,
            ThreeWayStatus::Conflict
                | ThreeWayStatus::AddedBothDiff
                | ThreeWayStatus::ModifyDelete
                | ThreeWayStatus::DeleteModify
        )
    }

    /// Returns a display string for the status
    pub fn display_str(&self) -> &'static str {
        match self {
            ThreeWayStatus::Unchanged => "unchanged",
            ThreeWayStatus::OursOnly => "ours-only",
            ThreeWayStatus::TheirsOnly => "theirs-only",
            ThreeWayStatus::BothSame => "both-same",
            ThreeWayStatus::Conflict => "CONFLICT",
            ThreeWayStatus::AddedOurs => "added-ours",
            ThreeWayStatus::AddedTheirs => "added-theirs",
            ThreeWayStatus::AddedBothSame => "added-both-same",
            ThreeWayStatus::AddedBothDiff => "CONFLICT (added-both-diff)",
            ThreeWayStatus::DeletedOurs => "deleted-ours",
            ThreeWayStatus::DeletedTheirs => "deleted-theirs",
            ThreeWayStatus::DeletedBoth => "deleted-both",
            ThreeWayStatus::ModifyDelete => "CONFLICT (modify-delete)",
            ThreeWayStatus::DeleteModify => "CONFLICT (delete-modify)",
        }
    }

    /// Returns a short matrix indicator for base
    pub fn base_indicator(&self) -> &'static str {
        match self {
            ThreeWayStatus::AddedOurs
            | ThreeWayStatus::AddedTheirs
            | ThreeWayStatus::AddedBothSame
            | ThreeWayStatus::AddedBothDiff => "-",
            _ => "○",
        }
    }

    /// Returns a short matrix indicator for ours
    pub fn ours_indicator(&self) -> &'static str {
        match self {
            ThreeWayStatus::Unchanged | ThreeWayStatus::TheirsOnly => "=",
            ThreeWayStatus::OursOnly | ThreeWayStatus::BothSame | ThreeWayStatus::Conflict | ThreeWayStatus::ModifyDelete => "M",
            ThreeWayStatus::AddedOurs | ThreeWayStatus::AddedBothSame | ThreeWayStatus::AddedBothDiff => "A",
            ThreeWayStatus::DeletedOurs | ThreeWayStatus::DeletedBoth | ThreeWayStatus::DeleteModify => "D",
            ThreeWayStatus::AddedTheirs | ThreeWayStatus::DeletedTheirs => "-",
        }
    }

    /// Returns a short matrix indicator for theirs
    pub fn theirs_indicator(&self) -> &'static str {
        match self {
            ThreeWayStatus::Unchanged | ThreeWayStatus::OursOnly => "=",
            ThreeWayStatus::TheirsOnly | ThreeWayStatus::BothSame | ThreeWayStatus::Conflict | ThreeWayStatus::DeleteModify => "M",
            ThreeWayStatus::AddedTheirs | ThreeWayStatus::AddedBothSame | ThreeWayStatus::AddedBothDiff => "A",
            ThreeWayStatus::DeletedTheirs | ThreeWayStatus::DeletedBoth | ThreeWayStatus::ModifyDelete => "D",
            ThreeWayStatus::AddedOurs | ThreeWayStatus::DeletedOurs => "-",
        }
    }
}

/// Three-way comparison entry
#[derive(Debug, Clone)]
pub struct ThreeWayEntry {
    pub relative_path: PathBuf,
    pub is_dir: bool,
    pub status: ThreeWayStatus,
    /// File size in base (None if not exists)
    pub base_size: Option<u64>,
    /// File size in ours (None if not exists)
    pub ours_size: Option<u64>,
    /// File size in theirs (None if not exists)
    pub theirs_size: Option<u64>,
}

/// Three-way comparison result
pub struct ThreeWayDiffResult {
    pub entries: Vec<ThreeWayEntry>,
    pub base_dir: PathBuf,
    pub ours_dir: PathBuf,
    pub theirs_dir: PathBuf,
    /// Total unique paths across all three directories
    pub total_paths: usize,
}

impl ThreeWayDiffResult {
    /// Count entries by status
    pub fn count_by_status(&self, status: &ThreeWayStatus) -> usize {
        self.entries.iter().filter(|e| &e.status == status).count()
    }

    /// Count all conflict entries
    pub fn count_conflicts(&self) -> usize {
        self.entries.iter().filter(|e| e.status.is_conflict()).count()
    }

    /// Returns true if there are any differences
    pub fn has_differences(&self) -> bool {
        self.entries.iter().any(|e| e.status != ThreeWayStatus::Unchanged)
    }

    /// Returns true if there are any conflicts
    pub fn has_conflicts(&self) -> bool {
        self.entries.iter().any(|e| e.status.is_conflict())
    }

    /// Get entries that need to be copied (non-unchanged, optionally conflict-only)
    pub fn get_copy_entries(&self, conflict_only: bool) -> Vec<&ThreeWayEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if conflict_only {
                    e.status.is_conflict()
                } else {
                    e.status != ThreeWayStatus::Unchanged && e.status != ThreeWayStatus::DeletedBoth
                }
            })
            .collect()
    }
}

/// Three-way copy result
#[derive(Debug, Default)]
pub struct ThreeWayCopyResult {
    pub copied_ours: usize,
    pub copied_theirs: usize,
    pub copied_both_same: usize,
    pub copied_conflicts: usize,
    pub errors: Vec<CopyError>,
}

/// Patch generation result for a single file
#[derive(Debug, Clone)]
struct PatchInfo {
    relative_path: PathBuf,
    is_binary: bool,
    patch_generated: bool,
}

/// Patch error information
#[derive(Debug, Clone)]
struct PatchError {
    relative_path: PathBuf,
    error: String,
}

/// Patch generation result
#[derive(Debug, Default)]
struct PatchResult {
    patches: Vec<PatchInfo>,
    errors: Vec<PatchError>,
    total_generated: usize,
    total_skipped: usize,
}

/// Copy error information
#[derive(Debug, Clone)]
struct CopyError {
    relative_path: PathBuf,
    error: String,
}

/// Copy result
#[derive(Debug, Default)]
struct CopyResult {
    copied_count: usize,
    errors: Vec<CopyError>,
}

/// Options to include in the summary output
struct SummaryOptions {
    exclude_patterns: Vec<String>,
    dry_run: bool,
    both_versions: bool,
    check_permissions: PermissionCheckMode,
    config_file: Option<PathBuf>,
    output_dir: PathBuf,
    patch: bool,
    patch_file: Option<PathBuf>,
    patch_result: Option<PatchResult>,
    copy_result: Option<CopyResult>,
    excel_fold_level: Option<u16>,
    show_unchanged: bool,
    // Output filter options
    filter_status: Vec<String>,
    stats_only: bool,
    no_tree: bool,
    no_details: bool,
    // Output format: true = full path (file), false = grouped by directory (console)
    output_to_file: bool,
    // Copy options
    copy_deleted: bool,
    preserve_timestamps: bool,
}

impl DiffResult {
    fn has_differences(&self) -> bool {
        // Unchanged entries don't count as differences
        let has_real_changes = self.entries.iter().any(|e| !matches!(e.status, FileStatus::Unchanged));
        has_real_changes || !self.permission_changes.is_empty()
    }

    fn count_by_status(&self) -> (usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) {
        let mut added_files = 0;
        let mut added_dirs = 0;
        let mut modified_files = 0;
        let mut deleted_files = 0;
        let mut deleted_dirs = 0;
        let mut symlinks = 0;
        let mut special_files = 0;
        let mut errors = 0;
        let mut unchanged_files = 0;

        for entry in &self.entries {
            match &entry.status {
                FileStatus::Added => {
                    if entry.is_dir {
                        added_dirs += 1;
                    } else {
                        added_files += 1;
                    }
                }
                FileStatus::Modified => modified_files += 1,
                FileStatus::Deleted => {
                    if entry.is_dir {
                        deleted_dirs += 1;
                    } else {
                        deleted_files += 1;
                    }
                }
                FileStatus::Unchanged => unchanged_files += 1,
                FileStatus::Symlink { .. } => symlinks += 1,
                FileStatus::SpecialFile { .. } => special_files += 1,
                FileStatus::PermissionDenied { .. } => errors += 1,
            }
        }

        let permission_changes = self.permission_changes.len();

        (added_files, added_dirs, modified_files, deleted_files, deleted_dirs, symlinks, permission_changes, errors, unchanged_files, special_files)
    }

    /// Calculate unchanged file count (files in both source and target with no changes)
    /// This works regardless of show_unchanged option
    fn unchanged_count(&self) -> usize {
        // Count symlinks that were changed (not added or deleted)
        let symlink_changed = self.entries.iter()
            .filter(|e| matches!(&e.status, FileStatus::Symlink { change_type: SymlinkChangeType::Changed, .. }))
            .count();

        // Count modified files
        let modified = self.entries.iter()
            .filter(|e| matches!(e.status, FileStatus::Modified))
            .count();

        // Unchanged = common files - modified - symlink_changed - permission_changes
        self.common_count.saturating_sub(modified + symlink_changed + self.permission_changes.len())
    }

    /// Calculate total unique paths (source ∪ target)
    fn total_unique_paths(&self) -> usize {
        self.source_count + self.target_count - self.common_count
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Resolve configuration from CLI args and optional config file
    let config = ResolvedConfig::from_args(args)?;

    // Validate source directory (ours in three-way mode)
    if !config.source_dir.exists() {
        bail!("Source directory does not exist: {}", config.source_dir.display());
    }
    if !config.source_dir.is_dir() {
        bail!("Source path is not a directory: {}", config.source_dir.display());
    }

    // Validate target directory (theirs in three-way mode)
    if !config.target_dir.exists() {
        bail!("Target directory does not exist: {}", config.target_dir.display());
    }
    if !config.target_dir.is_dir() {
        bail!("Target path is not a directory: {}", config.target_dir.display());
    }

    // Validate base directory for three-way mode
    if config.three_way {
        if let Some(ref base_dir) = config.base_dir {
            if !base_dir.exists() {
                bail!("Base directory does not exist: {}", base_dir.display());
            }
            if !base_dir.is_dir() {
                bail!("Base path is not a directory: {}", base_dir.display());
            }
        }
    }

    // Handle output directory
    if config.output_dir.exists() {
        if config.force {
            // Check if the path is dangerous to delete (error immediately)
            if let Some(reason) = is_dangerous_path(&config.output_dir) {
                bail!("Cannot delete output directory: {}", reason);
            }

            // Ask for user confirmation
            if !confirm_deletion(&config.output_dir)? {
                bail!("Deletion cancelled by user");
            }

            if config.verbose {
                println_to_stdout(&format!("Removing existing output directory: {}", config.output_dir.display()));
            }
            fs::remove_dir_all(&config.output_dir)
                .with_context(|| format!("Failed to remove output directory: {}", config.output_dir.display()))?;
        } else {
            bail!(
                "Output directory already exists: {}\nUse --force to delete and re-run",
                config.output_dir.display()
            );
        }
    }

    // Parse exclude patterns
    let exclude_patterns: Vec<Pattern> = config
        .exclude
        .iter()
        .map(|p| Pattern::new(p).with_context(|| format!("Invalid glob pattern: {}", p)))
        .collect::<Result<Vec<_>>>()?;

    // Branch based on three-way mode
    if config.three_way {
        run_three_way_mode(&config, &exclude_patterns)
    } else {
        run_two_way_mode(&config, &exclude_patterns)
    }
}

/// Run two-way (normal) comparison mode
fn run_two_way_mode(config: &ResolvedConfig, exclude_patterns: &[Pattern]) -> Result<()> {
    // Compare directories
    let diff_result = compare_directories(
        &config.source_dir,
        &config.target_dir,
        exclude_patterns,
        config.verbose,
        config.check_permissions,
        config.show_unchanged,
    )?;

    // Copy files (if not dry-run and has differences)
    let copy_result = if !config.dry_run && (diff_result.has_differences() || config.copy_deleted) {
        Some(copy_diff_files(
            &diff_result,
            &config.output_dir,
            config.verbose,
            config.both_versions,
            config.copy_deleted,
            config.preserve_timestamps,
        )?)
    } else {
        None
    };

    // Phase 4: Generate patches if requested (after copying, before summary)
    let patch_result = if (config.patch || config.patch_file.is_some()) && diff_result.has_differences() && !config.dry_run {
        print_phase(4, 5, "Generating patches...");
        Some(generate_patches(
            &diff_result,
            &config.output_dir,
            config.patch,
            config.patch_file.as_deref(),
            config.verbose,
        )?)
    } else {
        None
    };

    // Phase 5: Generate and output summary
    print_phase(5, 5, "Writing summary...");

    let summary_options = SummaryOptions {
        exclude_patterns: config.exclude.clone(),
        dry_run: config.dry_run,
        both_versions: config.both_versions,
        check_permissions: config.check_permissions,
        config_file: config.config_file.clone(),
        output_dir: config.output_dir.clone(),
        patch: config.patch,
        patch_file: config.patch_file.clone(),
        patch_result,
        copy_result,
        excel_fold_level: config.excel_fold_level,
        show_unchanged: config.show_unchanged,
        filter_status: config.filter_status.clone(),
        stats_only: config.stats_only,
        no_tree: config.no_tree,
        no_details: config.no_details,
        output_to_file: config.summary.is_some(),
        copy_deleted: config.copy_deleted,
        preserve_timestamps: config.preserve_timestamps,
    };
    let summary = generate_summary(&diff_result, &summary_options);

    // Output summary
    if let Some(summary_path) = &config.summary {
        let mut file = File::create(summary_path)
            .with_context(|| format!("Failed to create summary file: {}", summary_path.display()))?;
        file.write_all(summary.as_bytes())?;
        if is_terminal() {
            println_to_stdout(&format!("Summary written to: {}", summary_path.display()));
        }
    } else {
        println_to_stdout(&summary);
    }

    // Output Excel summary if requested (skip if stats_only)
    if let Some(excel_path) = &config.excel {
        if config.stats_only {
            if is_terminal() {
                println_to_stdout("Excel output skipped (--stats-only mode)");
            }
        } else {
            generate_excel_summary(&diff_result, &summary_options, excel_path)
                .with_context(|| format!("Failed to create Excel file: {}", excel_path.display()))?;
            if is_terminal() {
                println_to_stdout(&format!("Excel summary written to: {}", excel_path.display()));
            }
        }
    }

    // Save config file if requested
    if let Some(save_config_path) = &config.save_config {
        config.save_config_file(save_config_path)?;
        if is_terminal() {
            println_to_stdout(&format!("Config saved to: {}", save_config_path.display()));
        }
    }

    if is_terminal() {
        println_to_stdout("Done.");
    }

    // Return appropriate exit code
    if !diff_result.has_differences() {
        std::process::exit(2);
    }

    Ok(())
}

/// Run three-way comparison mode
fn run_three_way_mode(config: &ResolvedConfig, exclude_patterns: &[Pattern]) -> Result<()> {
    let base_dir = config.base_dir.as_ref().expect("Base directory required for three-way mode");

    // Compare three directories
    let diff_result = compare_three_way_directories(
        base_dir,
        &config.source_dir,  // ours
        &config.target_dir,  // theirs
        exclude_patterns,
        config.verbose,
    )?;

    // Copy files (if not dry-run and has differences)
    let copy_result = if !config.dry_run && diff_result.has_differences() {
        Some(copy_three_way_files(
            &diff_result,
            &config.output_dir,
            config.merge_style,
            config.conflict_only,
            config.verbose,
        )?)
    } else {
        None
    };

    // Phase 5: Generate and output summary
    print_phase(5, 5, "Writing summary...");

    let summary_options = ThreeWaySummaryOptions {
        exclude_patterns: config.exclude.clone(),
        dry_run: config.dry_run,
        merge_style: config.merge_style,
        conflict_only: config.conflict_only,
        config_file: config.config_file.clone(),
        output_dir: config.output_dir.clone(),
        copy_result,
        filter_status: config.filter_status.clone(),
        stats_only: config.stats_only,
        no_tree: config.no_tree,
        no_details: config.no_details,
        output_to_file: config.summary.is_some(),
    };
    let summary = generate_three_way_summary(&diff_result, &summary_options);

    // Output summary
    if let Some(summary_path) = &config.summary {
        let mut file = File::create(summary_path)
            .with_context(|| format!("Failed to create summary file: {}", summary_path.display()))?;
        file.write_all(summary.as_bytes())?;
        if is_terminal() {
            println_to_stdout(&format!("Summary written to: {}", summary_path.display()));
        }
    } else {
        println_to_stdout(&summary);
    }

    // Output Excel summary if requested
    if let Some(excel_path) = &config.excel {
        generate_three_way_excel(&diff_result, &summary_options, excel_path)
            .with_context(|| format!("Failed to create Excel file: {}", excel_path.display()))?;
        if is_terminal() {
            println_to_stdout(&format!("Excel summary written to: {}", excel_path.display()));
        }
    }

    // Save config file if requested
    if let Some(save_config_path) = &config.save_config {
        config.save_config_file(save_config_path)?;
        if is_terminal() {
            println_to_stdout(&format!("Config saved to: {}", save_config_path.display()));
        }
    }

    if is_terminal() {
        println_to_stdout("Done.");
    }

    // Return appropriate exit code
    // 0: differences found, no conflicts
    // 2: no differences
    // 3: conflicts found
    if !diff_result.has_differences() {
        std::process::exit(2);
    } else if diff_result.has_conflicts() {
        std::process::exit(3);
    }

    Ok(())
}

/// Compare a single file and return the diff entry if any
fn compare_single_file(
    rel_path: &PathBuf,
    source_dir: &Path,
    target_dir: &Path,
    source_paths: &BTreeSet<PathBuf>,
    check_permissions: PermissionCheckMode,
    show_unchanged: bool,
) -> (Option<DiffEntry>, Option<PermissionChange>) {
    let target_path = target_dir.join(rel_path);
    let source_path = source_dir.join(rel_path);

    // Check if symlink (target side)
    let target_symlink = get_symlink_info(&target_path);
    let source_symlink = get_symlink_info(&source_path);

    match (&target_symlink, &source_symlink) {
        // Both are symlinks
        (Some(target_info), Some(source_info)) => {
            // Check if target path changed OR if broken status changed
            if target_info.target != source_info.target || target_info.exists != source_info.exists {
                return (Some(DiffEntry {
                    relative_path: rel_path.clone(),
                    is_dir: false,
                    status: FileStatus::Symlink {
                        change_type: SymlinkChangeType::Changed,
                        current: Some(target_info.clone()),
                        previous: Some(source_info.clone()),
                    },
                }), None);
            }
            return (None, None);
        }
        // Only target has symlink (added) - source was a regular file or didn't exist
        (Some(target_info), None) => {
            return (Some(DiffEntry {
                relative_path: rel_path.clone(),
                is_dir: false,
                status: FileStatus::Symlink {
                    change_type: SymlinkChangeType::Added,
                    current: Some(target_info.clone()),
                    previous: None,
                },
            }), None);
        }
        // Source has symlink, target is a regular file (symlink became file)
        (None, Some(source_info)) => {
            // Symlink was replaced by a regular file - report as symlink deleted
            return (Some(DiffEntry {
                relative_path: rel_path.clone(),
                is_dir: false,
                status: FileStatus::Symlink {
                    change_type: SymlinkChangeType::Deleted,
                    current: None,
                    previous: Some(source_info.clone()),
                },
            }), None);
        }
        // Neither is symlink - continue normal processing
        (None, None) => {}
    }

    // Check if special file (socket, fifo, device, etc.) - these cannot be copied
    if let Some(file_type) = get_special_file_type(&target_path) {
        return (Some(DiffEntry {
            relative_path: rel_path.clone(),
            is_dir: false,
            status: FileStatus::SpecialFile { file_type },
        }), None);
    }

    let is_dir = target_path.is_dir();

    if !source_paths.contains(rel_path) {
        // Added (only in target)
        // Check if special file in source that was deleted
        if let Some(file_type) = get_special_file_type(&source_path) {
            return (Some(DiffEntry {
                relative_path: rel_path.clone(),
                is_dir: false,
                status: FileStatus::SpecialFile { file_type },
            }), None);
        }
        return (Some(DiffEntry {
            relative_path: rel_path.clone(),
            is_dir,
            status: FileStatus::Added,
        }), None);
    } else if !is_dir {
        // Check if modified
        match files_differ(&source_path, &target_path) {
            Ok(true) => {
                return (Some(DiffEntry {
                    relative_path: rel_path.clone(),
                    is_dir: false,
                    status: FileStatus::Modified,
                }), None);
            }
            Ok(false) => {
                // Content is the same, check permissions if enabled
                if should_check_permissions(rel_path, check_permissions) {
                    if let Some(change) = check_permission_change(&source_path, &target_path, rel_path) {
                        return (None, Some(change));
                    }
                }
                // If show_unchanged is enabled, return unchanged entry
                if show_unchanged {
                    return (Some(DiffEntry {
                        relative_path: rel_path.clone(),
                        is_dir: false,
                        status: FileStatus::Unchanged,
                    }), None);
                }
            }
            Err(err) => {
                return (Some(DiffEntry {
                    relative_path: rel_path.clone(),
                    is_dir: false,
                    status: FileStatus::PermissionDenied {
                        error: err.to_string(),
                    },
                }), None);
            }
        }
    }

    (None, None)
}

fn compare_directories(
    source_dir: &Path,
    target_dir: &Path,
    exclude_patterns: &[Pattern],
    verbose: bool,
    check_permissions: PermissionCheckMode,
    show_unchanged: bool,
) -> Result<DiffResult> {
    const TOTAL_PHASES: usize = 5;

    let mut initial_entries = Vec::new();
    let mut source_paths: BTreeSet<PathBuf> = BTreeSet::new();
    let mut target_paths: BTreeSet<PathBuf> = BTreeSet::new();

    // Phase 1: Scanning
    print_phase(1, TOTAL_PHASES, "Scanning directories...");

    // Collect source paths
    for entry in WalkDir::new(source_dir).min_depth(1) {
        match entry {
            Ok(e) => {
                let rel_path = e.path().strip_prefix(source_dir).unwrap().to_path_buf();
                if !is_excluded(&rel_path, exclude_patterns) {
                    source_paths.insert(rel_path);
                }
            }
            Err(err) => {
                if let Some(path) = err.path() {
                    let rel_path = path.strip_prefix(source_dir).unwrap_or(path).to_path_buf();
                    initial_entries.push(DiffEntry {
                        relative_path: rel_path,
                        is_dir: false,
                        status: FileStatus::PermissionDenied {
                            error: err.to_string(),
                        },
                    });
                }
            }
        }
    }

    // Collect target paths
    for entry in WalkDir::new(target_dir).min_depth(1) {
        match entry {
            Ok(e) => {
                let rel_path = e.path().strip_prefix(target_dir).unwrap().to_path_buf();
                if !is_excluded(&rel_path, exclude_patterns) {
                    target_paths.insert(rel_path);
                }
            }
            Err(err) => {
                if let Some(path) = err.path() {
                    let rel_path = path.strip_prefix(target_dir).unwrap_or(path).to_path_buf();
                    initial_entries.push(DiffEntry {
                        relative_path: rel_path,
                        is_dir: false,
                        status: FileStatus::PermissionDenied {
                            error: err.to_string(),
                        },
                    });
                }
            }
        }
    }

    // Calculate total files to compare
    let deleted_count = source_paths.iter().filter(|p| !target_paths.contains(*p)).count();
    let total_files = target_paths.len() + deleted_count;

    if is_terminal() {
        println_to_stdout(&format!("Found {} items.", total_files));
    }

    // Phase 2: Comparing (parallel)
    let progress_counter = AtomicUsize::new(0);
    let target_paths_vec: Vec<_> = target_paths.iter().cloned().collect();

    // Start progress display thread
    let total_for_progress = total_files;
    let progress_counter_ref = &progress_counter;

    // Compare target files in parallel
    let target_results: Vec<_> = target_paths_vec
        .par_iter()
        .map(|rel_path| {
            let result = compare_single_file(
                rel_path,
                source_dir,
                target_dir,
                &source_paths,
                check_permissions,
                show_unchanged,
            );

            let count = progress_counter_ref.fetch_add(1, Ordering::Relaxed) + 1;
            if count % 100 == 0 || count == total_for_progress {
                print_progress_atomic(progress_counter_ref, total_for_progress, 2, TOTAL_PHASES, "Comparing");
            }

            if verbose && is_terminal() {
                // Note: verbose output in parallel may interleave, but that's acceptable
            }

            result
        })
        .collect();

    // Check for deleted files (only in source) - also in parallel
    let deleted_paths: Vec<_> = source_paths
        .iter()
        .filter(|p| !target_paths.contains(*p))
        .cloned()
        .collect();

    let deleted_results: Vec<_> = deleted_paths
        .par_iter()
        .map(|rel_path| {
            let source_path = source_dir.join(rel_path);

            let count = progress_counter_ref.fetch_add(1, Ordering::Relaxed) + 1;
            if count % 100 == 0 || count == total_for_progress {
                print_progress_atomic(progress_counter_ref, total_for_progress, 2, TOTAL_PHASES, "Comparing");
            }

            // Check if symlink (deleted symlink)
            if let Some(source_info) = get_symlink_info(&source_path) {
                return Some(DiffEntry {
                    relative_path: rel_path.clone(),
                    is_dir: false,
                    status: FileStatus::Symlink {
                        change_type: SymlinkChangeType::Deleted,
                        current: None,
                        previous: Some(source_info),
                    },
                });
            }

            let is_dir = source_path.is_dir();
            Some(DiffEntry {
                relative_path: rel_path.clone(),
                is_dir,
                status: FileStatus::Deleted,
            })
        })
        .collect();

    // Clear progress line
    clear_progress_line();

    // Collect results
    let mut entries = initial_entries;
    let mut permission_changes = Vec::new();

    for (entry_opt, perm_opt) in target_results {
        if let Some(entry) = entry_opt {
            entries.push(entry);
        }
        if let Some(perm) = perm_opt {
            permission_changes.push(perm);
        }
    }

    for entry_opt in deleted_results {
        if let Some(entry) = entry_opt {
            entries.push(entry);
        }
    }

    if is_terminal() {
        println_to_stdout(&format!("Compared {} items.", total_files));
    }

    // Sort entries by path
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    // Sort permission changes by path
    permission_changes.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    // Calculate counts for statistics
    let source_count = source_paths.len();
    let target_count = target_paths.len();
    let common_count = source_paths.intersection(&target_paths).count();

    Ok(DiffResult {
        entries,
        permission_changes,
        source_dir: source_dir.to_path_buf(),
        target_dir: target_dir.to_path_buf(),
        source_count,
        target_count,
        common_count,
    })
}

// ============================================================================
// Three-way comparison functions
// ============================================================================

/// Compare three directories (base, ours, theirs) and return three-way diff result
fn compare_three_way_directories(
    base_dir: &Path,
    ours_dir: &Path,
    theirs_dir: &Path,
    exclude_patterns: &[Pattern],
    verbose: bool,
) -> Result<ThreeWayDiffResult> {
    const TOTAL_PHASES: usize = 5;

    // Phase 1: Scanning
    print_phase(1, TOTAL_PHASES, "Scanning directories...");

    let mut base_paths: BTreeSet<PathBuf> = BTreeSet::new();
    let mut ours_paths: BTreeSet<PathBuf> = BTreeSet::new();
    let mut theirs_paths: BTreeSet<PathBuf> = BTreeSet::new();

    // Collect paths from all three directories
    for entry in WalkDir::new(base_dir).min_depth(1) {
        if let Ok(e) = entry {
            let rel_path = e.path().strip_prefix(base_dir).unwrap().to_path_buf();
            if !is_excluded(&rel_path, exclude_patterns) {
                base_paths.insert(rel_path);
            }
        }
    }

    for entry in WalkDir::new(ours_dir).min_depth(1) {
        if let Ok(e) = entry {
            let rel_path = e.path().strip_prefix(ours_dir).unwrap().to_path_buf();
            if !is_excluded(&rel_path, exclude_patterns) {
                ours_paths.insert(rel_path);
            }
        }
    }

    for entry in WalkDir::new(theirs_dir).min_depth(1) {
        if let Ok(e) = entry {
            let rel_path = e.path().strip_prefix(theirs_dir).unwrap().to_path_buf();
            if !is_excluded(&rel_path, exclude_patterns) {
                theirs_paths.insert(rel_path);
            }
        }
    }

    // Get all unique paths across all three directories
    let mut all_paths: BTreeSet<PathBuf> = BTreeSet::new();
    all_paths.extend(base_paths.iter().cloned());
    all_paths.extend(ours_paths.iter().cloned());
    all_paths.extend(theirs_paths.iter().cloned());

    let total_paths = all_paths.len();

    if is_terminal() {
        println_to_stdout(&format!("Found {} items.", total_paths));
    }

    // Phase 2: Comparing (parallel)
    let progress_counter = AtomicUsize::new(0);
    let all_paths_vec: Vec<_> = all_paths.iter().cloned().collect();

    let entries: Vec<ThreeWayEntry> = all_paths_vec
        .par_iter()
        .map(|rel_path| {
            let result = compare_three_way_single_file(
                rel_path,
                base_dir,
                ours_dir,
                theirs_dir,
                &base_paths,
                &ours_paths,
                &theirs_paths,
            );

            let count = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if count % 100 == 0 || count == total_paths {
                print_progress_atomic(&progress_counter, total_paths, 2, TOTAL_PHASES, "Comparing");
            }

            result
        })
        .collect();

    clear_progress_line();

    if is_terminal() {
        println_to_stdout(&format!("Compared {} items.", total_paths));
    }

    // Sort entries by path
    let mut sorted_entries = entries;
    sorted_entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(ThreeWayDiffResult {
        entries: sorted_entries,
        base_dir: base_dir.to_path_buf(),
        ours_dir: ours_dir.to_path_buf(),
        theirs_dir: theirs_dir.to_path_buf(),
        total_paths,
    })
}

/// Compare a single file across three directories
fn compare_three_way_single_file(
    rel_path: &PathBuf,
    base_dir: &Path,
    ours_dir: &Path,
    theirs_dir: &Path,
    base_paths: &BTreeSet<PathBuf>,
    ours_paths: &BTreeSet<PathBuf>,
    theirs_paths: &BTreeSet<PathBuf>,
) -> ThreeWayEntry {
    let base_path = base_dir.join(rel_path);
    let ours_path = ours_dir.join(rel_path);
    let theirs_path = theirs_dir.join(rel_path);

    let in_base = base_paths.contains(rel_path);
    let in_ours = ours_paths.contains(rel_path);
    let in_theirs = theirs_paths.contains(rel_path);

    // Get file sizes
    let base_size = if in_base { fs::metadata(&base_path).ok().map(|m| m.len()) } else { None };
    let ours_size = if in_ours { fs::metadata(&ours_path).ok().map(|m| m.len()) } else { None };
    let theirs_size = if in_theirs { fs::metadata(&theirs_path).ok().map(|m| m.len()) } else { None };

    // Determine if it's a directory (check any existing path)
    let is_dir = if in_base {
        base_path.is_dir()
    } else if in_ours {
        ours_path.is_dir()
    } else {
        theirs_path.is_dir()
    };

    // Determine status based on existence and content
    let status = match (in_base, in_ours, in_theirs) {
        // File exists in all three
        (true, true, true) => {
            if is_dir {
                ThreeWayStatus::Unchanged
            } else {
                let base_eq_ours = files_equal(&base_path, &ours_path);
                let base_eq_theirs = files_equal(&base_path, &theirs_path);
                let ours_eq_theirs = files_equal(&ours_path, &theirs_path);

                match (base_eq_ours, base_eq_theirs, ours_eq_theirs) {
                    (true, true, true) => ThreeWayStatus::Unchanged,
                    (false, true, false) => ThreeWayStatus::OursOnly,
                    (true, false, false) => ThreeWayStatus::TheirsOnly,
                    (false, false, true) => ThreeWayStatus::BothSame,
                    (false, false, false) => ThreeWayStatus::Conflict,
                    // These cases shouldn't happen logically, but handle them
                    _ => ThreeWayStatus::Unchanged,
                }
            }
        }
        // File only in base (deleted in both ours and theirs)
        (true, false, false) => ThreeWayStatus::DeletedBoth,
        // File in base and ours only (deleted in theirs)
        (true, true, false) => {
            if is_dir || files_equal(&base_path, &ours_path) {
                ThreeWayStatus::DeletedTheirs
            } else {
                ThreeWayStatus::ModifyDelete
            }
        }
        // File in base and theirs only (deleted in ours)
        (true, false, true) => {
            if is_dir || files_equal(&base_path, &theirs_path) {
                ThreeWayStatus::DeletedOurs
            } else {
                ThreeWayStatus::DeleteModify
            }
        }
        // File only in ours (added in ours)
        (false, true, false) => ThreeWayStatus::AddedOurs,
        // File only in theirs (added in theirs)
        (false, false, true) => ThreeWayStatus::AddedTheirs,
        // File in both ours and theirs but not in base (added in both)
        (false, true, true) => {
            if is_dir || files_equal(&ours_path, &theirs_path) {
                ThreeWayStatus::AddedBothSame
            } else {
                ThreeWayStatus::AddedBothDiff
            }
        }
        // File in none (shouldn't happen)
        (false, false, false) => ThreeWayStatus::Unchanged,
    };

    ThreeWayEntry {
        relative_path: rel_path.clone(),
        is_dir,
        status,
        base_size,
        ours_size,
        theirs_size,
    }
}

/// Check if two files have equal content
fn files_equal(path1: &Path, path2: &Path) -> bool {
    // Quick size check first
    let size1 = fs::metadata(path1).map(|m| m.len()).unwrap_or(0);
    let size2 = fs::metadata(path2).map(|m| m.len()).unwrap_or(0);

    if size1 != size2 {
        return false;
    }

    // Compare hashes
    let hash1 = compute_file_hash(path1).ok();
    let hash2 = compute_file_hash(path2).ok();

    match (hash1, hash2) {
        (Some(h1), Some(h2)) => h1 == h2,
        _ => false,
    }
}

fn is_excluded(path: &Path, patterns: &[Pattern]) -> bool {
    let path_str = path.to_string_lossy();

    patterns.iter().any(|p| {
        // Match against full path
        if p.matches(&path_str) {
            return true;
        }

        // Also match against each path component
        // This allows patterns like "__pycache__" to match "src/__pycache__/file.py"
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                if p.matches(&name.to_string_lossy()) {
                    return true;
                }
            }
        }

        false
    })
}

/// Check if a path is dangerous to delete
/// Returns Some(reason) if dangerous, None if safe
fn is_dangerous_path(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        // Check the original path first (handles symlinks like /lib -> /usr/lib)
        let original_str = path.to_string_lossy();
        let original_str = original_str.trim_end_matches('/');

        // Handle root directory special case
        if original_str.is_empty() || original_str == "/" {
            return Some("Root directory '/' cannot be deleted".to_string());
        }

        // Check against dangerous system paths (original path)
        for dangerous in DANGEROUS_PATHS_UNIX {
            if original_str == *dangerous {
                return Some(format!("System directory '{}' cannot be deleted", dangerous));
            }
        }

        // Also check canonical path if different (for symlinks pointing to dangerous locations)
        if let Ok(canonical) = path.canonicalize() {
            let canonical_str = canonical.to_string_lossy();
            let canonical_str = canonical_str.trim_end_matches('/');

            for dangerous in DANGEROUS_PATHS_UNIX {
                if canonical_str == *dangerous {
                    return Some(format!("System directory '{}' (resolves to '{}') cannot be deleted", original_str, dangerous));
                }
            }
        }

        // Block /home/<username> (user home directory)
        if original_str.starts_with("/home/") {
            let after_home = &original_str[6..]; // Skip "/home/"
            if !after_home.contains('/') && !after_home.is_empty() {
                return Some(format!("User home directory '{}' cannot be deleted", original_str));
            }
        }
    }

    #[cfg(windows)]
    {
        // Try to canonicalize, but fall back to the original path
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_lowercase();
        let path_str = path_str.trim_end_matches('\\');

        // Block drive roots (C:\, D:\, etc.)
        if path_str.len() == 2 && path_str.ends_with(':') {
            return Some(format!("Drive root '{}' cannot be deleted", path_str));
        }

        // Check for dangerous Windows directories
        for dangerous in DANGEROUS_PATHS_WINDOWS {
            // Match C:\Windows, D:\Windows, etc.
            if path_str.len() >= 3 && path_str.chars().nth(1) == Some(':') {
                let after_drive = &path_str[3..]; // Skip "C:\"
                if after_drive == *dangerous {
                    return Some(format!("System directory '{}' cannot be deleted", path_str));
                }
            }
        }

        // Block C:\Users\<username> and C:\Users\<username>\<foldername>
        if path_str.len() >= 3 && path_str.chars().nth(1) == Some(':') {
            let after_drive = &path_str[3..]; // Skip "C:\"
            if after_drive.starts_with("users\\") {
                let after_users = &after_drive[6..]; // Skip "users\"
                let parts: Vec<&str> = after_users.split('\\').collect();
                // Block: C:\Users\username (1 part) or C:\Users\username\foldername (2 parts)
                if parts.len() <= 2 && !parts.is_empty() && !parts[0].is_empty() {
                    return Some(format!("User directory '{}' cannot be deleted", path_str));
                }
            }
        }
    }

    None
}

/// Ask for user confirmation before deletion
/// Returns true if user confirms, false if user declines
/// Skips prompt and returns true if stdin is not a terminal (non-interactive mode)
fn confirm_deletion(path: &Path) -> Result<bool> {
    use std::io::{self, BufRead, IsTerminal};

    // Skip confirmation if stdin is not a terminal (non-interactive mode)
    // This allows scripts and automated tests to run without interaction
    if !io::stdin().is_terminal() {
        return Ok(true);
    }

    println_to_stdout(&format!(
        "\nWarning: About to delete directory:\n  {}\n",
        path.display()
    ));
    print_to_stdout("Are you sure you want to delete this directory? [yes/no]: ");

    // Flush stdout to ensure the prompt is displayed
    io::stdout().flush().ok();

    let stdin = io::stdin();
    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(input == "yes" || input == "y")
}

/// Get symlink information for a path
fn get_symlink_info(path: &Path) -> Option<SymlinkInfo> {
    if path.is_symlink() {
        let target = fs::read_link(path).unwrap_or_default();
        // Check if symlink target exists by following the symlink
        // path.exists() follows symlinks, returns false for broken symlinks
        let exists = path.exists();
        let is_dir = path.is_dir();
        Some(SymlinkInfo { target, exists, is_dir })
    } else {
        None
    }
}

/// Check if a file is a special file (socket, fifo, device, etc.)
/// Returns the special file type if it is, None otherwise
#[cfg(unix)]
fn get_special_file_type(path: &Path) -> Option<SpecialFileType> {
    use std::os::unix::fs::FileTypeExt;

    // Don't follow symlinks - check the file itself
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return None,
    };

    let file_type = metadata.file_type();

    if file_type.is_socket() {
        Some(SpecialFileType::Socket)
    } else if file_type.is_fifo() {
        Some(SpecialFileType::Fifo)
    } else if file_type.is_block_device() {
        Some(SpecialFileType::BlockDevice)
    } else if file_type.is_char_device() {
        Some(SpecialFileType::CharDevice)
    } else {
        None
    }
}

#[cfg(windows)]
fn get_special_file_type(_path: &Path) -> Option<SpecialFileType> {
    // Windows doesn't have Unix-style special files
    None
}

/// Check if a file should have its permissions checked based on mode
fn should_check_permissions(path: &Path, mode: PermissionCheckMode) -> bool {
    match mode {
        PermissionCheckMode::None => false,
        PermissionCheckMode::All => true,
        PermissionCheckMode::Scripts => {
            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                SCRIPT_EXTENSIONS.contains(&ext_lower.as_str())
            } else {
                false
            }
        }
    }
}

/// Get file permission mode as a string
#[cfg(unix)]
fn get_file_mode(path: &Path) -> Option<String> {
    fs::metadata(path).ok().map(|m| {
        let mode = m.permissions().mode() & 0o777;
        format!("{:03o}", mode)
    })
}

#[cfg(windows)]
fn get_file_mode(path: &Path) -> Option<String> {
    fs::metadata(path).ok().map(|m| {
        if m.permissions().readonly() {
            "readonly".to_string()
        } else {
            "writable".to_string()
        }
    })
}

/// Check if permissions differ between two files
fn check_permission_change(source: &Path, target: &Path, rel_path: &Path) -> Option<PermissionChange> {
    let old_mode = get_file_mode(source)?;
    let new_mode = get_file_mode(target)?;

    if old_mode != new_mode {
        Some(PermissionChange {
            relative_path: rel_path.to_path_buf(),
            old_mode,
            new_mode,
        })
    } else {
        None
    }
}

fn files_differ(path1: &Path, path2: &Path) -> Result<bool> {
    let meta1 = fs::metadata(path1)?;
    let meta2 = fs::metadata(path2)?;

    // Quick check: if sizes differ, files are different
    if meta1.len() != meta2.len() {
        return Ok(true);
    }

    // Compare using BLAKE3 hash (fast and collision-resistant)
    let hash1 = compute_file_hash(path1)?;
    let hash2 = compute_file_hash(path2)?;

    Ok(hash1 != hash2)
}

fn compute_file_hash(path: &Path) -> Result<blake3::Hash> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536]; // 64KB buffer for better performance

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}

/// Copy a single file and optionally preserve timestamps
fn copy_file_with_timestamp(src: &Path, dst: &Path, preserve_timestamps: bool) -> Result<()> {
    fs::copy(src, dst)?;
    if preserve_timestamps {
        if let Ok(metadata) = fs::metadata(src) {
            let file_time = FileTime::from_last_modification_time(&metadata);
            let _ = set_file_mtime(dst, file_time);
        }
    }
    Ok(())
}

/// Copy a single file entry
fn copy_single_file(
    entry: &DiffEntry,
    source_dir: &Path,
    target_dir: &Path,
    output_dir: &Path,
    both_versions: bool,
    copy_deleted: bool,
    preserve_timestamps: bool,
) -> Result<()> {
    match &entry.status {
        FileStatus::Added => {
            let src = target_dir.join(&entry.relative_path);
            let dst = output_dir.join(&entry.relative_path);

            if entry.is_dir {
                fs::create_dir_all(&dst)?;
            } else {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                copy_file_with_timestamp(&src, &dst, preserve_timestamps)?;
            }
        }
        FileStatus::Modified => {
            if entry.is_dir {
                return Ok(());
            }

            let src_new = target_dir.join(&entry.relative_path);
            let dst_base = output_dir.join(&entry.relative_path);

            if let Some(parent) = dst_base.parent() {
                fs::create_dir_all(parent)?;
            }

            if both_versions {
                let src_old = source_dir.join(&entry.relative_path);
                let dst_old = add_extension(&dst_base, "old");
                let dst_new = add_extension(&dst_base, "new");

                copy_file_with_timestamp(&src_old, &dst_old, preserve_timestamps)?;
                copy_file_with_timestamp(&src_new, &dst_new, preserve_timestamps)?;
            } else {
                copy_file_with_timestamp(&src_new, &dst_base, preserve_timestamps)?;
            }
        }
        FileStatus::Deleted => {
            if !copy_deleted {
                return Ok(());
            }

            let src = source_dir.join(&entry.relative_path);
            let dst = output_dir.join(&entry.relative_path);

            if entry.is_dir {
                fs::create_dir_all(&dst)?;
            } else {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                // Copy deleted file with .deleted extension
                let dst_deleted = add_extension(&dst, "deleted");
                copy_file_with_timestamp(&src, &dst_deleted, preserve_timestamps)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn copy_diff_files(
    diff_result: &DiffResult,
    output_dir: &Path,
    verbose: bool,
    both_versions: bool,
    copy_deleted: bool,
    preserve_timestamps: bool,
) -> Result<CopyResult> {
    fs::create_dir_all(output_dir)?;

    // Filter entries that need to be copied
    let entries_to_copy: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| {
            matches!(e.status, FileStatus::Added | FileStatus::Modified)
                || (copy_deleted && matches!(e.status, FileStatus::Deleted))
        })
        .collect();

    let total_files = entries_to_copy.len();

    if total_files == 0 {
        return Ok(CopyResult::default());
    }

    // Phase 3: Copying (parallel)
    print_phase(3, 5, "Copying files...");

    let progress_counter = AtomicUsize::new(0);
    let success_counter = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::new());

    entries_to_copy
        .par_iter()
        .for_each(|entry| {
            let count = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if count % 50 == 0 || count == total_files {
                print_progress_atomic(&progress_counter, total_files, 3, 4, "Copying");
            }

            if let Err(e) = copy_single_file(
                entry,
                &diff_result.source_dir,
                &diff_result.target_dir,
                output_dir,
                both_versions,
                copy_deleted,
                preserve_timestamps,
            ) {
                let mut errs = errors.lock().unwrap();
                errs.push(CopyError {
                    relative_path: entry.relative_path.clone(),
                    error: e.to_string(),
                });
            } else {
                success_counter.fetch_add(1, Ordering::Relaxed);
            }

            if verbose && is_terminal() {
                // Verbose output may interleave in parallel, which is acceptable
            }
        });

    clear_progress_line();

    let copy_errors = errors.into_inner().unwrap();
    let copied_count = success_counter.load(Ordering::Relaxed);

    // Print errors to console but don't exit
    if !copy_errors.is_empty() {
        for err in &copy_errors {
            eprintln!("Copy failed: {}: {}", err.relative_path.display(), err.error);
        }
    }

    if is_terminal() {
        if copy_errors.is_empty() {
            println_to_stdout(&format!("Copied {} files.", copied_count));
        } else {
            println_to_stdout(&format!("Copied {} files ({} failed).", copied_count, copy_errors.len()));
        }
    }

    Ok(CopyResult {
        copied_count,
        errors: copy_errors,
    })
}

/// Copy files based on three-way diff result
fn copy_three_way_files(
    diff_result: &ThreeWayDiffResult,
    output_dir: &Path,
    merge_style: MergeStyle,
    conflict_only: bool,
    verbose: bool,
) -> Result<ThreeWayCopyResult> {
    fs::create_dir_all(output_dir)?;

    let entries_to_copy = diff_result.get_copy_entries(conflict_only);
    let total_files = entries_to_copy.len();

    if total_files == 0 {
        return Ok(ThreeWayCopyResult::default());
    }

    // Phase 3: Copying
    print_phase(3, 5, "Copying files...");

    let progress_counter = AtomicUsize::new(0);
    let copied_ours = AtomicUsize::new(0);
    let copied_theirs = AtomicUsize::new(0);
    let copied_both_same = AtomicUsize::new(0);
    let copied_conflicts = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::new());

    entries_to_copy
        .par_iter()
        .for_each(|entry| {
            let count = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if count % 50 == 0 || count == total_files {
                print_progress_atomic(&progress_counter, total_files, 3, 5, "Copying");
            }

            let result = copy_three_way_single_file(
                entry,
                &diff_result.base_dir,
                &diff_result.ours_dir,
                &diff_result.theirs_dir,
                output_dir,
                merge_style,
            );

            match result {
                Ok(what_copied) => {
                    match what_copied {
                        ThreeWayCopied::Ours => { copied_ours.fetch_add(1, Ordering::Relaxed); }
                        ThreeWayCopied::Theirs => { copied_theirs.fetch_add(1, Ordering::Relaxed); }
                        ThreeWayCopied::BothSame => { copied_both_same.fetch_add(1, Ordering::Relaxed); }
                        ThreeWayCopied::Conflict => { copied_conflicts.fetch_add(1, Ordering::Relaxed); }
                        ThreeWayCopied::None => {}
                    }
                }
                Err(e) => {
                    let mut errs = errors.lock().unwrap();
                    errs.push(CopyError {
                        relative_path: entry.relative_path.clone(),
                        error: e.to_string(),
                    });
                }
            }
        });

    clear_progress_line();

    let copy_errors = errors.into_inner().unwrap();
    let total_copied = copied_ours.load(Ordering::Relaxed)
        + copied_theirs.load(Ordering::Relaxed)
        + copied_both_same.load(Ordering::Relaxed)
        + copied_conflicts.load(Ordering::Relaxed);

    if !copy_errors.is_empty() {
        for err in &copy_errors {
            eprintln!("Copy failed: {}: {}", err.relative_path.display(), err.error);
        }
    }

    if is_terminal() {
        if copy_errors.is_empty() {
            println_to_stdout(&format!("Copied {} files.", total_copied));
        } else {
            println_to_stdout(&format!("Copied {} files ({} failed).", total_copied, copy_errors.len()));
        }
    }

    Ok(ThreeWayCopyResult {
        copied_ours: copied_ours.load(Ordering::Relaxed),
        copied_theirs: copied_theirs.load(Ordering::Relaxed),
        copied_both_same: copied_both_same.load(Ordering::Relaxed),
        copied_conflicts: copied_conflicts.load(Ordering::Relaxed),
        errors: copy_errors,
    })
}

/// What was actually copied in three-way mode
#[derive(Debug, Clone, Copy)]
enum ThreeWayCopied {
    Ours,
    Theirs,
    BothSame,
    Conflict,
    None,
}

/// Copy a single file in three-way mode
fn copy_three_way_single_file(
    entry: &ThreeWayEntry,
    base_dir: &Path,
    ours_dir: &Path,
    theirs_dir: &Path,
    output_dir: &Path,
    merge_style: MergeStyle,
) -> Result<ThreeWayCopied> {
    let base_path = base_dir.join(&entry.relative_path);
    let ours_path = ours_dir.join(&entry.relative_path);
    let theirs_path = theirs_dir.join(&entry.relative_path);
    let output_path = output_dir.join(&entry.relative_path);

    // Create parent directory
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Handle directories
    if entry.is_dir {
        fs::create_dir_all(&output_path)?;
        return Ok(ThreeWayCopied::None);
    }

    match &entry.status {
        // Copy ours version
        ThreeWayStatus::OursOnly | ThreeWayStatus::AddedOurs | ThreeWayStatus::DeletedTheirs => {
            fs::copy(&ours_path, &output_path)?;
            Ok(ThreeWayCopied::Ours)
        }
        // Copy theirs version
        ThreeWayStatus::TheirsOnly | ThreeWayStatus::AddedTheirs | ThreeWayStatus::DeletedOurs => {
            fs::copy(&theirs_path, &output_path)?;
            Ok(ThreeWayCopied::Theirs)
        }
        // Both same - copy either (use ours)
        ThreeWayStatus::BothSame | ThreeWayStatus::AddedBothSame => {
            fs::copy(&ours_path, &output_path)?;
            Ok(ThreeWayCopied::BothSame)
        }
        // Conflicts - depends on merge_style
        ThreeWayStatus::Conflict | ThreeWayStatus::AddedBothDiff => {
            match merge_style {
                MergeStyle::All => {
                    // Copy all three versions with extensions
                    if base_path.exists() {
                        fs::copy(&base_path, add_extension(&output_path, "base"))?;
                    }
                    fs::copy(&ours_path, add_extension(&output_path, "ours"))?;
                    fs::copy(&theirs_path, add_extension(&output_path, "theirs"))?;
                }
                MergeStyle::Ours => {
                    fs::copy(&ours_path, &output_path)?;
                }
                MergeStyle::Theirs => {
                    fs::copy(&theirs_path, &output_path)?;
                }
            }
            Ok(ThreeWayCopied::Conflict)
        }
        // Modify/Delete conflicts
        ThreeWayStatus::ModifyDelete => {
            match merge_style {
                MergeStyle::All => {
                    fs::copy(&base_path, add_extension(&output_path, "base"))?;
                    fs::copy(&ours_path, add_extension(&output_path, "ours"))?;
                    // theirs deleted, so no file to copy
                }
                MergeStyle::Ours => {
                    fs::copy(&ours_path, &output_path)?;
                }
                MergeStyle::Theirs => {
                    // theirs deleted the file, so don't copy anything
                }
            }
            Ok(ThreeWayCopied::Conflict)
        }
        ThreeWayStatus::DeleteModify => {
            match merge_style {
                MergeStyle::All => {
                    fs::copy(&base_path, add_extension(&output_path, "base"))?;
                    // ours deleted, so no file to copy
                    fs::copy(&theirs_path, add_extension(&output_path, "theirs"))?;
                }
                MergeStyle::Ours => {
                    // ours deleted the file, so don't copy anything
                }
                MergeStyle::Theirs => {
                    fs::copy(&theirs_path, &output_path)?;
                }
            }
            Ok(ThreeWayCopied::Conflict)
        }
        // No action needed
        ThreeWayStatus::Unchanged | ThreeWayStatus::DeletedBoth => {
            Ok(ThreeWayCopied::None)
        }
    }
}

/// Add an extension suffix to a path (e.g., "file.txt" -> "file.txt.old")
fn add_extension(path: &Path, ext: &str) -> PathBuf {
    let mut new_path = path.as_os_str().to_owned();
    new_path.push(".");
    new_path.push(ext);
    PathBuf::from(new_path)
}

/// Check if a file is binary by reading first bytes and checking for null bytes or invalid UTF-8
fn is_binary_file(path: &Path) -> bool {
    const SAMPLE_SIZE: usize = 8192;

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut reader = BufReader::new(file);
    let mut buffer = vec![0u8; SAMPLE_SIZE];

    let bytes_read = match reader.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return false,
    };

    if bytes_read == 0 {
        return false; // Empty file is not binary
    }

    let sample = &buffer[..bytes_read];

    // Check for null bytes (common in binary files)
    if sample.contains(&0) {
        return true;
    }

    // Check if it's valid UTF-8
    std::str::from_utf8(sample).is_err()
}

/// Generate unified diff for a single modified file
fn generate_unified_diff(
    source_path: &Path,
    target_path: &Path,
    relative_path: &Path,
) -> Option<String> {
    use similar::{ChangeTag, TextDiff};

    // Read both files as strings
    let old_content = match fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let new_content = match fs::read_to_string(target_path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Generate unified diff
    let diff = TextDiff::from_lines(&old_content, &new_content);

    let mut output = String::new();
    let old_path = format!("a/{}", relative_path.display());
    let new_path = format!("b/{}", relative_path.display());

    output.push_str(&format!("--- {}\n", old_path));
    output.push_str(&format!("+++ {}\n", new_path));

    // Generate hunks
    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        output.push_str(&format!("{}", hunk.header()));
        for change in hunk.iter_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            // Handle lines that don't end with newline
            if change.missing_newline() {
                output.push_str(&format!("{}{}\n\\ No newline at end of file\n", sign, change.value().trim_end_matches('\n')));
            } else {
                output.push_str(&format!("{}{}", sign, change.value()));
            }
        }
    }

    if output.lines().count() <= 2 {
        // Only header, no changes
        return None;
    }

    Some(output)
}

/// Generate patches for modified files
fn generate_patches(
    diff_result: &DiffResult,
    output_dir: &Path,
    individual_patches: bool,
    combined_patch_path: Option<&Path>,
    verbose: bool,
) -> Result<PatchResult> {
    let mut result = PatchResult::default();
    let mut combined_output = String::new();

    // Get modified entries
    let modified_entries: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Modified) && !e.is_dir)
        .collect();

    if modified_entries.is_empty() {
        return Ok(result);
    }

    if is_terminal() && verbose {
        println_to_stdout("Generating patches...");
    }

    for entry in &modified_entries {
        let source_path = diff_result.source_dir.join(&entry.relative_path);
        let target_path = diff_result.target_dir.join(&entry.relative_path);

        let is_binary = is_binary_file(&source_path) || is_binary_file(&target_path);

        if is_binary {
            result.patches.push(PatchInfo {
                relative_path: entry.relative_path.clone(),
                is_binary: true,
                patch_generated: false,
            });
            result.total_skipped += 1;

            if combined_patch_path.is_some() {
                combined_output.push_str(&format!(
                    "Binary files a/{} and b/{} differ\n",
                    entry.relative_path.display(),
                    entry.relative_path.display()
                ));
            }
            continue;
        }

        // Generate diff
        if let Some(diff_content) = generate_unified_diff(&source_path, &target_path, &entry.relative_path) {
            // Write individual patch file
            if individual_patches {
                let patch_path = output_dir.join(add_extension(&entry.relative_path, "patch"));
                let write_result = (|| -> Result<()> {
                    if let Some(parent) = patch_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&patch_path, &diff_content)?;
                    Ok(())
                })();

                match write_result {
                    Ok(()) => {
                        result.patches.push(PatchInfo {
                            relative_path: entry.relative_path.clone(),
                            is_binary: false,
                            patch_generated: true,
                        });
                        result.total_generated += 1;

                        if verbose && is_terminal() {
                            println_to_stdout(&format!("  Generated: {}", patch_path.display()));
                        }
                    }
                    Err(e) => {
                        eprintln!("Patch failed: {}: {}", entry.relative_path.display(), e);
                        result.errors.push(PatchError {
                            relative_path: entry.relative_path.clone(),
                            error: e.to_string(),
                        });
                    }
                }
            } else {
                result.patches.push(PatchInfo {
                    relative_path: entry.relative_path.clone(),
                    is_binary: false,
                    patch_generated: true,
                });
                result.total_generated += 1;
            }

            // Append to combined output
            if combined_patch_path.is_some() {
                combined_output.push_str(&diff_content);
                combined_output.push('\n');
            }
        } else {
            // Diff generation failed or no actual changes
            result.patches.push(PatchInfo {
                relative_path: entry.relative_path.clone(),
                is_binary: false,
                patch_generated: false,
            });
        }
    }

    // Write combined patch file
    if let Some(path) = combined_patch_path {
        if !combined_output.is_empty() {
            if let Err(e) = fs::write(path, &combined_output) {
                eprintln!("Failed to write combined patch file: {}: {}", path.display(), e);
                result.errors.push(PatchError {
                    relative_path: path.to_path_buf(),
                    error: e.to_string(),
                });
            } else if verbose && is_terminal() {
                println_to_stdout(&format!("Combined patch written to: {}", path.display()));
            }
        }
    }

    if is_terminal() {
        if result.errors.is_empty() {
            println_to_stdout(&format!(
                "Patches: {} generated, {} skipped (binary)",
                result.total_generated,
                result.total_skipped
            ));
        } else {
            println_to_stdout(&format!(
                "Patches: {} generated, {} skipped (binary), {} failed",
                result.total_generated,
                result.total_skipped,
                result.errors.len()
            ));
        }
    }

    Ok(result)
}

/// All possible two-way status values
const TWO_WAY_ALL_STATUSES: &[&str] = &[
    "added", "modified", "deleted", "unchanged", "symlink", "special", "error", "permission"
];

/// Resolve filter status list into a set of included statuses
/// Handles `all` keyword and `^` prefix for exclusion
/// Processing is left-to-right (last wins)
fn resolve_filter_statuses(filter_status: &[String], all_statuses: &[&str]) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    let mut result: HashSet<String> = HashSet::new();

    for filter in filter_status {
        // Split by comma to support both multiple --filter-status and comma-separated values
        for part in filter.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            let lower = part.to_lowercase();

            if lower.starts_with('^') {
                // Exclusion: remove from set
                let status = lower[1..].to_string();
                result.remove(&status);
            } else if lower == "all" {
                // Add all statuses
                for s in all_statuses {
                    result.insert(s.to_string());
                }
            } else {
                // Inclusion: add to set
                result.insert(lower);
            }
        }
    }

    result
}

/// Check if an entry matches the status filter (two-way mode)
fn entry_matches_filter(entry: &DiffEntry, filter_status: &[String]) -> bool {
    if filter_status.is_empty() {
        return true;
    }

    let resolved = resolve_filter_statuses(filter_status, TWO_WAY_ALL_STATUSES);

    if resolved.is_empty() {
        return false;
    }

    let entry_status = match &entry.status {
        FileStatus::Added => "added",
        FileStatus::Modified => "modified",
        FileStatus::Deleted => "deleted",
        FileStatus::Unchanged => "unchanged",
        FileStatus::Symlink { .. } => "symlink",
        FileStatus::SpecialFile { .. } => "special",
        FileStatus::PermissionDenied { .. } => "error",
    };

    resolved.contains(entry_status)
}

/// Check if a permission change matches the filter
fn permission_matches_filter(filter_status: &[String]) -> bool {
    if filter_status.is_empty() {
        return true;
    }
    let resolved = resolve_filter_statuses(filter_status, TWO_WAY_ALL_STATUSES);
    resolved.contains("permission")
}

/// Check if a specific status is filtered out (for statistics display)
fn is_status_filtered_out(status: &str, filter_status: &[String]) -> bool {
    if filter_status.is_empty() {
        return false;
    }
    let resolved = resolve_filter_statuses(filter_status, TWO_WAY_ALL_STATUSES);
    !resolved.contains(status)
}

/// All three-way status values for filter resolution
const THREE_WAY_ALL_STATUSES: &[&str] = &[
    "unchanged", "ours-only", "theirs-only", "both-same", "conflict",
    "added-ours", "added-theirs", "added-both-same", "added-both-diff",
    "deleted-ours", "deleted-theirs", "deleted-both",
    "modify-delete", "delete-modify"
];

/// Convert ThreeWayStatus to filter string
fn three_way_status_to_filter_str(status: &ThreeWayStatus) -> &'static str {
    match status {
        ThreeWayStatus::Unchanged => "unchanged",
        ThreeWayStatus::OursOnly => "ours-only",
        ThreeWayStatus::TheirsOnly => "theirs-only",
        ThreeWayStatus::BothSame => "both-same",
        ThreeWayStatus::Conflict => "conflict",
        ThreeWayStatus::AddedOurs => "added-ours",
        ThreeWayStatus::AddedTheirs => "added-theirs",
        ThreeWayStatus::AddedBothSame => "added-both-same",
        ThreeWayStatus::AddedBothDiff => "added-both-diff",
        ThreeWayStatus::DeletedOurs => "deleted-ours",
        ThreeWayStatus::DeletedTheirs => "deleted-theirs",
        ThreeWayStatus::DeletedBoth => "deleted-both",
        ThreeWayStatus::ModifyDelete => "modify-delete",
        ThreeWayStatus::DeleteModify => "delete-modify",
    }
}

/// Check if a three-way status is filtered out
fn is_three_way_status_filtered_out(status: &ThreeWayStatus, filter_status: &[String]) -> bool {
    if filter_status.is_empty() {
        return false;
    }
    let resolved = resolve_filter_statuses(filter_status, THREE_WAY_ALL_STATUSES);
    !resolved.contains(three_way_status_to_filter_str(status))
}

fn generate_summary(diff_result: &DiffResult, options: &SummaryOptions) -> String {
    let mut output = String::new();
    let now = Local::now();

    output.push_str("rs_diffcopy Summary\n");
    output.push_str("================\n");
    output.push_str(&format!("Source: {}\n", diff_result.source_dir.display()));
    output.push_str(&format!("Target: {}\n", diff_result.target_dir.display()));
    output.push_str(&format!("Output: {}\n", options.output_dir.display()));
    output.push_str(&format!("Date: {}\n", now.format("%Y-%m-%d %H:%M:%S")));
    output.push('\n');

    // Options section
    let mut has_options = false;
    let mut options_output = String::new();

    if options.dry_run {
        options_output.push_str("  Mode: Dry-run (no files copied)\n");
        has_options = true;
    }
    if options.both_versions {
        options_output.push_str("  Copy mode: Both versions (.old/.new)\n");
        has_options = true;
    }
    if options.check_permissions != PermissionCheckMode::None {
        let mode_str = match options.check_permissions {
            PermissionCheckMode::Scripts => "scripts",
            PermissionCheckMode::All => "all",
            PermissionCheckMode::None => "none",
        };
        options_output.push_str(&format!("  Permission check: {}\n", mode_str));
        has_options = true;
    }
    if let Some(config_path) = &options.config_file {
        options_output.push_str(&format!("  Config file: {}\n", config_path.display()));
        has_options = true;
    }
    if !options.exclude_patterns.is_empty() {
        options_output.push_str("  Exclude patterns:\n");
        for pattern in &options.exclude_patterns {
            options_output.push_str(&format!("    - {}\n", pattern));
        }
        has_options = true;
    }
    if options.patch {
        options_output.push_str("  Patch mode: Individual files (.patch)\n");
        has_options = true;
    }
    if let Some(patch_path) = &options.patch_file {
        options_output.push_str(&format!("  Combined patch file: {}\n", patch_path.display()));
        has_options = true;
    }
    if options.show_unchanged {
        options_output.push_str("  Show unchanged: Yes\n");
        has_options = true;
    }
    if !options.filter_status.is_empty() {
        options_output.push_str(&format!("  Filter status: {}\n", options.filter_status.join(", ")));
        has_options = true;
    }
    if options.stats_only {
        options_output.push_str("  Stats only: Yes\n");
        has_options = true;
    }
    if options.no_tree && !options.stats_only {
        options_output.push_str("  No tree: Yes\n");
        has_options = true;
    }
    if options.no_details && !options.stats_only {
        options_output.push_str("  No details: Yes\n");
        has_options = true;
    }
    if options.copy_deleted {
        options_output.push_str("  Copy deleted: Yes (.deleted)\n");
        has_options = true;
    }
    if options.preserve_timestamps {
        options_output.push_str("  Preserve timestamps: Yes\n");
        has_options = true;
    }

    if has_options {
        output.push_str("Options:\n");
        output.push_str(&options_output);
        output.push('\n');
    }

    if !diff_result.has_differences() {
        output.push_str("No differences found.\n");
        return output;
    }

    let (added_files, added_dirs, modified_files, deleted_files, deleted_dirs, symlinks, permission_changes, errors, _, special_files) =
        diff_result.count_by_status();

    // Calculate unchanged count (always shown, regardless of show_unchanged option)
    let unchanged_files = diff_result.unchanged_count();

    // Check which statuses are filtered out (using resolved filter with all/^ support)
    let has_filter = !options.filter_status.is_empty();
    let filter_added = is_status_filtered_out("added", &options.filter_status);
    let filter_modified = is_status_filtered_out("modified", &options.filter_status);
    let filter_deleted = is_status_filtered_out("deleted", &options.filter_status);
    let filter_symlink = is_status_filtered_out("symlink", &options.filter_status);
    let filter_special = is_status_filtered_out("special", &options.filter_status);
    let filter_permission = is_status_filtered_out("permission", &options.filter_status);
    let filter_error = is_status_filtered_out("error", &options.filter_status);
    let filter_unchanged = is_status_filtered_out("unchanged", &options.filter_status);

    // Statistics (always show full counts, with "(filtered out)" suffix when filtered)
    if added_files > 0 || added_dirs > 0 {
        let mut parts = Vec::new();
        if added_files > 0 {
            parts.push(format!("{} files", added_files));
        }
        if added_dirs > 0 {
            parts.push(format!("{} dirs", added_dirs));
        }
        let suffix = if filter_added { "    (filtered out)" } else { "" };
        output.push_str(&format!("Added:      {}{}\n", parts.join(", "), suffix));
    }
    if modified_files > 0 {
        let suffix = if filter_modified { "    (filtered out)" } else { "" };
        output.push_str(&format!("Modified:   {} files{}\n", modified_files, suffix));
    }
    if deleted_files > 0 || deleted_dirs > 0 {
        let mut parts = Vec::new();
        if deleted_files > 0 {
            parts.push(format!("{} files", deleted_files));
        }
        if deleted_dirs > 0 {
            parts.push(format!("{} dirs", deleted_dirs));
        }
        let suffix = if filter_deleted { "    (filtered out)" } else { "" };
        output.push_str(&format!("Deleted:    {}{}\n", parts.join(", "), suffix));
    }
    if symlinks > 0 {
        let suffix = if filter_symlink { "    (filtered out)" } else { "" };
        output.push_str(&format!("Symlinks:   {} files{}\n", symlinks, suffix));
    }
    if special_files > 0 {
        let suffix = if filter_special { "    (filtered out)" } else { "" };
        output.push_str(&format!("Special:    {} files{}\n", special_files, suffix));
    }
    if permission_changes > 0 {
        let suffix = if filter_permission { "    (filtered out)" } else { "" };
        output.push_str(&format!("Permissions: {} files{}\n", permission_changes, suffix));
    }
    if errors > 0 {
        let suffix = if filter_error { "    (filtered out)" } else { "" };
        output.push_str(&format!("Errors:     {} files{}\n", errors, suffix));
    }
    // Always show unchanged count
    let suffix = if filter_unchanged { "    (filtered out)" } else { "" };
    output.push_str(&format!("Unchanged:  {} files{}\n", unchanged_files, suffix));

    // Total = unique paths in source ∪ target
    let total = diff_result.total_unique_paths();
    output.push_str("--------------------------\n");
    output.push_str(&format!("Total:     {} items\n", total));

    // Show filtered count if filter is applied
    if has_filter {
        let filtered_entries: Vec<_> = diff_result.entries.iter()
            .filter(|e| entry_matches_filter(e, &options.filter_status))
            .collect();
        let filtered_perm = if permission_matches_filter(&options.filter_status) {
            diff_result.permission_changes.len()
        } else {
            0
        };
        output.push_str(&format!("Showing:   {} items (filtered)\n", filtered_entries.len() + filtered_perm));
    }
    output.push('\n');

    // stats_only mode: stop here
    if options.stats_only {
        return output;
    }

    // File Tree (skip if no_tree)
    if !options.no_tree {
        let filtered_suffix = if has_filter { " (filtered)" } else { "" };
        output.push_str("================\n");
        output.push_str(&format!("File Tree{}\n", filtered_suffix));
        output.push_str("================\n");

        // Filter entries for tree display
        let tree_entries: Vec<DiffEntry> = diff_result.entries.iter()
            .filter(|e| entry_matches_filter(e, &options.filter_status))
            .cloned()
            .collect();

        // Both console and file output use tree structure
        output.push_str(&generate_tree(&tree_entries));
        output.push('\n');
    }

    // Details sections (skip if no_details)
    if !options.no_details {
        let filtered_suffix = if has_filter { " (filtered)" } else { "" };

        // Added Details (only if not filtered out)
        if !filter_added {
            let added_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::Added))
                .collect();

            if !added_entries.is_empty() {
                output.push_str("================\n");
                output.push_str(&format!("Added Files{}\n", filtered_suffix));
                output.push_str("================\n");

                let added_dirs: Vec<_> = added_entries.iter().filter(|e| e.is_dir).collect();
                let added_files: Vec<_> = added_entries.iter().filter(|e| !e.is_dir).collect();

                if !added_dirs.is_empty() {
                    output.push_str("Directories:\n");
                    for entry in added_dirs {
                        output.push_str(&format!("  {}/\n", entry.relative_path.display()));
                    }
                    output.push('\n');
                }

                if !added_files.is_empty() {
                    output.push_str("Files:\n");
                    for entry in added_files {
                        output.push_str(&format!("  {}\n", entry.relative_path.display()));
                    }
                    output.push('\n');
                }
            }
        }

        // Modified Details (only if not filtered out)
        if !filter_modified {
            let modified_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::Modified))
                .collect();

            if !modified_entries.is_empty() {
                output.push_str("================\n");
                output.push_str(&format!("Modified Files{}\n", filtered_suffix));
                output.push_str("================\n");
                for entry in modified_entries {
                    output.push_str(&format!("  {}\n", entry.relative_path.display()));
                }
                output.push('\n');
            }
        }

        // Deleted Details (only if not filtered out)
        let deleted_entries: Vec<_> = if !filter_deleted {
            diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::Deleted))
                .collect()
        } else {
            vec![]
        };

        if !deleted_entries.is_empty() {
            output.push_str("================\n");
            output.push_str(&format!("Deleted Files{}\n", filtered_suffix));
            output.push_str("================\n");

            let deleted_dirs: Vec<_> = deleted_entries.iter().filter(|e| e.is_dir).collect();
            let deleted_files: Vec<_> = deleted_entries.iter().filter(|e| !e.is_dir).collect();

            if !deleted_dirs.is_empty() {
                output.push_str("Directories:\n");
                for entry in deleted_dirs {
                    output.push_str(&format!("  {}/\n", entry.relative_path.display()));
                }
                output.push('\n');
            }

            if !deleted_files.is_empty() {
                output.push_str("Files:\n");
                for entry in deleted_files {
                    output.push_str(&format!("  {}\n", entry.relative_path.display()));
                }
                output.push('\n');
            }
        }

        // Unchanged Details (only if show_unchanged and not filtered out)
        if options.show_unchanged && !filter_unchanged {
            let unchanged_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::Unchanged))
                .collect();

            if !unchanged_entries.is_empty() {
                output.push_str("================\n");
                output.push_str(&format!("Unchanged Files{}\n", filtered_suffix));
                output.push_str("================\n");
                for entry in unchanged_entries {
                    output.push_str(&format!("  {}\n", entry.relative_path.display()));
                }
                output.push('\n');
            }
        }

        // Symlink Details (only if not filtered out)
        if !filter_symlink {
            let symlink_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::Symlink { .. }))
                .collect();

            if !symlink_entries.is_empty() {
                output.push_str("================\n");
                output.push_str(&format!("Symlink Details{}\n", filtered_suffix));
                output.push_str("================\n");

                // Group by change type
                let added: Vec<_> = symlink_entries.iter()
                    .filter(|e| matches!(&e.status, FileStatus::Symlink { change_type: SymlinkChangeType::Added, .. }))
                    .collect();
                let deleted: Vec<_> = symlink_entries.iter()
                    .filter(|e| matches!(&e.status, FileStatus::Symlink { change_type: SymlinkChangeType::Deleted, .. }))
                    .collect();
                let changed: Vec<_> = symlink_entries.iter()
                    .filter(|e| matches!(&e.status, FileStatus::Symlink { change_type: SymlinkChangeType::Changed, .. }))
                    .collect();

                // Added symlinks
                if !added.is_empty() {
                    output.push_str("Added:\n");
                    for entry in added {
                        if let FileStatus::Symlink { current: Some(info), .. } = &entry.status {
                            let type_str = if info.is_dir { "directory" } else { "file" };
                            let status_str = if info.exists { "OK" } else { "BROKEN (target does not exist)" };
                            output.push_str(&format!(
                                "  {} -> {}\n    Type: {} | Status: {}\n\n",
                                entry.relative_path.display(),
                                info.target.display(),
                                type_str,
                                status_str
                            ));
                        }
                    }
                }

                // Deleted symlinks
                if !deleted.is_empty() {
                    output.push_str("Deleted:\n");
                    for entry in deleted {
                        if let FileStatus::Symlink { previous: Some(info), .. } = &entry.status {
                            let type_str = if info.is_dir { "directory" } else { "file" };
                            output.push_str(&format!(
                                "  {} -> {}\n    Type: {}\n\n",
                                entry.relative_path.display(),
                                info.target.display(),
                                type_str
                            ));
                        }
                    }
                }

                // Changed symlinks
                if !changed.is_empty() {
                    output.push_str("Changed:\n");
                    for entry in changed {
                        if let FileStatus::Symlink { current: Some(curr), previous: Some(prev), .. } = &entry.status {
                            let prev_type = if prev.is_dir { "directory" } else { "file" };
                            let curr_type = if curr.is_dir { "directory" } else { "file" };
                            let prev_status = if prev.exists { "OK" } else { "BROKEN" };
                            let curr_status = if curr.exists { "OK" } else { "BROKEN" };
                            output.push_str(&format!(
                                "  {}\n    Before: {} ({}, {})\n    After:  {} ({}, {})\n\n",
                                entry.relative_path.display(),
                                prev.target.display(),
                                prev_type,
                                prev_status,
                                curr.target.display(),
                                curr_type,
                                curr_status
                            ));
                        }
                    }
                }
            }
        }

        // Special Files (sockets, fifos, devices, etc.) - only if not filtered out
        if !filter_special {
            let special_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::SpecialFile { .. }))
                .collect();

            if !special_entries.is_empty() {
                output.push_str("================\n");
                output.push_str(&format!("Special Files (skipped){}\n", filtered_suffix));
                output.push_str("================\n");
                for entry in special_entries {
                    if let FileStatus::SpecialFile { file_type } = &entry.status {
                        output.push_str(&format!("{}: {}\n", entry.relative_path.display(), file_type));
                    }
                }
                output.push('\n');
            }
        }

        // Permission Changes - only if not filtered out
        if !filter_permission && !diff_result.permission_changes.is_empty() {
            output.push_str("================\n");
            output.push_str(&format!("Permission Changes{}\n", filtered_suffix));
            output.push_str("================\n");
            for change in &diff_result.permission_changes {
                output.push_str(&format!(
                    "{}: {} -> {}\n",
                    change.relative_path.display(),
                    change.old_mode,
                    change.new_mode
                ));
            }
            output.push('\n');
        }

        // Errors - only if not filtered out
        if !filter_error {
            let error_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::PermissionDenied { .. }))
                .collect();

            if !error_entries.is_empty() {
                output.push_str("================\n");
                output.push_str(&format!("Errors{}\n", filtered_suffix));
                output.push_str("================\n");
                for entry in error_entries {
                    if let FileStatus::PermissionDenied { error } = &entry.status {
                        output.push_str(&format!("{}: {}\n", entry.relative_path.display(), error));
                    }
                }
            }
        }

        // Patch Details
        if let Some(patch_result) = &options.patch_result {
            if !patch_result.patches.is_empty() || !patch_result.errors.is_empty() {
                output.push_str("================\n");
                output.push_str("Patch Details\n");
                output.push_str("================\n");

                let generated: Vec<_> = patch_result.patches.iter().filter(|p| p.patch_generated).collect();
                let skipped: Vec<_> = patch_result.patches.iter().filter(|p| p.is_binary).collect();

                if patch_result.errors.is_empty() {
                    output.push_str(&format!(
                        "Generated: {} patches, Skipped: {} (binary)\n\n",
                        patch_result.total_generated,
                        patch_result.total_skipped
                    ));
                } else {
                    output.push_str(&format!(
                        "Generated: {} patches, Skipped: {} (binary), Failed: {}\n\n",
                        patch_result.total_generated,
                        patch_result.total_skipped,
                        patch_result.errors.len()
                    ));
                }

                if !generated.is_empty() {
                    output.push_str("Generated:\n");
                    for patch in generated {
                        if options.patch {
                            output.push_str(&format!("  {}.patch\n", patch.relative_path.display()));
                        } else {
                            output.push_str(&format!("  {}\n", patch.relative_path.display()));
                        }
                    }
                    output.push('\n');
                }

                if !skipped.is_empty() {
                    output.push_str("Skipped (binary):\n");
                    for patch in skipped {
                        output.push_str(&format!("  {} [skip]\n", patch.relative_path.display()));
                    }
                    output.push('\n');
                }

                if !patch_result.errors.is_empty() {
                    output.push_str("Failed:\n");
                    for err in &patch_result.errors {
                        output.push_str(&format!("  {}: {}\n", err.relative_path.display(), err.error));
                    }
                    output.push('\n');
                }
            }
        }

        // Copy Failed
        if let Some(copy_result) = &options.copy_result {
            if !copy_result.errors.is_empty() {
                output.push_str("================\n");
                output.push_str("Copy Failed\n");
                output.push_str("================\n");
                output.push_str(&format!(
                    "Failed: {} files (Copied: {} files)\n\n",
                    copy_result.errors.len(),
                    copy_result.copied_count
                ));
                for err in &copy_result.errors {
                    output.push_str(&format!("  {}: {}\n", err.relative_path.display(), err.error));
                }
                output.push('\n');
            }
        }
    } // End of !options.no_details block

    output
}

// ============================================================================
// Three-way summary and Excel output
// ============================================================================

/// Options for three-way summary generation
struct ThreeWaySummaryOptions {
    exclude_patterns: Vec<String>,
    dry_run: bool,
    merge_style: MergeStyle,
    conflict_only: bool,
    config_file: Option<PathBuf>,
    output_dir: PathBuf,
    copy_result: Option<ThreeWayCopyResult>,
    filter_status: Vec<String>,
    stats_only: bool,
    no_tree: bool,
    no_details: bool,
    // Output format: true = full path (file), false = grouped by directory (console)
    output_to_file: bool,
}

/// Generate three-way summary
fn generate_three_way_summary(diff_result: &ThreeWayDiffResult, options: &ThreeWaySummaryOptions) -> String {
    let mut output = String::new();
    let now = Local::now();

    output.push_str("rs_diffcopy Summary (Three-way)\n");
    output.push_str("================================\n");
    output.push_str(&format!("Base:   {}\n", diff_result.base_dir.display()));
    output.push_str(&format!("Ours:   {}\n", diff_result.ours_dir.display()));
    output.push_str(&format!("Theirs: {}\n", diff_result.theirs_dir.display()));
    output.push_str(&format!("Output: {}\n", options.output_dir.display()));
    output.push_str(&format!("Date:   {}\n", now.format("%Y-%m-%d %H:%M:%S")));
    output.push('\n');

    // Options section
    if options.dry_run || options.conflict_only || options.merge_style != MergeStyle::All
        || options.config_file.is_some() || !options.exclude_patterns.is_empty()
        || !options.filter_status.is_empty() || options.stats_only || options.no_tree || options.no_details
    {
        output.push_str("Options:\n");
        if options.dry_run {
            output.push_str("  Mode: Dry-run (no files copied)\n");
        }
        let merge_str = match options.merge_style {
            MergeStyle::All => "all",
            MergeStyle::Ours => "ours",
            MergeStyle::Theirs => "theirs",
        };
        output.push_str(&format!("  Merge style: {}\n", merge_str));
        if options.conflict_only {
            output.push_str("  Conflict only: yes\n");
        }
        if let Some(ref config) = options.config_file {
            output.push_str(&format!("  Config file: {}\n", config.display()));
        }
        if !options.exclude_patterns.is_empty() {
            output.push_str("  Exclude patterns:\n");
            for pattern in &options.exclude_patterns {
                output.push_str(&format!("    - {}\n", pattern));
            }
        }
        // Filter options
        if !options.filter_status.is_empty() {
            output.push_str(&format!("  Filter status: {}\n", options.filter_status.join(", ")));
        }
        if options.stats_only {
            output.push_str("  Output: Statistics only\n");
        }
        if options.no_tree {
            output.push_str("  No file matrix: Yes\n");
        }
        if options.no_details {
            output.push_str("  No details: Yes\n");
        }
        output.push('\n');
    }

    // Check if there are any differences
    if !diff_result.has_differences() {
        output.push_str("No differences found.\n");
        return output;
    }

    // Change Matrix (statistics)
    output.push_str("================\n");
    output.push_str("Change Matrix\n");
    output.push_str("================\n");
    output.push_str("Status          | Count\n");
    output.push_str("----------------|------\n");

    let unchanged = diff_result.count_by_status(&ThreeWayStatus::Unchanged);
    let ours_only = diff_result.count_by_status(&ThreeWayStatus::OursOnly);
    let theirs_only = diff_result.count_by_status(&ThreeWayStatus::TheirsOnly);
    let both_same = diff_result.count_by_status(&ThreeWayStatus::BothSame);
    let conflict = diff_result.count_by_status(&ThreeWayStatus::Conflict);
    let added_ours = diff_result.count_by_status(&ThreeWayStatus::AddedOurs);
    let added_theirs = diff_result.count_by_status(&ThreeWayStatus::AddedTheirs);
    let added_both_same = diff_result.count_by_status(&ThreeWayStatus::AddedBothSame);
    let added_both_diff = diff_result.count_by_status(&ThreeWayStatus::AddedBothDiff);
    let deleted_ours = diff_result.count_by_status(&ThreeWayStatus::DeletedOurs);
    let deleted_theirs = diff_result.count_by_status(&ThreeWayStatus::DeletedTheirs);
    let deleted_both = diff_result.count_by_status(&ThreeWayStatus::DeletedBoth);
    let modify_delete = diff_result.count_by_status(&ThreeWayStatus::ModifyDelete);
    let delete_modify = diff_result.count_by_status(&ThreeWayStatus::DeleteModify);

    output.push_str(&format!("Unchanged       | {:>5}\n", unchanged));
    output.push_str(&format!("Ours only       | {:>5}\n", ours_only));
    output.push_str(&format!("Theirs only     | {:>5}\n", theirs_only));
    output.push_str(&format!("Both same       | {:>5}\n", both_same));
    output.push_str(&format!("Conflict        | {:>5}\n", conflict));
    output.push_str(&format!("Added (ours)    | {:>5}\n", added_ours));
    output.push_str(&format!("Added (theirs)  | {:>5}\n", added_theirs));
    output.push_str(&format!("Added (both)    | {:>5}\n", added_both_same + added_both_diff));
    output.push_str(&format!("Deleted (ours)  | {:>5}\n", deleted_ours));
    output.push_str(&format!("Deleted (theirs)| {:>5}\n", deleted_theirs));
    output.push_str(&format!("Deleted (both)  | {:>5}\n", deleted_both));
    output.push_str(&format!("Modify/Delete   | {:>5}\n", modify_delete + delete_modify));
    output.push_str("--------------------------\n");
    output.push_str(&format!("Total           | {:>5}\n", diff_result.total_paths));
    output.push_str(&format!("Conflicts       | {:>5}\n", diff_result.count_conflicts()));
    output.push('\n');

    // Return early if stats_only
    if options.stats_only {
        return output;
    }

    // File Tree (three-way)
    if !options.no_tree {
        output.push_str("================\n");
        output.push_str("File Tree\n");
        output.push_str("================\n");

        // Collect filtered entries
        let filtered_entries: Vec<_> = diff_result.entries.iter()
            .filter(|entry| {
                // Skip unchanged by default (unless explicitly included in filter)
                if entry.status == ThreeWayStatus::Unchanged && options.filter_status.is_empty() {
                    return false;
                }
                // Apply filter
                !is_three_way_status_filtered_out(&entry.status, &options.filter_status)
            })
            .collect();

        if options.output_to_file {
            // File output: Aligned tree format
            output.push_str(&generate_three_way_tree_file(&filtered_entries));
        } else {
            // Console output: Compact tree format
            output.push_str(&generate_three_way_tree_console(&filtered_entries));
        }
        output.push('\n');
    }

    // Conflict Details
    if !options.no_details {
        let conflicts: Vec<_> = diff_result.entries.iter()
            .filter(|e| e.status.is_conflict() && !is_three_way_status_filtered_out(&e.status, &options.filter_status))
            .collect();
        if !conflicts.is_empty() {
            output.push_str("================\n");
            output.push_str("Conflict Details\n");
            output.push_str("================\n");

            for (i, entry) in conflicts.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, entry.relative_path.display()));
                output.push_str(&format!("   Type: {}\n", entry.status.display_str()));
                if let Some(size) = entry.base_size {
                    output.push_str(&format!("   Base: {} bytes\n", size));
                }
                if let Some(size) = entry.ours_size {
                    output.push_str(&format!("   Ours: {} bytes\n", size));
                } else {
                    output.push_str("   Ours: deleted\n");
                }
                if let Some(size) = entry.theirs_size {
                    output.push_str(&format!("   Theirs: {} bytes\n", size));
                } else {
                    output.push_str("   Theirs: deleted\n");
                }
                output.push('\n');
            }
        }
    }

    // Copy errors
    if let Some(ref copy_result) = options.copy_result {
        if !copy_result.errors.is_empty() {
            output.push_str("================\n");
            output.push_str("Copy Failed\n");
            output.push_str("================\n");
            output.push_str(&format!("Failed: {} files\n\n", copy_result.errors.len()));
            for err in &copy_result.errors {
                output.push_str(&format!("  {}: {}\n", err.relative_path.display(), err.error));
            }
            output.push('\n');
        }
    }

    output
}

/// Generate Excel summary for three-way comparison
fn generate_three_way_excel(diff_result: &ThreeWayDiffResult, options: &ThreeWaySummaryOptions, excel_path: &Path) -> Result<()> {
    let mut workbook = Workbook::new();
    let now = Local::now();

    // Define formats
    let title_format = Format::new()
        .set_bold()
        .set_font_size(16)
        .set_align(FormatAlign::Left);

    let header_format = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);

    let conflict_format = Format::new()
        .set_bold()
        .set_font_color(Color::RGB(0xCC0000));

    let ours_format = Format::new()
        .set_font_color(Color::RGB(0x008000));

    let theirs_format = Format::new()
        .set_font_color(Color::RGB(0x0066CC));

    let both_same_format = Format::new()
        .set_font_color(Color::RGB(0x00BFFF));

    let unchanged_format = Format::new()
        .set_font_color(Color::RGB(0x808080));

    // ========== Summary Sheet ==========
    let summary_sheet = workbook.add_worksheet();
    summary_sheet.set_name("Summary")?;
    summary_sheet.set_column_width(0, 20)?;
    summary_sheet.set_column_width(1, 60)?;

    let mut row = 0u32;
    summary_sheet.write_with_format(row, 0, "rs_diffcopy Summary (Three-way)", &title_format)?;
    row += 2;

    // Basic info
    summary_sheet.write(row, 0, "Base:")?;
    summary_sheet.write(row, 1, diff_result.base_dir.display().to_string())?;
    row += 1;
    summary_sheet.write(row, 0, "Ours:")?;
    summary_sheet.write(row, 1, diff_result.ours_dir.display().to_string())?;
    row += 1;
    summary_sheet.write(row, 0, "Theirs:")?;
    summary_sheet.write(row, 1, diff_result.theirs_dir.display().to_string())?;
    row += 1;
    summary_sheet.write(row, 0, "Output:")?;
    summary_sheet.write(row, 1, options.output_dir.display().to_string())?;
    row += 1;
    summary_sheet.write(row, 0, "Date:")?;
    summary_sheet.write(row, 1, now.format("%Y-%m-%d %H:%M:%S").to_string())?;
    row += 2;

    // Statistics
    summary_sheet.write_with_format(row, 0, "Statistics", &header_format)?;
    summary_sheet.write_with_format(row, 1, "Count", &header_format)?;
    row += 1;

    let stats = [
        ("Unchanged", diff_result.count_by_status(&ThreeWayStatus::Unchanged)),
        ("Ours only", diff_result.count_by_status(&ThreeWayStatus::OursOnly)),
        ("Theirs only", diff_result.count_by_status(&ThreeWayStatus::TheirsOnly)),
        ("Both same", diff_result.count_by_status(&ThreeWayStatus::BothSame)),
        ("Conflict", diff_result.count_by_status(&ThreeWayStatus::Conflict)),
        ("Added (ours)", diff_result.count_by_status(&ThreeWayStatus::AddedOurs)),
        ("Added (theirs)", diff_result.count_by_status(&ThreeWayStatus::AddedTheirs)),
        ("Added (both)", diff_result.count_by_status(&ThreeWayStatus::AddedBothSame) + diff_result.count_by_status(&ThreeWayStatus::AddedBothDiff)),
        ("Deleted (ours)", diff_result.count_by_status(&ThreeWayStatus::DeletedOurs)),
        ("Deleted (theirs)", diff_result.count_by_status(&ThreeWayStatus::DeletedTheirs)),
        ("Deleted (both)", diff_result.count_by_status(&ThreeWayStatus::DeletedBoth)),
        ("Total", diff_result.total_paths),
        ("Conflicts", diff_result.count_conflicts()),
    ];

    for (label, count) in stats {
        summary_sheet.write(row, 0, label)?;
        summary_sheet.write(row, 1, count as f64)?;
        row += 1;
    }

    // ========== File Matrix Sheet ==========
    let matrix_sheet = workbook.add_worksheet();
    matrix_sheet.set_name("File Matrix")?;
    matrix_sheet.set_column_width(0, 50)?;
    matrix_sheet.set_column_width(1, 8)?;
    matrix_sheet.set_column_width(2, 8)?;
    matrix_sheet.set_column_width(3, 8)?;
    matrix_sheet.set_column_width(4, 25)?;

    row = 0;
    matrix_sheet.write_with_format(row, 0, "File", &header_format)?;
    matrix_sheet.write_with_format(row, 1, "Base", &header_format)?;
    matrix_sheet.write_with_format(row, 2, "Ours", &header_format)?;
    matrix_sheet.write_with_format(row, 3, "Theirs", &header_format)?;
    matrix_sheet.write_with_format(row, 4, "Status", &header_format)?;
    row += 1;

    for entry in &diff_result.entries {
        // Apply filter
        if is_three_way_status_filtered_out(&entry.status, &options.filter_status) {
            continue;
        }
        let format = if entry.status.is_conflict() {
            &conflict_format
        } else {
            match &entry.status {
                ThreeWayStatus::OursOnly | ThreeWayStatus::AddedOurs => &ours_format,
                ThreeWayStatus::TheirsOnly | ThreeWayStatus::AddedTheirs => &theirs_format,
                ThreeWayStatus::BothSame | ThreeWayStatus::AddedBothSame => &both_same_format,
                ThreeWayStatus::Unchanged => &unchanged_format,
                _ => &unchanged_format,
            }
        };

        matrix_sheet.write_with_format(row, 0, entry.relative_path.display().to_string(), format)?;
        matrix_sheet.write_with_format(row, 1, entry.status.base_indicator(), format)?;
        matrix_sheet.write_with_format(row, 2, entry.status.ours_indicator(), format)?;
        matrix_sheet.write_with_format(row, 3, entry.status.theirs_indicator(), format)?;
        matrix_sheet.write_with_format(row, 4, entry.status.display_str(), format)?;
        row += 1;
    }

    // ========== Conflicts Sheet ==========
    let conflicts: Vec<_> = diff_result.entries.iter()
        .filter(|e| e.status.is_conflict() && !is_three_way_status_filtered_out(&e.status, &options.filter_status))
        .collect();
    if !conflicts.is_empty() {
        let conflict_sheet = workbook.add_worksheet();
        conflict_sheet.set_name("Conflicts")?;
        conflict_sheet.set_column_width(0, 50)?;
        conflict_sheet.set_column_width(1, 25)?;
        conflict_sheet.set_column_width(2, 15)?;
        conflict_sheet.set_column_width(3, 15)?;
        conflict_sheet.set_column_width(4, 15)?;

        row = 0;
        conflict_sheet.write_with_format(row, 0, "File", &header_format)?;
        conflict_sheet.write_with_format(row, 1, "Type", &header_format)?;
        conflict_sheet.write_with_format(row, 2, "Base Size", &header_format)?;
        conflict_sheet.write_with_format(row, 3, "Ours Size", &header_format)?;
        conflict_sheet.write_with_format(row, 4, "Theirs Size", &header_format)?;
        row += 1;

        for entry in &conflicts {
            conflict_sheet.write_with_format(row, 0, entry.relative_path.display().to_string(), &conflict_format)?;
            conflict_sheet.write_with_format(row, 1, entry.status.display_str(), &conflict_format)?;
            if let Some(size) = entry.base_size {
                conflict_sheet.write(row, 2, size as f64)?;
            } else {
                conflict_sheet.write(row, 2, "-")?;
            }
            if let Some(size) = entry.ours_size {
                conflict_sheet.write(row, 3, size as f64)?;
            } else {
                conflict_sheet.write(row, 3, "deleted")?;
            }
            if let Some(size) = entry.theirs_size {
                conflict_sheet.write(row, 4, size as f64)?;
            } else {
                conflict_sheet.write(row, 4, "deleted")?;
            }
            row += 1;
        }
    }

    workbook.save(excel_path)?;
    Ok(())
}

/// Generate Excel summary report
fn generate_excel_summary(diff_result: &DiffResult, options: &SummaryOptions, excel_path: &Path) -> Result<()> {
    let mut workbook = Workbook::new();
    let now = Local::now();

    // Define formats
    let title_format = Format::new()
        .set_bold()
        .set_font_size(16)
        .set_font_color(Color::RGB(0x2E5090))
        .set_align(FormatAlign::Left);

    let header_format = Format::new()
        .set_bold()
        .set_font_size(12)
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White)
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin);

    let section_header_format = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_background_color(Color::RGB(0xD9E2F3))
        .set_font_color(Color::RGB(0x2E5090))
        .set_border(FormatBorder::Thin);

    let info_label_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Left);

    let info_value_format = Format::new()
        .set_align(FormatAlign::Left);

    let cell_format = Format::new()
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left);

    let added_format = Format::new()
        .set_font_color(Color::RGB(0x008000))
        .set_align(FormatAlign::Left);

    let modified_format = Format::new()
        .set_font_color(Color::RGB(0x0066CC))
        .set_align(FormatAlign::Left);

    let deleted_format = Format::new()
        .set_font_color(Color::RGB(0xCC0000))
        .set_align(FormatAlign::Left);

    let symlink_format = Format::new()
        .set_font_color(Color::RGB(0x9933FF))
        .set_align(FormatAlign::Left);

    let tree_format = Format::new()
        .set_font_name("Consolas")
        .set_font_size(10);

    let number_format = Format::new()
        .set_align(FormatAlign::Right)
        .set_border(FormatBorder::Thin);

    // ==================== Summary Sheet ====================
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Summary")?;

    // Set column widths
    worksheet.set_column_width(0, 20)?;
    worksheet.set_column_width(1, 60)?;
    worksheet.set_column_width(2, 15)?;

    let mut row: u32 = 0;

    // Title
    worksheet.merge_range(row, 0, row, 2, "rs_diffcopy Summary Report", &title_format)?;
    row += 2;

    // Basic information
    worksheet.write_with_format(row, 0, "Source:", &info_label_format)?;
    worksheet.write_with_format(row, 1, diff_result.source_dir.display().to_string(), &info_value_format)?;
    row += 1;

    worksheet.write_with_format(row, 0, "Target:", &info_label_format)?;
    worksheet.write_with_format(row, 1, diff_result.target_dir.display().to_string(), &info_value_format)?;
    row += 1;

    worksheet.write_with_format(row, 0, "Output:", &info_label_format)?;
    worksheet.write_with_format(row, 1, options.output_dir.display().to_string(), &info_value_format)?;
    row += 1;

    worksheet.write_with_format(row, 0, "Date:", &info_label_format)?;
    worksheet.write_with_format(row, 1, now.format("%Y-%m-%d %H:%M:%S").to_string(), &info_value_format)?;
    row += 2;

    // Options section (if any)
    let mut has_options = false;
    if options.dry_run || options.both_versions || options.check_permissions != PermissionCheckMode::None
        || options.config_file.is_some() || !options.exclude_patterns.is_empty()
        || options.patch || options.patch_file.is_some()
        || !options.filter_status.is_empty() || options.stats_only || options.no_tree || options.no_details {
        has_options = true;
    }

    if has_options {
        worksheet.merge_range(row, 0, row, 2, "Options", &section_header_format)?;
        row += 1;

        if options.dry_run {
            worksheet.write_with_format(row, 0, "Mode:", &info_label_format)?;
            worksheet.write_with_format(row, 1, "Dry-run (no files copied)", &info_value_format)?;
            row += 1;
        }
        if options.both_versions {
            worksheet.write_with_format(row, 0, "Copy mode:", &info_label_format)?;
            worksheet.write_with_format(row, 1, "Both versions (.old/.new)", &info_value_format)?;
            row += 1;
        }
        if options.check_permissions != PermissionCheckMode::None {
            let mode_str = match options.check_permissions {
                PermissionCheckMode::Scripts => "scripts",
                PermissionCheckMode::All => "all",
                PermissionCheckMode::None => "none",
            };
            worksheet.write_with_format(row, 0, "Permission check:", &info_label_format)?;
            worksheet.write_with_format(row, 1, mode_str, &info_value_format)?;
            row += 1;
        }
        if let Some(config_path) = &options.config_file {
            worksheet.write_with_format(row, 0, "Config file:", &info_label_format)?;
            worksheet.write_with_format(row, 1, config_path.display().to_string(), &info_value_format)?;
            row += 1;
        }
        if options.patch {
            worksheet.write_with_format(row, 0, "Patch mode:", &info_label_format)?;
            worksheet.write_with_format(row, 1, "Individual files (.patch)", &info_value_format)?;
            row += 1;
        }
        if let Some(patch_path) = &options.patch_file {
            worksheet.write_with_format(row, 0, "Combined patch:", &info_label_format)?;
            worksheet.write_with_format(row, 1, patch_path.display().to_string(), &info_value_format)?;
            row += 1;
        }
        if !options.exclude_patterns.is_empty() {
            worksheet.write_with_format(row, 0, "Exclude patterns:", &info_label_format)?;
            worksheet.write_with_format(row, 1, options.exclude_patterns.join(", "), &info_value_format)?;
            row += 1;
        }
        // Filter options
        if !options.filter_status.is_empty() {
            worksheet.write_with_format(row, 0, "Filter status:", &info_label_format)?;
            worksheet.write_with_format(row, 1, options.filter_status.join(", "), &info_value_format)?;
            row += 1;
        }
        if options.stats_only {
            worksheet.write_with_format(row, 0, "Output:", &info_label_format)?;
            worksheet.write_with_format(row, 1, "Statistics only", &info_value_format)?;
            row += 1;
        }
        if options.no_tree {
            worksheet.write_with_format(row, 0, "No tree:", &info_label_format)?;
            worksheet.write_with_format(row, 1, "Yes", &info_value_format)?;
            row += 1;
        }
        if options.no_details {
            worksheet.write_with_format(row, 0, "No details:", &info_label_format)?;
            worksheet.write_with_format(row, 1, "Yes", &info_value_format)?;
            row += 1;
        }
        row += 1;
    }

    // Statistics section
    let (added_files, added_dirs, modified_files, deleted_files, deleted_dirs, symlinks, permission_changes, errors, _, special_files) =
        diff_result.count_by_status();
    let unchanged_files = diff_result.unchanged_count();

    // Helper to get filtered suffix (using resolved filter with all/^ support)
    let filtered_suffix = |status: &str| -> &str {
        if is_status_filtered_out(status, &options.filter_status) { " (filtered out)" } else { "" }
    };

    worksheet.merge_range(row, 0, row, 2, "Statistics", &section_header_format)?;
    row += 1;

    worksheet.write_with_format(row, 0, "Category", &header_format)?;
    worksheet.write_with_format(row, 1, "Description", &header_format)?;
    worksheet.write_with_format(row, 2, "Count", &header_format)?;
    row += 1;

    if added_files > 0 || added_dirs > 0 {
        let label = format!("Added{}", filtered_suffix("added"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        let desc = if added_dirs > 0 {
            format!("{} files, {} dirs", added_files, added_dirs)
        } else {
            format!("{} files", added_files)
        };
        worksheet.write_with_format(row, 1, &desc, &added_format)?;
        worksheet.write_number_with_format(row, 2, (added_files + added_dirs) as f64, &number_format)?;
        row += 1;
    }

    if modified_files > 0 {
        let label = format!("Modified{}", filtered_suffix("modified"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        worksheet.write_with_format(row, 1, format!("{} files", modified_files), &modified_format)?;
        worksheet.write_number_with_format(row, 2, modified_files as f64, &number_format)?;
        row += 1;
    }

    if deleted_files > 0 || deleted_dirs > 0 {
        let label = format!("Deleted{}", filtered_suffix("deleted"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        let desc = if deleted_dirs > 0 {
            format!("{} files, {} dirs", deleted_files, deleted_dirs)
        } else {
            format!("{} files", deleted_files)
        };
        worksheet.write_with_format(row, 1, &desc, &deleted_format)?;
        worksheet.write_number_with_format(row, 2, (deleted_files + deleted_dirs) as f64, &number_format)?;
        row += 1;
    }

    if symlinks > 0 {
        let label = format!("Symlinks{}", filtered_suffix("symlink"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        worksheet.write_with_format(row, 1, format!("{} files", symlinks), &symlink_format)?;
        worksheet.write_number_with_format(row, 2, symlinks as f64, &number_format)?;
        row += 1;
    }

    if special_files > 0 {
        let special_format = Format::new()
            .set_font_color(Color::RGB(0x666666))
            .set_align(FormatAlign::Left);
        let label = format!("Special{}", filtered_suffix("special"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        worksheet.write_with_format(row, 1, format!("{} files (sockets, fifos, etc.)", special_files), &special_format)?;
        worksheet.write_number_with_format(row, 2, special_files as f64, &number_format)?;
        row += 1;
    }

    if permission_changes > 0 {
        let label = format!("Permissions{}", filtered_suffix("permission"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        worksheet.write_with_format(row, 1, format!("{} files", permission_changes), &info_value_format)?;
        worksheet.write_number_with_format(row, 2, permission_changes as f64, &number_format)?;
        row += 1;
    }

    if errors > 0 {
        let label = format!("Errors{}", filtered_suffix("error"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        worksheet.write_with_format(row, 1, format!("{} files", errors), &deleted_format)?;
        worksheet.write_number_with_format(row, 2, errors as f64, &number_format)?;
        row += 1;
    }

    // Always show unchanged count
    {
        let unchanged_format = Format::new()
            .set_font_color(Color::RGB(0x808080))
            .set_align(FormatAlign::Left);
        let label = format!("Unchanged{}", filtered_suffix("unchanged"));
        worksheet.write_with_format(row, 0, &label, &cell_format)?;
        worksheet.write_with_format(row, 1, format!("{} files", unchanged_files), &unchanged_format)?;
        worksheet.write_number_with_format(row, 2, unchanged_files as f64, &number_format)?;
        row += 1;
    }

    // Total = unique paths in source ∪ target
    let total = diff_result.total_unique_paths();
    let total_row_format = Format::new()
        .set_bold()
        .set_border(FormatBorder::Thin)
        .set_background_color(Color::RGB(0xF2F2F2));
    worksheet.write_with_format(row, 0, "Total", &total_row_format)?;
    worksheet.write_with_format(row, 1, "", &total_row_format)?;
    worksheet.write_number_with_format(row, 2, total as f64, &total_row_format)?;
    row += 1;

    // Show filtered count if filter is active
    if !options.filter_status.is_empty() {
        let filtered_count = diff_result.entries.iter()
            .filter(|e| entry_matches_filter(e, &options.filter_status))
            .count();
        let perm_filtered_count = if permission_matches_filter(&options.filter_status) {
            diff_result.permission_changes.len()
        } else {
            0
        };
        let showing_count = filtered_count + perm_filtered_count;
        row += 1;
        worksheet.write_with_format(row, 0, "Showing:", &info_label_format)?;
        worksheet.write_with_format(row, 1, format!("{} items (filtered)", showing_count), &info_value_format)?;
    }

    // Return early if stats_only
    if options.stats_only {
        workbook.save(excel_path)?;
        return Ok(());
    }

    // ==================== File Tree Sheet ====================
    if !options.no_tree {
        let tree_sheet = workbook.add_worksheet();
        tree_sheet.set_name("File Tree")?;

        // Set column widths for tree display
        for col in 0..10 {
            tree_sheet.set_column_width(col, 4)?;
        }
        tree_sheet.set_column_width(10, 40)?;  // File name column
        tree_sheet.set_column_width(11, 15)?;  // Status column

        let mut tree_row: u32 = 0;
        tree_sheet.write_with_format(tree_row, 0, "File Tree", &title_format)?;
        tree_row += 2;

        // Write tree header
        tree_sheet.merge_range(tree_row, 0, tree_row, 10, "Path", &header_format)?;
        tree_sheet.write_with_format(tree_row, 11, "Status", &header_format)?;
        tree_row += 1;

        // Build tree and write to Excel (filter entries if filter is active)
        let unchanged_format_tree = Format::new()
            .set_font_color(Color::RGB(0x808080))
            .set_font_name("Consolas")
            .set_font_size(10);
        let filtered_entries: Vec<DiffEntry> = if !options.filter_status.is_empty() {
            diff_result.entries.iter()
                .filter(|e| entry_matches_filter(e, &options.filter_status))
                .cloned()
                .collect()
        } else {
            diff_result.entries.clone()
        };
        write_excel_tree(&filtered_entries, tree_sheet, &mut tree_row, &tree_format, &added_format, &modified_format, &deleted_format, &symlink_format, &unchanged_format_tree, options.excel_fold_level)?;
    }

    // ==================== Details Sheet ====================
    if !options.no_details {
        let details_sheet = workbook.add_worksheet();
        details_sheet.set_name("Details")?;

        details_sheet.set_column_width(0, 12)?;  // Type
        details_sheet.set_column_width(1, 40)?;  // Directory
        details_sheet.set_column_width(2, 30)?;  // File
        details_sheet.set_column_width(3, 30)?;  // Notes

        let mut details_row: u32 = 0;
        details_sheet.write_with_format(details_row, 0, "Change Details", &title_format)?;
        details_row += 2;

        // Helper function to split path into directory and file name
        fn split_path(path: &Path) -> (String, String) {
            let parent = path.parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let file_name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent, file_name)
        }

        // Helper to check if status should be shown (filter with ^exclusion and all support)
        let show_added = !is_status_filtered_out("added", &options.filter_status);
        let show_modified = !is_status_filtered_out("modified", &options.filter_status);
        let show_deleted = !is_status_filtered_out("deleted", &options.filter_status);
        let show_unchanged = !is_status_filtered_out("unchanged", &options.filter_status);
        let show_symlink = !is_status_filtered_out("symlink", &options.filter_status);
        let show_special = !is_status_filtered_out("special", &options.filter_status);
        let show_permission = !is_status_filtered_out("permission", &options.filter_status);
        let show_error = !is_status_filtered_out("error", &options.filter_status);

        // Added files
        if show_added {
            let added_entries: Vec<_> = diff_result.entries.iter()
                .filter(|e| matches!(e.status, FileStatus::Added))
                .collect();

            if !added_entries.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Added Files", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Type", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Notes", &header_format)?;
                details_row += 1;

                for entry in &added_entries {
                    let type_str = if entry.is_dir { "Directory" } else { "File" };
                    let (dir, file) = split_path(&entry.relative_path);
                    details_sheet.write_with_format(details_row, 0, type_str, &cell_format)?;
                    details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                    details_sheet.write_with_format(details_row, 2, &file, &added_format)?;
                    details_sheet.write_with_format(details_row, 3, "", &cell_format)?;
                    details_row += 1;
                }
                details_row += 1;
            }
        }

        // Modified files
        if show_modified {
            let modified_entries: Vec<_> = diff_result.entries.iter()
                .filter(|e| matches!(e.status, FileStatus::Modified))
                .collect();

            if !modified_entries.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Modified Files", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Type", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Notes", &header_format)?;
                details_row += 1;

                for entry in &modified_entries {
                    let (dir, file) = split_path(&entry.relative_path);
                    details_sheet.write_with_format(details_row, 0, "File", &cell_format)?;
                    details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                    details_sheet.write_with_format(details_row, 2, &file, &modified_format)?;
                    details_sheet.write_with_format(details_row, 3, "", &cell_format)?;
                    details_row += 1;
                }
                details_row += 1;
            }
        }

        // Deleted files
        if show_deleted {
            let deleted_entries: Vec<_> = diff_result.entries.iter()
                .filter(|e| matches!(e.status, FileStatus::Deleted))
                .collect();

            if !deleted_entries.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Deleted Files", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Type", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Notes", &header_format)?;
                details_row += 1;

                for entry in &deleted_entries {
                    let type_str = if entry.is_dir { "Directory" } else { "File" };
                    let (dir, file) = split_path(&entry.relative_path);
                    details_sheet.write_with_format(details_row, 0, type_str, &cell_format)?;
                    details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                    details_sheet.write_with_format(details_row, 2, &file, &deleted_format)?;
                    details_sheet.write_with_format(details_row, 3, "", &cell_format)?;
                    details_row += 1;
                }
                details_row += 1;
            }
        }

        // Unchanged files
        if show_unchanged {
            let unchanged_entries: Vec<_> = diff_result.entries.iter()
                .filter(|e| matches!(e.status, FileStatus::Unchanged))
                .collect();

            if !unchanged_entries.is_empty() {
                let unchanged_format = Format::new()
                    .set_font_color(Color::RGB(0x808080))
                    .set_align(FormatAlign::Left);

                details_sheet.merge_range(details_row, 0, details_row, 3, "Unchanged Files", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Type", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Notes", &header_format)?;
                details_row += 1;

                for entry in &unchanged_entries {
                    let (dir, file) = split_path(&entry.relative_path);
                    details_sheet.write_with_format(details_row, 0, "File", &cell_format)?;
                    details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                    details_sheet.write_with_format(details_row, 2, &file, &unchanged_format)?;
                    details_sheet.write_with_format(details_row, 3, "", &cell_format)?;
                    details_row += 1;
                }
                details_row += 1;
            }
        }

        // Symlink details
        if show_symlink {
            let symlink_entries: Vec<_> = diff_result.entries.iter()
                .filter(|e| matches!(e.status, FileStatus::Symlink { .. }))
                .collect();

            if !symlink_entries.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Symlink Details", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Change", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Target", &header_format)?;
                details_row += 1;

                for entry in &symlink_entries {
                    if let FileStatus::Symlink { change_type, current, previous } = &entry.status {
                        let change_str = match change_type {
                            SymlinkChangeType::Added => "Added",
                            SymlinkChangeType::Deleted => "Deleted",
                            SymlinkChangeType::Changed => "Changed",
                        };
                        let target_str = match (current, previous) {
                            (Some(info), _) => {
                                let broken = if !info.exists { " (BROKEN)" } else { "" };
                                format!("{}{}", info.target.display(), broken)
                            }
                            (None, Some(info)) => info.target.display().to_string(),
                            _ => String::new(),
                        };
                        let (dir, file) = split_path(&entry.relative_path);
                        details_sheet.write_with_format(details_row, 0, change_str, &cell_format)?;
                        details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                        details_sheet.write_with_format(details_row, 2, &file, &symlink_format)?;
                        details_sheet.write_with_format(details_row, 3, target_str, &cell_format)?;
                        details_row += 1;
                    }
                }
                details_row += 1;
            }
        }

        // Special files
        if show_special {
            let special_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::SpecialFile { .. }))
                .collect();

            if !special_entries.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Special Files (skipped)", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Type", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Notes", &header_format)?;
                details_row += 1;

                let special_file_format = Format::new()
                    .set_font_color(Color::RGB(0x666666))
                    .set_align(FormatAlign::Left);

                for entry in special_entries {
                    if let FileStatus::SpecialFile { file_type } = &entry.status {
                        let type_str = file_type.to_string();
                        let (dir, file) = split_path(&entry.relative_path);
                        details_sheet.write_with_format(details_row, 0, &type_str, &cell_format)?;
                        details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                        details_sheet.write_with_format(details_row, 2, &file, &special_file_format)?;
                        details_sheet.write_with_format(details_row, 3, "Cannot be copied", &cell_format)?;
                        details_row += 1;
                    }
                }
                details_row += 1;
            }
        }

        // Permission changes
        if show_permission && !diff_result.permission_changes.is_empty() {
            details_sheet.merge_range(details_row, 0, details_row, 3, "Permission Changes", &section_header_format)?;
            details_row += 1;

            details_sheet.write_with_format(details_row, 0, "Old", &header_format)?;
            details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
            details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
            details_sheet.write_with_format(details_row, 3, "New", &header_format)?;
            details_row += 1;

            for change in &diff_result.permission_changes {
                let (dir, file) = split_path(&change.relative_path);
                details_sheet.write_with_format(details_row, 0, &change.old_mode, &cell_format)?;
                details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                details_sheet.write_with_format(details_row, 2, &file, &info_value_format)?;
                details_sheet.write_with_format(details_row, 3, &change.new_mode, &cell_format)?;
                details_row += 1;
            }
        }

        // Error entries (PermissionDenied)
        if show_error {
            let error_entries: Vec<_> = diff_result
                .entries
                .iter()
                .filter(|e| matches!(e.status, FileStatus::PermissionDenied { .. }))
                .collect();

            if !error_entries.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Errors", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Type", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Error", &header_format)?;
                details_row += 1;

                let error_text_format = Format::new()
                    .set_font_color(Color::RGB(0xCC0000))
                    .set_align(FormatAlign::Left);

                for entry in error_entries {
                    if let FileStatus::PermissionDenied { error: message } = &entry.status {
                        let (dir, file) = split_path(&entry.relative_path);
                        details_sheet.write_with_format(details_row, 0, "Error", &cell_format)?;
                        details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                        details_sheet.write_with_format(details_row, 2, &file, &error_text_format)?;
                        details_sheet.write_with_format(details_row, 3, message, &cell_format)?;
                        details_row += 1;
                    }
                }
                details_row += 1;
            }
        }

        // Patch details if available (always show, not affected by filter)
        if let Some(patch_result) = &options.patch_result {
            if !patch_result.patches.is_empty() || !patch_result.errors.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Patch Details", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Status", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Notes", &header_format)?;
                details_row += 1;

                for patch in &patch_result.patches {
                    let status = if patch.is_binary { "Skipped" } else { "Generated" };
                    let notes = if patch.is_binary { "Binary file" } else { "" };
                    let status_format = if patch.is_binary { &deleted_format } else { &added_format };
                    let (dir, file) = split_path(&patch.relative_path);
                    details_sheet.write_with_format(details_row, 0, status, &cell_format)?;
                    details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                    details_sheet.write_with_format(details_row, 2, &file, status_format)?;
                    details_sheet.write_with_format(details_row, 3, notes, &cell_format)?;
                    details_row += 1;
                }

                // Patch errors
                let patch_error_format = Format::new()
                    .set_font_color(Color::RGB(0xCC0000))
                    .set_align(FormatAlign::Left);

                for err in &patch_result.errors {
                    let (dir, file) = split_path(&err.relative_path);
                    details_sheet.write_with_format(details_row, 0, "Failed", &deleted_format)?;
                    details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                    details_sheet.write_with_format(details_row, 2, &file, &patch_error_format)?;
                    details_sheet.write_with_format(details_row, 3, &err.error, &cell_format)?;
                    details_row += 1;
                }
            }
        }

        // Copy failed (always show, not affected by filter)
        if let Some(copy_result) = &options.copy_result {
            if !copy_result.errors.is_empty() {
                details_sheet.merge_range(details_row, 0, details_row, 3, "Copy Failed", &section_header_format)?;
                details_row += 1;

                details_sheet.write_with_format(details_row, 0, "Status", &header_format)?;
                details_sheet.write_with_format(details_row, 1, "Directory", &header_format)?;
                details_sheet.write_with_format(details_row, 2, "File", &header_format)?;
                details_sheet.write_with_format(details_row, 3, "Error", &header_format)?;
                details_row += 1;

                let error_format = Format::new()
                    .set_font_color(Color::RGB(0xCC0000))
                    .set_align(FormatAlign::Left);

                for err in &copy_result.errors {
                    let (dir, file) = split_path(&err.relative_path);
                    details_sheet.write_with_format(details_row, 0, "Failed", &deleted_format)?;
                    details_sheet.write_with_format(details_row, 1, &dir, &cell_format)?;
                    details_sheet.write_with_format(details_row, 2, &file, &error_format)?;
                    details_sheet.write_with_format(details_row, 3, &err.error, &cell_format)?;
                    details_row += 1;
                }
            }
        }
    }

    workbook.save(excel_path)?;

    Ok(())
}

/// Write file tree to Excel sheet with indentation using cells
fn write_excel_tree(
    entries: &[DiffEntry],
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: &mut u32,
    tree_format: &Format,
    added_format: &Format,
    modified_format: &Format,
    deleted_format: &Format,
    symlink_format: &Format,
    unchanged_format: &Format,
    fold_level: Option<u16>,
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    // Build tree structure
    let mut tree: BTreeMap<PathBuf, Vec<&DiffEntry>> = BTreeMap::new();
    for entry in entries {
        let parent = entry.relative_path.parent().unwrap_or(Path::new("")).to_path_buf();
        tree.entry(parent).or_default().push(entry);
    }

    // Get all unique directory paths
    let mut all_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in entries {
        let mut current = entry.relative_path.parent();
        while let Some(dir) = current {
            if !dir.as_os_str().is_empty() {
                all_dirs.insert(dir.to_path_buf());
            }
            current = dir.parent();
        }
    }

    // Write root
    sheet.write_with_format(*row, 0, ".", tree_format)?;
    *row += 1;

    // Get root level items
    let root_entries = tree.get(&PathBuf::new()).cloned().unwrap_or_default();
    let root_dirs: Vec<_> = all_dirs.iter()
        .filter(|d| d.parent().is_none() || d.parent() == Some(Path::new("")))
        .collect();

    let mut root_items: Vec<(PathBuf, Option<&DiffEntry>)> = Vec::new();
    for dir in &root_dirs {
        root_items.push(((*dir).clone(), None));
    }
    for entry in &root_entries {
        if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
            root_items.push((entry.relative_path.clone(), Some(entry)));
        }
    }
    root_items.sort_by(|a, b| a.0.cmp(&b.0));

    write_excel_tree_items(entries, &all_dirs, &root_items, sheet, row, 0, tree_format, added_format, modified_format, deleted_format, symlink_format, unchanged_format, fold_level)?;

    Ok(())
}

/// Write tree items recursively
fn write_excel_tree_items(
    entries: &[DiffEntry],
    all_dirs: &BTreeSet<PathBuf>,
    items: &[(PathBuf, Option<&DiffEntry>)],
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: &mut u32,
    depth: u16,
    tree_format: &Format,
    added_format: &Format,
    modified_format: &Format,
    deleted_format: &Format,
    symlink_format: &Format,
    unchanged_format: &Format,
    fold_level: Option<u16>,
) -> Result<()> {
    for (i, (path, entry_opt)) in items.iter().enumerate() {
        let is_last = i == items.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };

        // Column for prefix is at depth position, name is at depth + 1
        let prefix_col = depth;
        let name_col = depth + 1;

        // Write tree connector
        sheet.write_with_format(*row, prefix_col, prefix, tree_format)?;

        // Get the name and status
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        if let Some(entry) = entry_opt {
            // It's a file
            let (display_name, status_format) = get_entry_display(entry, &name, added_format, modified_format, deleted_format, symlink_format, unchanged_format);
            sheet.write_with_format(*row, name_col, &display_name, status_format)?;
            sheet.write_with_format(*row, 11, get_status_string(&entry.status), status_format)?;
        } else {
            // It's a directory
            let dir_entry = entries.iter().find(|e| e.relative_path == *path && e.is_dir);
            let dir_name = format!("{}/", name);
            if let Some(entry) = dir_entry {
                let status_format = match &entry.status {
                    FileStatus::Added => added_format,
                    FileStatus::Deleted => deleted_format,
                    _ => tree_format,
                };
                sheet.write_with_format(*row, name_col, &dir_name, status_format)?;
                sheet.write_with_format(*row, 11, get_status_string(&entry.status), status_format)?;
            } else {
                sheet.write_with_format(*row, name_col, &dir_name, tree_format)?;
            }

            *row += 1;

            // Write children
            let mut child_items: Vec<(PathBuf, Option<&DiffEntry>)> = Vec::new();
            for dir in all_dirs {
                if dir.parent() == Some(path.as_path()) {
                    child_items.push((dir.clone(), None));
                }
            }
            for entry in entries {
                if entry.relative_path.parent() == Some(path.as_path()) {
                    if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
                        child_items.push((entry.relative_path.clone(), Some(entry)));
                    }
                }
            }
            child_items.sort_by(|a, b| a.0.cmp(&b.0));

            if !child_items.is_empty() {
                // Record start row for children
                let children_start_row = *row;

                write_excel_tree_items(entries, all_dirs, &child_items, sheet, row, depth + 1, tree_format, added_format, modified_format, deleted_format, symlink_format, unchanged_format, fold_level)?;

                // Apply row grouping if fold_level is specified and depth >= fold_level
                // This groups the children of this directory
                if let Some(level) = fold_level {
                    if depth + 1 >= level && *row > children_start_row {
                        // Group and collapse the children rows
                        sheet.group_rows_collapsed(children_start_row, *row - 1)?;
                    }
                }
            }
            continue;
        }

        *row += 1;
    }

    Ok(())
}

/// Get display name and format for entry
fn get_entry_display<'a>(
    entry: &DiffEntry,
    name: &str,
    added_format: &'a Format,
    modified_format: &'a Format,
    deleted_format: &'a Format,
    symlink_format: &'a Format,
    unchanged_format: &'a Format,
) -> (String, &'a Format) {
    match &entry.status {
        FileStatus::Added => (name.to_string(), added_format),
        FileStatus::Modified => (name.to_string(), modified_format),
        FileStatus::Deleted => (name.to_string(), deleted_format),
        FileStatus::Unchanged => (name.to_string(), unchanged_format),
        FileStatus::Symlink { current, previous, .. } => {
            let target = match (current, previous) {
                (Some(info), _) => format!(" -> {}", info.target.display()),
                (None, Some(info)) => format!(" -> {}", info.target.display()),
                _ => String::new(),
            };
            (format!("{}{}", name, target), symlink_format)
        }
        FileStatus::SpecialFile { .. } => (name.to_string(), unchanged_format),
        FileStatus::PermissionDenied { .. } => (name.to_string(), deleted_format),
    }
}

/// Get status string for display
fn get_status_string(status: &FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "[added]",
        FileStatus::Modified => "[modified]",
        FileStatus::Deleted => "[deleted]",
        FileStatus::Unchanged => "[unchanged]",
        FileStatus::Symlink { change_type, current, .. } => {
            match change_type {
                SymlinkChangeType::Added => {
                    if current.as_ref().map(|i| !i.exists).unwrap_or(false) {
                        "[symlink: added, broken]"
                    } else {
                        "[symlink: added]"
                    }
                }
                SymlinkChangeType::Deleted => "[symlink: deleted]",
                SymlinkChangeType::Changed => "[symlink: changed]",
            }
        }
        FileStatus::SpecialFile { .. } => "[special]",
        FileStatus::PermissionDenied { .. } => "[permission denied]",
    }
}

fn generate_tree(entries: &[DiffEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str(".\n");

    // Build tree structure
    let mut tree: BTreeMap<PathBuf, Vec<&DiffEntry>> = BTreeMap::new();

    for entry in entries {
        let parent = entry.relative_path.parent().unwrap_or(Path::new("")).to_path_buf();
        tree.entry(parent).or_default().push(entry);
    }

    // Get all unique directory paths
    let mut all_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in entries {
        let mut current = entry.relative_path.parent();
        while let Some(dir) = current {
            if !dir.as_os_str().is_empty() {
                all_dirs.insert(dir.to_path_buf());
            }
            current = dir.parent();
        }
    }

    // Generate tree output
    let root_entries = tree.get(&PathBuf::new()).cloned().unwrap_or_default();
    let root_dirs: Vec<_> = all_dirs
        .iter()
        .filter(|d| d.parent().is_none() || d.parent() == Some(Path::new("")))
        .collect();

    // Combine directories and files at root level
    let mut root_items: Vec<(PathBuf, Option<&DiffEntry>)> = Vec::new();

    for dir in &root_dirs {
        root_items.push(((*dir).clone(), None));
    }
    for entry in &root_entries {
        if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
            root_items.push((entry.relative_path.clone(), Some(entry)));
        }
    }
    root_items.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (path, entry_opt)) in root_items.iter().enumerate() {
        let is_last = i == root_items.len() - 1;
        let prefix = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        if let Some(entry) = entry_opt {
            output.push_str(&format!("{}{}\n", prefix, format_entry(entry)));
        } else {
            // It's a directory
            let dir_entry = entries.iter().find(|e| e.relative_path == *path && e.is_dir);
            let tag = dir_entry.map(|e| format_status_tag(&e.status)).unwrap_or_default();
            output.push_str(&format!("{}{}/{}\n", prefix, path.display(), tag));
            output.push_str(&generate_subtree(entries, &all_dirs, path, child_prefix));
        }
    }

    output
}

fn generate_subtree(
    entries: &[DiffEntry],
    all_dirs: &BTreeSet<PathBuf>,
    parent: &Path,
    prefix: &str,
) -> String {
    let mut output = String::new();

    // Get items in this directory
    let mut items: Vec<(PathBuf, Option<&DiffEntry>)> = Vec::new();

    // Subdirectories
    for dir in all_dirs {
        if dir.parent() == Some(parent) {
            items.push((dir.clone(), None));
        }
    }

    // Files in this directory
    for entry in entries {
        if entry.relative_path.parent() == Some(parent) {
            if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
                items.push((entry.relative_path.clone(), Some(entry)));
            }
        }
    }

    items.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (path, entry_opt)) in items.iter().enumerate() {
        let is_last = i == items.len() - 1;
        let line_prefix = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        if let Some(entry) = entry_opt {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            output.push_str(&format!("{}{}{}\n", prefix, line_prefix, format_entry_with_name(&name, entry)));
        } else {
            // Directory
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let dir_entry = entries.iter().find(|e| e.relative_path == *path && e.is_dir);
            let tag = dir_entry.map(|e| format_status_tag(&e.status)).unwrap_or_default();
            output.push_str(&format!("{}{}{}/{}\n", prefix, line_prefix, name, tag));
            output.push_str(&generate_subtree(entries, all_dirs, path, &child_prefix));
        }
    }

    output
}

fn format_entry(entry: &DiffEntry) -> String {
    let name = entry.relative_path.file_name().unwrap_or_default().to_string_lossy();
    format_entry_with_name(&name, entry)
}

fn format_entry_with_name(name: &str, entry: &DiffEntry) -> String {
    match &entry.status {
        FileStatus::Symlink { change_type, current, previous } => {
            let target_display = match (current, previous) {
                (Some(info), _) => format!(" -> {}", info.target.display()),
                (None, Some(info)) => format!(" -> {}", info.target.display()),
                (None, None) => String::new(),
            };

            let change_tag = match change_type {
                SymlinkChangeType::Added => {
                    let broken = match current {
                        Some(info) if !info.exists => ", broken",
                        _ => "",
                    };
                    format!("added{}", broken)
                }
                SymlinkChangeType::Deleted => "deleted".to_string(),
                SymlinkChangeType::Changed => {
                    // Check if it's only a broken status change
                    let curr_broken = current.as_ref().map(|i| !i.exists).unwrap_or(false);
                    let prev_broken = previous.as_ref().map(|i| !i.exists).unwrap_or(false);
                    let same_target = match (current, previous) {
                        (Some(c), Some(p)) => c.target == p.target,
                        _ => false,
                    };

                    if same_target && !prev_broken && curr_broken {
                        "broken".to_string()
                    } else if curr_broken {
                        "changed, broken".to_string()
                    } else {
                        "changed".to_string()
                    }
                }
            };

            format!("{}{} [symlink: {}]", name, target_display, change_tag)
        }
        _ => {
            let suffix = if entry.is_dir { "/" } else { "" };
            let tag = format_status_tag(&entry.status);
            format!("{}{} {}", name, suffix, tag)
        }
    }
}

fn format_status_tag(status: &FileStatus) -> String {
    match status {
        FileStatus::Added => "[added]".to_string(),
        FileStatus::Modified => "[modified]".to_string(),
        FileStatus::Deleted => "[deleted]".to_string(),
        FileStatus::Unchanged => "[unchanged]".to_string(),
        FileStatus::Symlink { change_type, current, previous } => {
            match change_type {
                SymlinkChangeType::Added => {
                    let broken = match current {
                        Some(info) if !info.exists => ", broken",
                        _ => "",
                    };
                    format!("[symlink: added{}]", broken)
                }
                SymlinkChangeType::Deleted => "[symlink: deleted]".to_string(),
                SymlinkChangeType::Changed => {
                    // Check if it's a broken status change
                    let curr_broken = current.as_ref().map(|i| !i.exists).unwrap_or(false);
                    let prev_broken = previous.as_ref().map(|i| !i.exists).unwrap_or(false);
                    let same_target = match (current, previous) {
                        (Some(c), Some(p)) => c.target == p.target,
                        _ => false,
                    };

                    if same_target && !prev_broken && curr_broken {
                        // Same target but became broken
                        "[symlink: broken]".to_string()
                    } else if curr_broken {
                        "[symlink: changed, broken]".to_string()
                    } else {
                        "[symlink: changed]".to_string()
                    }
                }
            }
        }
        FileStatus::SpecialFile { file_type } => format!("[special: {}]", file_type),
        FileStatus::PermissionDenied { .. } => "[permission denied]".to_string(),
    }
}

/// Calculate display width considering full-width characters
/// Full-width characters (Japanese, ○, ● etc.) count as 2, ASCII as 1
fn display_width(s: &str) -> usize {
    s.chars().map(|c| {
        if c.is_ascii() {
            1
        } else {
            2 // Japanese, ○, ●, etc.
        }
    }).sum()
}

/// Pad string to target display width
fn pad_to_width(s: &str, target_width: usize) -> String {
    let current_width = display_width(s);
    if current_width >= target_width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(target_width - current_width))
    }
}

/// Format three-way status indicators compact (for console)
fn format_three_way_indicators_compact(status: &ThreeWayStatus) -> String {
    format!("[{}{}{}]",
        status.base_indicator(),
        status.ours_indicator(),
        status.theirs_indicator()
    )
}

/// Format three-way status label
fn format_three_way_status_label(status: &ThreeWayStatus) -> String {
    if status.is_conflict() {
        "CONFLICT".to_string()
    } else {
        status.display_str().to_string()
    }
}

/// Generate three-way tree output for console (compact format)
fn generate_three_way_tree_console(entries: &[&ThreeWayEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str("Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted\n");
    output.push_str(".\n");

    // Build tree structure
    let mut tree: BTreeMap<PathBuf, Vec<&ThreeWayEntry>> = BTreeMap::new();
    for entry in entries {
        let parent = entry.relative_path.parent().unwrap_or(Path::new("")).to_path_buf();
        tree.entry(parent).or_default().push(entry);
    }

    // Get all unique directory paths
    let mut all_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in entries {
        let mut current = entry.relative_path.parent();
        while let Some(dir) = current {
            if !dir.as_os_str().is_empty() {
                all_dirs.insert(dir.to_path_buf());
            }
            current = dir.parent();
        }
    }

    // Generate tree output
    let root_entries = tree.get(&PathBuf::new()).cloned().unwrap_or_default();
    let root_dirs: Vec<_> = all_dirs
        .iter()
        .filter(|d| d.parent().is_none() || d.parent() == Some(Path::new("")))
        .collect();

    // Combine directories and files at root level
    let mut root_items: Vec<(PathBuf, Option<&ThreeWayEntry>)> = Vec::new();
    for dir in &root_dirs {
        root_items.push(((*dir).clone(), None));
    }
    for entry in &root_entries {
        if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
            root_items.push((entry.relative_path.clone(), Some(entry)));
        }
    }
    root_items.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (path, entry_opt)) in root_items.iter().enumerate() {
        let is_last = i == root_items.len() - 1;
        let prefix = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        if let Some(entry) = entry_opt {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let indicators = format_three_way_indicators_compact(&entry.status);
            let label = format_three_way_status_label(&entry.status);
            output.push_str(&format!("{}{} {} {}\n", prefix, name, indicators, label));
        } else {
            // It's a directory
            output.push_str(&format!("{}{}/\n", prefix, path.display()));
            output.push_str(&generate_three_way_subtree_console(entries, &all_dirs, path, child_prefix));
        }
    }

    output
}

fn generate_three_way_subtree_console(
    entries: &[&ThreeWayEntry],
    all_dirs: &BTreeSet<PathBuf>,
    parent: &Path,
    prefix: &str,
) -> String {
    let mut output = String::new();

    // Get items in this directory
    let mut items: Vec<(PathBuf, Option<&ThreeWayEntry>)> = Vec::new();

    // Subdirectories
    for dir in all_dirs {
        if dir.parent() == Some(parent) {
            items.push((dir.clone(), None));
        }
    }

    // Files in this directory
    for entry in entries {
        if entry.relative_path.parent() == Some(parent) {
            if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
                items.push((entry.relative_path.clone(), Some(entry)));
            }
        }
    }

    items.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (path, entry_opt)) in items.iter().enumerate() {
        let is_last = i == items.len() - 1;
        let line_prefix = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        if let Some(entry) = entry_opt {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let indicators = format_three_way_indicators_compact(&entry.status);
            let label = format_three_way_status_label(&entry.status);
            output.push_str(&format!("{}{}{} {} {}\n", prefix, line_prefix, name, indicators, label));
        } else {
            // Directory
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            output.push_str(&format!("{}{}{}/\n", prefix, line_prefix, name));
            output.push_str(&generate_three_way_subtree_console(entries, all_dirs, path, &child_prefix));
        }
    }

    output
}

/// Generate three-way tree output for file (aligned format)
fn generate_three_way_tree_file(entries: &[&ThreeWayEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    // First pass: calculate max width needed
    let mut max_width: usize = 0;

    // Build tree structure and calculate widths
    let mut tree: BTreeMap<PathBuf, Vec<&ThreeWayEntry>> = BTreeMap::new();
    for entry in entries {
        let parent = entry.relative_path.parent().unwrap_or(Path::new("")).to_path_buf();
        tree.entry(parent).or_default().push(entry);
    }

    let mut all_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in entries {
        let mut current = entry.relative_path.parent();
        while let Some(dir) = current {
            if !dir.as_os_str().is_empty() {
                all_dirs.insert(dir.to_path_buf());
            }
            current = dir.parent();
        }
    }

    // Calculate max width by simulating the tree generation
    fn calc_width_recursive(
        entries: &[&ThreeWayEntry],
        all_dirs: &BTreeSet<PathBuf>,
        parent: &Path,
        depth: usize,
        max_width: &mut usize,
    ) {
        let indent_width = depth * 4; // "│   " or "    " = 4 chars
        let prefix_width = 4; // "├── " or "└── " = 4 chars

        // Subdirectories
        for dir in all_dirs {
            if dir.parent() == Some(parent) {
                let name = dir.file_name().unwrap_or_default().to_string_lossy();
                let line_width = indent_width + prefix_width + display_width(&name) + 1; // +1 for "/"
                if line_width > *max_width {
                    *max_width = line_width;
                }
                calc_width_recursive(entries, all_dirs, dir, depth + 1, max_width);
            }
        }

        // Files
        for entry in entries {
            if entry.relative_path.parent() == Some(parent) {
                if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
                    let name = entry.relative_path.file_name().unwrap_or_default().to_string_lossy();
                    let line_width = indent_width + prefix_width + display_width(&name);
                    if line_width > *max_width {
                        *max_width = line_width;
                    }
                }
            }
        }
    }

    // Calculate width for root level
    let root_entries = tree.get(&PathBuf::new()).cloned().unwrap_or_default();
    let root_dirs: Vec<_> = all_dirs
        .iter()
        .filter(|d| d.parent().is_none() || d.parent() == Some(Path::new("")))
        .collect();

    for dir in &root_dirs {
        let name = dir.file_name().unwrap_or_default().to_string_lossy();
        let line_width = 4 + display_width(&name) + 1; // prefix + name + "/"
        if line_width > max_width {
            max_width = line_width;
        }
        calc_width_recursive(entries, &all_dirs, dir, 1, &mut max_width);
    }

    for entry in &root_entries {
        if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
            let name = entry.relative_path.file_name().unwrap_or_default().to_string_lossy();
            let line_width = 4 + display_width(&name);
            if line_width > max_width {
                max_width = line_width;
            }
        }
    }

    // Ensure minimum width for header
    let header_text = "Legend: [Base|Ours|Theirs]";
    max_width = max_width.max(display_width(header_text) + 10);

    // Add padding for alignment
    max_width += 2;

    // Now generate the output with alignment
    let mut output = String::new();
    output.push_str("Legend: [Base|Ours|Theirs] ○=exists -=missing ==same M=modified A=added D=deleted\n");

    // Header line with column labels
    let header_padding = " ".repeat(max_width.saturating_sub(1));
    output.push_str(&format!("{}B  O  T\n", header_padding));
    output.push_str(".\n");

    // Generate tree with alignment
    let mut root_items: Vec<(PathBuf, Option<&ThreeWayEntry>)> = Vec::new();
    for dir in &root_dirs {
        root_items.push(((*dir).clone(), None));
    }
    for entry in &root_entries {
        if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
            root_items.push((entry.relative_path.clone(), Some(entry)));
        }
    }
    root_items.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (path, entry_opt)) in root_items.iter().enumerate() {
        let is_last = i == root_items.len() - 1;
        let prefix = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        if let Some(entry) = entry_opt {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let tree_part = format!("{}{}", prefix, name);
            let padded = pad_to_width(&tree_part, max_width);
            let label = format_three_way_status_label(&entry.status);
            output.push_str(&format!("{}[{}  {}  {}] {}\n",
                padded,
                entry.status.base_indicator(),
                entry.status.ours_indicator(),
                entry.status.theirs_indicator(),
                label
            ));
        } else {
            // Directory
            output.push_str(&format!("{}{}/\n", prefix, path.display()));
            output.push_str(&generate_three_way_subtree_file(entries, &all_dirs, path, child_prefix, max_width));
        }
    }

    output
}

fn generate_three_way_subtree_file(
    entries: &[&ThreeWayEntry],
    all_dirs: &BTreeSet<PathBuf>,
    parent: &Path,
    prefix: &str,
    max_width: usize,
) -> String {
    let mut output = String::new();

    let mut items: Vec<(PathBuf, Option<&ThreeWayEntry>)> = Vec::new();

    for dir in all_dirs {
        if dir.parent() == Some(parent) {
            items.push((dir.clone(), None));
        }
    }

    for entry in entries {
        if entry.relative_path.parent() == Some(parent) {
            if !entry.is_dir || !all_dirs.contains(&entry.relative_path) {
                items.push((entry.relative_path.clone(), Some(entry)));
            }
        }
    }

    items.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (path, entry_opt)) in items.iter().enumerate() {
        let is_last = i == items.len() - 1;
        let line_prefix = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        if let Some(entry) = entry_opt {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let tree_part = format!("{}{}{}", prefix, line_prefix, name);
            let padded = pad_to_width(&tree_part, max_width);
            let label = format_three_way_status_label(&entry.status);
            output.push_str(&format!("{}[{}  {}  {}] {}\n",
                padded,
                entry.status.base_indicator(),
                entry.status.ours_indicator(),
                entry.status.theirs_indicator(),
                label
            ));
        } else {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            output.push_str(&format!("{}{}{}/\n", prefix, line_prefix, name));
            output.push_str(&generate_three_way_subtree_file(entries, all_dirs, path, &child_prefix, max_width));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_temp_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    fn create_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    // ==================== compute_file_hash tests ====================

    #[test]
    fn test_compute_file_hash_same_content() {
        let temp = create_temp_dir();
        let file1 = create_file(temp.path(), "file1.txt", "Hello, World!");
        let file2 = create_file(temp.path(), "file2.txt", "Hello, World!");

        let hash1 = compute_file_hash(&file1).unwrap();
        let hash2 = compute_file_hash(&file2).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_file_hash_different_content() {
        let temp = create_temp_dir();
        let file1 = create_file(temp.path(), "file1.txt", "Hello, World!");
        let file2 = create_file(temp.path(), "file2.txt", "Goodbye, World!");

        let hash1 = compute_file_hash(&file1).unwrap();
        let hash2 = compute_file_hash(&file2).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_file_hash_empty_file() {
        let temp = create_temp_dir();
        let file1 = create_file(temp.path(), "empty1.txt", "");
        let file2 = create_file(temp.path(), "empty2.txt", "");

        let hash1 = compute_file_hash(&file1).unwrap();
        let hash2 = compute_file_hash(&file2).unwrap();

        assert_eq!(hash1, hash2);
    }

    // ==================== files_differ tests ====================

    #[test]
    fn test_files_differ_same_content() {
        let temp = create_temp_dir();
        let file1 = create_file(temp.path(), "file1.txt", "Same content");
        let file2 = create_file(temp.path(), "file2.txt", "Same content");

        assert!(!files_differ(&file1, &file2).unwrap());
    }

    #[test]
    fn test_files_differ_different_content() {
        let temp = create_temp_dir();
        let file1 = create_file(temp.path(), "file1.txt", "Content A");
        let file2 = create_file(temp.path(), "file2.txt", "Content B");

        assert!(files_differ(&file1, &file2).unwrap());
    }

    #[test]
    fn test_files_differ_different_size() {
        let temp = create_temp_dir();
        let file1 = create_file(temp.path(), "file1.txt", "Short");
        let file2 = create_file(temp.path(), "file2.txt", "Much longer content");

        assert!(files_differ(&file1, &file2).unwrap());
    }

    #[test]
    fn test_files_differ_binary_content() {
        let temp = create_temp_dir();
        let path1 = temp.path().join("binary1.bin");
        let path2 = temp.path().join("binary2.bin");

        let binary_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        fs::write(&path1, &binary_data).unwrap();
        fs::write(&path2, &binary_data).unwrap();

        assert!(!files_differ(&path1, &path2).unwrap());
    }

    // ==================== is_excluded tests ====================

    #[test]
    fn test_is_excluded_match() {
        let patterns = vec![Pattern::new("*.log").unwrap()];
        assert!(is_excluded(Path::new("debug.log"), &patterns));
        assert!(is_excluded(Path::new("error.log"), &patterns));
    }

    #[test]
    fn test_is_excluded_no_match() {
        let patterns = vec![Pattern::new("*.log").unwrap()];
        assert!(!is_excluded(Path::new("main.rs"), &patterns));
        assert!(!is_excluded(Path::new("config.toml"), &patterns));
    }

    #[test]
    fn test_is_excluded_multiple_patterns() {
        let patterns = vec![
            Pattern::new("*.log").unwrap(),
            Pattern::new("*.tmp").unwrap(),
            Pattern::new("node_modules").unwrap(),
        ];
        assert!(is_excluded(Path::new("debug.log"), &patterns));
        assert!(is_excluded(Path::new("cache.tmp"), &patterns));
        assert!(is_excluded(Path::new("node_modules"), &patterns));
        assert!(!is_excluded(Path::new("main.rs"), &patterns));
    }

    #[test]
    fn test_is_excluded_path_component() {
        // Test that patterns match path components, not just full paths
        let patterns = vec![
            Pattern::new("__pycache__").unwrap(),
            Pattern::new("node_modules").unwrap(),
            Pattern::new(".git").unwrap(),
        ];
        // Should match as path component
        assert!(is_excluded(Path::new("src/__pycache__"), &patterns));
        assert!(is_excluded(Path::new("src/__pycache__/module.pyc"), &patterns));
        assert!(is_excluded(Path::new("project/node_modules/package"), &patterns));
        assert!(is_excluded(Path::new(".git/config"), &patterns));
        // Should match exact path
        assert!(is_excluded(Path::new("__pycache__"), &patterns));
        assert!(is_excluded(Path::new("node_modules"), &patterns));
        // Should not match partial names
        assert!(!is_excluded(Path::new("my__pycache__dir"), &patterns));
        assert!(!is_excluded(Path::new("not_node_modules"), &patterns));
    }

    #[test]
    fn test_is_excluded_empty_patterns() {
        let patterns: Vec<Pattern> = vec![];
        assert!(!is_excluded(Path::new("any_file.txt"), &patterns));
    }

    // ==================== is_dangerous_path tests ====================

    #[test]
    #[cfg(unix)]
    fn test_is_dangerous_path_unix_system_dirs() {
        // System directories should be dangerous
        assert!(is_dangerous_path(Path::new("/")).is_some());
        assert!(is_dangerous_path(Path::new("/usr")).is_some());
        assert!(is_dangerous_path(Path::new("/lib")).is_some());
        assert!(is_dangerous_path(Path::new("/etc")).is_some());
        assert!(is_dangerous_path(Path::new("/var")).is_some());
        assert!(is_dangerous_path(Path::new("/home")).is_some());
        assert!(is_dangerous_path(Path::new("/bin")).is_some());
        assert!(is_dangerous_path(Path::new("/tmp")).is_some());
    }

    #[test]
    #[cfg(unix)]
    fn test_is_dangerous_path_unix_user_home() {
        // User home directory should be dangerous
        assert!(is_dangerous_path(Path::new("/home/user")).is_some());
        assert!(is_dangerous_path(Path::new("/home/testuser")).is_some());
        // But subdirectories of home should be safe
        assert!(is_dangerous_path(Path::new("/home/user/projects")).is_none());
        assert!(is_dangerous_path(Path::new("/home/user/Documents")).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_is_dangerous_path_unix_safe_paths() {
        // Regular paths should be safe
        assert!(is_dangerous_path(Path::new("/home/user/projects/myapp")).is_none());
        assert!(is_dangerous_path(Path::new("/tmp/test_output")).is_none());
        assert!(is_dangerous_path(Path::new("/var/tmp/mydata")).is_none());
    }

    #[test]
    #[cfg(windows)]
    fn test_is_dangerous_path_windows_drive_roots() {
        // Drive roots should be dangerous
        assert!(is_dangerous_path(Path::new("C:\\")).is_some());
        assert!(is_dangerous_path(Path::new("D:\\")).is_some());
    }

    #[test]
    #[cfg(windows)]
    fn test_is_dangerous_path_windows_system_dirs() {
        // System directories should be dangerous
        assert!(is_dangerous_path(Path::new("C:\\Windows")).is_some());
        assert!(is_dangerous_path(Path::new("C:\\Program Files")).is_some());
        assert!(is_dangerous_path(Path::new("C:\\Program Files (x86)")).is_some());
    }

    #[test]
    #[cfg(windows)]
    fn test_is_dangerous_path_windows_user_dirs() {
        // User directories should be dangerous
        assert!(is_dangerous_path(Path::new("C:\\Users\\TestUser")).is_some());
        assert!(is_dangerous_path(Path::new("C:\\Users\\TestUser\\Desktop")).is_some());
        assert!(is_dangerous_path(Path::new("C:\\Users\\TestUser\\Documents")).is_some());
        // But deeper subdirectories should be safe
        assert!(is_dangerous_path(Path::new("C:\\Users\\TestUser\\Documents\\Projects")).is_none());
    }

    #[test]
    #[cfg(windows)]
    fn test_is_dangerous_path_windows_safe_paths() {
        // Regular paths should be safe
        assert!(is_dangerous_path(Path::new("C:\\Users\\TestUser\\Documents\\Projects\\myapp")).is_none());
        assert!(is_dangerous_path(Path::new("D:\\Projects\\output")).is_none());
    }

    #[test]
    fn test_is_dangerous_path_relative_paths() {
        // Relative paths that don't exist should be safe (they'll fail canonicalization)
        assert!(is_dangerous_path(Path::new("output")).is_none());
        assert!(is_dangerous_path(Path::new("./output")).is_none());
        assert!(is_dangerous_path(Path::new("../output")).is_none());
    }

    // ==================== compare_directories tests ====================

    #[test]
    fn test_compare_directories_added_file() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(target.path(), "new_file.txt", "New content");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, false).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, FileStatus::Added);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("new_file.txt"));
    }

    #[test]
    fn test_compare_directories_deleted_file() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "old_file.txt", "Old content");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, false).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, FileStatus::Deleted);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("old_file.txt"));
    }

    #[test]
    fn test_compare_directories_modified_file() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "file.txt", "Original content");
        create_file(target.path(), "file.txt", "Modified content");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, false).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, FileStatus::Modified);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("file.txt"));
    }

    #[test]
    fn test_compare_directories_unchanged_file() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "file.txt", "Same content");
        create_file(target.path(), "file.txt", "Same content");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, false).unwrap();

        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_compare_directories_with_subdirectories() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "src/main.rs", "fn main() {}");
        create_file(target.path(), "src/main.rs", "fn main() { println!(\"Hello\"); }");
        create_file(target.path(), "src/lib.rs", "pub fn hello() {}");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, false).unwrap();

        assert_eq!(result.entries.len(), 2);

        let paths: Vec<_> = result.entries.iter().map(|e| &e.relative_path).collect();
        assert!(paths.contains(&&PathBuf::from("src/lib.rs")));
        assert!(paths.contains(&&PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_compare_directories_with_exclude() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(target.path(), "main.rs", "fn main() {}");
        create_file(target.path(), "debug.log", "log content");

        let patterns = vec![Pattern::new("*.log").unwrap()];
        let result = compare_directories(source.path(), target.path(), &patterns, false, PermissionCheckMode::None, false).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("main.rs"));
    }

    #[test]
    fn test_compare_directories_added_empty_directory() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        fs::create_dir(target.path().join("new_dir")).unwrap();

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, false).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, FileStatus::Added);
        assert!(result.entries[0].is_dir);
    }

    // ==================== DiffResult tests ====================

    #[test]
    fn test_diff_result_has_differences() {
        let result = DiffResult {
            entries: vec![DiffEntry {
                relative_path: PathBuf::from("file.txt"),
                is_dir: false,
                status: FileStatus::Added,
            }],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        assert!(result.has_differences());
    }

    #[test]
    fn test_diff_result_no_differences() {
        let result = DiffResult {
            entries: vec![],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        assert!(!result.has_differences());
    }

    #[test]
    fn test_diff_result_count_by_status() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("new1.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("new2.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("new_dir"),
                    is_dir: true,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("modified.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
                DiffEntry {
                    relative_path: PathBuf::from("deleted.txt"),
                    is_dir: false,
                    status: FileStatus::Deleted,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        let (added_files, added_dirs, modified, deleted_files, deleted_dirs, symlinks, perm_changes, errors, unchanged, special_files) =
            result.count_by_status();

        assert_eq!(added_files, 2);
        assert_eq!(added_dirs, 1);
        assert_eq!(modified, 1);
        assert_eq!(deleted_files, 1);
        assert_eq!(deleted_dirs, 0);
        assert_eq!(symlinks, 0);
        assert_eq!(perm_changes, 0);
        assert_eq!(errors, 0);
        assert_eq!(unchanged, 0);
        assert_eq!(special_files, 0);
    }

    // ==================== format_status_tag tests ====================

    #[test]
    fn test_format_status_tag() {
        assert_eq!(format_status_tag(&FileStatus::Added), "[added]");
        assert_eq!(format_status_tag(&FileStatus::Modified), "[modified]");
        assert_eq!(format_status_tag(&FileStatus::Deleted), "[deleted]");
        // Test symlink added (not broken)
        assert_eq!(
            format_status_tag(&FileStatus::Symlink {
                change_type: SymlinkChangeType::Added,
                current: Some(SymlinkInfo {
                    target: PathBuf::from("/tmp"),
                    exists: true,
                    is_dir: true
                }),
                previous: None,
            }),
            "[symlink: added]"
        );
        // Test symlink added (broken)
        assert_eq!(
            format_status_tag(&FileStatus::Symlink {
                change_type: SymlinkChangeType::Added,
                current: Some(SymlinkInfo {
                    target: PathBuf::from("/nonexistent"),
                    exists: false,
                    is_dir: false
                }),
                previous: None,
            }),
            "[symlink: added, broken]"
        );
        // Test symlink deleted
        assert_eq!(
            format_status_tag(&FileStatus::Symlink {
                change_type: SymlinkChangeType::Deleted,
                current: None,
                previous: Some(SymlinkInfo {
                    target: PathBuf::from("/tmp"),
                    exists: true,
                    is_dir: true
                }),
            }),
            "[symlink: deleted]"
        );
        // Test symlink changed
        assert_eq!(
            format_status_tag(&FileStatus::Symlink {
                change_type: SymlinkChangeType::Changed,
                current: Some(SymlinkInfo {
                    target: PathBuf::from("/new"),
                    exists: true,
                    is_dir: false
                }),
                previous: Some(SymlinkInfo {
                    target: PathBuf::from("/old"),
                    exists: true,
                    is_dir: false
                }),
            }),
            "[symlink: changed]"
        );
        assert_eq!(
            format_status_tag(&FileStatus::PermissionDenied {
                error: "Permission denied".to_string()
            }),
            "[permission denied]"
        );
    }

    // ==================== generate_summary tests ====================

    fn default_summary_options() -> SummaryOptions {
        SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        }
    }

    #[test]
    fn test_generate_summary_no_differences() {
        let result = DiffResult {
            entries: vec![],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        let summary = generate_summary(&result, &default_summary_options());

        assert!(summary.contains("No differences found."));
        assert!(summary.contains("Source: /source"));
        assert!(summary.contains("Target: /target"));
        assert!(summary.contains("Output: /output"));
    }

    #[test]
    fn test_generate_summary_with_differences() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("new_file.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("modified_file.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        let summary = generate_summary(&result, &default_summary_options());

        assert!(summary.contains("Added:"));
        assert!(summary.contains("Modified:"));
        assert!(summary.contains("File Tree"));
        assert!(summary.contains("[added]"));
        assert!(summary.contains("[modified]"));
        // Check for detailed sections
        assert!(summary.contains("Added Files"));
        assert!(summary.contains("Modified Files"));
    }

    #[test]
    fn test_generate_summary_with_deleted() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("old_dir"),
                    is_dir: true,
                    status: FileStatus::Deleted,
                },
                DiffEntry {
                    relative_path: PathBuf::from("old_file.txt"),
                    is_dir: false,
                    status: FileStatus::Deleted,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        let summary = generate_summary(&result, &default_summary_options());

        assert!(summary.contains("Deleted Files"));
        assert!(summary.contains("Directories:"));
        assert!(summary.contains("old_dir/"));
        assert!(summary.contains("Files:"));
        assert!(summary.contains("old_file.txt"));
    }

    #[test]
    fn test_generate_summary_with_options() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("file.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec!["*.log".to_string(), "__pycache__".to_string()],
            dry_run: true,
            both_versions: true,
            check_permissions: PermissionCheckMode::Scripts,
            config_file: Some(PathBuf::from("config.toml")),
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);

        assert!(summary.contains("Options:"));
        assert!(summary.contains("Dry-run"));
        assert!(summary.contains("Both versions"));
        assert!(summary.contains("Permission check: scripts"));
        assert!(summary.contains("Config file: config.toml"));
        assert!(summary.contains("Exclude patterns:"));
        assert!(summary.contains("*.log"));
        assert!(summary.contains("__pycache__"));
    }

    // ==================== copy_diff_files tests ====================

    #[test]
    fn test_copy_diff_files() {
        let source = create_temp_dir();
        let target = create_temp_dir();
        let output = create_temp_dir();

        create_file(target.path(), "new_file.txt", "New content");
        create_file(target.path(), "src/main.rs", "fn main() {}");

        let diff_result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("new_file.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("src/main.rs"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
            ],
            permission_changes: vec![],
            source_dir: source.path().to_path_buf(),
            target_dir: target.path().to_path_buf(),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        copy_diff_files(&diff_result, output.path(), false, false, false, false).unwrap();

        assert!(output.path().join("new_file.txt").exists());
        assert!(output.path().join("src/main.rs").exists());

        let content = fs::read_to_string(output.path().join("new_file.txt")).unwrap();
        assert_eq!(content, "New content");
    }

    #[test]
    fn test_copy_diff_files_creates_directories() {
        let source = create_temp_dir();
        let target = create_temp_dir();
        let output = create_temp_dir();

        fs::create_dir_all(target.path().join("deep/nested/dir")).unwrap();
        create_file(target.path(), "deep/nested/dir/file.txt", "Content");

        let diff_result = DiffResult {
            entries: vec![DiffEntry {
                relative_path: PathBuf::from("deep/nested/dir/file.txt"),
                is_dir: false,
                status: FileStatus::Added,
            }],
            permission_changes: vec![],
            source_dir: source.path().to_path_buf(),
            target_dir: target.path().to_path_buf(),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        copy_diff_files(&diff_result, output.path(), false, false, false, false).unwrap();

        assert!(output.path().join("deep/nested/dir/file.txt").exists());
    }

    #[test]
    fn test_copy_diff_files_skips_deleted() {
        let source = create_temp_dir();
        let target = create_temp_dir();
        let output = create_temp_dir();

        create_file(source.path(), "deleted_file.txt", "Old content");

        let diff_result = DiffResult {
            entries: vec![DiffEntry {
                relative_path: PathBuf::from("deleted_file.txt"),
                is_dir: false,
                status: FileStatus::Deleted,
            }],
            permission_changes: vec![],
            source_dir: source.path().to_path_buf(),
            target_dir: target.path().to_path_buf(),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        copy_diff_files(&diff_result, output.path(), false, false, false, false).unwrap();

        assert!(!output.path().join("deleted_file.txt").exists());
    }

    // ==================== both_versions tests ====================

    #[test]
    fn test_copy_diff_files_both_versions() {
        let source = create_temp_dir();
        let target = create_temp_dir();
        let output = create_temp_dir();

        create_file(source.path(), "file.txt", "Old content");
        create_file(target.path(), "file.txt", "New content");

        let diff_result = DiffResult {
            entries: vec![DiffEntry {
                relative_path: PathBuf::from("file.txt"),
                is_dir: false,
                status: FileStatus::Modified,
            }],
            permission_changes: vec![],
            source_dir: source.path().to_path_buf(),
            target_dir: target.path().to_path_buf(),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        copy_diff_files(&diff_result, output.path(), false, true, false, false).unwrap();

        assert!(output.path().join("file.txt.old").exists());
        assert!(output.path().join("file.txt.new").exists());

        let old_content = fs::read_to_string(output.path().join("file.txt.old")).unwrap();
        let new_content = fs::read_to_string(output.path().join("file.txt.new")).unwrap();

        assert_eq!(old_content, "Old content");
        assert_eq!(new_content, "New content");
    }

    #[test]
    fn test_copy_diff_files_both_versions_with_subdirectory() {
        let source = create_temp_dir();
        let target = create_temp_dir();
        let output = create_temp_dir();

        create_file(source.path(), "src/main.rs", "fn main() {}");
        create_file(target.path(), "src/main.rs", "fn main() { println!(\"Hello\"); }");

        let diff_result = DiffResult {
            entries: vec![DiffEntry {
                relative_path: PathBuf::from("src/main.rs"),
                is_dir: false,
                status: FileStatus::Modified,
            }],
            permission_changes: vec![],
            source_dir: source.path().to_path_buf(),
            target_dir: target.path().to_path_buf(),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        copy_diff_files(&diff_result, output.path(), false, true, false, false).unwrap();

        assert!(output.path().join("src/main.rs.old").exists());
        assert!(output.path().join("src/main.rs.new").exists());
    }

    #[test]
    fn test_add_extension() {
        let path = PathBuf::from("/path/to/file.txt");
        let result = add_extension(&path, "old");
        assert_eq!(result, PathBuf::from("/path/to/file.txt.old"));

        let path2 = PathBuf::from("file");
        let result2 = add_extension(&path2, "new");
        assert_eq!(result2, PathBuf::from("file.new"));
    }

    // ==================== copy_deleted tests ====================

    #[test]
    fn test_copy_diff_files_copy_deleted() {
        let source = create_temp_dir();
        let target = create_temp_dir();
        let output = create_temp_dir();

        create_file(source.path(), "deleted_file.txt", "Old content");

        let diff_result = DiffResult {
            entries: vec![DiffEntry {
                relative_path: PathBuf::from("deleted_file.txt"),
                is_dir: false,
                status: FileStatus::Deleted,
            }],
            permission_changes: vec![],
            source_dir: source.path().to_path_buf(),
            target_dir: target.path().to_path_buf(),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        // copy_deleted = true
        copy_diff_files(&diff_result, output.path(), false, false, true, false).unwrap();

        // File should be copied with .deleted extension
        assert!(output.path().join("deleted_file.txt.deleted").exists());
        let content = fs::read_to_string(output.path().join("deleted_file.txt.deleted")).unwrap();
        assert_eq!(content, "Old content");
    }

    #[test]
    fn test_copy_diff_files_copy_deleted_with_subdirectory() {
        let source = create_temp_dir();
        let target = create_temp_dir();
        let output = create_temp_dir();

        create_file(source.path(), "src/old_module.rs", "// old module");

        let diff_result = DiffResult {
            entries: vec![DiffEntry {
                relative_path: PathBuf::from("src/old_module.rs"),
                is_dir: false,
                status: FileStatus::Deleted,
            }],
            permission_changes: vec![],
            source_dir: source.path().to_path_buf(),
            target_dir: target.path().to_path_buf(),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        copy_diff_files(&diff_result, output.path(), false, false, true, false).unwrap();

        assert!(output.path().join("src/old_module.rs.deleted").exists());
    }

    // ==================== preserve_timestamps tests ====================

    #[test]
    fn test_copy_file_with_timestamp_preserves_mtime() {
        use filetime::FileTime;

        let source = create_temp_dir();
        let dest = create_temp_dir();

        let src_file = source.path().join("file.txt");
        fs::write(&src_file, "content").unwrap();

        // Set a specific timestamp
        let specific_time = FileTime::from_unix_time(1609459200, 0); // 2021-01-01 00:00:00 UTC
        filetime::set_file_mtime(&src_file, specific_time).unwrap();

        let dst_file = dest.path().join("file.txt");
        copy_file_with_timestamp(&src_file, &dst_file, true).unwrap();

        let dst_metadata = fs::metadata(&dst_file).unwrap();
        let dst_mtime = FileTime::from_last_modification_time(&dst_metadata);

        assert_eq!(dst_mtime.unix_seconds(), specific_time.unix_seconds());
    }

    #[test]
    fn test_copy_file_without_timestamp_does_not_preserve() {
        use filetime::FileTime;

        let source = create_temp_dir();
        let dest = create_temp_dir();

        let src_file = source.path().join("file.txt");
        fs::write(&src_file, "content").unwrap();

        // Set an old timestamp
        let old_time = FileTime::from_unix_time(1000000000, 0); // 2001-09-09
        filetime::set_file_mtime(&src_file, old_time).unwrap();

        let dst_file = dest.path().join("file.txt");
        copy_file_with_timestamp(&src_file, &dst_file, false).unwrap();

        let dst_metadata = fs::metadata(&dst_file).unwrap();
        let dst_mtime = FileTime::from_last_modification_time(&dst_metadata);

        // The destination file should have a recent timestamp, not the old one
        assert_ne!(dst_mtime.unix_seconds(), old_time.unix_seconds());
    }

    // ==================== should_check_permissions tests ====================

    #[test]
    fn test_should_check_permissions_none() {
        assert!(!should_check_permissions(Path::new("script.sh"), PermissionCheckMode::None));
        assert!(!should_check_permissions(Path::new("file.txt"), PermissionCheckMode::None));
    }

    #[test]
    fn test_should_check_permissions_all() {
        assert!(should_check_permissions(Path::new("script.sh"), PermissionCheckMode::All));
        assert!(should_check_permissions(Path::new("file.txt"), PermissionCheckMode::All));
        assert!(should_check_permissions(Path::new("readme.md"), PermissionCheckMode::All));
    }

    #[test]
    fn test_should_check_permissions_scripts() {
        // Script files should be checked
        assert!(should_check_permissions(Path::new("script.sh"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.bash"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.py"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.rb"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.pl"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.js"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.php"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.ps1"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.bat"), PermissionCheckMode::Scripts));

        // Non-script files should not be checked
        assert!(!should_check_permissions(Path::new("file.txt"), PermissionCheckMode::Scripts));
        assert!(!should_check_permissions(Path::new("file.rs"), PermissionCheckMode::Scripts));
        assert!(!should_check_permissions(Path::new("file.md"), PermissionCheckMode::Scripts));
        assert!(!should_check_permissions(Path::new("file.json"), PermissionCheckMode::Scripts));
    }

    #[test]
    fn test_should_check_permissions_case_insensitive() {
        assert!(should_check_permissions(Path::new("script.SH"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.PY"), PermissionCheckMode::Scripts));
        assert!(should_check_permissions(Path::new("script.Py"), PermissionCheckMode::Scripts));
    }

    // ==================== permission change detection tests ====================

    #[cfg(unix)]
    #[test]
    fn test_get_file_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp = create_temp_dir();
        let file = create_file(temp.path(), "test.sh", "#!/bin/bash\necho hello");

        // Set file to 755
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&file, perms).unwrap();

        let mode = get_file_mode(&file);
        assert_eq!(mode, Some("755".to_string()));

        // Set file to 644
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file, perms).unwrap();

        let mode = get_file_mode(&file);
        assert_eq!(mode, Some("644".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_check_permission_change_detected() {
        use std::os::unix::fs::PermissionsExt;

        let source_temp = create_temp_dir();
        let target_temp = create_temp_dir();

        let source_file = create_file(source_temp.path(), "script.sh", "#!/bin/bash");
        let target_file = create_file(target_temp.path(), "script.sh", "#!/bin/bash");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        let change = check_permission_change(&source_file, &target_file, Path::new("script.sh"));

        assert!(change.is_some());
        let change = change.unwrap();
        assert_eq!(change.old_mode, "755");
        assert_eq!(change.new_mode, "644");
    }

    #[cfg(unix)]
    #[test]
    fn test_check_permission_change_not_detected() {
        use std::os::unix::fs::PermissionsExt;

        let source_temp = create_temp_dir();
        let target_temp = create_temp_dir();

        let source_file = create_file(source_temp.path(), "script.sh", "#!/bin/bash");
        let target_file = create_file(target_temp.path(), "script.sh", "#!/bin/bash");

        // Set same permissions
        let mut perms = fs::metadata(&source_file).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&source_file, perms.clone()).unwrap();
        fs::set_permissions(&target_file, perms).unwrap();

        let change = check_permission_change(&source_file, &target_file, Path::new("script.sh"));

        assert!(change.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_compare_directories_with_permission_check() {
        use std::os::unix::fs::PermissionsExt;

        let source = create_temp_dir();
        let target = create_temp_dir();

        // Create files with same content but different permissions
        let source_file = create_file(source.path(), "script.sh", "#!/bin/bash\necho hello");
        let target_file = create_file(target.path(), "script.sh", "#!/bin/bash\necho hello");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        // With permission check enabled for scripts
        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::Scripts, false).unwrap();

        assert!(result.entries.is_empty()); // Content is same, so no file entries
        assert_eq!(result.permission_changes.len(), 1);
        assert_eq!(result.permission_changes[0].old_mode, "755");
        assert_eq!(result.permission_changes[0].new_mode, "644");
    }

    #[cfg(unix)]
    #[test]
    fn test_compare_directories_permission_check_none() {
        use std::os::unix::fs::PermissionsExt;

        let source = create_temp_dir();
        let target = create_temp_dir();

        // Create files with same content but different permissions
        let source_file = create_file(source.path(), "script.sh", "#!/bin/bash\necho hello");
        let target_file = create_file(target.path(), "script.sh", "#!/bin/bash\necho hello");

        // Set different permissions
        let mut perms_old = fs::metadata(&source_file).unwrap().permissions();
        perms_old.set_mode(0o755);
        fs::set_permissions(&source_file, perms_old).unwrap();

        let mut perms_new = fs::metadata(&target_file).unwrap().permissions();
        perms_new.set_mode(0o644);
        fs::set_permissions(&target_file, perms_new).unwrap();

        // With permission check disabled (default)
        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, false).unwrap();

        assert!(result.entries.is_empty());
        assert!(result.permission_changes.is_empty());
    }

    #[test]
    fn test_diff_result_has_differences_permission_only() {
        let result = DiffResult {
            entries: vec![],
            permission_changes: vec![PermissionChange {
                relative_path: PathBuf::from("script.sh"),
                old_mode: "755".to_string(),
                new_mode: "644".to_string(),
            }],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        assert!(result.has_differences());
    }

    // ==================== is_binary_file tests ====================

    #[test]
    fn test_is_binary_file_text() {
        let temp = create_temp_dir();
        let file = create_file(temp.path(), "text.txt", "Hello, world!\nThis is text.\n");
        assert!(!is_binary_file(&file));
    }

    #[test]
    fn test_is_binary_file_binary() {
        let temp = create_temp_dir();
        let binary_path = temp.path().join("binary.bin");
        let binary_data: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0x00, 0x89, 0x50];
        fs::write(&binary_path, &binary_data).unwrap();
        assert!(is_binary_file(&binary_path));
    }

    #[test]
    fn test_is_binary_file_empty() {
        let temp = create_temp_dir();
        let file = create_file(temp.path(), "empty.txt", "");
        assert!(!is_binary_file(&file));
    }

    #[test]
    fn test_is_binary_file_nonexistent() {
        let path = PathBuf::from("/nonexistent/file.txt");
        assert!(!is_binary_file(&path));
    }

    // ==================== generate_unified_diff tests ====================

    #[test]
    fn test_generate_unified_diff_basic() {
        let temp = create_temp_dir();
        let old_file = create_file(temp.path(), "old.txt", "line1\nline2\nline3\n");
        let new_file = create_file(temp.path(), "new.txt", "line1\nmodified line2\nline3\n");

        let diff = generate_unified_diff(&old_file, &new_file, Path::new("file.txt"));
        assert!(diff.is_some());

        let diff_content = diff.unwrap();
        assert!(diff_content.contains("--- a/file.txt"));
        assert!(diff_content.contains("+++ b/file.txt"));
        assert!(diff_content.contains("-line2"));
        assert!(diff_content.contains("+modified line2"));
    }

    #[test]
    fn test_generate_unified_diff_no_changes() {
        let temp = create_temp_dir();
        let file1 = create_file(temp.path(), "file1.txt", "same content\n");
        let file2 = create_file(temp.path(), "file2.txt", "same content\n");

        let diff = generate_unified_diff(&file1, &file2, Path::new("file.txt"));
        assert!(diff.is_none()); // No changes, so no diff
    }

    #[test]
    fn test_generate_unified_diff_new_lines() {
        let temp = create_temp_dir();
        let old_file = create_file(temp.path(), "old.txt", "line1\n");
        let new_file = create_file(temp.path(), "new.txt", "line1\nline2\nline3\n");

        let diff = generate_unified_diff(&old_file, &new_file, Path::new("file.txt"));
        assert!(diff.is_some());

        let diff_content = diff.unwrap();
        assert!(diff_content.contains("+line2"));
        assert!(diff_content.contains("+line3"));
    }

    // ==================== generate_summary with patch tests ====================

    #[test]
    fn test_generate_summary_with_patch_options() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("file.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: true,
            patch_file: Some(PathBuf::from("all.patch")),
            patch_result: Some(PatchResult {
                patches: vec![
                    PatchInfo {
                        relative_path: PathBuf::from("file.txt"),
                        is_binary: false,
                        patch_generated: true,
                    },
                ],
                errors: vec![],
                total_generated: 1,
                total_skipped: 0,
            }),
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        assert!(summary.contains("Patch mode: Individual files (.patch)"));
        assert!(summary.contains("Combined patch file: all.patch"));
        assert!(summary.contains("Patch Details"));
        assert!(summary.contains("Generated: 1 patches"));
    }

    #[test]
    fn test_generate_summary_with_skipped_binary() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("image.png"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 0,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: true,
            patch_file: None,
            patch_result: Some(PatchResult {
                patches: vec![
                    PatchInfo {
                        relative_path: PathBuf::from("image.png"),
                        is_binary: true,
                        patch_generated: false,
                    },
                ],
                errors: vec![],
                total_generated: 0,
                total_skipped: 1,
            }),
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        assert!(summary.contains("Skipped: 1 (binary)"));
        assert!(summary.contains("Skipped (binary):"));
        assert!(summary.contains("image.png [skip]"));
    }

    // ==================== show_unchanged tests ====================

    #[test]
    fn test_compare_directories_unchanged_file_with_show_unchanged() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "file.txt", "Same content");
        create_file(target.path(), "file.txt", "Same content");

        // With show_unchanged = true, unchanged files should be included
        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, true).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, FileStatus::Unchanged);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("file.txt"));
    }

    #[test]
    fn test_compare_directories_mixed_with_show_unchanged() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "same.txt", "Same content");
        create_file(target.path(), "same.txt", "Same content");
        create_file(source.path(), "modified.txt", "Old content");
        create_file(target.path(), "modified.txt", "New content");
        create_file(target.path(), "added.txt", "Added content");
        create_file(source.path(), "deleted.txt", "Deleted content");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None, true).unwrap();

        assert_eq!(result.entries.len(), 4);

        let statuses: std::collections::HashMap<_, _> = result.entries.iter()
            .map(|e| (e.relative_path.to_string_lossy().to_string(), &e.status))
            .collect();

        assert_eq!(statuses.get("same.txt"), Some(&&FileStatus::Unchanged));
        assert_eq!(statuses.get("modified.txt"), Some(&&FileStatus::Modified));
        assert_eq!(statuses.get("added.txt"), Some(&&FileStatus::Added));
        assert_eq!(statuses.get("deleted.txt"), Some(&&FileStatus::Deleted));
    }

    #[test]
    fn test_diff_result_unchanged_count() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("modified.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 5,
            target_count: 5,
            common_count: 4,
        };

        // common_count=4, modified=1, so unchanged = 4 - 1 = 3
        assert_eq!(result.unchanged_count(), 3);
    }

    #[test]
    fn test_diff_result_total_unique_paths() {
        let result = DiffResult {
            entries: vec![],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 10,
            target_count: 12,
            common_count: 8,
        };

        // total = source + target - common = 10 + 12 - 8 = 14
        assert_eq!(result.total_unique_paths(), 14);
    }

    #[test]
    fn test_diff_result_has_differences_ignores_unchanged() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("unchanged.txt"),
                    is_dir: false,
                    status: FileStatus::Unchanged,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 1,
            target_count: 1,
            common_count: 1,
        };

        // Unchanged entries should not be counted as differences
        assert!(!result.has_differences());
    }

    #[test]
    fn test_generate_summary_with_show_unchanged() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("unchanged.txt"),
                    is_dir: false,
                    status: FileStatus::Unchanged,
                },
                DiffEntry {
                    relative_path: PathBuf::from("modified.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 2,
            target_count: 2,
            common_count: 2,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: true,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        assert!(summary.contains("Show unchanged: Yes"));
        assert!(summary.contains("Unchanged Files"));
        assert!(summary.contains("unchanged.txt"));
    }

    #[test]
    fn test_generate_summary_always_shows_unchanged_count() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("modified.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 3,
            target_count: 3,
            common_count: 2,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        // Even with show_unchanged=false, the unchanged count should be displayed in statistics
        assert!(summary.contains("Unchanged:"));
    }

    // ==================== Special File Tests ====================

    #[test]
    fn test_generate_summary_with_special_files() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("test.socket"),
                    is_dir: false,
                    status: FileStatus::SpecialFile {
                        file_type: SpecialFileType::Socket,
                    },
                },
                DiffEntry {
                    relative_path: PathBuf::from("normal.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 2,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        assert!(summary.contains("Special:"), "Should show special files count in statistics");
        assert!(summary.contains("test.socket"), "Should show special file in file tree");
        assert!(summary.contains("[special: socket]"), "Should show special file status tag");
        assert!(summary.contains("Special Files (skipped)"), "Should have Special Files section");
    }

    // ==================== Filter Tests ====================

    #[test]
    fn test_generate_summary_with_filter_status() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("added.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("modified.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
                DiffEntry {
                    relative_path: PathBuf::from("deleted.txt"),
                    is_dir: false,
                    status: FileStatus::Deleted,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 2,
            target_count: 2,
            common_count: 1,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec!["added".to_string()],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        // Filter should show only added entries in tree and details
        assert!(summary.contains("added.txt"), "Should show added file");
        assert!(summary.contains("(filtered out)"), "Should mark filtered items");
        assert!(summary.contains("items (filtered)"), "Should show filtered count");
    }

    #[test]
    fn test_generate_summary_with_stats_only() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("file.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 1,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: true,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        // Should have statistics but no file tree or details
        assert!(summary.contains("rs_diffcopy Summary"), "Should have summary header");
        assert!(summary.contains("Added:"), "Should show added count");
        assert!(!summary.contains("File Tree"), "Should NOT have file tree");
        assert!(!summary.contains("Added Files"), "Should NOT have Added Files section");
    }

    #[test]
    fn test_generate_summary_with_no_tree() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("file.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 1,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: true,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        // Should have header and details but no file tree
        assert!(summary.contains("rs_diffcopy Summary"), "Should have summary header");
        assert!(!summary.contains("File Tree"), "Should NOT have file tree");
        assert!(summary.contains("Added Files"), "Should have Added Files section");
    }

    #[test]
    fn test_generate_summary_with_no_details() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("file.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 1,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: true,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        // Should have header and file tree but no details
        assert!(summary.contains("rs_diffcopy Summary"), "Should have summary header");
        assert!(summary.contains("File Tree"), "Should have file tree");
        assert!(!summary.contains("Added Files"), "Should NOT have Added Files section");
    }

    // ==================== Copy Error Tests ====================

    #[test]
    fn test_generate_summary_with_copy_errors() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("file.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 0,
            target_count: 1,
            common_count: 0,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: Some(CopyResult {
                copied_count: 0,
                errors: vec![
                    CopyError {
                        relative_path: PathBuf::from("file.txt"),
                        error: "No space left on device".to_string(),
                    },
                ],
            }),
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        assert!(summary.contains("Copy Failed"), "Should have Copy Failed section");
        assert!(summary.contains("Failed: 1 files"), "Should show failed count");
        assert!(summary.contains("No space left on device"), "Should show error message");
    }

    // ==================== Patch Error Tests ====================

    #[test]
    fn test_generate_summary_with_patch_errors() {
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("file.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 1,
            target_count: 1,
            common_count: 1,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: true,
            patch_file: None,
            patch_result: Some(PatchResult {
                patches: vec![],
                errors: vec![
                    PatchError {
                        relative_path: PathBuf::from("file.txt"),
                        error: "Failed to read source file".to_string(),
                    },
                ],
                total_generated: 0,
                total_skipped: 0,
            }),
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        assert!(summary.contains("Patch Details"), "Should have Patch Details section");
        assert!(summary.contains("Failed: 1"), "Should show failed count in patch summary");
        assert!(summary.contains("Failed to read source file"), "Should show error message");
    }

    // ==================== Config Generation Tests ====================

    #[test]
    fn test_generate_config_content_basic() {
        let config = ResolvedConfig {
            source_dir: PathBuf::from("/path/to/source"),
            target_dir: PathBuf::from("/path/to/target"),
            output_dir: PathBuf::from("/path/to/output"),
            exclude: vec![],
            force: false,
            verbose: false,
            dry_run: false,
            both_versions: false,
            summary: None,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            patch: false,
            patch_file: None,
            excel: None,
            excel_fold_level: None,
            show_unchanged: false,
            save_config: None,
            three_way: false,
            base_dir: None,
            merge_style: MergeStyle::All,
            conflict_only: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let content = config.generate_config_content();

        // Check required settings
        assert!(content.contains("source = \"/path/to/source\""));
        assert!(content.contains("target = \"/path/to/target\""));
        assert!(content.contains("output = \"/path/to/output\""));

        // Check dry_run is commented out
        assert!(content.contains("# dry_run = false"));
        assert!(content.contains("# 注: dry_run はこの設定ファイルでは無効になっています"));

        // Check header comments
        assert!(content.contains("# rs_diffcopy 設定ファイル"));
        assert!(content.contains("# このファイルは --save-config オプションにより自動生成されました"));
    }

    #[test]
    fn test_generate_config_content_with_options() {
        let config = ResolvedConfig {
            source_dir: PathBuf::from("./old"),
            target_dir: PathBuf::from("./new"),
            output_dir: PathBuf::from("./output"),
            exclude: vec!["*.log".to_string(), "node_modules".to_string()],
            force: true,
            verbose: true,
            dry_run: true,
            both_versions: true,
            summary: Some(PathBuf::from("./summary.txt")),
            check_permissions: PermissionCheckMode::Scripts,
            config_file: None,
            patch: true,
            patch_file: Some(PathBuf::from("./changes.patch")),
            excel: Some(PathBuf::from("./report.xlsx")),
            excel_fold_level: Some(2),
            show_unchanged: true,
            save_config: None,
            three_way: false,
            base_dir: None,
            merge_style: MergeStyle::All,
            conflict_only: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let content = config.generate_config_content();

        // Check options are included
        assert!(content.contains("force = true"));
        assert!(content.contains("verbose = true"));
        assert!(content.contains("both_versions = true"));
        assert!(content.contains("summary = \"./summary.txt\""));
        assert!(content.contains("check_permissions = \"scripts\""));
        assert!(content.contains("patch = true"));
        assert!(content.contains("patch_file = \"./changes.patch\""));
        assert!(content.contains("excel = \"./report.xlsx\""));
        assert!(content.contains("excel_fold_level = 2"));
        assert!(content.contains("show_unchanged = true"));

        // Check exclude patterns
        assert!(content.contains("exclude = ["));
        assert!(content.contains("\"*.log\""));
        assert!(content.contains("\"node_modules\""));

        // dry_run should still be commented out even if true
        assert!(content.contains("# dry_run = true"));
    }

    #[test]
    fn test_generate_config_content_empty_exclude() {
        let config = ResolvedConfig {
            source_dir: PathBuf::from("./old"),
            target_dir: PathBuf::from("./new"),
            output_dir: PathBuf::from("./output"),
            exclude: vec![],
            force: false,
            verbose: false,
            dry_run: false,
            both_versions: false,
            summary: None,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            patch: false,
            patch_file: None,
            excel: None,
            excel_fold_level: None,
            show_unchanged: false,
            save_config: None,
            three_way: false,
            base_dir: None,
            merge_style: MergeStyle::All,
            conflict_only: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let content = config.generate_config_content();

        // When exclude is empty, should show commented sample
        assert!(content.contains("# exclude = ["));
        assert!(content.contains("#     \"*.log\""));
    }

    #[test]
    fn test_generate_config_content_permission_modes() {
        // Test "none" mode
        let config_none = ResolvedConfig {
            source_dir: PathBuf::from("./old"),
            target_dir: PathBuf::from("./new"),
            output_dir: PathBuf::from("./output"),
            exclude: vec![],
            force: false,
            verbose: false,
            dry_run: false,
            both_versions: false,
            summary: None,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            patch: false,
            patch_file: None,
            excel: None,
            excel_fold_level: None,
            show_unchanged: false,
            save_config: None,
            three_way: false,
            base_dir: None,
            merge_style: MergeStyle::All,
            conflict_only: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            copy_deleted: false,
            preserve_timestamps: false,
        };
        assert!(config_none.generate_config_content().contains("check_permissions = \"none\""));

        // Test "all" mode
        let config_all = ResolvedConfig {
            check_permissions: PermissionCheckMode::All,
            ..config_none
        };
        assert!(config_all.generate_config_content().contains("check_permissions = \"all\""));
    }

    // ==================== Three-way Comparison Tests ====================

    #[test]
    fn test_three_way_status_is_conflict() {
        assert!(!ThreeWayStatus::Unchanged.is_conflict());
        assert!(!ThreeWayStatus::OursOnly.is_conflict());
        assert!(!ThreeWayStatus::TheirsOnly.is_conflict());
        assert!(!ThreeWayStatus::BothSame.is_conflict());
        assert!(ThreeWayStatus::Conflict.is_conflict());
        assert!(!ThreeWayStatus::AddedOurs.is_conflict());
        assert!(!ThreeWayStatus::AddedTheirs.is_conflict());
        assert!(!ThreeWayStatus::AddedBothSame.is_conflict());
        assert!(ThreeWayStatus::AddedBothDiff.is_conflict());
        assert!(!ThreeWayStatus::DeletedOurs.is_conflict());
        assert!(!ThreeWayStatus::DeletedTheirs.is_conflict());
        assert!(!ThreeWayStatus::DeletedBoth.is_conflict());
        assert!(ThreeWayStatus::ModifyDelete.is_conflict());
        assert!(ThreeWayStatus::DeleteModify.is_conflict());
    }

    #[test]
    fn test_three_way_status_display_str() {
        assert_eq!(ThreeWayStatus::Unchanged.display_str(), "unchanged");
        assert_eq!(ThreeWayStatus::OursOnly.display_str(), "ours-only");
        assert_eq!(ThreeWayStatus::TheirsOnly.display_str(), "theirs-only");
        assert_eq!(ThreeWayStatus::BothSame.display_str(), "both-same");
        assert_eq!(ThreeWayStatus::Conflict.display_str(), "CONFLICT");
        assert_eq!(ThreeWayStatus::AddedOurs.display_str(), "added-ours");
        assert_eq!(ThreeWayStatus::AddedTheirs.display_str(), "added-theirs");
        assert_eq!(ThreeWayStatus::AddedBothSame.display_str(), "added-both-same");
        assert_eq!(ThreeWayStatus::AddedBothDiff.display_str(), "CONFLICT (added-both-diff)");
        assert_eq!(ThreeWayStatus::DeletedOurs.display_str(), "deleted-ours");
        assert_eq!(ThreeWayStatus::DeletedTheirs.display_str(), "deleted-theirs");
        assert_eq!(ThreeWayStatus::DeletedBoth.display_str(), "deleted-both");
        assert_eq!(ThreeWayStatus::ModifyDelete.display_str(), "CONFLICT (modify-delete)");
        assert_eq!(ThreeWayStatus::DeleteModify.display_str(), "CONFLICT (delete-modify)");
    }

    #[test]
    fn test_three_way_status_indicators() {
        // Test base_indicator
        assert_eq!(ThreeWayStatus::Unchanged.base_indicator(), "○");
        assert_eq!(ThreeWayStatus::AddedOurs.base_indicator(), "-");
        assert_eq!(ThreeWayStatus::AddedTheirs.base_indicator(), "-");
        assert_eq!(ThreeWayStatus::AddedBothSame.base_indicator(), "-");
        assert_eq!(ThreeWayStatus::AddedBothDiff.base_indicator(), "-");
        assert_eq!(ThreeWayStatus::OursOnly.base_indicator(), "○");
        assert_eq!(ThreeWayStatus::DeletedBoth.base_indicator(), "○");

        // Test ours_indicator
        assert_eq!(ThreeWayStatus::Unchanged.ours_indicator(), "=");
        assert_eq!(ThreeWayStatus::OursOnly.ours_indicator(), "M");
        assert_eq!(ThreeWayStatus::TheirsOnly.ours_indicator(), "=");
        assert_eq!(ThreeWayStatus::AddedOurs.ours_indicator(), "A");
        assert_eq!(ThreeWayStatus::DeletedOurs.ours_indicator(), "D");
        assert_eq!(ThreeWayStatus::DeletedBoth.ours_indicator(), "D");
        assert_eq!(ThreeWayStatus::DeleteModify.ours_indicator(), "D");

        // Test theirs_indicator
        assert_eq!(ThreeWayStatus::Unchanged.theirs_indicator(), "=");
        assert_eq!(ThreeWayStatus::OursOnly.theirs_indicator(), "=");
        assert_eq!(ThreeWayStatus::TheirsOnly.theirs_indicator(), "M");
        assert_eq!(ThreeWayStatus::AddedTheirs.theirs_indicator(), "A");
        assert_eq!(ThreeWayStatus::DeletedTheirs.theirs_indicator(), "D");
        assert_eq!(ThreeWayStatus::DeletedBoth.theirs_indicator(), "D");
        assert_eq!(ThreeWayStatus::ModifyDelete.theirs_indicator(), "D");
    }

    // Helper function to create ThreeWayEntry for tests
    fn make_three_way_entry(path: &str, status: ThreeWayStatus) -> ThreeWayEntry {
        ThreeWayEntry {
            relative_path: PathBuf::from(path),
            is_dir: false,
            status,
            base_size: Some(100),
            ours_size: Some(100),
            theirs_size: Some(100),
        }
    }

    // Helper function to create ThreeWayDiffResult for tests
    fn make_three_way_result(entries: Vec<ThreeWayEntry>) -> ThreeWayDiffResult {
        let total = entries.len();
        ThreeWayDiffResult {
            entries,
            base_dir: PathBuf::from("/base"),
            ours_dir: PathBuf::from("/ours"),
            theirs_dir: PathBuf::from("/theirs"),
            total_paths: total,
        }
    }

    #[test]
    fn test_three_way_diff_result_has_differences() {
        // No entries = no differences
        let result = make_three_way_result(vec![]);
        assert!(!result.has_differences());

        // Only unchanged = no differences
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::Unchanged),
        ]);
        assert!(!result.has_differences());

        // Has ours-only = has differences
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::OursOnly),
        ]);
        assert!(result.has_differences());

        // Has conflict = has differences
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::Conflict),
        ]);
        assert!(result.has_differences());
    }

    #[test]
    fn test_three_way_diff_result_has_conflicts() {
        // No entries = no conflicts
        let result = make_three_way_result(vec![]);
        assert!(!result.has_conflicts());

        // Only unchanged = no conflicts
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::Unchanged),
        ]);
        assert!(!result.has_conflicts());

        // Has ours-only (not conflict) = no conflicts
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::OursOnly),
        ]);
        assert!(!result.has_conflicts());

        // Has Conflict = has conflicts
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::Conflict),
        ]);
        assert!(result.has_conflicts());

        // Has AddedBothDiff = has conflicts
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::AddedBothDiff),
        ]);
        assert!(result.has_conflicts());

        // Has ModifyDelete = has conflicts
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::ModifyDelete),
        ]);
        assert!(result.has_conflicts());

        // Has DeleteModify = has conflicts
        let result = make_three_way_result(vec![
            make_three_way_entry("file.txt", ThreeWayStatus::DeleteModify),
        ]);
        assert!(result.has_conflicts());
    }

    #[test]
    fn test_three_way_diff_result_count_by_status() {
        let result = make_three_way_result(vec![
            make_three_way_entry("a.txt", ThreeWayStatus::Unchanged),
            make_three_way_entry("b.txt", ThreeWayStatus::Unchanged),
            make_three_way_entry("c.txt", ThreeWayStatus::OursOnly),
            make_three_way_entry("d.txt", ThreeWayStatus::Conflict),
            make_three_way_entry("e.txt", ThreeWayStatus::Conflict),
            make_three_way_entry("f.txt", ThreeWayStatus::AddedBothDiff),
        ]);

        assert_eq!(result.count_by_status(&ThreeWayStatus::Unchanged), 2);
        assert_eq!(result.count_by_status(&ThreeWayStatus::OursOnly), 1);
        assert_eq!(result.count_by_status(&ThreeWayStatus::Conflict), 2);
        assert_eq!(result.count_by_status(&ThreeWayStatus::AddedBothDiff), 1);
        assert_eq!(result.count_by_status(&ThreeWayStatus::TheirsOnly), 0);
    }

    #[test]
    fn test_three_way_diff_result_conflict_count() {
        let result = make_three_way_result(vec![
            make_three_way_entry("a.txt", ThreeWayStatus::Unchanged),
            make_three_way_entry("b.txt", ThreeWayStatus::OursOnly),
            make_three_way_entry("c.txt", ThreeWayStatus::Conflict),
            make_three_way_entry("d.txt", ThreeWayStatus::AddedBothDiff),
            make_three_way_entry("e.txt", ThreeWayStatus::ModifyDelete),
            make_three_way_entry("f.txt", ThreeWayStatus::DeleteModify),
        ]);

        assert_eq!(result.count_conflicts(), 4);
    }

    #[test]
    fn test_merge_style_default() {
        assert_eq!(MergeStyle::default(), MergeStyle::All);
    }

    #[test]
    fn test_files_equal_same_content() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");

        fs::write(&file1, "same content").unwrap();
        fs::write(&file2, "same content").unwrap();

        assert!(files_equal(&file1, &file2));
    }

    #[test]
    fn test_files_equal_different_content() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");

        fs::write(&file1, "content A").unwrap();
        fs::write(&file2, "content B").unwrap();

        assert!(!files_equal(&file1, &file2));
    }

    #[test]
    fn test_files_equal_different_size() {
        let dir = tempfile::tempdir().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");

        fs::write(&file1, "short").unwrap();
        fs::write(&file2, "much longer content").unwrap();

        assert!(!files_equal(&file1, &file2));
    }

    #[test]
    fn test_compare_three_way_directories_all_unchanged() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Create identical files in all three directories
        fs::write(base.path().join("file.txt"), "content").unwrap();
        fs::write(ours.path().join("file.txt"), "content").unwrap();
        fs::write(theirs.path().join("file.txt"), "content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::Unchanged);
        assert!(!result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_ours_only() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Base and theirs have same content, ours is different
        fs::write(base.path().join("file.txt"), "base content").unwrap();
        fs::write(ours.path().join("file.txt"), "ours modified").unwrap();
        fs::write(theirs.path().join("file.txt"), "base content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::OursOnly);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_theirs_only() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Base and ours have same content, theirs is different
        fs::write(base.path().join("file.txt"), "base content").unwrap();
        fs::write(ours.path().join("file.txt"), "base content").unwrap();
        fs::write(theirs.path().join("file.txt"), "theirs modified").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::TheirsOnly);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_both_same() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Both modified to the same content
        fs::write(base.path().join("file.txt"), "base content").unwrap();
        fs::write(ours.path().join("file.txt"), "same modified").unwrap();
        fs::write(theirs.path().join("file.txt"), "same modified").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::BothSame);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_conflict() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Both modified to different content
        fs::write(base.path().join("file.txt"), "base content").unwrap();
        fs::write(ours.path().join("file.txt"), "ours modified").unwrap();
        fs::write(theirs.path().join("file.txt"), "theirs modified").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::Conflict);
        assert!(result.has_differences());
        assert!(result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_added_ours() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // File added only in ours
        fs::write(ours.path().join("new_file.txt"), "new content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::AddedOurs);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_added_theirs() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // File added only in theirs
        fs::write(theirs.path().join("new_file.txt"), "new content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::AddedTheirs);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_added_both_same() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Same file added in both
        fs::write(ours.path().join("new_file.txt"), "same content").unwrap();
        fs::write(theirs.path().join("new_file.txt"), "same content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::AddedBothSame);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_added_both_diff() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Different content added in both
        fs::write(ours.path().join("new_file.txt"), "ours content").unwrap();
        fs::write(theirs.path().join("new_file.txt"), "theirs content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::AddedBothDiff);
        assert!(result.has_differences());
        assert!(result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_deleted_ours() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // File in base and theirs, deleted in ours
        fs::write(base.path().join("file.txt"), "content").unwrap();
        fs::write(theirs.path().join("file.txt"), "content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::DeletedOurs);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_deleted_theirs() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // File in base and ours, deleted in theirs
        fs::write(base.path().join("file.txt"), "content").unwrap();
        fs::write(ours.path().join("file.txt"), "content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::DeletedTheirs);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_deleted_both() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // File in base, deleted in both
        fs::write(base.path().join("file.txt"), "content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::DeletedBoth);
        assert!(result.has_differences());
        assert!(!result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_modify_delete() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // File modified in ours, deleted in theirs
        fs::write(base.path().join("file.txt"), "base content").unwrap();
        fs::write(ours.path().join("file.txt"), "modified content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::ModifyDelete);
        assert!(result.has_differences());
        assert!(result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_directories_delete_modify() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // File deleted in ours, modified in theirs
        fs::write(base.path().join("file.txt"), "base content").unwrap();
        fs::write(theirs.path().join("file.txt"), "modified content").unwrap();

        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &[],
            false,
        ).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, ThreeWayStatus::DeleteModify);
        assert!(result.has_differences());
        assert!(result.has_conflicts());
    }

    #[test]
    fn test_compare_three_way_with_exclude() {
        let base = tempfile::tempdir().unwrap();
        let ours = tempfile::tempdir().unwrap();
        let theirs = tempfile::tempdir().unwrap();

        // Create files including one that should be excluded
        fs::write(base.path().join("file.txt"), "base").unwrap();
        fs::write(ours.path().join("file.txt"), "ours").unwrap();
        fs::write(theirs.path().join("file.txt"), "theirs").unwrap();

        fs::write(base.path().join("file.log"), "base log").unwrap();
        fs::write(ours.path().join("file.log"), "ours log").unwrap();
        fs::write(theirs.path().join("file.log"), "theirs log").unwrap();

        let exclude_patterns = vec![Pattern::new("*.log").unwrap()];
        let result = compare_three_way_directories(
            base.path(),
            ours.path(),
            theirs.path(),
            &exclude_patterns,
            false,
        ).unwrap();

        // Only file.txt should be compared (file.log is excluded)
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("file.txt"));
    }

    #[test]
    fn test_generate_three_way_summary_no_differences() {
        let result = ThreeWayDiffResult {
            entries: vec![],
            base_dir: PathBuf::from("/base"),
            ours_dir: PathBuf::from("/ours"),
            theirs_dir: PathBuf::from("/theirs"),
            total_paths: 0,
        };

        let options = ThreeWaySummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            merge_style: MergeStyle::All,
            conflict_only: false,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            copy_result: None,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
        };

        let summary = generate_three_way_summary(&result, &options);

        assert!(summary.contains("rs_diffcopy Summary (Three-way)"));
        assert!(summary.contains("No differences found."));
    }

    #[test]
    fn test_generate_three_way_summary_with_conflicts() {
        let result = ThreeWayDiffResult {
            entries: vec![
                make_three_way_entry("file1.txt", ThreeWayStatus::OursOnly),
                make_three_way_entry("file2.txt", ThreeWayStatus::Conflict),
                make_three_way_entry("file3.txt", ThreeWayStatus::AddedBothDiff),
            ],
            base_dir: PathBuf::from("/base"),
            ours_dir: PathBuf::from("/ours"),
            theirs_dir: PathBuf::from("/theirs"),
            total_paths: 3,
        };

        let options = ThreeWaySummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            merge_style: MergeStyle::All,
            conflict_only: false,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            copy_result: None,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
        };

        let summary = generate_three_way_summary(&result, &options);

        assert!(summary.contains("rs_diffcopy Summary (Three-way)"));
        assert!(summary.contains("Change Matrix"));
        assert!(summary.contains("Conflicts"));
        assert!(summary.contains("CONFLICT"));
    }

    // ==================== Filter Resolution Tests ====================

    #[test]
    fn test_resolve_filter_statuses_basic() {
        // Basic include
        let filter = vec!["added".to_string(), "modified".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));
        assert!(resolved.contains("modified"));
        assert!(!resolved.contains("deleted"));
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn test_resolve_filter_statuses_all_keyword() {
        // all keyword includes everything
        let filter = vec!["all".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));
        assert!(resolved.contains("modified"));
        assert!(resolved.contains("deleted"));
        assert!(resolved.contains("unchanged"));
        assert!(resolved.contains("symlink"));
        assert!(resolved.contains("special"));
        assert!(resolved.contains("error"));
        assert!(resolved.contains("permission"));
        assert_eq!(resolved.len(), 8);
    }

    #[test]
    fn test_resolve_filter_statuses_exclusion() {
        // ^ prefix excludes
        let filter = vec!["all".to_string(), "^unchanged".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));
        assert!(resolved.contains("modified"));
        assert!(resolved.contains("deleted"));
        assert!(!resolved.contains("unchanged"));  // excluded
        assert_eq!(resolved.len(), 7);
    }

    #[test]
    fn test_resolve_filter_statuses_multiple_exclusions() {
        // Multiple exclusions
        let filter = vec!["all".to_string(), "^unchanged".to_string(), "^error".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));
        assert!(!resolved.contains("unchanged"));  // excluded
        assert!(!resolved.contains("error"));      // excluded
        assert_eq!(resolved.len(), 6);
    }

    #[test]
    fn test_resolve_filter_statuses_last_wins() {
        // Last wins: add, exclude, add again
        let filter = vec!["added".to_string(), "^added".to_string(), "added".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));  // Re-added after exclusion
    }

    #[test]
    fn test_resolve_filter_statuses_comma_separated() {
        // Comma-separated values in single string
        let filter = vec!["added,modified,deleted".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));
        assert!(resolved.contains("modified"));
        assert!(resolved.contains("deleted"));
        assert_eq!(resolved.len(), 3);
    }

    #[test]
    fn test_resolve_filter_statuses_comma_with_exclusion() {
        // Comma-separated with exclusion
        let filter = vec!["all,^unchanged,^error".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));
        assert!(!resolved.contains("unchanged"));
        assert!(!resolved.contains("error"));
        assert_eq!(resolved.len(), 6);
    }

    #[test]
    fn test_resolve_filter_statuses_case_insensitive() {
        // Case insensitive
        let filter = vec!["ADDED".to_string(), "Modified".to_string()];
        let resolved = resolve_filter_statuses(&filter, TWO_WAY_ALL_STATUSES);
        assert!(resolved.contains("added"));
        assert!(resolved.contains("modified"));
    }

    #[test]
    fn test_is_status_filtered_out_empty() {
        // Empty filter means nothing is filtered out
        let filter: Vec<String> = vec![];
        assert!(!is_status_filtered_out("added", &filter));
        assert!(!is_status_filtered_out("deleted", &filter));
    }

    #[test]
    fn test_is_status_filtered_out_specific() {
        // Only added is included
        let filter = vec!["added".to_string()];
        assert!(!is_status_filtered_out("added", &filter));  // included
        assert!(is_status_filtered_out("modified", &filter));  // not included
        assert!(is_status_filtered_out("deleted", &filter));   // not included
    }

    #[test]
    fn test_is_status_filtered_out_with_exclusion() {
        // All except unchanged
        let filter = vec!["all,^unchanged".to_string()];
        assert!(!is_status_filtered_out("added", &filter));
        assert!(!is_status_filtered_out("modified", &filter));
        assert!(is_status_filtered_out("unchanged", &filter));  // excluded
    }

    // ==================== Three-way Filter Tests ====================

    #[test]
    fn test_three_way_filter_basic() {
        let filter = vec!["conflict".to_string()];
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::Conflict, &filter));
        assert!(is_three_way_status_filtered_out(&ThreeWayStatus::OursOnly, &filter));
        assert!(is_three_way_status_filtered_out(&ThreeWayStatus::Unchanged, &filter));
    }

    #[test]
    fn test_three_way_filter_all_keyword() {
        let filter = vec!["all".to_string()];
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::Conflict, &filter));
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::OursOnly, &filter));
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::Unchanged, &filter));
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::AddedBothDiff, &filter));
    }

    #[test]
    fn test_three_way_filter_exclusion() {
        let filter = vec!["all,^unchanged".to_string()];
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::Conflict, &filter));
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::OursOnly, &filter));
        assert!(is_three_way_status_filtered_out(&ThreeWayStatus::Unchanged, &filter));  // excluded
    }

    #[test]
    fn test_three_way_filter_conflicts_only() {
        // Show only conflict-type statuses
        let filter = vec!["conflict,added-both-diff,modify-delete,delete-modify".to_string()];
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::Conflict, &filter));
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::AddedBothDiff, &filter));
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::ModifyDelete, &filter));
        assert!(!is_three_way_status_filtered_out(&ThreeWayStatus::DeleteModify, &filter));
        assert!(is_three_way_status_filtered_out(&ThreeWayStatus::OursOnly, &filter));
        assert!(is_three_way_status_filtered_out(&ThreeWayStatus::BothSame, &filter));
    }

    #[test]
    fn test_three_way_status_to_filter_str() {
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::Unchanged), "unchanged");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::OursOnly), "ours-only");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::TheirsOnly), "theirs-only");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::BothSame), "both-same");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::Conflict), "conflict");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::AddedOurs), "added-ours");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::AddedTheirs), "added-theirs");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::AddedBothSame), "added-both-same");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::AddedBothDiff), "added-both-diff");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::DeletedOurs), "deleted-ours");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::DeletedTheirs), "deleted-theirs");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::DeletedBoth), "deleted-both");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::ModifyDelete), "modify-delete");
        assert_eq!(three_way_status_to_filter_str(&ThreeWayStatus::DeleteModify), "delete-modify");
    }

    #[test]
    fn test_generate_three_way_summary_with_filter() {
        let result = ThreeWayDiffResult {
            entries: vec![
                make_three_way_entry("file1.txt", ThreeWayStatus::Unchanged),
                make_three_way_entry("file2.txt", ThreeWayStatus::OursOnly),
                make_three_way_entry("file3.txt", ThreeWayStatus::Conflict),
            ],
            base_dir: PathBuf::from("/base"),
            ours_dir: PathBuf::from("/ours"),
            theirs_dir: PathBuf::from("/theirs"),
            total_paths: 3,
        };

        // Filter to show only conflict
        let options = ThreeWaySummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            merge_style: MergeStyle::All,
            conflict_only: false,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            copy_result: None,
            filter_status: vec!["conflict".to_string()],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false,
        };

        let summary = generate_three_way_summary(&result, &options);

        // File Matrix should only show conflict entry
        assert!(summary.contains("file3.txt"), "Should show conflict file");
        // file1.txt (unchanged) should not appear in matrix (either filtered or skipped by default)
        // file2.txt (ours-only) should be filtered out
    }

    // ==================== Output Format Tests ====================

    #[test]
    fn test_file_tree_console_format() {
        // Console format should use tree structure
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("dir/file1.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("dir/file2.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 1,
            target_count: 2,
            common_count: 1,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false, // Console format
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        // Console format uses tree structure with indent markers
        assert!(summary.contains("File Tree"), "Should have File Tree section");
        assert!(summary.contains("├") || summary.contains("└"), "Console format should use tree markers");
    }

    #[test]
    fn test_file_tree_file_format() {
        // File format should use tree structure (same as console)
        let result = DiffResult {
            entries: vec![
                DiffEntry {
                    relative_path: PathBuf::from("dir/file1.txt"),
                    is_dir: false,
                    status: FileStatus::Added,
                },
                DiffEntry {
                    relative_path: PathBuf::from("dir/file2.txt"),
                    is_dir: false,
                    status: FileStatus::Modified,
                },
            ],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
            source_count: 1,
            target_count: 2,
            common_count: 1,
        };

        let options = SummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            both_versions: false,
            check_permissions: PermissionCheckMode::None,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            patch: false,
            patch_file: None,
            patch_result: None,
            copy_result: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: true, // File format
            copy_deleted: false,
            preserve_timestamps: false,
        };

        let summary = generate_summary(&result, &options);
        // Both console and file use tree format
        assert!(summary.contains("File Tree"), "Should have File Tree section");
        assert!(summary.contains("├") || summary.contains("└"), "File format should use tree markers");
        assert!(summary.contains("file1.txt"), "Should show file in tree");
    }

    #[test]
    fn test_three_way_file_tree_console_format() {
        // Console format should use compact tree with indicators
        let result = ThreeWayDiffResult {
            entries: vec![
                ThreeWayEntry {
                    relative_path: PathBuf::from("dir/file1.txt"),
                    is_dir: false,
                    base_size: Some(100),
                    ours_size: Some(150),
                    theirs_size: Some(100),
                    status: ThreeWayStatus::OursOnly,
                },
            ],
            base_dir: PathBuf::from("/base"),
            ours_dir: PathBuf::from("/ours"),
            theirs_dir: PathBuf::from("/theirs"),
            total_paths: 1,
        };

        let options = ThreeWaySummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            merge_style: MergeStyle::All,
            conflict_only: false,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            copy_result: None,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: false, // Console format
        };

        let summary = generate_three_way_summary(&result, &options);
        // Console format should use tree with compact indicators
        assert!(summary.contains("File Tree"), "Should have File Tree section");
        assert!(summary.contains("Legend:"), "Should have legend");
        assert!(summary.contains("[○M=]"), "Console format should have compact indicators");
        assert!(summary.contains("ours-only"), "Should show status label");
    }

    #[test]
    fn test_three_way_file_tree_file_format() {
        // File format should use aligned tree with indicators
        let result = ThreeWayDiffResult {
            entries: vec![
                ThreeWayEntry {
                    relative_path: PathBuf::from("dir/file1.txt"),
                    is_dir: false,
                    base_size: Some(100),
                    ours_size: Some(150),
                    theirs_size: Some(100),
                    status: ThreeWayStatus::OursOnly,
                },
            ],
            base_dir: PathBuf::from("/base"),
            ours_dir: PathBuf::from("/ours"),
            theirs_dir: PathBuf::from("/theirs"),
            total_paths: 1,
        };

        let options = ThreeWaySummaryOptions {
            exclude_patterns: vec![],
            dry_run: false,
            merge_style: MergeStyle::All,
            conflict_only: false,
            config_file: None,
            output_dir: PathBuf::from("/output"),
            copy_result: None,
            filter_status: vec![],
            stats_only: false,
            no_tree: false,
            no_details: false,
            output_to_file: true, // File format
        };

        let summary = generate_three_way_summary(&result, &options);
        // File format should use aligned tree with spaced indicators
        assert!(summary.contains("File Tree"), "Should have File Tree section");
        assert!(summary.contains("B  O  T"), "File format should have column header");
        assert!(summary.contains("[○  M  =]"), "File format should have spaced indicators");
        assert!(summary.contains("file1.txt"), "Should show file name in tree");
    }

    #[test]
    fn test_display_width() {
        // Test ASCII characters
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("test.txt"), 8);

        // Test Japanese characters (full-width)
        assert_eq!(display_width("日本語"), 6); // 3 chars * 2
        assert_eq!(display_width("テスト"), 6); // 3 chars * 2

        // Test mixed
        assert_eq!(display_width("file_日本語.txt"), 15); // 9 ASCII (file_.txt) + 3*2 Japanese

        // Test special characters
        assert_eq!(display_width("○"), 2);
        assert_eq!(display_width("●"), 2);
        assert_eq!(display_width("[○M=]"), 6); // 4 ASCII + 1 full-width
    }

    #[test]
    fn test_pad_to_width() {
        assert_eq!(pad_to_width("hello", 10), "hello     ");
        assert_eq!(pad_to_width("日本語", 10), "日本語    "); // 6 width + 4 spaces
        assert_eq!(pad_to_width("test", 4), "test"); // exact width
        assert_eq!(pad_to_width("toolong", 5), "toolong"); // over width, no truncation
    }
}
