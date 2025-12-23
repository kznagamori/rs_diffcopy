use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::{Parser, ValueEnum};
use glob::Pattern;
use rayon::prelude::*;
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
    #[arg(long, value_name = "PATH")]
    patch_file: Option<PathBuf>,
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
        })
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileStatus {
    Added,
    Modified,
    Deleted,
    Symlink {
        change_type: SymlinkChangeType,
        current: Option<SymlinkInfo>,  // None if deleted
        previous: Option<SymlinkInfo>, // None if added, Some if changed/deleted
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
}

/// Patch generation result for a single file
#[derive(Debug, Clone)]
struct PatchInfo {
    relative_path: PathBuf,
    is_binary: bool,
    patch_generated: bool,
}

/// Patch generation result
#[derive(Debug, Default)]
struct PatchResult {
    patches: Vec<PatchInfo>,
    total_generated: usize,
    total_skipped: usize,
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
}

impl DiffResult {
    fn has_differences(&self) -> bool {
        !self.entries.is_empty() || !self.permission_changes.is_empty()
    }

    fn count_by_status(&self) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
        let mut added_files = 0;
        let mut added_dirs = 0;
        let mut modified_files = 0;
        let mut deleted_files = 0;
        let mut deleted_dirs = 0;
        let mut symlinks = 0;
        let mut errors = 0;

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
                FileStatus::Symlink { .. } => symlinks += 1,
                FileStatus::PermissionDenied { .. } => errors += 1,
            }
        }

        let permission_changes = self.permission_changes.len();

        (added_files, added_dirs, modified_files, deleted_files, deleted_dirs, symlinks, permission_changes, errors)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Resolve configuration from CLI args and optional config file
    let config = ResolvedConfig::from_args(args)?;

    // Validate source directory
    if !config.source_dir.exists() {
        bail!("Source directory does not exist: {}", config.source_dir.display());
    }
    if !config.source_dir.is_dir() {
        bail!("Source path is not a directory: {}", config.source_dir.display());
    }

    // Validate target directory
    if !config.target_dir.exists() {
        bail!("Target directory does not exist: {}", config.target_dir.display());
    }
    if !config.target_dir.is_dir() {
        bail!("Target path is not a directory: {}", config.target_dir.display());
    }

    // Handle output directory
    if config.output_dir.exists() {
        if config.force {
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

    // Compare directories
    let diff_result = compare_directories(
        &config.source_dir,
        &config.target_dir,
        &exclude_patterns,
        config.verbose,
        config.check_permissions,
    )?;

    // Copy files (if not dry-run and has differences)
    if !config.dry_run && diff_result.has_differences() {
        copy_diff_files(&diff_result, &config.output_dir, config.verbose, config.both_versions)?;
    }

    // Generate patches if requested (after copying, before summary)
    let patch_result = if (config.patch || config.patch_file.is_some()) && diff_result.has_differences() && !config.dry_run {
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

    // Phase 4: Generate and output summary
    print_phase(4, 4, "Writing summary...");

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

    if is_terminal() {
        println_to_stdout("Done.");
    }

    // Return appropriate exit code
    if !diff_result.has_differences() {
        std::process::exit(2);
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

    let is_dir = target_path.is_dir();

    if !source_paths.contains(rel_path) {
        // Added (only in target)
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
) -> Result<DiffResult> {
    const TOTAL_PHASES: usize = 4;

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

    Ok(DiffResult {
        entries,
        permission_changes,
        source_dir: source_dir.to_path_buf(),
        target_dir: target_dir.to_path_buf(),
    })
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

/// Copy a single file entry
fn copy_single_file(
    entry: &DiffEntry,
    source_dir: &Path,
    target_dir: &Path,
    output_dir: &Path,
    both_versions: bool,
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
                fs::copy(&src, &dst)?;
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

                fs::copy(&src_old, &dst_old)?;
                fs::copy(&src_new, &dst_new)?;
            } else {
                fs::copy(&src_new, &dst_base)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn copy_diff_files(diff_result: &DiffResult, output_dir: &Path, verbose: bool, both_versions: bool) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    // Filter entries that need to be copied
    let entries_to_copy: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Added | FileStatus::Modified))
        .collect();

    let total_files = entries_to_copy.len();

    if total_files == 0 {
        return Ok(());
    }

    // Phase 3: Copying (parallel)
    print_phase(3, 4, "Copying files...");

    let progress_counter = AtomicUsize::new(0);
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
            ) {
                let mut errs = errors.lock().unwrap();
                errs.push(format!("{}: {}", entry.relative_path.display(), e));
            }

            if verbose && is_terminal() {
                // Verbose output may interleave in parallel, which is acceptable
            }
        });

    clear_progress_line();

    // Check for errors
    let errs = errors.into_inner().unwrap();
    if !errs.is_empty() {
        for err in &errs {
            eprintln!("Error: {}", err);
        }
        bail!("Failed to copy {} files", errs.len());
    }

    if is_terminal() {
        println_to_stdout(&format!("Copied {} files.", total_files));
    }

    Ok(())
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
            result.patches.push(PatchInfo {
                relative_path: entry.relative_path.clone(),
                is_binary: false,
                patch_generated: true,
            });
            result.total_generated += 1;

            // Write individual patch file
            if individual_patches {
                let patch_path = output_dir.join(add_extension(&entry.relative_path, "patch"));
                if let Some(parent) = patch_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&patch_path, &diff_content)?;

                if verbose && is_terminal() {
                    println_to_stdout(&format!("  Generated: {}", patch_path.display()));
                }
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
            fs::write(path, &combined_output)?;
            if verbose && is_terminal() {
                println_to_stdout(&format!("Combined patch written to: {}", path.display()));
            }
        }
    }

    if is_terminal() {
        println_to_stdout(&format!(
            "Patches: {} generated, {} skipped (binary)",
            result.total_generated,
            result.total_skipped
        ));
    }

    Ok(result)
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

    if has_options {
        output.push_str("Options:\n");
        output.push_str(&options_output);
        output.push('\n');
    }

    if !diff_result.has_differences() {
        output.push_str("No differences found.\n");
        return output;
    }

    let (added_files, added_dirs, modified_files, deleted_files, deleted_dirs, symlinks, permission_changes, errors) =
        diff_result.count_by_status();

    // Statistics
    if added_files > 0 || added_dirs > 0 {
        let mut parts = Vec::new();
        if added_files > 0 {
            parts.push(format!("{} files", added_files));
        }
        if added_dirs > 0 {
            parts.push(format!("{} dirs", added_dirs));
        }
        output.push_str(&format!("Added:      {}\n", parts.join(", ")));
    }
    if modified_files > 0 {
        output.push_str(&format!("Modified:   {} files\n", modified_files));
    }
    if deleted_files > 0 || deleted_dirs > 0 {
        let mut parts = Vec::new();
        if deleted_files > 0 {
            parts.push(format!("{} files", deleted_files));
        }
        if deleted_dirs > 0 {
            parts.push(format!("{} dirs", deleted_dirs));
        }
        output.push_str(&format!("Deleted:    {}\n", parts.join(", ")));
    }
    if symlinks > 0 {
        output.push_str(&format!("Symlinks:   {} files\n", symlinks));
    }
    if permission_changes > 0 {
        output.push_str(&format!("Permissions: {} files\n", permission_changes));
    }
    if errors > 0 {
        output.push_str(&format!("Errors:     {} files\n", errors));
    }

    let total = added_files + added_dirs + modified_files + deleted_files + deleted_dirs + symlinks + permission_changes + errors;
    output.push_str("--------------------------\n");
    output.push_str(&format!("Total:     {} items\n", total));
    output.push('\n');

    // File Tree
    output.push_str("================\n");
    output.push_str("File Tree\n");
    output.push_str("================\n");
    output.push_str(&generate_tree(&diff_result.entries));
    output.push('\n');

    // Added Details
    let added_entries: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Added))
        .collect();

    if !added_entries.is_empty() {
        output.push_str("================\n");
        output.push_str("Added Files\n");
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

    // Modified Details
    let modified_entries: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Modified))
        .collect();

    if !modified_entries.is_empty() {
        output.push_str("================\n");
        output.push_str("Modified Files\n");
        output.push_str("================\n");
        for entry in modified_entries {
            output.push_str(&format!("  {}\n", entry.relative_path.display()));
        }
        output.push('\n');
    }

    // Deleted Details
    let deleted_entries: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Deleted))
        .collect();

    if !deleted_entries.is_empty() {
        output.push_str("================\n");
        output.push_str("Deleted Files\n");
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

    // Symlink Details
    let symlink_entries: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Symlink { .. }))
        .collect();

    if !symlink_entries.is_empty() {
        output.push_str("================\n");
        output.push_str("Symlink Details\n");
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

    // Permission Changes
    if !diff_result.permission_changes.is_empty() {
        output.push_str("================\n");
        output.push_str("Permission Changes\n");
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

    // Errors
    let error_entries: Vec<_> = diff_result
        .entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::PermissionDenied { .. }))
        .collect();

    if !error_entries.is_empty() {
        output.push_str("================\n");
        output.push_str("Errors\n");
        output.push_str("================\n");
        for entry in error_entries {
            if let FileStatus::PermissionDenied { error } = &entry.status {
                output.push_str(&format!("{}: {}\n", entry.relative_path.display(), error));
            }
        }
    }

    // Patch Details
    if let Some(patch_result) = &options.patch_result {
        if !patch_result.patches.is_empty() {
            output.push_str("================\n");
            output.push_str("Patch Details\n");
            output.push_str("================\n");

            let generated: Vec<_> = patch_result.patches.iter().filter(|p| p.patch_generated).collect();
            let skipped: Vec<_> = patch_result.patches.iter().filter(|p| p.is_binary).collect();

            output.push_str(&format!(
                "Generated: {} patches, Skipped: {} (binary)\n\n",
                patch_result.total_generated,
                patch_result.total_skipped
            ));

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
        }
    }

    output
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
        FileStatus::PermissionDenied { .. } => "[permission denied]".to_string(),
    }
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

    // ==================== compare_directories tests ====================

    #[test]
    fn test_compare_directories_added_file() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(target.path(), "new_file.txt", "New content");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].status, FileStatus::Added);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("new_file.txt"));
    }

    #[test]
    fn test_compare_directories_deleted_file() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "old_file.txt", "Old content");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None).unwrap();

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

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None).unwrap();

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

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None).unwrap();

        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_compare_directories_with_subdirectories() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        create_file(source.path(), "src/main.rs", "fn main() {}");
        create_file(target.path(), "src/main.rs", "fn main() { println!(\"Hello\"); }");
        create_file(target.path(), "src/lib.rs", "pub fn hello() {}");

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None).unwrap();

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
        let result = compare_directories(source.path(), target.path(), &patterns, false, PermissionCheckMode::None).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].relative_path, PathBuf::from("main.rs"));
    }

    #[test]
    fn test_compare_directories_added_empty_directory() {
        let source = create_temp_dir();
        let target = create_temp_dir();

        fs::create_dir(target.path().join("new_dir")).unwrap();

        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None).unwrap();

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
        };

        let (added_files, added_dirs, modified, deleted_files, deleted_dirs, symlinks, perm_changes, errors) =
            result.count_by_status();

        assert_eq!(added_files, 2);
        assert_eq!(added_dirs, 1);
        assert_eq!(modified, 1);
        assert_eq!(deleted_files, 1);
        assert_eq!(deleted_dirs, 0);
        assert_eq!(symlinks, 0);
        assert_eq!(perm_changes, 0);
        assert_eq!(errors, 0);
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
        }
    }

    #[test]
    fn test_generate_summary_no_differences() {
        let result = DiffResult {
            entries: vec![],
            permission_changes: vec![],
            source_dir: PathBuf::from("/source"),
            target_dir: PathBuf::from("/target"),
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
        };

        copy_diff_files(&diff_result, output.path(), false, false).unwrap();

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
        };

        copy_diff_files(&diff_result, output.path(), false, false).unwrap();

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
        };

        copy_diff_files(&diff_result, output.path(), false, false).unwrap();

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
        };

        copy_diff_files(&diff_result, output.path(), false, true).unwrap();

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
        };

        copy_diff_files(&diff_result, output.path(), false, true).unwrap();

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
        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::Scripts).unwrap();

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
        let result = compare_directories(source.path(), target.path(), &[], false, PermissionCheckMode::None).unwrap();

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
                total_generated: 1,
                total_skipped: 0,
            }),
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
                total_generated: 0,
                total_skipped: 1,
            }),
        };

        let summary = generate_summary(&result, &options);
        assert!(summary.contains("Skipped: 1 (binary)"));
        assert!(summary.contains("Skipped (binary):"));
        assert!(summary.contains("image.png [skip]"));
    }
}
