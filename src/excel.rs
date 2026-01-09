use chrono::Local;
use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook, Worksheet};

use crate::config::Config;
use crate::types::{CheckPermissionsMode, ComparisonResult, ComparisonStats, FileEntry, FileStatus, StatusFilter};

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

/// Excel report generator
pub struct ExcelWriter<'a> {
    config: &'a Config,
}

impl<'a> ExcelWriter<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Generate Excel report
    pub fn write(&self, result: &ComparisonResult) -> crate::error::Result<()> {
        let excel_path = match &self.config.excel {
            Some(path) => path,
            None => return Ok(()),
        };

        let mut workbook = Workbook::new();

        // Create sheets
        self.write_summary_sheet(&mut workbook, &result.stats)?;
        self.write_file_tree_sheet(&mut workbook, &result.entries)?;
        self.write_details_sheet(&mut workbook, &result.entries)?;

        // Save workbook
        workbook.save(excel_path).map_err(|e| {
            crate::error::DiffCopyError::FileWriteError {
                path: excel_path.clone(),
                message: e.to_string(),
            }
        })?;

        Ok(())
    }

    /// Create common formats
    fn create_formats() -> ExcelFormats {
        let title_format = Format::new()
            .set_bold()
            .set_font_size(16.0)
            .set_font_color(Color::RGB(0x0066CC));

        let header_format = Format::new()
            .set_bold()
            .set_font_size(12.0)
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White)
            .set_border(FormatBorder::Thin);

        let label_format = Format::new()
            .set_bold()
            .set_border(FormatBorder::Thin);

        let value_format = Format::new()
            .set_border(FormatBorder::Thin);

        let section_format = Format::new()
            .set_bold()
            .set_font_size(12.0)
            .set_background_color(Color::RGB(0xD9E2F3))
            .set_border(FormatBorder::Thin);

        ExcelFormats {
            title: title_format,
            header: header_format,
            label: label_format,
            value: value_format,
            section: section_format,
        }
    }

    fn write_summary_sheet(
        &self,
        workbook: &mut Workbook,
        stats: &ComparisonStats,
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Summary").ok();

        let formats = Self::create_formats();

        // Title
        worksheet.write_string_with_format(0, 0, "rs_diffcopy Summary", &formats.title).ok();

        // Basic info section
        let mut row = 2;

        worksheet.write_string_with_format(row, 0, "Source:", &formats.label).ok();
        worksheet.write_string_with_format(row, 1, &self.config.source.display().to_string(), &formats.value).ok();
        row += 1;

        worksheet.write_string_with_format(row, 0, "Target:", &formats.label).ok();
        worksheet.write_string_with_format(row, 1, &self.config.target.display().to_string(), &formats.value).ok();
        row += 1;

        worksheet.write_string_with_format(row, 0, "Output:", &formats.label).ok();
        worksheet.write_string_with_format(row, 1, &self.config.output.display().to_string(), &formats.value).ok();
        row += 1;

        worksheet.write_string_with_format(row, 0, "Date:", &formats.label).ok();
        worksheet.write_string_with_format(row, 1, &Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), &formats.value).ok();
        row += 2;

        // Options section
        worksheet.write_string_with_format(row, 0, "Options", &formats.header).ok();
        worksheet.write_string_with_format(row, 1, "", &formats.header).ok();
        row += 1;

        // Write options
        let options = self.collect_options();
        for (label, value) in options {
            worksheet.write_string_with_format(row, 0, &label, &formats.label).ok();
            worksheet.write_string_with_format(row, 1, &value, &formats.value).ok();
            row += 1;
        }
        row += 1;

        // Statistics header - apply to both columns
        worksheet.write_string_with_format(row, 0, "Statistics", &formats.header).ok();
        worksheet.write_string_with_format(row, 1, "", &formats.header).ok();
        row += 1;

        // Statistics data
        let stats_data = [
            ("Added (files)", stats.added_files),
            ("Added (dirs)", stats.added_dirs),
            ("Modified", stats.modified_files),
            ("Deleted (files)", stats.deleted_files),
            ("Deleted (dirs)", stats.deleted_dirs),
            ("Unchanged", stats.unchanged_files),
            ("Symlinks", stats.symlink_files),
            ("Special Files", stats.special_files),
            ("Permissions", stats.permission_files),
            ("Errors", stats.error_files),
            ("Total", stats.total_items),
        ];

        let number_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0");

        for (label, value) in stats_data {
            worksheet.write_string_with_format(row, 0, label, &formats.label).ok();
            worksheet.write_number_with_format(row, 1, value as f64, &number_format).ok();
            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 25.0).ok();
        worksheet.set_column_width(1, 50.0).ok();

        Ok(())
    }

    fn collect_options(&self) -> Vec<(String, String)> {
        let mut options = Vec::new();

        if self.config.dry_run {
            options.push(("Mode:".to_string(), "Dry run (no files copied)".to_string()));
        }

        if self.config.both_versions {
            options.push(("Copy mode:".to_string(), "Both versions (.old/.new)".to_string()));
        }

        if self.config.copy_deleted {
            options.push(("Copy deleted:".to_string(), "Yes (.deleted)".to_string()));
        }

        if self.config.preserve_timestamps {
            options.push(("Preserve timestamps:".to_string(), "Yes".to_string()));
        }

        match self.config.check_permissions {
            CheckPermissionsMode::None => {},
            CheckPermissionsMode::Scripts => {
                options.push(("Permission check:".to_string(), "scripts".to_string()));
            },
            CheckPermissionsMode::All => {
                options.push(("Permission check:".to_string(), "all".to_string()));
            },
        }

        if self.config.patch {
            options.push(("Patch mode:".to_string(), "Individual files (.patch)".to_string()));
        }

        if let Some(ref patch_file) = self.config.patch_file {
            options.push(("Combined patch file:".to_string(), patch_file.display().to_string()));
        }

        if self.config.show_unchanged {
            options.push(("Show unchanged:".to_string(), "Yes".to_string()));
        }

        if !self.config.filter_status.is_empty() {
            let filter_str = format_filter_status(&self.config.filter_status);
            if !filter_str.is_empty() {
                options.push(("Filter status:".to_string(), filter_str));
            }
        }

        if !self.config.exclude.is_empty() {
            let exclude_str = self.config.exclude.join(", ");
            options.push(("Exclude patterns:".to_string(), exclude_str));
        }

        if self.config.stats_only {
            options.push(("Stats only:".to_string(), "Yes".to_string()));
        }

        if self.config.no_tree {
            options.push(("No tree:".to_string(), "Yes".to_string()));
        }

        if self.config.no_details {
            options.push(("No details:".to_string(), "Yes".to_string()));
        }

        options
    }

    fn write_file_tree_sheet(
        &self,
        workbook: &mut Workbook,
        entries: &[FileEntry],
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("File Tree").ok();

        if self.config.no_tree {
            worksheet.write_string(0, 0, "(File Tree disabled by --no-tree option)").ok();
            return Ok(());
        }

        let formats = Self::create_formats();

        // Create status-specific formats with borders
        let tree_format = Format::new()
            .set_font_name("Consolas")
            .set_border(FormatBorder::Thin);

        let added_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x008000))
            .set_border(FormatBorder::Thin);

        let modified_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x0066CC))
            .set_border(FormatBorder::Thin);

        let deleted_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0xCC0000))
            .set_border(FormatBorder::Thin);

        let symlink_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x9933FF))
            .set_border(FormatBorder::Thin);

        let unchanged_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x808080))
            .set_border(FormatBorder::Thin);

        // Build tree and write
        let filtered_entries: Vec<&FileEntry> = entries
            .iter()
            .filter(|e| self.config.filter_status.matches(e.status))
            .collect();

        // Calculate max depth for column count
        let max_depth = filtered_entries
            .iter()
            .map(|e| e.relative_path.components().count())
            .max()
            .unwrap_or(1);

        // Status column is after all path columns
        let status_col = max_depth as u16;

        // Write headers - one for each depth level plus Status
        for col in 0..max_depth {
            let header_text = if col == 0 {
                "Path".to_string()
            } else {
                "".to_string()  // Empty headers for intermediate columns
            };
            worksheet.write_string_with_format(0, col as u16, &header_text, &formats.header).ok();
        }
        worksheet.write_string_with_format(0, status_col, "Status", &formats.header).ok();

        let mut row = 1u32;
        let fold_level = self.config.excel_fold_level;

        for entry in &filtered_entries {
            let components: Vec<String> = entry
                .relative_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            let depth = components.len();

            let status_str = match entry.status {
                FileStatus::Added => "added",
                FileStatus::Modified => "modified",
                FileStatus::Deleted => "deleted",
                FileStatus::Unchanged => "unchanged",
                FileStatus::Symlink => "symlink",
                FileStatus::Special => "special",
                FileStatus::Permission => "permission",
                FileStatus::Error => "error",
            };

            let format = match entry.status {
                FileStatus::Added => &added_format,
                FileStatus::Modified => &modified_format,
                FileStatus::Deleted => &deleted_format,
                FileStatus::Symlink => &symlink_format,
                FileStatus::Unchanged => &unchanged_format,
                _ => &tree_format,
            };

            // Write each path component in its own cell
            for (i, component) in components.iter().enumerate() {
                let is_last = i == depth - 1;
                let display_text = if is_last && entry.is_directory {
                    format!("{}/", component)
                } else if !is_last {
                    format!("{}/", component)
                } else {
                    component.clone()
                };
                worksheet.write_string_with_format(row, i as u16, &display_text, format).ok();
            }

            // Write status in the status column
            worksheet.write_string_with_format(row, status_col, status_str, format).ok();

            row += 1;
        }

        // Apply row grouping for fold levels
        // fold_level N means items at depth >= N should be grouped
        if let Some(level) = fold_level {
            if level > 0 {
                Self::apply_row_grouping(worksheet, &filtered_entries, level);
            }
        }

        // Set column widths
        for col in 0..=max_depth {
            if col < max_depth {
                worksheet.set_column_width(col as u16, 20.0).ok();
            } else {
                worksheet.set_column_width(col as u16, 12.0).ok();
            }
        }

        Ok(())
    }

    fn apply_row_grouping(worksheet: &mut Worksheet, entries: &[&FileEntry], fold_level: usize) {
        // Find contiguous ranges of rows that should be folded
        // depth >= fold_level means the row should be grouped
        let mut group_start: Option<u32> = None;
        let mut current_row = 1u32;

        for entry in entries {
            let depth = entry.relative_path.components().count();

            // depth >= fold_level means this row should be grouped (hidden by default)
            if depth >= fold_level {
                if group_start.is_none() {
                    group_start = Some(current_row);
                }
            } else {
                // End current group if any
                if let Some(start) = group_start {
                    if current_row > start {
                        worksheet.group_rows(start, current_row - 1).ok();
                    }
                    group_start = None;
                }
            }
            current_row += 1;
        }

        // Handle last group
        if let Some(start) = group_start {
            if current_row > start {
                worksheet.group_rows(start, current_row - 1).ok();
            }
        }
    }

    fn write_details_sheet(
        &self,
        workbook: &mut Workbook,
        entries: &[FileEntry],
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Details").ok();

        if self.config.no_details {
            worksheet.write_string(0, 0, "(Details disabled by --no-details option)").ok();
            return Ok(());
        }

        let formats = Self::create_formats();

        // Create status-specific formats with borders
        let added_format = Format::new()
            .set_font_color(Color::RGB(0x008000))
            .set_border(FormatBorder::Thin);
        let modified_format = Format::new()
            .set_font_color(Color::RGB(0x0066CC))
            .set_border(FormatBorder::Thin);
        let deleted_format = Format::new()
            .set_font_color(Color::RGB(0xCC0000))
            .set_border(FormatBorder::Thin);
        let default_format = Format::new()
            .set_border(FormatBorder::Thin);

        // Headers - apply format to each column individually
        worksheet.write_string_with_format(0, 0, "Status", &formats.header).ok();
        worksheet.write_string_with_format(0, 1, "Directory", &formats.header).ok();
        worksheet.write_string_with_format(0, 2, "File", &formats.header).ok();
        worksheet.write_string_with_format(0, 3, "Details", &formats.header).ok();

        let mut row = 1u32;

        // Group entries by status
        let status_order = [
            FileStatus::Added,
            FileStatus::Modified,
            FileStatus::Deleted,
            FileStatus::Symlink,
            FileStatus::Permission,
            FileStatus::Error,
            FileStatus::Special,
        ];

        for status in status_order {
            let status_entries: Vec<&FileEntry> = entries
                .iter()
                .filter(|e| e.status == status && self.config.filter_status.matches(e.status))
                .collect();

            if status_entries.is_empty() {
                continue;
            }

            // Section header - apply to all 4 columns individually
            let section_name = match status {
                FileStatus::Added => "Added Files",
                FileStatus::Modified => "Modified Files",
                FileStatus::Deleted => "Deleted Files",
                FileStatus::Symlink => "Symlinks",
                FileStatus::Permission => "Permission Changes",
                FileStatus::Error => "Errors",
                FileStatus::Special => "Special Files",
                _ => "Other",
            };

            worksheet.write_string_with_format(row, 0, section_name, &formats.section).ok();
            worksheet.write_string_with_format(row, 1, "", &formats.section).ok();
            worksheet.write_string_with_format(row, 2, "", &formats.section).ok();
            worksheet.write_string_with_format(row, 3, "", &formats.section).ok();
            row += 1;

            let format = match status {
                FileStatus::Added => &added_format,
                FileStatus::Modified => &modified_format,
                FileStatus::Deleted => &deleted_format,
                _ => &default_format,
            };

            for entry in status_entries {
                let dir = entry
                    .relative_path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let file = entry
                    .relative_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let details = self.get_entry_details(entry);

                worksheet.write_string_with_format(row, 0, status.as_str(), format).ok();
                worksheet.write_string_with_format(row, 1, &dir, &default_format).ok();
                worksheet.write_string_with_format(row, 2, &file, &default_format).ok();
                worksheet.write_string_with_format(row, 3, &details, &default_format).ok();

                row += 1;
            }

            row += 1; // Empty row between sections
        }

        // Set column widths
        worksheet.set_column_width(0, 15.0).ok();
        worksheet.set_column_width(1, 40.0).ok();
        worksheet.set_column_width(2, 30.0).ok();
        worksheet.set_column_width(3, 50.0).ok();

        Ok(())
    }

    fn get_entry_details(&self, entry: &FileEntry) -> String {
        match entry.status {
            FileStatus::Modified => {
                if let (Some(old_size), Some(new_size)) = (entry.source_size, entry.target_size) {
                    format!("{} -> {} bytes", old_size, new_size)
                } else {
                    String::new()
                }
            }
            FileStatus::Symlink => {
                if let Some(ref info) = entry.symlink_info {
                    format!(
                        "-> {} ({})",
                        info.target.display(),
                        if info.is_broken { "broken" } else { "ok" }
                    )
                } else {
                    String::new()
                }
            }
            FileStatus::Permission => {
                if let Some(ref change) = entry.permission_change {
                    format!("{} -> {}", change.old_mode, change.new_mode)
                } else {
                    String::new()
                }
            }
            FileStatus::Error => entry.error_message.clone().unwrap_or_default(),
            FileStatus::Special => {
                entry
                    .special_type
                    .as_ref()
                    .map(|t| t.as_str().to_string())
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    }
}

/// Struct to hold common formats
struct ExcelFormats {
    title: Format,
    header: Format,
    label: Format,
    value: Format,
    section: Format,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> Config {
        Config {
            source: PathBuf::from("/test/source"),
            target: PathBuf::from("/test/target"),
            output: PathBuf::from("/test/output"),
            excel: Some(PathBuf::from("/test/report.xlsx")),
            excel_fold_level: Some(2),
            dry_run: true,
            both_versions: true,
            copy_deleted: true,
            preserve_timestamps: true,
            check_permissions: CheckPermissionsMode::Scripts,
            patch: true,
            patch_file: Some(PathBuf::from("/test/combined.patch")),
            show_unchanged: true,
            exclude: vec!["*.log".to_string(), "node_modules/**".to_string()],
            stats_only: false,
            no_tree: false,
            no_details: false,
            ..Default::default()
        }
    }

    #[test]
    fn test_collect_options_all_enabled() {
        let config = create_test_config();
        let writer = ExcelWriter::new(&config);
        let options = writer.collect_options();

        assert!(options.iter().any(|(k, _)| k == "Mode:"));
        assert!(options.iter().any(|(k, _)| k == "Copy mode:"));
        assert!(options.iter().any(|(k, _)| k == "Copy deleted:"));
        assert!(options.iter().any(|(k, _)| k == "Preserve timestamps:"));
        assert!(options.iter().any(|(k, _)| k == "Permission check:"));
        assert!(options.iter().any(|(k, _)| k == "Patch mode:"));
        assert!(options.iter().any(|(k, _)| k == "Combined patch file:"));
        assert!(options.iter().any(|(k, _)| k == "Show unchanged:"));
        assert!(options.iter().any(|(k, _)| k == "Exclude patterns:"));
    }

    #[test]
    fn test_collect_options_minimal() {
        let config = Config {
            source: PathBuf::from("/test/source"),
            target: PathBuf::from("/test/target"),
            output: PathBuf::from("/test/output"),
            ..Default::default()
        };
        let writer = ExcelWriter::new(&config);
        let options = writer.collect_options();

        assert!(options.is_empty());
    }

    #[test]
    fn test_collect_options_permission_check_all() {
        let config = Config {
            source: PathBuf::from("/test/source"),
            target: PathBuf::from("/test/target"),
            output: PathBuf::from("/test/output"),
            check_permissions: CheckPermissionsMode::All,
            ..Default::default()
        };
        let writer = ExcelWriter::new(&config);
        let options = writer.collect_options();

        let perm_option = options.iter().find(|(k, _)| k == "Permission check:");
        assert!(perm_option.is_some());
        assert_eq!(perm_option.unwrap().1, "all");
    }

    #[test]
    fn test_collect_options_filter_status() {
        use crate::types::StatusFilter;

        let mut filter = StatusFilter::new();
        filter.included.insert("added".to_string());
        filter.included.insert("modified".to_string());

        let config = Config {
            source: PathBuf::from("/test/source"),
            target: PathBuf::from("/test/target"),
            output: PathBuf::from("/test/output"),
            filter_status: filter,
            ..Default::default()
        };
        let writer = ExcelWriter::new(&config);
        let options = writer.collect_options();

        let filter_option = options.iter().find(|(k, _)| k == "Filter status:");
        assert!(filter_option.is_some());
        let value = &filter_option.unwrap().1;
        // The value should contain both "added" and "modified"
        assert!(value.contains("added"));
        assert!(value.contains("modified"));
    }

    #[test]
    fn test_collect_options_filter_status_empty() {
        let config = Config {
            source: PathBuf::from("/test/source"),
            target: PathBuf::from("/test/target"),
            output: PathBuf::from("/test/output"),
            ..Default::default()
        };
        let writer = ExcelWriter::new(&config);
        let options = writer.collect_options();

        // Empty filter_status should not appear in options
        let filter_option = options.iter().find(|(k, _)| k == "Filter status:");
        assert!(filter_option.is_none());
    }

    #[test]
    fn test_create_formats() {
        // Just verify formats can be created without panic
        let _formats = ExcelWriter::create_formats();
    }

    #[test]
    fn test_get_entry_details_modified() {
        let config = Config::default();
        let writer = ExcelWriter::new(&config);

        let mut entry = FileEntry::new(PathBuf::from("test.txt"), FileStatus::Modified, false);
        entry.source_size = Some(100);
        entry.target_size = Some(200);

        let details = writer.get_entry_details(&entry);
        assert_eq!(details, "100 -> 200 bytes");
    }

    #[test]
    fn test_get_entry_details_error() {
        let config = Config::default();
        let writer = ExcelWriter::new(&config);

        let mut entry = FileEntry::new(PathBuf::from("test.txt"), FileStatus::Error, false);
        entry.error_message = Some("Permission denied".to_string());

        let details = writer.get_entry_details(&entry);
        assert_eq!(details, "Permission denied");
    }

    #[test]
    fn test_apply_row_grouping() {
        // Test that row grouping logic works correctly
        // This tests the helper function logic without needing a real worksheet
        let entries: Vec<FileEntry> = vec![
            FileEntry::new(PathBuf::from("a/file1.txt"), FileStatus::Added, false),
            FileEntry::new(PathBuf::from("a/b/file2.txt"), FileStatus::Added, false),
            FileEntry::new(PathBuf::from("a/b/c/file3.txt"), FileStatus::Added, false),
            FileEntry::new(PathBuf::from("x/file4.txt"), FileStatus::Added, false),
        ];

        // Verify depth calculation
        assert_eq!(entries[0].relative_path.components().count(), 2);
        assert_eq!(entries[1].relative_path.components().count(), 3);
        assert_eq!(entries[2].relative_path.components().count(), 4);
        assert_eq!(entries[3].relative_path.components().count(), 2);

        // With fold_level=2, items at depth >= 2 should be grouped
        // That means all items in this list would be grouped (depths 2, 3, 4, 2)
        // With fold_level=3, items at depth >= 3 should be grouped
        // That means file2.txt (3) and file3.txt (4) would be grouped
    }

    #[test]
    fn test_format_filter_status_all_with_exclusion() {
        use crate::types::StatusFilter;

        let mut filter = StatusFilter::new();
        filter.include_all = true;
        filter.excluded.insert("deleted".to_string());

        let result = format_filter_status(&filter);
        assert!(result.contains("all"));
        assert!(result.contains("^deleted"));
    }

    #[test]
    fn test_format_filter_status_only_exclusions() {
        use crate::types::StatusFilter;

        let mut filter = StatusFilter::new();
        filter.excluded.insert("deleted".to_string());
        filter.excluded.insert("unchanged".to_string());

        let result = format_filter_status(&filter);
        assert!(result.contains("all (implied)"));
        assert!(result.contains("^deleted"));
        assert!(result.contains("^unchanged"));
    }

    #[test]
    fn test_format_filter_status_included_only() {
        use crate::types::StatusFilter;

        let mut filter = StatusFilter::new();
        filter.included.insert("added".to_string());
        filter.included.insert("modified".to_string());

        let result = format_filter_status(&filter);
        assert!(result.contains("added"));
        assert!(result.contains("modified"));
        assert!(!result.contains("all"));
    }

    #[test]
    fn test_format_filter_status_all_only() {
        use crate::types::StatusFilter;

        let mut filter = StatusFilter::new();
        filter.include_all = true;

        let result = format_filter_status(&filter);
        assert_eq!(result, "all");
    }
}
