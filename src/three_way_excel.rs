use chrono::Local;
use rust_xlsxwriter::{Color, Format, FormatBorder, Workbook, Worksheet};
use std::collections::HashSet;

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
        self.write_file_tree_sheet(&mut workbook, &result.entries)?;
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

        let label_format = Format::new().set_bold();

        // Border formats for sections
        let border_format = Format::new().set_border(FormatBorder::Thin);
        let label_border_format = Format::new().set_bold().set_border(FormatBorder::Thin);

        // Title
        worksheet.write_string(0, 0, "rs_diffcopy Summary (Three-way)").ok();
        worksheet.set_row_format(0, &title_format).ok();

        // Basic info
        let mut row = 2;

        let base_path = self.config.base.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        worksheet.write_string_with_format(row, 0, "Base:", &label_format).ok();
        worksheet.write_string(row, 1, &base_path).ok();
        row += 1;

        worksheet.write_string_with_format(row, 0, "Ours:", &label_format).ok();
        worksheet
            .write_string(row, 1, &self.config.source.display().to_string())
            .ok();
        row += 1;

        worksheet.write_string_with_format(row, 0, "Theirs:", &label_format).ok();
        worksheet
            .write_string(row, 1, &self.config.target.display().to_string())
            .ok();
        row += 1;

        worksheet.write_string_with_format(row, 0, "Output:", &label_format).ok();
        worksheet
            .write_string(row, 1, &self.config.output.display().to_string())
            .ok();
        row += 1;

        worksheet.write_string_with_format(row, 0, "Date:", &label_format).ok();
        worksheet
            .write_string(row, 1, &Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .ok();
        row += 2;

        // Options section
        if self.has_options() {
            worksheet.write_string_with_format(row, 0, "Options", &header_format).ok();
            worksheet.write_string_with_format(row, 1, "", &header_format).ok();
            row += 1;

            if self.config.dry_run {
                worksheet.write_string_with_format(row, 0, "Mode:", &label_border_format).ok();
                worksheet.write_string_with_format(row, 1, "Dry run (no files copied)", &border_format).ok();
                row += 1;
            }

            worksheet.write_string_with_format(row, 0, "Merge style:", &label_border_format).ok();
            worksheet.write_string_with_format(row, 1, self.config.merge_style.as_str(), &border_format).ok();
            row += 1;

            if self.config.conflict_only {
                worksheet.write_string_with_format(row, 0, "Conflict only:", &label_border_format).ok();
                worksheet.write_string_with_format(row, 1, "Yes", &border_format).ok();
                row += 1;
            }

            // Filter status
            if !self.config.filter_status.is_empty() {
                worksheet.write_string_with_format(row, 0, "Filter status:", &label_border_format).ok();
                let filter_str = self.config.filter_status.to_display_string();
                worksheet.write_string_with_format(row, 1, &filter_str, &border_format).ok();
                row += 1;
            }

            // Exclude patterns
            if !self.config.exclude.is_empty() {
                worksheet.write_string_with_format(row, 0, "Exclude patterns:", &label_border_format).ok();
                worksheet.write_string_with_format(row, 1, &self.config.exclude.join(", "), &border_format).ok();
                row += 1;
            }

            row += 1;
        }

        // Statistics header
        worksheet.write_string_with_format(row, 0, "Change Matrix", &header_format).ok();
        worksheet.write_string_with_format(row, 1, "", &header_format).ok();
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
            worksheet.write_string_with_format(row, 0, label, &border_format).ok();
            worksheet.write_number_with_format(row, 1, value as f64, &border_format).ok();
            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 20.0).ok();
        worksheet.set_column_width(1, 50.0).ok();

        Ok(())
    }

    fn has_options(&self) -> bool {
        self.config.dry_run
            || self.config.merge_style != crate::types::MergeStyle::All
            || self.config.conflict_only
            || !self.config.filter_status.is_empty()
            || !self.config.exclude.is_empty()
    }

    fn write_file_tree_sheet(
        &self,
        workbook: &mut Workbook,
        entries: &[ThreeWayEntry],
    ) -> crate::error::Result<()> {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("File Tree").ok();

        if self.config.no_tree {
            worksheet.write_string(0, 0, "(File Tree disabled by --no-tree option)").ok();
            return Ok(());
        }

        // Header format
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White);

        // Status-specific formats with borders
        let tree_format = Format::new()
            .set_font_name("Consolas")
            .set_border(FormatBorder::Thin);

        let unchanged_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x808080))
            .set_border(FormatBorder::Thin);

        let ours_only_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x008000))
            .set_border(FormatBorder::Thin);

        let theirs_only_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x0066CC))
            .set_border(FormatBorder::Thin);

        let both_same_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x00B0F0))
            .set_border(FormatBorder::Thin);

        let conflict_format = Format::new()
            .set_font_name("Consolas")
            .set_bold()
            .set_font_color(Color::RGB(0xCC0000))
            .set_border(FormatBorder::Thin);

        let added_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x92D050))
            .set_border(FormatBorder::Thin);

        let deleted_format = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0xFFC7CE))
            .set_border(FormatBorder::Thin);

        // Bottom border formats
        let tree_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let unchanged_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x808080))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let ours_only_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x008000))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let theirs_only_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x0066CC))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let both_same_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x00B0F0))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let conflict_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_bold()
            .set_font_color(Color::RGB(0xCC0000))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let added_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x92D050))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let deleted_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0xFFC7CE))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        // Filter entries
        let filtered_entries: Vec<&ThreeWayEntry> = entries
            .iter()
            .filter(|e| self.config.filter_status.matches_three_way(e.status))
            .collect();

        // Calculate max depth for column count
        let max_depth = filtered_entries
            .iter()
            .map(|e| e.relative_path.components().count())
            .max()
            .unwrap_or(1);

        // Status column is after all path columns
        let status_col = max_depth as u16;

        // Write headers
        for col in 0..max_depth {
            let header_text = if col == 0 {
                "Path".to_string()
            } else {
                "".to_string()
            };
            worksheet.write_string_with_format(0, col as u16, &header_text, &header_format).ok();
        }
        worksheet.write_string_with_format(0, status_col, "Status", &header_format).ok();

        // Track written directory paths (for expansion)
        let mut written_dirs: HashSet<String> = HashSet::new();
        // Track previous row's components for deduplication
        let mut prev_components: Vec<String> = Vec::new();
        let mut row = 1u32;
        let fold_level = self.config.excel_fold_level;

        // Track row info for directory boundaries and grouping
        let mut row_info: Vec<(usize, bool, String)> = Vec::new();

        // Pre-calculate boundary rows
        let boundary_rows = Self::calculate_boundary_rows_three_way(&filtered_entries);

        for entry in &filtered_entries {
            let components: Vec<String> = entry
                .relative_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            let depth = components.len();

            let status_str = entry.status.as_str();

            // Select format based on status
            let (format, format_bottom) = match entry.status {
                ThreeWayStatus::Unchanged => (&unchanged_format, &unchanged_format_bottom),
                ThreeWayStatus::OursOnly => (&ours_only_format, &ours_only_format_bottom),
                ThreeWayStatus::TheirsOnly => (&theirs_only_format, &theirs_only_format_bottom),
                ThreeWayStatus::BothSame => (&both_same_format, &both_same_format_bottom),
                ThreeWayStatus::Conflict
                | ThreeWayStatus::AddedBothDiff
                | ThreeWayStatus::ModifyDelete
                | ThreeWayStatus::DeleteModify => (&conflict_format, &conflict_format_bottom),
                ThreeWayStatus::AddedOurs
                | ThreeWayStatus::AddedTheirs
                | ThreeWayStatus::AddedBothSame => (&added_format, &added_format_bottom),
                ThreeWayStatus::DeletedOurs
                | ThreeWayStatus::DeletedTheirs
                | ThreeWayStatus::DeletedBoth => (&deleted_format, &deleted_format_bottom),
            };

            // Check if we need to expand intermediate directories
            let mut expand_from = 0usize;
            for i in 0..depth.saturating_sub(1) {
                let dir_path = components[..=i].join("/");
                if !written_dirs.contains(&dir_path) {
                    expand_from = i;
                    break;
                }
                expand_from = i + 1;
            }

            // Check if parent directory has changed
            let parent_changed = if !prev_components.is_empty() {
                components.first() != prev_components.first()
            } else {
                true
            };

            // Write intermediate directory rows if needed
            if depth > 1 && (parent_changed || expand_from < depth - 1) {
                for i in expand_from..depth.saturating_sub(1) {
                    let dir_path = components[..=i].join("/");
                    if written_dirs.contains(&dir_path) {
                        continue;
                    }
                    written_dirs.insert(dir_path);

                    let dir_name = format!("{}/", &components[i]);

                    let should_write = if i < prev_components.len() {
                        let parent_changed_for_row = (0..i).any(|j| {
                            j >= prev_components.len() || components[j] != prev_components[j]
                        });
                        parent_changed_for_row || components[i] != prev_components[i]
                    } else {
                        true
                    };

                    let dir_format = if boundary_rows.contains(&row) {
                        &tree_format_bottom
                    } else {
                        &tree_format
                    };

                    for col in 0..i {
                        worksheet.write_string_with_format(row, col as u16, "", dir_format).ok();
                    }

                    if should_write {
                        worksheet.write_string_with_format(row, i as u16, &dir_name, dir_format).ok();
                    } else {
                        worksheet.write_string_with_format(row, i as u16, "", dir_format).ok();
                    }

                    for col in (i + 1)..max_depth {
                        worksheet.write_string_with_format(row, col as u16, "", dir_format).ok();
                    }

                    worksheet.write_string_with_format(row, status_col, "", dir_format).ok();

                    let first_level = components.first().cloned().unwrap_or_default();
                    row_info.push((i + 1, false, first_level));

                    prev_components = components[..=i].to_vec();
                    row += 1;
                }
            }

            // Write the file/entry row
            let row_format = if boundary_rows.contains(&row) {
                format_bottom
            } else {
                format
            };

            for (i, component) in components.iter().enumerate() {
                let is_last = i == depth - 1;
                let display_text = if is_last && entry.is_directory {
                    format!("{}/", component)
                } else if !is_last {
                    format!("{}/", component)
                } else {
                    component.clone()
                };

                let should_write = if i < prev_components.len() {
                    let parent_changed_for_row = (0..i).any(|j| {
                        j >= prev_components.len() || components[j] != prev_components[j]
                    });
                    parent_changed_for_row || components[i] != prev_components[i]
                } else {
                    true
                };

                if should_write {
                    worksheet.write_string_with_format(row, i as u16, &display_text, row_format).ok();
                } else {
                    worksheet.write_string_with_format(row, i as u16, "", row_format).ok();
                }
            }

            for col in depth..max_depth {
                worksheet.write_string_with_format(row, col as u16, "", row_format).ok();
            }

            worksheet.write_string_with_format(row, status_col, status_str, row_format).ok();

            let first_level = components.first().cloned().unwrap_or_default();
            row_info.push((depth, true, first_level));

            if depth > 1 {
                let dir_path = components[..depth-1].join("/");
                written_dirs.insert(dir_path);
            }
            prev_components = components;
            row += 1;
        }

        // Apply row grouping for fold levels
        if let Some(level) = fold_level {
            if level > 0 {
                Self::apply_row_grouping_expanded(worksheet, &row_info, level);
            }
        }

        // Set column widths
        for col in 0..=max_depth {
            if col < max_depth {
                worksheet.set_column_width(col as u16, 20.0).ok();
            } else {
                worksheet.set_column_width(col as u16, 15.0).ok();
            }
        }

        Ok(())
    }

    /// Pre-calculate which rows are directory boundaries for three-way entries
    fn calculate_boundary_rows_three_way(entries: &[&ThreeWayEntry]) -> HashSet<u32> {
        let mut boundary_rows = HashSet::new();
        let mut row_info: Vec<String> = Vec::new();
        let mut written_dirs: HashSet<String> = HashSet::new();

        for entry in entries {
            let components: Vec<String> = entry
                .relative_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            let depth = components.len();
            let first_level = components.first().cloned().unwrap_or_default();

            let mut expand_from = 0usize;
            for i in 0..depth.saturating_sub(1) {
                let dir_path = components[..=i].join("/");
                if !written_dirs.contains(&dir_path) {
                    expand_from = i;
                    break;
                }
                expand_from = i + 1;
            }

            let parent_changed = row_info.last().map_or(true, |prev_first| *prev_first != first_level);

            if depth > 1 && (parent_changed || expand_from < depth - 1) {
                for i in expand_from..depth.saturating_sub(1) {
                    let dir_path = components[..=i].join("/");
                    if written_dirs.contains(&dir_path) {
                        continue;
                    }
                    written_dirs.insert(dir_path);
                    row_info.push(first_level.clone());
                }
            }

            if depth > 1 {
                let dir_path = components[..depth-1].join("/");
                written_dirs.insert(dir_path);
            }
            row_info.push(first_level);
        }

        for i in 0..row_info.len() {
            let is_boundary = if i + 1 >= row_info.len() {
                true
            } else {
                row_info[i] != row_info[i + 1]
            };

            if is_boundary {
                let excel_row = (i + 1) as u32;
                boundary_rows.insert(excel_row);
            }
        }

        boundary_rows
    }

    /// Apply row grouping with expanded intermediate directory rows
    fn apply_row_grouping_expanded(worksheet: &mut Worksheet, row_info: &[(usize, bool, String)], fold_level: usize) {
        if row_info.is_empty() {
            return;
        }

        let mut groups: Vec<(u32, u32)> = Vec::new();
        let mut current_group_start: Option<u32> = None;

        for (idx, (depth, _, _)) in row_info.iter().enumerate() {
            let row = (idx + 1) as u32;

            if *depth >= fold_level {
                if current_group_start.is_none() {
                    current_group_start = Some(row);
                }
            } else {
                if let Some(start) = current_group_start {
                    if row > start {
                        groups.push((start, row - 1));
                    }
                }
                current_group_start = None;
            }
        }

        if let Some(start) = current_group_start {
            let last_row = row_info.len() as u32;
            if last_row >= start {
                groups.push((start, last_row));
            }
        }

        for (start, end) in groups {
            worksheet.group_rows(start, end).ok();
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

        // Headers - write individually with format to each column (9 columns total)
        worksheet.write_string_with_format(0, 0, "Directory", &header_format).ok();
        worksheet.write_string_with_format(0, 1, "Filename", &header_format).ok();
        worksheet.write_string_with_format(0, 2, "Type", &header_format).ok();
        worksheet.write_string_with_format(0, 3, "Base Hash", &header_format).ok();
        worksheet.write_string_with_format(0, 4, "Ours Hash", &header_format).ok();
        worksheet.write_string_with_format(0, 5, "Theirs Hash", &header_format).ok();
        worksheet.write_string_with_format(0, 6, "Base Size", &header_format).ok();
        worksheet.write_string_with_format(0, 7, "Ours Size", &header_format).ok();
        worksheet.write_string_with_format(0, 8, "Theirs Size", &header_format).ok();

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

            // Split path into directory and filename
            let path_str = entry.relative_path.display().to_string();
            let (directory, filename) = if let Some(parent) = entry.relative_path.parent() {
                let parent_str = parent.display().to_string();
                let file_str = entry.relative_path.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                if parent_str.is_empty() {
                    ("".to_string(), file_str)
                } else {
                    (parent_str, file_str)
                }
            } else {
                ("".to_string(), path_str)
            };

            worksheet.write_string_with_format(row, 0, &directory, &conflict_format).ok();
            worksheet.write_string_with_format(row, 1, &filename, &conflict_format).ok();
            worksheet.write_string_with_format(row, 2, conflict_type, &conflict_format).ok();

            let base_hash = entry.base_hash.as_ref().map(|h| &h[..8.min(h.len())]).unwrap_or("-");
            let ours_hash = entry.ours_hash.as_ref().map(|h| &h[..8.min(h.len())]).unwrap_or("-");
            let theirs_hash = entry.theirs_hash.as_ref().map(|h| &h[..8.min(h.len())]).unwrap_or("-");

            worksheet.write_string(row, 3, base_hash).ok();
            worksheet.write_string(row, 4, ours_hash).ok();
            worksheet.write_string(row, 5, theirs_hash).ok();

            let base_size = entry.base_size.map(|s| s.to_string()).unwrap_or("-".to_string());
            let ours_size = entry.ours_size.map(|s| s.to_string()).unwrap_or("-".to_string());
            let theirs_size = entry.theirs_size.map(|s| s.to_string()).unwrap_or("-".to_string());

            worksheet.write_string(row, 6, &base_size).ok();
            worksheet.write_string(row, 7, &ours_size).ok();
            worksheet.write_string(row, 8, &theirs_size).ok();

            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 30.0).ok();  // Directory
        worksheet.set_column_width(1, 25.0).ok();  // Filename
        worksheet.set_column_width(2, 20.0).ok();  // Type
        worksheet.set_column_width(3, 15.0).ok();  // Base Hash
        worksheet.set_column_width(4, 15.0).ok();  // Ours Hash
        worksheet.set_column_width(5, 15.0).ok();  // Theirs Hash
        worksheet.set_column_width(6, 12.0).ok();  // Base Size
        worksheet.set_column_width(7, 12.0).ok();  // Ours Size
        worksheet.set_column_width(8, 12.0).ok();  // Theirs Size

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

        // Headers - write individually with format to each column (4 columns total)
        worksheet.write_string_with_format(0, 0, "Directory", &header_format).ok();
        worksheet.write_string_with_format(0, 1, "Filename", &header_format).ok();
        worksheet.write_string_with_format(0, 2, "Status", &header_format).ok();
        worksheet.write_string_with_format(0, 3, "Source", &header_format).ok();

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

            // Split path into directory and filename
            let path_str = entry.relative_path.display().to_string();
            let (directory, filename) = if let Some(parent) = entry.relative_path.parent() {
                let parent_str = parent.display().to_string();
                let file_str = entry.relative_path.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                if parent_str.is_empty() {
                    ("".to_string(), file_str)
                } else {
                    (parent_str, file_str)
                }
            } else {
                ("".to_string(), path_str)
            };

            worksheet.write_string(row, 0, &directory).ok();
            worksheet.write_string(row, 1, &filename).ok();
            worksheet.write_string(row, 2, entry.status.as_str()).ok();
            worksheet.write_string(row, 3, source).ok();

            row += 1;
        }

        // Set column widths
        worksheet.set_column_width(0, 30.0).ok();  // Directory
        worksheet.set_column_width(1, 25.0).ok();  // Filename
        worksheet.set_column_width(2, 20.0).ok();  // Status
        worksheet.set_column_width(3, 25.0).ok();  // Source

        Ok(())
    }
}
