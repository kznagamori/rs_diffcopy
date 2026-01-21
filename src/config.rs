use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::error::{DiffCopyError, Result};
use crate::types::{CheckPermissionsMode, ColorMode, LogLevel, MergeStyle, StatusFilter};

/// Application-wide settings (settings.toml)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_dir: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_exclude: Option<Vec<String>>,
}

impl AppSettings {
    pub fn load_from_exe_dir() -> Option<Self> {
        let exe_path = std::env::current_exe().ok()?;
        let exe_dir = exe_path.parent()?;
        let settings_path = exe_dir.join("settings.toml");

        if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path).ok()?;
            toml::from_str(&content).ok()
        } else {
            None
        }
    }
}

/// Input configuration file (diffcopy.toml)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub force: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub verbose: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub both_versions: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_permissions: Option<String>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub patch: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_file: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub excel: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub excel_fold_level: Option<usize>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub show_unchanged: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter_status: Vec<String>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub stats_only: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub no_tree: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub no_details: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub copy_deleted: bool,

    #[serde(default, skip_serializing_if = "is_false")]
    pub preserve_timestamps: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_dir: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,

    // Three-way options
    #[serde(default, skip_serializing_if = "is_false")]
    pub three_way: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_style: Option<String>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub conflict_only: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl InputConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| DiffCopyError::ConfigReadError {
            path: path.to_path_buf(),
            source: e,
        })?;

        toml::from_str(&content).map_err(|e| DiffCopyError::ConfigParseError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }
}

/// Merged configuration from all sources
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub source: PathBuf,
    pub target: PathBuf,
    pub output: PathBuf,
    pub exclude: Vec<String>,
    pub force: bool,
    pub verbose: bool,
    pub dry_run: bool,
    pub both_versions: bool,
    pub summary: Option<PathBuf>,
    pub check_permissions: CheckPermissionsMode,
    pub patch: bool,
    pub patch_file: Option<PathBuf>,
    pub excel: Option<PathBuf>,
    pub excel_fold_level: Option<usize>,
    pub show_unchanged: bool,
    pub filter_status: StatusFilter,
    pub stats_only: bool,
    pub no_tree: bool,
    pub no_details: bool,
    pub copy_deleted: bool,
    pub preserve_timestamps: bool,
    pub workers: usize,
    pub temp_dir: Option<PathBuf>,
    pub color: ColorMode,
    pub log_level: LogLevel,

    // Three-way options
    pub three_way: bool,
    pub base: Option<PathBuf>,
    pub merge_style: MergeStyle,
    pub conflict_only: bool,

    // Save config option
    pub save_config: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            target: PathBuf::new(),
            output: PathBuf::new(),
            exclude: Vec::new(),
            force: false,
            verbose: false,
            dry_run: false,
            both_versions: false,
            summary: None,
            check_permissions: CheckPermissionsMode::None,
            patch: false,
            patch_file: None,
            excel: None,
            excel_fold_level: None,
            show_unchanged: false,
            filter_status: StatusFilter::default(),
            stats_only: false,
            no_tree: false,
            no_details: false,
            copy_deleted: false,
            preserve_timestamps: false,
            workers: 1,
            temp_dir: None,
            color: ColorMode::Auto,
            log_level: LogLevel::Warn,
            three_way: false,
            base: None,
            merge_style: MergeStyle::All,
            conflict_only: false,
            save_config: None,
        }
    }
}

impl Config {
    /// Build config from CLI args, input config file, and app settings
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        // Load app settings from exe directory
        let app_settings = AppSettings::load_from_exe_dir().unwrap_or_default();

        // Load input config if specified
        let input_config = if let Some(config_path) = &cli.config {
            Some(InputConfig::load(config_path)?)
        } else {
            None
        };

        // Merge configurations with priority: CLI > InputConfig > AppSettings > Default

