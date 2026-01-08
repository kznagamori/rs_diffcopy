use chrono::Local;
use rust_xlsxwriter::{Color, Format, Workbook};

use crate::config::Config;
use crate::types::{ComparisonResult, ComparisonStats, FileEntry, FileStatus};

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

    fn write_summary_sheet(
        &self,
        workbook: &mut Workbook,
        stats: &ComparisonStats,
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Summary").ok();

        // Formats
        let title_format = Format::new()
            .set_bold()
            .set_font_size(16.0)
            .set_font_color(Color::RGB(0x0066CC));

        let header_format = Format::new()
            .set_bold()
            .set_font_size(12.0)
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        // Title
        worksheet.write_string(0, 0, "rs_diffcopy Summary").ok();
        worksheet.set_row_format(0, &title_format).ok();

        // Basic info
        let mut row = 2;

        worksheet.write_string(row, 0, "Source:").ok();
        worksheet
            .write_string(row, 1, &self.config.source.display().to_string())
            .ok();
        row += 1;

        worksheet.write_string(row, 0, "Target:").ok();
        worksheet
            .write_string(row, 1, &self.config.target.display().to_string())
            .ok();
        row += 1;

        worksheet.write_string(row, 0, "Output:").ok();
        worksheet
            .write_string(row, 1, &self.config.output.display().to_string())
            .ok();
        row += 1;

        worksheet.write_string(row, 0, "Date:").ok();
        worksheet
            .write_string(row, 1, &Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .ok();
        row += 2;

        // Statistics header
        worksheet.write_string(row, 0, "Statistics").ok();
        worksheet.set_row_format(row, &header_format).ok();
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

        for (label, value) in stats_data {
            worksheet.write_string(row, 0, label).ok();
            worksheet.write_number(row, 1, value as f64).ok();
            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 20.0).ok();
        worksheet.set_column_width(1, 50.0).ok();

        Ok(())
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

        // Formats
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        let tree_format = Format::new().set_font_name("Consolas");

        let added_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x008000));

        let modified_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x0066CC));

        let deleted_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0xCC0000));

        let symlink_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x9933FF));

        let unchanged_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x808080));

        // Headers
        worksheet.write_string(0, 0, "Path").ok();
        worksheet.write_string(0, 1, "Status").ok();
        worksheet.set_row_format(0, &header_format).ok();

        // Build tree and write
        let filtered_entries: Vec<&FileEntry> = entries
            .iter()
            .filter(|e| self.config.filter_status.matches(e.status))
            .collect();

        let mut row = 1u32;
        let fold_level = self.config.excel_fold_level;

        for entry in &filtered_entries {
            let depth = entry.relative_path.components().count();
            let indent = "  ".repeat(depth.saturating_sub(1));
            let name = entry
                .relative_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| entry.relative_path.to_string_lossy().to_string());

            let display_name = if entry.is_directory {
                format!("{}{}/", indent, name)
            } else {
                format!("{}{}", indent, name)
            };

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

            worksheet.write_string_with_format(row, 0, &display_name, format).ok();
            worksheet.write_string_with_format(row, 1, status_str, format).ok();

            // Note: Row grouping/folding is not currently implemented
            // as rust_xlsxwriter API for outline levels may differ
            let _ = fold_level; // Silence unused variable warning

            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 60.0).ok();
        worksheet.set_column_width(1, 15.0).ok();

        Ok(())
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

        // Formats
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        let section_format = Format::new()
            .set_bold()
            .set_font_size(12.0)
            .set_background_color(Color::RGB(0xD9E2F3));

        let added_format = Format::new().set_font_color(Color::RGB(0x008000));
        let modified_format = Format::new().set_font_color(Color::RGB(0x0066CC));
        let deleted_format = Format::new().set_font_color(Color::RGB(0xCC0000));

        // Headers
        worksheet.write_string(0, 0, "Status").ok();
        worksheet.write_string(0, 1, "Directory").ok();
        worksheet.write_string(0, 2, "File").ok();
        worksheet.write_string(0, 3, "Details").ok();
        worksheet.set_row_format(0, &header_format).ok();

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

            // Section header
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

            worksheet.write_string_with_format(row, 0, section_name, &section_format).ok();
            worksheet.merge_range(row, 0, row, 3, section_name, &section_format).ok();
            row += 1;

            let format = match status {
                FileStatus::Added => &added_format,
                FileStatus::Modified => &modified_format,
                FileStatus::Deleted => &deleted_format,
                _ => &Format::new(),
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
                worksheet.write_string(row, 1, &dir).ok();
                worksheet.write_string(row, 2, &file).ok();
                worksheet.write_string(row, 3, &details).ok();

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
