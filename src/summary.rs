use chrono::Local;
use colored::Colorize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::types::{
    ColorMode, ComparisonResult, ComparisonStats, FileEntry, FileStatus, PatchResult,
    PermissionChange, SpecialFileType, StatusFilter,
};
use crate::utils::{display_width, is_terminal, println_cp932};

/// Format StatusFilter for display in Options section
fn format_filter_status(filter: &StatusFilter) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Add "all" if include_all is set, or if only exclusions exist
    if filter.include_all {
        parts.push("all".to_string());
    } else if !filter.included.is_empty() {
        // Add included statuses
        let mut included: Vec<&str> = filter.included.iter().map(|s| s.as_str()).collect();
        included.sort();
        parts.extend(included.into_iter().map(|s| s.to_string()));
    } else if !filter.excluded.is_empty() {
        // Only exclusions exist, imply "all"
        parts.push("all (implied)".to_string());
    }

    // Add excluded statuses with ^ prefix
    if !filter.excluded.is_empty() {
        let mut excluded: Vec<&str> = filter.excluded.iter().map(|s| s.as_str()).collect();
        excluded.sort();
        for ex in excluded {
            parts.push(format!("^{}", ex));
        }
    }

    parts.join(", ")
}

/// Summary writer for console and file output
pub struct SummaryWriter<'a> {
    config: &'a Config,
    use_color: bool,
}

impl<'a> SummaryWriter<'a> {
    pub fn new(config: &'a Config) -> Self {
        let use_color = match config.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_terminal(),
        };