        // Required paths
        let source = cli
            .source
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.source.clone()))
            .ok_or_else(|| DiffCopyError::MissingConfigField("source".to_string()))?;

        let target = cli
            .target
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.target.clone()))
            .ok_or_else(|| DiffCopyError::MissingConfigField("target".to_string()))?;

        let output = cli
            .output
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.output.clone()))
            .ok_or_else(|| DiffCopyError::MissingConfigField("output".to_string()))?;

        // Exclude patterns: merge from all sources
        let mut exclude = Vec::new();
        if let Some(ref defaults) = app_settings.default_exclude {
            exclude.extend(defaults.clone());
        }
        if let Some(ref ic) = input_config {
            exclude.extend(ic.exclude.clone());
        }
        exclude.extend(cli.exclude.clone());

        // Boolean options
        let force = cli.force
            || input_config.as_ref().is_some_and(|c| c.force);
        let verbose = cli.verbose
            || input_config.as_ref().is_some_and(|c| c.verbose);
        let dry_run = cli.dry_run
            || input_config.as_ref().is_some_and(|c| c.dry_run);
        let both_versions = cli.both_versions
            || input_config.as_ref().is_some_and(|c| c.both_versions);
        let show_unchanged = cli.show_unchanged
            || input_config.as_ref().is_some_and(|c| c.show_unchanged);
        let stats_only = cli.stats_only
            || input_config.as_ref().is_some_and(|c| c.stats_only);
        let no_tree =
            cli.no_tree || input_config.as_ref().is_some_and(|c| c.no_tree);
        let no_details = cli.no_details
            || input_config.as_ref().is_some_and(|c| c.no_details);
        let copy_deleted = cli.copy_deleted
            || input_config.as_ref().is_some_and(|c| c.copy_deleted);
        let preserve_timestamps = cli.preserve_timestamps
            || input_config
                .as_ref()
                .is_some_and(|c| c.preserve_timestamps);
        let patch =
            cli.patch || input_config.as_ref().is_some_and(|c| c.patch);
        let three_way = cli.three_way
            || input_config.as_ref().is_some_and(|c| c.three_way);
        let conflict_only = cli.conflict_only
            || input_config.as_ref().is_some_and(|c| c.conflict_only);

        // Optional paths
        let summary = cli
            .summary
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.summary.clone()));
        let patch_file = cli
            .patch_file
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.patch_file.clone()));
        let excel = cli
            .excel
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.excel.clone()));
        let excel_fold_level = cli
            .excel_fold_level
            .or_else(|| input_config.as_ref().and_then(|c| c.excel_fold_level));
        let base = cli
            .base
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.base.clone()));
        let temp_dir = cli
            .temp_dir
            .clone()
            .or_else(|| input_config.as_ref().and_then(|c| c.temp_dir.clone()))
            .or(app_settings.temp_dir);

        // Check permissions mode
        let check_permissions = if cli.check_permissions != "none" {
            cli.get_check_permissions_mode()
                .ok_or_else(|| DiffCopyError::InvalidCheckPermissionsMode(cli.check_permissions.clone()))?
        } else if let Some(ref ic) = input_config {
            if let Some(ref cp) = ic.check_permissions {
                CheckPermissionsMode::from_str(cp)
                    .ok_or_else(|| DiffCopyError::InvalidCheckPermissionsMode(cp.clone()))?
            } else {
                CheckPermissionsMode::None
            }
        } else {
            CheckPermissionsMode::None
        };

        // Merge style
        let merge_style = if cli.merge_style != "all" {
            cli.get_merge_style()
                .ok_or_else(|| DiffCopyError::InvalidMergeStyle(cli.merge_style.clone()))?
        } else if let Some(ref ic) = input_config {
            if let Some(ref ms) = ic.merge_style {
                MergeStyle::from_str(ms)
                    .ok_or_else(|| DiffCopyError::InvalidMergeStyle(ms.clone()))?
            } else {
                MergeStyle::All
            }
        } else {
            MergeStyle::All
        };

        // Color mode
        let color = if cli.color != "auto" {
            cli.get_color_mode()
                .ok_or_else(|| DiffCopyError::InvalidColorMode(cli.color.clone()))?
        } else if let Some(ref ic) = input_config {
            if let Some(ref c) = ic.color {
                ColorMode::from_str(c)
                    .ok_or_else(|| DiffCopyError::InvalidColorMode(c.clone()))?
            } else if let Some(ref c) = app_settings.color {
                ColorMode::from_str(c)
                    .ok_or_else(|| DiffCopyError::InvalidColorMode(c.clone()))?
            } else {
                ColorMode::Auto
            }
        } else if let Some(ref c) = app_settings.color {
            ColorMode::from_str(c)
                .ok_or_else(|| DiffCopyError::InvalidColorMode(c.clone()))?
        } else {
            ColorMode::Auto
        };

        // Log level
        let log_level = if cli.log_level != "warn" {
            cli.get_log_level()
                .ok_or_else(|| DiffCopyError::InvalidLogLevel(cli.log_level.clone()))?
        } else if let Some(ref ic) = input_config {
            if let Some(ref ll) = ic.log_level {
                LogLevel::from_str(ll)
                    .ok_or_else(|| DiffCopyError::InvalidLogLevel(ll.clone()))?
            } else if let Some(ref ll) = app_settings.log_level {
                LogLevel::from_str(ll)
                    .ok_or_else(|| DiffCopyError::InvalidLogLevel(ll.clone()))?
            } else {
                LogLevel::Warn
            }
        } else if let Some(ref ll) = app_settings.log_level {
            LogLevel::from_str(ll)
                .ok_or_else(|| DiffCopyError::InvalidLogLevel(ll.clone()))?
        } else {
            LogLevel::Warn
        };

        // Workers
        let workers = cli
            .workers
            .or_else(|| input_config.as_ref().and_then(|c| c.workers))
            .or(app_settings.workers)
            .unwrap_or_else(num_cpus);

        // Filter status
        let filter_status = if !cli.filter_status.is_empty() {
            cli.parse_filter_status()?
        } else if let Some(ref ic) = input_config {
            if !ic.filter_status.is_empty() {
                parse_filter_status_vec(&ic.filter_status)?
            } else {
                StatusFilter::new()
            }
        } else {
            StatusFilter::new()
        };

        // Auto-enable show_unchanged if filter_status includes unchanged
        // This prevents empty results when filtering by unchanged without --show-unchanged
        let show_unchanged = show_unchanged
            || filter_status.included.contains("unchanged")
            || (filter_status.include_all && !filter_status.excluded.contains("unchanged"));

        Ok(Config {
            source,
            target,
            output,
            exclude,
            force,
            verbose,
            dry_run,
            both_versions,
            summary,
            check_permissions,
            patch,
            patch_file,
            excel,
            excel_fold_level,
            show_unchanged,
            filter_status,
            stats_only,
            no_tree,
            no_details,
            copy_deleted,
            preserve_timestamps,
            workers,
            temp_dir,
            color,
            log_level,
            three_way,
            base,
            merge_style,
            conflict_only,
            save_config: cli.save_config.clone(),
        })
    }

    /// Generate TOML config content for --save-config
    pub fn to_toml_string(&self) -> String {
        let mut lines = Vec::new();

        lines.push("# rs_diffcopy 設定ファイル".to_string());
        lines.push("# このファイルは --save-config オプションにより自動生成されました".to_string());
        lines.push("# 設定を変更して再利用することができます".to_string());
        lines.push(String::new());

        lines.push("# 必須設定".to_string());
        lines.push(format!(
            "source = \"{}\"  # 比較元ディレクトリ（変更する場合はパスを修正してください）",
            self.source.display()
        ));
        lines.push(format!(
            "target = \"{}\"  # 比較先ディレクトリ（変更する場合はパスを修正してください）",
            self.target.display()
        ));
        lines.push(format!(
            "output = \"{}\"  # 出力ディレクトリ（変更する場合はパスを修正してください）",
            self.output.display()
        ));
        lines.push(String::new());

        lines.push("# オプション設定".to_string());
        lines.push(format!("force = {}", self.force));
        lines.push(format!("verbose = {}", self.verbose));
        lines.push("# 注: dry_run はこの設定ファイルでは無効になっています".to_string());
        lines.push("# 必要に応じてコメントを外してください".to_string());
        lines.push(format!("# dry_run = {}", self.dry_run));
        lines.push(format!("both_versions = {}", self.both_versions));

        if let Some(ref summary) = self.summary {
            lines.push(format!("summary = \"{}\"", summary.display()));
        } else {
            lines.push("# summary = \"./summary.txt\"  # サマリー出力ファイル".to_string());
        }

        lines.push(format!(
            "check_permissions = \"{}\"  # none / scripts / all",
            self.check_permissions.as_str()
        ));
        lines.push(format!("patch = {}  # 個別パッチファイル生成", self.patch));

        if let Some(ref patch_file) = self.patch_file {
            lines.push(format!(
                "patch_file = \"{}\"  # 統合パッチファイル",
                patch_file.display()
            ));
        } else {
            lines.push("# patch_file = \"\"  # 統合パッチファイル（空欄で無効）".to_string());
        }

        if let Some(ref excel) = self.excel {
            lines.push(format!(
                "excel = \"{}\"  # Excel出力ファイル",
                excel.display()
            ));
        } else {
            lines.push("# excel = \"\"  # Excel出力ファイル（空欄で無効）".to_string());
        }

        if let Some(level) = self.excel_fold_level {
            lines.push(format!(
                "excel_fold_level = {}  # Excelファイルツリーの折りたたみレベル",
                level
            ));
        } else {
            lines.push(
                "# excel_fold_level = 2  # Excelファイルツリーの折りたたみレベル（省略時は折りたたみなし）"
                    .to_string(),
            );
        }

        lines.push(format!(
            "show_unchanged = {}  # 変更なしファイルをサマリーに表示",
            self.show_unchanged
        ));
        lines.push(String::new());

        lines.push("# 三者間比較オプション".to_string());
        lines.push(format!("three_way = {}  # 三者間比較モード", self.three_way));
        if let Some(ref base) = self.base {
            lines.push(format!(
                "base = \"{}\"  # 共通祖先ディレクトリ",
                base.display()
            ));
        } else {
            lines.push("# base = \"./base_version\"  # 共通祖先ディレクトリ".to_string());
        }
        lines.push(format!(
            "merge_style = \"{}\"  # all / ours / theirs",
            self.merge_style.as_str()
        ));
        lines.push(format!(
            "conflict_only = {}  # コンフリクトのみ出力",
            self.conflict_only
        ));
        lines.push(String::new());

        lines.push("# 出力フィルター設定".to_string());
        if !self.filter_status.is_empty() {
            let statuses: Vec<String> = self
                .filter_status
                .included
                .iter()
                .cloned()
                .collect();
            if !statuses.is_empty() {
                lines.push(format!(
                    "filter_status = {:?}  # 表示するステータス",
                    statuses
                ));
            }
        } else {
            lines.push(
                "# filter_status = [\"added\", \"modified\"]  # 表示するステータス（省略時は全て表示）"
                    .to_string(),
            );
        }
        lines.push(format!("stats_only = {}  # 統計情報のみ表示", self.stats_only));
        lines.push(format!("no_tree = {}  # File Treeセクション非表示", self.no_tree));
        lines.push(format!(
            "no_details = {}  # 詳細セクション非表示",
            self.no_details
        ));
        lines.push(String::new());

        lines.push("# コピーオプション".to_string());
        lines.push(format!(
            "copy_deleted = {}  # 削除ファイルもコピー",
            self.copy_deleted
        ));
        lines.push(format!(
            "preserve_timestamps = {}  # タイムスタンプを保持",
            self.preserve_timestamps
        ));
        lines.push(String::new());

        lines.push("# 除外パターン（glob形式、複数指定可）".to_string());
        if !self.exclude.is_empty() {
            lines.push("exclude = [".to_string());
            for pattern in &self.exclude {
                lines.push(format!("    \"{}\",", pattern));
            }
            lines.push("]".to_string());
        } else {
            lines.push("# exclude = [".to_string());
            lines.push("#     \"*.log\",".to_string());
            lines.push("#     \"*.tmp\",".to_string());
            lines.push("#     \"node_modules\",".to_string());
            lines.push("#     \".git\",".to_string());
            lines.push("# ]".to_string());
        }

        lines.join("\n")
    }
}

