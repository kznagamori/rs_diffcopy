use chrono::Local;
use rust_xlsxwriter::{Color, Format, Workbook};

use crate::config::Config;
use crate::types::{ThreeWayEntry, ThreeWayResult, ThreeWayStats, ThreeWayStatus};

/// Excel report generator for three-way comparison
pub struct ThreeWayExcelWriter<'a> {
    config: &'a Config,
}

impl<'a> ThreeWayExcelWriter<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Generate Excel report for three-way comparison
    pub fn write(&self, result: &ThreeWayResult) -> crate::error::Result<()> {
        let excel_path = match &self.config.excel {
            Some(path) => path,
            None => return Ok(()),
        };

        let mut workbook = Workbook::new();

        // Create sheets
        self.write_summary_sheet(&mut workbook, &result.stats)?;
        self.write_file_matrix_sheet(&mut workbook, &result.entries)?;
        self.write_conflicts_sheet(&mut workbook, &result.entries)?;
        self.write_copied_files_sheet(&mut workbook, &result.entries)?;

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
        stats: &ThreeWayStats,
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
        worksheet.write_string(0, 0, "rs_diffcopy Summary (Three-way)").ok();
        worksheet.set_row_format(0, &title_format).ok();

        // Basic info
        let mut row = 2;

        let base_path = self.config.base.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        worksheet.write_string(row, 0, "Base:").ok();
        worksheet.write_string(row, 1, &base_path).ok();
        row += 1;

        worksheet.write_string(row, 0, "Ours:").ok();
        worksheet
            .write_string(row, 1, &self.config.source.display().to_string())
            .ok();
        row += 1;

        worksheet.write_string(row, 0, "Theirs:").ok();
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
        worksheet.write_string(row, 0, "Change Matrix").ok();
        worksheet.set_row_format(row, &header_format).ok();
        row += 1;

        // Statistics data
        let stats_data = [
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
            ("Total", stats.total_items),
            ("Total Conflicts", stats.total_conflicts()),
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

    fn write_file_matrix_sheet(
        &self,
        workbook: &mut Workbook,
        entries: &[ThreeWayEntry],
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("File Matrix").ok();

        // Formats
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        let unchanged_format = Format::new().set_font_color(Color::RGB(0x808080));
        let ours_only_format = Format::new().set_font_color(Color::RGB(0x008000));
        let theirs_only_format = Format::new().set_font_color(Color::RGB(0x0066CC));
        let both_same_format = Format::new().set_font_color(Color::RGB(0x00B0F0));
        let conflict_format = Format::new()
            .set_bold()
            .set_font_color(Color::RGB(0xCC0000));
        let added_format = Format::new().set_font_color(Color::RGB(0x92D050));
        let deleted_format = Format::new().set_font_color(Color::RGB(0xFFC7CE));

        // Headers
        worksheet.write_string(0, 0, "Path").ok();
        worksheet.write_string(0, 1, "Base").ok();
        worksheet.write_string(0, 2, "Ours").ok();
        worksheet.write_string(0, 3, "Theirs").ok();
        worksheet.write_string(0, 4, "Status").ok();
        worksheet.set_row_format(0, &header_format).ok();

        let mut row = 1u32;

        for entry in entries {
            if !self.config.filter_status.matches_three_way(entry.status) {
                continue;
            }

            let base_indicator = if entry.base_exists { "○" } else { "-" };
            let ours_indicator = self.get_indicator(entry.base_exists, entry.ours_exists, &entry.base_hash, &entry.ours_hash);
            let theirs_indicator = self.get_indicator(entry.base_exists, entry.theirs_exists, &entry.base_hash, &entry.theirs_hash);

            let format = match entry.status {
                ThreeWayStatus::Unchanged => &unchanged_format,
                ThreeWayStatus::OursOnly => &ours_only_format,
                ThreeWayStatus::TheirsOnly => &theirs_only_format,
                ThreeWayStatus::BothSame => &both_same_format,
                ThreeWayStatus::Conflict
                | ThreeWayStatus::AddedBothDiff
                | ThreeWayStatus::ModifyDelete
                | ThreeWayStatus::DeleteModify => &conflict_format,
                ThreeWayStatus::AddedOurs
                | ThreeWayStatus::AddedTheirs
                | ThreeWayStatus::AddedBothSame => &added_format,
                ThreeWayStatus::DeletedOurs
                | ThreeWayStatus::DeletedTheirs
                | ThreeWayStatus::DeletedBoth => &deleted_format,
            };

            worksheet.write_string_with_format(row, 0, &entry.relative_path.display().to_string(), format).ok();
            worksheet.write_string_with_format(row, 1, base_indicator, format).ok();
            worksheet.write_string_with_format(row, 2, ours_indicator, format).ok();
            worksheet.write_string_with_format(row, 3, theirs_indicator, format).ok();
            worksheet.write_string_with_format(row, 4, entry.status.as_str(), format).ok();

            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 50.0).ok();
        worksheet.set_column_width(1, 10.0).ok();
        worksheet.set_column_width(2, 10.0).ok();
        worksheet.set_column_width(3, 10.0).ok();
        worksheet.set_column_width(4, 20.0).ok();

        Ok(())
    }

    fn get_indicator(
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

    fn write_conflicts_sheet(
        &self,
        workbook: &mut Workbook,
        entries: &[ThreeWayEntry],
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Conflicts").ok();

        // Formats
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        let conflict_format = Format::new()
            .set_font_color(Color::RGB(0xCC0000));

        // Headers
        worksheet.write_string(0, 0, "Path").ok();
        worksheet.write_string(0, 1, "Type").ok();
        worksheet.write_string(0, 2, "Base Hash").ok();
        worksheet.write_string(0, 3, "Ours Hash").ok();
        worksheet.write_string(0, 4, "Theirs Hash").ok();
        worksheet.write_string(0, 5, "Base Size").ok();
        worksheet.write_string(0, 6, "Ours Size").ok();
        worksheet.write_string(0, 7, "Theirs Size").ok();
        worksheet.set_row_format(0, &header_format).ok();

        let conflicts: Vec<&ThreeWayEntry> = entries
            .iter()
            .filter(|e| e.status.is_conflict())
            .collect();

        let mut row = 1u32;

        for entry in conflicts {
            let conflict_type = match entry.status {
                ThreeWayStatus::Conflict => "Content conflict",
                ThreeWayStatus::AddedBothDiff => "Add/Add conflict",
                ThreeWayStatus::ModifyDelete => "Modify/Delete",
                ThreeWayStatus::DeleteModify => "Delete/Modify",
                _ => "Unknown",
            };

            worksheet.write_string_with_format(row, 0, &entry.relative_path.display().to_string(), &conflict_format).ok();
            worksheet.write_string_with_format(row, 1, conflict_type, &conflict_format).ok();

            let base_hash = entry.base_hash.as_ref().map(|h| &h[..8.min(h.len())]).unwrap_or("-");
            let ours_hash = entry.ours_hash.as_ref().map(|h| &h[..8.min(h.len())]).unwrap_or("-");
            let theirs_hash = entry.theirs_hash.as_ref().map(|h| &h[..8.min(h.len())]).unwrap_or("-");

            worksheet.write_string(row, 2, base_hash).ok();
            worksheet.write_string(row, 3, ours_hash).ok();
            worksheet.write_string(row, 4, theirs_hash).ok();

            let base_size = entry.base_size.map(|s| s.to_string()).unwrap_or("-".to_string());
            let ours_size = entry.ours_size.map(|s| s.to_string()).unwrap_or("-".to_string());
            let theirs_size = entry.theirs_size.map(|s| s.to_string()).unwrap_or("-".to_string());

            worksheet.write_string(row, 5, &base_size).ok();
            worksheet.write_string(row, 6, &ours_size).ok();
            worksheet.write_string(row, 7, &theirs_size).ok();

            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 50.0).ok();
        worksheet.set_column_width(1, 20.0).ok();
        worksheet.set_column_width(2, 15.0).ok();
        worksheet.set_column_width(3, 15.0).ok();
        worksheet.set_column_width(4, 15.0).ok();
        worksheet.set_column_width(5, 12.0).ok();
        worksheet.set_column_width(6, 12.0).ok();
        worksheet.set_column_width(7, 12.0).ok();

        Ok(())
    }

    fn write_copied_files_sheet(
        &self,
        workbook: &mut Workbook,
        entries: &[ThreeWayEntry],
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Copied Files").ok();

        // Formats
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        // Headers
        worksheet.write_string(0, 0, "Path").ok();
        worksheet.write_string(0, 1, "Status").ok();
        worksheet.write_string(0, 2, "Source").ok();
        worksheet.set_row_format(0, &header_format).ok();

        let mut row = 1u32;

        for entry in entries {
            // Skip unchanged and deleted entries (not copied)
            if matches!(entry.status,
                ThreeWayStatus::Unchanged
                | ThreeWayStatus::DeletedOurs
                | ThreeWayStatus::DeletedTheirs
                | ThreeWayStatus::DeletedBoth
            ) {
                continue;
            }

            let source = match entry.status {
                ThreeWayStatus::OursOnly | ThreeWayStatus::AddedOurs => "ours",
                ThreeWayStatus::TheirsOnly | ThreeWayStatus::AddedTheirs => "theirs",
                ThreeWayStatus::BothSame | ThreeWayStatus::AddedBothSame => "ours (same)",
                ThreeWayStatus::Conflict | ThreeWayStatus::AddedBothDiff => {
                    match self.config.merge_style {
                        crate::types::MergeStyle::All => "base + ours + theirs",
                        crate::types::MergeStyle::Ours => "ours",
                        crate::types::MergeStyle::Theirs => "theirs",
                    }
                }
                ThreeWayStatus::ModifyDelete => {
                    match self.config.merge_style {
                        crate::types::MergeStyle::All => "base + ours",
                        crate::types::MergeStyle::Ours => "ours",
                        crate::types::MergeStyle::Theirs => "(deleted)",
                    }
                }
                ThreeWayStatus::DeleteModify => {
                    match self.config.merge_style {
                        crate::types::MergeStyle::All => "base + theirs",
                        crate::types::MergeStyle::Ours => "(deleted)",
                        crate::types::MergeStyle::Theirs => "theirs",
                    }
                }
                _ => "unknown",
            };

            worksheet.write_string(row, 0, &entry.relative_path.display().to_string()).ok();
            worksheet.write_string(row, 1, entry.status.as_str()).ok();
            worksheet.write_string(row, 2, source).ok();

            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 50.0).ok();
        worksheet.set_column_width(1, 20.0).ok();
        worksheet.set_column_width(2, 25.0).ok();

        Ok(())
    }
}
