use chrono::Local;
use colored::Colorize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::types::{ColorMode, ThreeWayEntry, ThreeWayResult, ThreeWayStats, ThreeWayStatus};
use crate::utils::{display_width, is_terminal};

/// Summary writer for three-way comparison
pub struct ThreeWaySummaryWriter<'a> {
    config: &'a Config,
    use_color: bool,
}

impl<'a> ThreeWaySummaryWriter<'a> {
    pub fn new(config: &'a Config) -> Self {
        let use_color = match config.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_terminal(),
        };

        Self { config, use_color }
    }

    /// Write summary to console and optionally to file
    pub fn write(&self, result: &ThreeWayResult) -> crate::error::Result<()> {
        // Write to console (compact format)
        let console_output = self.generate_summary(result, true);
        println!("{}", console_output);

        // Write to file if specified (aligned format)
        if let Some(ref summary_path) = self.config.summary {
            let file_output = self.generate_summary(result, false);
            self.write_to_file(summary_path, &file_output)?;
        }

        Ok(())
    }

    fn generate_summary(&self, result: &ThreeWayResult, for_console: bool) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&self.generate_header());
        output.push('\n');

        // Options
        if self.has_options() {
            output.push_str(&self.generate_options());
            output.push('\n');
        }

        // Change Matrix
        output.push_str(&self.generate_change_matrix(&result.stats));
        output.push('\n');

        // Check if no differences
        if !result.stats.has_differences() {
            output.push_str("No differences found.\n");
            return output;
        }

        // File Tree (unless --stats-only or --no-tree)
        if !self.config.stats_only && !self.config.no_tree {
            output.push_str(&self.generate_file_tree(&result.entries, for_console));
            output.push('\n');
        }

        // Conflict Details (unless --stats-only or --no-details)
        if !self.config.stats_only && !self.config.no_details {
            let conflicts: Vec<_> = result
                .entries
                .iter()
                .filter(|e| e.status.is_conflict())
                .filter(|e| self.config.filter_status.matches_three_way(e.status))
                .collect();
            if !conflicts.is_empty() {
                output.push_str(&self.generate_conflict_details(&conflicts));
            }
        }

        output
    }

    fn generate_header(&self) -> String {
        let mut output = String::new();

        output.push_str("rs_diffcopy Summary (Three-way)\n");
        output.push_str("===============================\n");

        let base_path = self.config.base.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        output.push_str(&format!("Base:   {}\n", base_path));
        output.push_str(&format!("Ours:   {}\n", self.config.source.display()));
        output.push_str(&format!("Theirs: {}\n", self.config.target.display()));
        output.push_str(&format!("Output: {}\n", self.config.output.display()));
        output.push_str(&format!("Date:   {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));

        output
    }

    fn has_options(&self) -> bool {
        self.config.dry_run
            || self.config.merge_style != crate::types::MergeStyle::All
            || self.config.conflict_only
            || !self.config.filter_status.is_empty()
            || !self.config.exclude.is_empty()
    }

    fn generate_options(&self) -> String {
        let mut output = String::new();

        output.push_str("\nOptions:\n");

        if self.config.dry_run {
            output.push_str("  Mode: Dry run (no files copied)\n");
        }

        output.push_str(&format!("  Merge style: {}\n", self.config.merge_style.as_str()));

        if self.config.conflict_only {
            output.push_str("  Conflict only: Yes\n");
        }

        if !self.config.filter_status.is_empty() {
            let filter_str = self.config.filter_status.to_display_string();
            output.push_str(&format!("  Filter status: {}\n", filter_str));
        }

        if !self.config.exclude.is_empty() {
            output.push_str("  Exclude patterns:\n");
            for pattern in &self.config.exclude {
                output.push_str(&format!("    - {}\n", pattern));
            }
        }

        output
    }

    fn generate_change_matrix(&self, stats: &ThreeWayStats) -> String {
        let mut output = String::new();

        output.push_str("================\nChange Matrix\n================\n");
        output.push_str("Status          | Count\n");
        output.push_str("----------------|------\n");

        let stat_lines = [
            ("Unchanged", stats.unchanged),
            ("Ours only", stats.ours_only),
            ("Theirs only", stats.theirs_only),
            ("Both same", stats.both_same),
            ("Conflict", stats.conflict),
            ("Added ours", stats.added_ours),
            ("Added theirs", stats.added_theirs),
            ("Added both same", stats.added_both_same),
            ("Added both diff", stats.added_both_diff),
            ("Deleted ours", stats.deleted_ours),
            ("Deleted theirs", stats.deleted_theirs),
            ("Deleted both", stats.deleted_both),
            ("Modify/Delete", stats.modify_delete),
            ("Delete/Modify", stats.delete_modify),
        ];

        for (label, count) in stat_lines {
            if count > 0 || label == "Unchanged" {
                output.push_str(&format!("{:<16}| {:>5}\n", label, count));
            }
        }

        output.push_str("--------------------------\n");
        output.push_str(&format!("Total           | {:>5}\n", stats.total_items));
        output.push_str(&format!("Conflicts       | {:>5}\n", stats.total_conflicts()));

        output
    }

    fn generate_file_tree(&self, entries: &[ThreeWayEntry], for_console: bool) -> String {
        let mut output = String::new();

        output.push_str("================\nFile Tree\n================\n");
        output.push_str("Legend: [Base|Ours|Theirs] ");
        output.push_str("○=exists -=missing ==same M=modified A=added D=deleted\n");

        // Build tree
        let tree = self.build_tree(entries);

        // Calculate max path width for file output alignment
        let max_width = if for_console {
            0 // Not used for console
        } else {
            self.calculate_max_path_width(&tree, "", true)
        };

        if !for_console {
            // Add header for aligned format with dynamic width
            let header_padding = if max_width > 0 { max_width } else { 40 };
            output.push_str(&format!("{:width$}B  O  T\n", "", width = header_padding));
        }

        output.push_str(".\n");

        // Render tree with max_width
        output.push_str(&self.render_tree(&tree, "", true, for_console, max_width));

        output
    }

    /// Calculate the maximum display width of all paths in the tree (for file entries only)
    fn calculate_max_path_width(
        &self,
        tree: &BTreeMap<String, ThreeWayTreeNode>,
        prefix: &str,
        is_root: bool,
    ) -> usize {
        let mut max_width: usize = 0;
        let items: Vec<_> = tree.iter().collect();
        let count = items.len();

        for (i, (_, node)) in items.iter().enumerate() {
            let is_last = i == count - 1;
            let connector = if is_root {
                ""
            } else if is_last {
                "└── "
            } else {
                "├── "
            };

            // new_prefix width must match: │(2) + 3 spaces = 5, so use 5 spaces when is_last
            let new_prefix = if is_root {
                prefix.to_string()
            } else if is_last {
                format!("{}     ", prefix)  // 5 spaces to match │(2)+3 = 5 width
            } else {
                format!("{}│   ", prefix)   // │(2) + 3 spaces = 5 width
            };

            // Only count files with entries (not directory-only nodes)
            if node.entry.is_some() {
                let name_with_dir = if node.entry.as_ref().is_some_and(|e| e.is_directory) {
                    format!("{}/", node.name)
                } else {
                    node.name.clone()
                };
                let path_display = format!("{}{}{}", prefix, connector, name_with_dir);
                let width = display_width(&path_display);
                if width > max_width {
                    max_width = width;
                }
            }

            if !node.children.is_empty() {
                let child_max = self.calculate_max_path_width(&node.children, &new_prefix, false);
                if child_max > max_width {
                    max_width = child_max;
                }
            }
        }

        max_width
    }

    fn build_tree(&self, entries: &[ThreeWayEntry]) -> BTreeMap<String, ThreeWayTreeNode> {
        let mut root: BTreeMap<String, ThreeWayTreeNode> = BTreeMap::new();

        let filtered_entries: Vec<&ThreeWayEntry> = entries
            .iter()
            .filter(|e| self.config.filter_status.matches_three_way(e.status))
            .collect();

        for entry in filtered_entries {
            let path_str = entry.relative_path.to_string_lossy().to_string();
            let parts: Vec<&str> = path_str
                .split('/')
                .filter(|s| !s.is_empty())
                .collect();

            self.insert_into_tree(&mut root, &parts, entry);
        }

        root
    }

    fn insert_into_tree(
        &self,
        tree: &mut BTreeMap<String, ThreeWayTreeNode>,
        parts: &[&str],
        entry: &ThreeWayEntry,
    ) {
        if parts.is_empty() {
            return;
        }

        let name = parts[0].to_string();
        let remaining = &parts[1..];

        let node = tree.entry(name.clone()).or_insert_with(|| ThreeWayTreeNode {
            name: name.clone(),
            entry: None,
            children: BTreeMap::new(),
        });

        if remaining.is_empty() {
            node.entry = Some(entry.clone());
        } else {
            self.insert_into_tree(&mut node.children, remaining, entry);
        }
    }

    fn render_tree(
        &self,
        tree: &BTreeMap<String, ThreeWayTreeNode>,
        prefix: &str,
        is_root: bool,
        for_console: bool,
        max_width: usize,
    ) -> String {
        let mut output = String::new();
        let items: Vec<_> = tree.iter().collect();
        let count = items.len();

        for (i, (_, node)) in items.iter().enumerate() {
            let is_last = i == count - 1;
            let connector = if is_root {
                ""
            } else if is_last {
                "└── "
            } else {
                "├── "
            };

            // new_prefix width must match: │(2) + 3 spaces = 5, so use 5 spaces when is_last
            let new_prefix = if is_root {
                prefix.to_string()
            } else if is_last {
                format!("{}     ", prefix)  // 5 spaces to match │(2)+3 = 5 width
            } else {
                format!("{}│   ", prefix)   // │(2) + 3 spaces = 5 width
            };

            let (indicator, status_label) = if let Some(ref entry) = node.entry {
                self.format_indicator(entry, for_console)
            } else if !node.children.is_empty() {
                (String::new(), String::new())
            } else {
                (String::new(), String::new())
            };

            let name_with_dir = if node.entry.as_ref().is_some_and(|e| e.is_directory) {
                format!("{}/", node.name)
            } else {
                node.name.clone()
            };

            if for_console {
                // Compact format: [○M=] status
                output.push_str(&format!(
                    "{}{}{} {} {}\n",
                    prefix, connector, name_with_dir, indicator, status_label
                ));
            } else {
                // Aligned format using dynamic max_width
                let path_display = format!("{}{}{}", prefix, connector, name_with_dir);
                if node.entry.is_some() && max_width > 0 {
                    let path_width = display_width(&path_display);
                    let padding = if path_width < max_width { max_width - path_width } else { 0 };
                    output.push_str(&format!(
                        "{}{} {} {}\n",
                        path_display,
                        " ".repeat(padding),
                        indicator,
                        status_label
                    ));
                } else {
                    // Directory nodes or no max_width
                    output.push_str(&format!("{}\n", path_display));
                }
            }

            if !node.children.is_empty() {
                output.push_str(&self.render_tree(&node.children, &new_prefix, false, for_console, max_width));
            }
        }

        output
    }

    fn format_indicator(&self, entry: &ThreeWayEntry, for_console: bool) -> (String, String) {
        let base_char = if entry.base_exists { "○" } else { "-" };
        let ours_char = self.get_change_char(entry.base_exists, entry.ours_exists, &entry.base_hash, &entry.ours_hash);
        let theirs_char = self.get_change_char(entry.base_exists, entry.theirs_exists, &entry.base_hash, &entry.theirs_hash);

        let indicator = if for_console {
            format!("[{}{}{}]", base_char, ours_char, theirs_char)
        } else {
            format!("[{}  {}  {}]", base_char, ours_char, theirs_char)
        };

        let status_label = entry.status.as_str().to_string();

        let colored_indicator = if for_console && self.use_color {
            if entry.status.is_conflict() {
                indicator.red().bold().to_string()
            } else {
                indicator.clone()
            }
        } else {
            indicator
        };

        let colored_label = if for_console && self.use_color {
            match entry.status {
                ThreeWayStatus::Conflict
                | ThreeWayStatus::AddedBothDiff
                | ThreeWayStatus::ModifyDelete
                | ThreeWayStatus::DeleteModify => status_label.red().bold().to_string(),
                ThreeWayStatus::OursOnly | ThreeWayStatus::AddedOurs => status_label.green().to_string(),
                ThreeWayStatus::TheirsOnly | ThreeWayStatus::AddedTheirs => status_label.blue().to_string(),
                ThreeWayStatus::BothSame | ThreeWayStatus::AddedBothSame => status_label.cyan().to_string(),
                ThreeWayStatus::Unchanged => status_label.dimmed().to_string(),
                _ => status_label,
            }
        } else {
            status_label
        };

        (colored_indicator, colored_label)
    }

    fn get_change_char(
        &self,
        base_exists: bool,
        side_exists: bool,
        base_hash: &Option<String>,
        side_hash: &Option<String>,
    ) -> &'static str {
        match (base_exists, side_exists) {
            (true, true) => {
                if base_hash == side_hash {
                    "="
                } else {
                    "M"
                }
            }
            (true, false) => "D",
            (false, true) => "A",
            (false, false) => "-",
        }
    }

    fn generate_conflict_details(&self, conflicts: &[&ThreeWayEntry]) -> String {
        let mut output = String::new();

        output.push_str("================\nConflict Details\n================\n");

        for (i, entry) in conflicts.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, entry.relative_path.display()));
            output.push_str(&format!("   Type: {}\n", self.get_conflict_type(entry)));

            if entry.base_exists {
                let size = entry.base_size.map(|s| format!("{} bytes", s)).unwrap_or_default();
                let hash = entry.base_hash.as_ref().map(|h| &h[..8]).unwrap_or("?");
                output.push_str(&format!("   Base:   {}... ({})\n", hash, size));
            }

            if entry.ours_exists {
                let size = entry.ours_size.map(|s| format!("{} bytes", s)).unwrap_or_default();
                let hash = entry.ours_hash.as_ref().map(|h| &h[..8]).unwrap_or("?");
                output.push_str(&format!("   Ours:   {}... ({})\n", hash, size));
            } else {
                output.push_str("   Ours:   Deleted\n");
            }

            if entry.theirs_exists {
                let size = entry.theirs_size.map(|s| format!("{} bytes", s)).unwrap_or_default();
                let hash = entry.theirs_hash.as_ref().map(|h| &h[..8]).unwrap_or("?");
                output.push_str(&format!("   Theirs: {}... ({})\n", hash, size));
            } else {
                output.push_str("   Theirs: Deleted\n");
            }

            output.push('\n');
        }

        output
    }

    fn get_conflict_type(&self, entry: &ThreeWayEntry) -> &'static str {
        match entry.status {
            ThreeWayStatus::Conflict => "Content conflict",
            ThreeWayStatus::AddedBothDiff => "Add/Add conflict (different content)",
            ThreeWayStatus::ModifyDelete => "Modify/Delete conflict",
            ThreeWayStatus::DeleteModify => "Delete/Modify conflict",
            _ => "Unknown",
        }
    }

    fn write_to_file(&self, path: &Path, content: &str) -> crate::error::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::error::DiffCopyError::DirectoryCreateError(parent.to_path_buf(), e.to_string())
            })?;
        }

        let mut file = File::create(path).map_err(|e| crate::error::DiffCopyError::FileWriteError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        file.write_all(content.as_bytes())
            .map_err(|e| crate::error::DiffCopyError::FileWriteError {
                path: path.to_path_buf(),
                message: e.to_string(),
            })?;

        Ok(())
    }
}

/// Helper struct for tree building
struct ThreeWayTreeNode {
    name: String,
    entry: Option<ThreeWayEntry>,
    children: BTreeMap<String, ThreeWayTreeNode>,
}