        Self { config, use_color }
    }

    /// Write summary to console and optionally to file
    pub fn write(&self, result: &ComparisonResult) -> crate::error::Result<()> {
        // Write to console (Windows: CP932, others: UTF-8)
        let console_output = self.generate_summary(result, true);
        println_cp932(&console_output);

        // Write to file if specified
        if let Some(ref summary_path) = self.config.summary {
            let file_output = self.generate_summary(result, false);
            self.write_to_file(summary_path, &file_output)?;
        }

        Ok(())
    }

    fn generate_summary(&self, result: &ComparisonResult, for_console: bool) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&self.generate_header());
        output.push('\n');

        // Options
        if self.has_options() {
            output.push_str(&self.generate_options());
            output.push('\n');
        }

        // Statistics
        output.push_str(&self.generate_statistics(&result.stats, for_console));
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

        // Details sections (unless --stats-only or --no-details)
        if !self.config.stats_only && !self.config.no_details {
            output.push_str(&self.generate_details(&result.entries, for_console));

            // Permission Changes
            let perm_changes = self.collect_permission_changes(&result.entries);
            if !perm_changes.is_empty() {
                output.push_str(&self.generate_permission_changes(&perm_changes));
            }

            // Errors
            let errors = self.collect_errors(&result.entries);
            if !errors.is_empty() {
                output.push_str(&self.generate_errors(&errors));
            }

            // Special Files
            let special_files = self.collect_special_files(&result.entries);
            if !special_files.is_empty() {
                output.push_str(&self.generate_special_files(&special_files));
            }

            // Patch Details
            if !result.patch_results.is_empty() {
                output.push_str(&self.generate_patch_details(&result.patch_results));
            }

            // Copy Failed
            let failed_copies = self.collect_failed_copies(&result.copy_results);
            if !failed_copies.is_empty() {
                output.push_str(&self.generate_copy_failed(&failed_copies, &result.copy_results));
            }
        }

        output
    }

    fn generate_header(&self) -> String {
        let mut output = String::new();

        output.push_str("rs_diffcopy Summary\n");
        output.push_str("===================\n");
        output.push_str(&format!("Source: {}\n", self.config.source.display()));
        output.push_str(&format!("Target: {}\n", self.config.target.display()));
        output.push_str(&format!("Output: {}\n", self.config.output.display()));
        output.push_str(&format!("Date: {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));

        output
    }

    fn has_options(&self) -> bool {
        self.config.dry_run
            || self.config.both_versions
            || self.config.copy_deleted
            || self.config.preserve_timestamps
            || self.config.check_permissions != crate::types::CheckPermissionsMode::None
            || self.config.patch
            || self.config.patch_file.is_some()
            || !self.config.exclude.is_empty()
            || !self.config.filter_status.is_empty()
    }

    fn generate_options(&self) -> String {
        let mut output = String::new();

        output.push_str("\nOptions:\n");

        if self.config.dry_run {
            output.push_str("  Mode: Dry run (no files copied)\n");
        }

        if self.config.both_versions {
            output.push_str("  Copy mode: Both versions (.old/.new)\n");
        }

        if self.config.copy_deleted {
            output.push_str("  Copy deleted: Yes (.deleted)\n");
        }

        if self.config.preserve_timestamps {
            output.push_str("  Preserve timestamps: Yes\n");
        }

        if self.config.check_permissions != crate::types::CheckPermissionsMode::None {
            output.push_str(&format!(
                "  Permission check: {}\n",
                self.config.check_permissions.as_str()
            ));
        }

        if self.config.patch {
            output.push_str("  Patch mode: Individual files (.patch)\n");
        }

        if let Some(ref patch_file) = self.config.patch_file {
            output.push_str(&format!(
                "  Combined patch file: {}\n",
                patch_file.display()
            ));
        }

        if !self.config.filter_status.is_empty() {
            let filter_str = format_filter_status(&self.config.filter_status);
            if !filter_str.is_empty() {
                output.push_str(&format!("  Filter status: {}\n", filter_str));
            }
        }

        if !self.config.exclude.is_empty() {
            output.push_str("  Exclude patterns:\n");
            for pattern in &self.config.exclude {
                output.push_str(&format!("    - {}\n", pattern));
            }
        }

        output
    }

    fn generate_statistics(&self, stats: &ComparisonStats, for_console: bool) -> String {
        let mut output = String::new();

        let filter = &self.config.filter_status;
        let is_filtered = !filter.is_empty();

        // Added
        output.push_str(&self.format_stat_line(
            "Added",
            stats.added_files,
            stats.added_dirs,
            filter.matches(FileStatus::Added),
            for_console,
            is_filtered,
        ));

        // Modified
        output.push_str(&self.format_stat_line_simple(
            "Modified",
            stats.modified_files,
            "files",
            filter.matches(FileStatus::Modified),
            for_console,
            is_filtered,
        ));

        // Deleted
        output.push_str(&self.format_stat_line(
            "Deleted",
            stats.deleted_files,
            stats.deleted_dirs,
            filter.matches(FileStatus::Deleted),
            for_console,
            is_filtered,
        ));

        // Symlinks
        if stats.symlink_files > 0 {
            output.push_str(&self.format_stat_line_simple(
                "Symlinks",
                stats.symlink_files,
                "files",
                filter.matches(FileStatus::Symlink),
                for_console,
                is_filtered,
            ));
        }

        // Special Files
        if stats.special_files > 0 {
            output.push_str(&self.format_stat_line_simple(
                "Special Files",
                stats.special_files,
                "file",
                filter.matches(FileStatus::Special),
                for_console,
                is_filtered,
            ));
        }

        // Permissions
        if stats.permission_files > 0 {
            output.push_str(&self.format_stat_line_simple(
                "Permissions",
                stats.permission_files,
                "files",
                filter.matches(FileStatus::Permission),
                for_console,
                is_filtered,
            ));
        }

        // Errors
        if stats.error_files > 0 {
            output.push_str(&self.format_stat_line_simple(
                "Errors",
                stats.error_files,
                "file",
                filter.matches(FileStatus::Error),
                for_console,
                is_filtered,
            ));
        }

        // Unchanged
        output.push_str(&self.format_stat_line_simple(
            "Unchanged",
            stats.unchanged_files,
            "files",
            filter.matches(FileStatus::Unchanged),
            for_console,
            is_filtered,
        ));

        // Separator
        output.push_str("--------------------------\n");

        // Total
        output.push_str(&format!("Total:        {} items\n", stats.total_items));

        // Filtered count if applicable
        if is_filtered {
            let shown_count = self.count_filtered_items(stats, filter);
            output.push_str(&format!("Showing:      {} items (filtered)\n", shown_count));
        }

        output
    }

    fn format_stat_line(
        &self,
        label: &str,
        files: usize,
        dirs: usize,
        included: bool,
        _for_console: bool,
        is_filtered: bool,
    ) -> String {
        let value = if dirs > 0 {
            format!("{} files, {} dir", files, dirs)
        } else {
            format!("{} files", files)
        };

        let filtered_marker = if is_filtered && !included {
            "    (filtered out)"
        } else {
            ""
        };

        format!("{:<14}{}{}\n", format!("{}:", label), value, filtered_marker)
    }

    fn format_stat_line_simple(
        &self,
        label: &str,
        count: usize,
        unit: &str,
        included: bool,
        _for_console: bool,
        is_filtered: bool,
    ) -> String {
        let filtered_marker = if is_filtered && !included {
            "    (filtered out)"
        } else {
            ""
        };

        format!(
            "{:<14}{} {}{}\n",
            format!("{}:", label),
            count,
            unit,
            filtered_marker
        )
    }

    fn count_filtered_items(&self, stats: &ComparisonStats, filter: &StatusFilter) -> usize {
        let mut count = 0;
        if filter.matches(FileStatus::Added) {
            count += stats.added_files + stats.added_dirs;
        }
        if filter.matches(FileStatus::Modified) {
            count += stats.modified_files;
        }
        if filter.matches(FileStatus::Deleted) {
            count += stats.deleted_files + stats.deleted_dirs;
        }
        if filter.matches(FileStatus::Unchanged) {
            count += stats.unchanged_files;
        }
        if filter.matches(FileStatus::Symlink) {
            count += stats.symlink_files;
        }
        if filter.matches(FileStatus::Special) {
            count += stats.special_files;
        }
        if filter.matches(FileStatus::Permission) {
            count += stats.permission_files;
        }
        if filter.matches(FileStatus::Error) {
            count += stats.error_files;
        }
        count
    }

    fn generate_file_tree(&self, entries: &[FileEntry], for_console: bool) -> String {
        let mut output = String::new();

        let is_filtered = !self.config.filter_status.is_empty();
        let header = if is_filtered {
            "File Tree (filtered)"
        } else {
            "File Tree"
        };

        output.push_str(&format!("================\n{}\n================\n", header));
        output.push_str(".\n");

        // Build tree structure
        let tree = self.build_tree(entries);

        // Calculate max path width for file output alignment
        let max_width = if for_console {
            0 // Not used for console
        } else {
            self.calculate_max_path_width(&tree, "", true)
        };

        output.push_str(&self.render_tree(&tree, "", true, for_console, max_width));

        output
    }

    /// Calculate the maximum display width of all paths in the tree (for file entries only)
    fn calculate_max_path_width(
        &self,
        tree: &BTreeMap<String, TreeNode>,
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

    fn build_tree(&self, entries: &[FileEntry]) -> BTreeMap<String, TreeNode> {
        let mut root: BTreeMap<String, TreeNode> = BTreeMap::new();

        let filtered_entries: Vec<&FileEntry> = entries
            .iter()
            .filter(|e| self.config.filter_status.matches(e.status))
            .collect();

        for entry in filtered_entries {
            // Use components() for cross-platform path handling (works with both / and \)
            let parts: Vec<String> = entry
                .relative_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();

            self.insert_into_tree(&mut root, &parts, entry);
        }

        root
    }

    fn insert_into_tree(
        &self,
        tree: &mut BTreeMap<String, TreeNode>,
        parts: &[String],
        entry: &FileEntry,
    ) {
        if parts.is_empty() {
            return;
        }

        let name = parts[0].clone();
        let remaining = &parts[1..];

        let node = tree.entry(name.clone()).or_insert_with(|| TreeNode {
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
        tree: &BTreeMap<String, TreeNode>,
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

            let status_tag = if let Some(ref entry) = node.entry {
                self.format_status_tag(entry, for_console)
            } else if !node.children.is_empty() {
                "/".to_string()
            } else {
                String::new()
            };

            let name_with_dir = if node.entry.as_ref().is_some_and(|e| e.is_directory) {
                format!("{}/", node.name)
            } else {
                node.name.clone()
            };

            if for_console {
                // Compact format for console
                output.push_str(&format!("{}{}{} {}\n", prefix, connector, name_with_dir, status_tag));
            } else {
                // Aligned format for file output
                let path_display = format!("{}{}{}", prefix, connector, name_with_dir);
                if node.entry.is_some() && max_width > 0 {
                    let path_width = display_width(&path_display);
                    let padding = if path_width < max_width { max_width - path_width } else { 0 };
                    output.push_str(&format!("{}{} {}\n", path_display, " ".repeat(padding), status_tag));
                } else {
                    // Directory nodes or no max_width
                    output.push_str(&format!("{} {}\n", path_display, status_tag));
                }
            }

            if !node.children.is_empty() {
                output.push_str(&self.render_tree(&node.children, &new_prefix, false, for_console, max_width));
            }
        }

        output
    }

    fn format_status_tag(&self, entry: &FileEntry, for_console: bool) -> String {
        let tag = match entry.status {
            FileStatus::Added => "[added]",
            FileStatus::Modified => "[modified]",
            FileStatus::Deleted => "[deleted]",
            FileStatus::Unchanged => "[unchanged]",
            FileStatus::Symlink => {
                if let Some(ref info) = entry.symlink_info {
                    if info.is_broken {
                        "[symlink: broken]"
                    } else {
                        "[symlink]"
                    }
                } else {
                    "[symlink]"
                }
            }
            FileStatus::Special => {
                if let Some(ref st) = entry.special_type {
                    match st {
                        SpecialFileType::Socket => "[special: socket]",
                        SpecialFileType::Fifo => "[special: fifo]",
                        SpecialFileType::BlockDevice => "[special: block device]",
                        SpecialFileType::CharDevice => "[special: char device]",
                    }
                } else {
                    "[special]"
                }
            }
            FileStatus::Permission => "[permission]",
            FileStatus::Error => "[permission denied]",
        };

        if for_console && self.use_color {
            match entry.status {
                FileStatus::Added => tag.green().to_string(),
                FileStatus::Modified => tag.yellow().to_string(),
                FileStatus::Deleted => tag.red().to_string(),
                FileStatus::Unchanged => tag.dimmed().to_string(),
                FileStatus::Symlink => tag.magenta().to_string(),
                FileStatus::Special => tag.cyan().to_string(),
                FileStatus::Permission => tag.yellow().to_string(),
                FileStatus::Error => tag.red().to_string(),
            }
        } else {
            tag.to_string()
        }
    }

    fn generate_details(&self, entries: &[FileEntry], for_console: bool) -> String {
        let mut output = String::new();

        // Added Files
        let added: Vec<_> = entries
            .iter()
            .filter(|e| e.status == FileStatus::Added && self.config.filter_status.matches(e.status))
            .collect();
        if !added.is_empty() {
            output.push_str(&self.generate_section("Added Files", &added, for_console));
        }

        // Modified Files
        let modified: Vec<_> = entries
            .iter()
            .filter(|e| e.status == FileStatus::Modified && self.config.filter_status.matches(e.status))
            .collect();
        if !modified.is_empty() {
            output.push_str(&self.generate_section("Modified Files", &modified, for_console));
        }

        // Deleted Files
        let deleted: Vec<_> = entries
            .iter()
            .filter(|e| e.status == FileStatus::Deleted && self.config.filter_status.matches(e.status))
            .collect();
        if !deleted.is_empty() {
            output.push_str(&self.generate_section("Deleted Files", &deleted, for_console));
        }

        // Unchanged Files (if show_unchanged)
        if self.config.show_unchanged {
            let unchanged: Vec<_> = entries
                .iter()
                .filter(|e| e.status == FileStatus::Unchanged && self.config.filter_status.matches(e.status))
                .collect();
            if !unchanged.is_empty() {
                output.push_str(&self.generate_section("Unchanged Files", &unchanged, for_console));
            }
        }

        // Symlink Details
        let symlinks: Vec<_> = entries
            .iter()
            .filter(|e| e.status == FileStatus::Symlink && self.config.filter_status.matches(e.status))
            .collect();
        if !symlinks.is_empty() {
            output.push_str(&self.generate_symlink_details(&symlinks));
        }

        output
    }

    fn generate_section(&self, title: &str, entries: &[&FileEntry], _for_console: bool) -> String {
        let mut output = String::new();

        let is_filtered = !self.config.filter_status.is_empty();
        let header = if is_filtered {
            format!("{} (filtered)", title)
        } else {
            title.to_string()
        };

        output.push_str(&format!("================\n{}\n================\n", header));

        // Separate directories and files
        let dirs: Vec<_> = entries.iter().filter(|e| e.is_directory).copied().collect();
        let files: Vec<_> = entries.iter().filter(|e| !e.is_directory).copied().collect();

        if !dirs.is_empty() {
            output.push_str("Directories:\n");
            for entry in &dirs {
                output.push_str(&format!("  {}/\n", entry.relative_path.display()));
            }
        }

        if !files.is_empty() {
            if !dirs.is_empty() {
                output.push_str("\nFiles:\n");
            } else {
                output.push_str("Files:\n");
            }
            for entry in &files {
                output.push_str(&format!("  {}\n", entry.relative_path.display()));
            }
        }

        output.push('\n');
        output
    }

    fn generate_symlink_details(&self, entries: &[&FileEntry]) -> String {
        let mut output = String::new();

        output.push_str("================\nSymlink Details\n================\n");

        for entry in entries {
            if let Some(ref info) = entry.symlink_info {
                output.push_str(&format!(
                    "  {} -> {}\n",
                    entry.relative_path.display(),
                    info.target.display()
                ));
                let type_str = if info.is_directory { "directory" } else { "file" };
                let status_str = if info.is_broken { "BROKEN" } else { "OK" };
                output.push_str(&format!("    Type: {} | Status: {}\n", type_str, status_str));
            } else {
                // symlink_info が取得できなかった場合もパスを表示
                output.push_str(&format!(
                    "  {} (symlink info unavailable)\n",
                    entry.relative_path.display()
                ));
            }
        }

        output.push('\n');
        output
    }

    fn collect_permission_changes<'b>(&self, entries: &'b [FileEntry]) -> Vec<&'b PermissionChange> {
        entries
            .iter()
            .filter_map(|e| e.permission_change.as_ref())
            .collect()
    }

    fn generate_permission_changes(&self, changes: &[&PermissionChange]) -> String {
        let mut output = String::new();

        output.push_str("================\nPermission Changes\n================\n");

        for change in changes {
            output.push_str(&format!(
                "{}: {} -> {}\n",
                change.path.display(),
                change.old_mode,
                change.new_mode
            ));
        }

        output.push('\n');
        output
    }

    fn collect_errors<'b>(&self, entries: &'b [FileEntry]) -> Vec<&'b FileEntry> {
        entries
            .iter()
            .filter(|e| e.status == FileStatus::Error)
            .collect()
    }

    fn generate_errors(&self, entries: &[&FileEntry]) -> String {
        let mut output = String::new();

        output.push_str("================\nErrors\n================\n");

        for entry in entries {
            let msg = entry.error_message.as_deref().unwrap_or("Unknown error");
            output.push_str(&format!("{}: {}\n", entry.relative_path.display(), msg));
        }

        output.push('\n');
        output
    }

    fn collect_special_files<'b>(&self, entries: &'b [FileEntry]) -> Vec<&'b FileEntry> {
        entries
            .iter()
            .filter(|e| e.status == FileStatus::Special)
            .collect()
    }

    fn generate_special_files(&self, entries: &[&FileEntry]) -> String {
        let mut output = String::new();

        output.push_str("================\nSpecial Files (skipped)\n================\n");

        for entry in entries {
            let type_str = entry
                .special_type
                .as_ref()
                .map(|t| t.as_str())
                .unwrap_or("unknown");
            output.push_str(&format!(
                "{} ({})\n",
                entry.relative_path.display(),
                type_str
            ));
        }

        output.push('\n');
        output
    }

    fn generate_patch_details(&self, results: &[PatchResult]) -> String {
        let mut output = String::new();

        let generated: Vec<_> = results.iter().filter(|r| r.generated).collect();
        let skipped_binary: Vec<_> = results.iter().filter(|r| r.skipped_binary).collect();
        let failed: Vec<_> = results.iter().filter(|r| r.error_message.is_some()).collect();

        output.push_str("================\nPatch Details\n================\n");
        output.push_str(&format!(
            "Generated: {} patches, Skipped: {} (binary), Failed: {}\n\n",
            generated.len(),
            skipped_binary.len(),
            failed.len()
        ));

        if !generated.is_empty() {
            output.push_str("Generated:\n");
            for result in &generated {
                output.push_str(&format!("  {}.patch\n", result.path.display()));
            }
            output.push('\n');
        }

        if !skipped_binary.is_empty() {
            output.push_str("Skipped (binary):\n");
            for result in &skipped_binary {
                output.push_str(&format!("  {} [skip]\n", result.path.display()));
            }
            output.push('\n');
        }

        if !failed.is_empty() {
            output.push_str("Failed:\n");
            for result in &failed {
                let msg = result.error_message.as_deref().unwrap_or("Unknown error");
                output.push_str(&format!("  {}: {}\n", result.path.display(), msg));
            }
            output.push('\n');
        }

        output
    }

    fn collect_failed_copies<'b>(
        &self,
        results: &'b [crate::types::CopyResult],
    ) -> Vec<&'b crate::types::CopyResult> {
        results.iter().filter(|r| !r.success).collect()
    }

    fn generate_copy_failed(
        &self,
        failed: &[&crate::types::CopyResult],
        all_results: &[crate::types::CopyResult],
    ) -> String {
        let mut output = String::new();

        let success_count = all_results.iter().filter(|r| r.success).count();

        output.push_str("================\nCopy Failed\n================\n");
        output.push_str(&format!(
            "Failed: {} files (Copied: {} files)\n\n",
            failed.len(),
            success_count
        ));

        for result in failed {
            let msg = result.error_message.as_deref().unwrap_or("Unknown error");
            output.push_str(&format!("  {}: {}\n", result.path.display(), msg));
        }

        output.push('\n');
        output
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
struct TreeNode {
    name: String,
    entry: Option<FileEntry>,
    children: BTreeMap<String, TreeNode>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    /// Test that path components are correctly extracted using Path::components()
    /// This ensures cross-platform compatibility (works with both / and \ separators)
    #[test]
    fn test_path_components_extraction() {
        // Test forward slash path (Unix-style)
        let path = PathBuf::from("dir1/dir2/file.txt");
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        assert_eq!(parts, vec!["dir1", "dir2", "file.txt"]);

        // Test single level path
        let path = PathBuf::from("file.txt");
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        assert_eq!(parts, vec!["file.txt"]);

        // Test deep nested path
        let path = PathBuf::from("a/b/c/d/e/f.txt");
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        assert_eq!(parts, vec!["a", "b", "c", "d", "e", "f.txt"]);
    }

    /// Test that paths with Japanese characters are correctly processed
    #[test]
    fn test_path_components_japanese() {
        let path = PathBuf::from("日本語/ディレクトリ/ファイル.txt");
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        assert_eq!(parts, vec!["日本語", "ディレクトリ", "ファイル.txt"]);
    }

    /// Test that empty path components are handled correctly
    #[test]
    fn test_path_components_normalized() {
        // Path::components() normalizes paths and removes empty components
        let path = PathBuf::from("dir1//dir2/file.txt");
        let parts: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        // Note: On Unix, // is normalized to /, so we get normal components
        assert!(parts.contains(&"dir1".to_string()));
        assert!(parts.contains(&"file.txt".to_string()));
    }
}