fn parse_filter_status_vec(statuses: &[String]) -> crate::error::Result<StatusFilter> {
    let mut filter = StatusFilter::new();

    if statuses.is_empty() {
        return Ok(filter);
    }

    // Check if first filter is exclusion
    if let Some(first) = statuses.first() {
        if first.starts_with('^') {
            filter.include_all = true;
        }
    }

    for status_str in statuses {
        let lowered = status_str.to_lowercase();

        if lowered == "all" {
            filter.include_all = true;
            continue;
        }

        // Get the value to validate (strip ^ prefix if present)
        let value_to_check = lowered.strip_prefix('^').unwrap_or(&lowered);

        // Validate the status value
        if !StatusFilter::is_valid_status(value_to_check) {
            return Err(crate::error::DiffCopyError::InvalidFilterStatus(
                status_str.clone(),
            ));
        }

        if let Some(stripped) = lowered.strip_prefix('^') {
            // Normalize alias to canonical name using FileStatus::from_str
            let canonical = if let Some(status) = crate::types::FileStatus::from_str(stripped) {
                status.as_str().to_string()
            } else {
                // Not a FileStatus alias, keep as-is (might be three-way status or group keyword)
                stripped.to_string()
            };
            // Always insert the canonical keyword (for two-way mode compatibility)
            filter.excluded.insert(canonical.clone());
            // Also expand group keywords for three-way mode
            if let Some(expanded) = StatusFilter::expand_three_way_group(&canonical) {
                for s in expanded {
                    filter.excluded.insert(s.to_string());
                }
            }
        } else {
            // Normalize alias to canonical name using FileStatus::from_str
            let canonical = if let Some(status) = crate::types::FileStatus::from_str(&lowered) {
                status.as_str().to_string()
            } else {
                // Not a FileStatus alias, keep as-is (might be three-way status or group keyword)
                lowered.clone()
            };
            // Always insert the canonical keyword (for two-way mode compatibility)
            filter.included.insert(canonical.clone());
            // Also expand group keywords for three-way mode
            if let Some(expanded) = StatusFilter::expand_three_way_group(&canonical) {
                for s in expanded {
                    filter.included.insert(s.to_string());
                }
            }
        }
    }

    Ok(filter)
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1)
}
