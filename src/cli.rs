use clap::Parser;
use std::path::PathBuf;

use crate::types::{CheckPermissionsMode, ColorMode, LogLevel, MergeStyle, StatusFilter};

/// A tool to compare directories and extract diff files with preserved structure
#[derive(Parser, Debug, Clone)]
#[command(name = "rs_diffcopy")]
#[command(version = "1.0.0")]
#[command(about = "Compare directories and extract diff files")]
#[command(long_about = "Compare two directories and extract files with differences, \
    preserving the directory structure. Designed for beginners and non-technical users \
    to visually understand changes between directory versions.")]
pub struct Cli {
    /// Source directory (comparison source)
    #[arg(short = 'S', long)]
    pub source: Option<PathBuf>,

    /// Target directory (comparison target)
    #[arg(short = 'T', long)]
    pub target: Option<PathBuf>,

    /// Output directory for diff files
    #[arg(short = 'O', long)]
    pub output: Option<PathBuf>,

    /// Config file path (TOML format)
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Exclude patterns (glob format, can be specified multiple times)
    #[arg(short = 'e', long, action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Force overwrite: delete output directory before execution (with confirmation)
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Save summary to file
    #[arg(short = 's', long)]
    pub summary: Option<PathBuf>,

    /// Verbose output mode (show file names during processing)
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Dry run: preview without copying files
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Copy both old and new versions (.old/.new extension)
    #[arg(short = 'b', long)]
    pub both_versions: bool,

    /// Permission check mode (none/scripts/all)
    #[arg(short = 'P', long, default_value = "none")]
    pub check_permissions: String,

    /// Generate individual patch files (.patch) for each modified file
    #[arg(short = 'p', long)]
    pub patch: bool,

    /// Generate combined patch file for all changes
    #[arg(short = 'F', long)]
    pub patch_file: Option<PathBuf>,

    /// Output summary as Excel file (.xlsx)
    #[arg(short = 'E', long)]
    pub excel: Option<PathBuf>,

    /// Excel file tree fold level (fold directories at this depth and deeper)
    #[arg(short = 'L', long)]
    pub excel_fold_level: Option<usize>,

    /// Show unchanged files in summary details
    #[arg(short = 'u', long)]
    pub show_unchanged: bool,

    /// Save current options to config file
    #[arg(short = 'C', long)]
    pub save_config: Option<PathBuf>,

    /// Filter by status (comma-separated, prefix ^ to exclude)
    #[arg(long, value_delimiter = ',')]
    pub filter_status: Vec<String>,

    /// Show only header/options/statistics (no tree or details)
    #[arg(long)]
    pub stats_only: bool,

    /// Hide File Tree section
    #[arg(long)]
    pub no_tree: bool,

    /// Hide details sections (Added/Modified/Deleted Files etc.)
    #[arg(long)]
    pub no_details: bool,

    /// Copy deleted files with .deleted extension
    #[arg(long)]
    pub copy_deleted: bool,

    /// Preserve file timestamps when copying
    #[arg(long)]
    pub preserve_timestamps: bool,

    /// Number of parallel workers
    #[arg(short = 'j', long)]
    pub workers: Option<usize>,

    /// Temporary directory for intermediate files
    #[arg(long)]
    pub temp_dir: Option<PathBuf>,

    /// Color output mode (auto/always/never)
    #[arg(long, default_value = "auto")]
    pub color: String,

    /// Log level (error/warn/info/debug)
    #[arg(long, default_value = "warn")]
    pub log_level: String,

    // Three-way mode options
    /// Enable three-way comparison mode
    #[arg(short = '3', long)]
    pub three_way: bool,

    /// Base (common ancestor) directory for three-way comparison
    #[arg(short = 'B', long)]
    pub base: Option<PathBuf>,

    /// Merge style for conflicts (all/ours/theirs)
    #[arg(short = 'M', long, default_value = "all")]
    pub merge_style: String,

    /// Output only conflict candidates
    #[arg(long)]
    pub conflict_only: bool,
}

impl Cli {
    pub fn get_check_permissions_mode(&self) -> Option<CheckPermissionsMode> {
        CheckPermissionsMode::from_str(&self.check_permissions)
    }

    pub fn get_merge_style(&self) -> Option<MergeStyle> {
        MergeStyle::from_str(&self.merge_style)
    }

    pub fn get_color_mode(&self) -> Option<ColorMode> {
        ColorMode::from_str(&self.color)
    }

    pub fn get_log_level(&self) -> Option<LogLevel> {
        LogLevel::from_str(&self.log_level)
    }

    pub fn parse_filter_status(&self) -> StatusFilter {
        let mut filter = StatusFilter::new();

        if self.filter_status.is_empty() {
            return filter;
        }

        // Check if first filter is exclusion (starts with ^)
        // If so, start with all statuses included
        if let Some(first) = self.filter_status.first() {
            if first.starts_with('^') {
                filter.include_all = true;
            }
        }

        for status_str in &self.filter_status {
            if status_str == "all" {
                filter.include_all = true;
                continue;
            }

            if let Some(stripped) = status_str.strip_prefix('^') {
                let lowered = stripped.to_lowercase();
                // Always insert the original keyword (for two-way mode compatibility)
                filter.excluded.insert(lowered.clone());
                // Also expand group keywords for three-way mode
                if let Some(expanded) = StatusFilter::expand_three_way_group(&lowered) {
                    for s in expanded {
                        filter.excluded.insert(s.to_string());
                    }
                }
            } else {
                let lowered = status_str.to_lowercase();
                // Always insert the original keyword (for two-way mode compatibility)
                filter.included.insert(lowered.clone());
                // Also expand group keywords for three-way mode
                if let Some(expanded) = StatusFilter::expand_three_way_group(&lowered) {
                    for s in expanded {
                        filter.included.insert(s.to_string());
                    }
                }
            }
        }

        filter
    }
}
