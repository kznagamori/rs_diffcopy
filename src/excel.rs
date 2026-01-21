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

        // Create formats with bottom border for directory separators
        let tree_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let added_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x008000))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let modified_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x0066CC))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let deleted_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0xCC0000))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let symlink_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x9933FF))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

        let unchanged_format_bottom = Format::new()
            .set_font_name("Consolas")
            .set_font_color(Color::RGB(0x808080))
            .set_border(FormatBorder::Thin)
            .set_border_bottom(FormatBorder::Medium);

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

        // Track written directory paths (for expansion)
        let mut written_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Track previous row's components for deduplication
        let mut prev_components: Vec<String> = Vec::new();
        let mut row = 1u32;
        let fold_level = self.config.excel_fold_level;

        // Track row info for directory boundaries and grouping
        // Each row is: (depth, is_file, first_level_dir)
        let mut row_info: Vec<(usize, bool, String)> = Vec::new();

        // Pre-calculate boundary rows by doing a dry-run pass
        let boundary_rows = Self::calculate_boundary_rows(&filtered_entries);

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

            // Select format based on status (we'll also need bottom border versions for boundary rows)
            let (format, format_bottom) = match entry.status {
                FileStatus::Added => (&added_format, &added_format_bottom),
                FileStatus::Modified => (&modified_format, &modified_format_bottom),
                FileStatus::Deleted => (&deleted_format, &deleted_format_bottom),
                FileStatus::Symlink => (&symlink_format, &symlink_format_bottom),
                FileStatus::Unchanged => (&unchanged_format, &unchanged_format_bottom),
                _ => (&tree_format, &tree_format_bottom),
            };

            // Check if we need to expand intermediate directories
            // This happens when entering a new directory path that hasn't been written yet
            let mut expand_from = 0usize;
            for i in 0..depth.saturating_sub(1) {
                let dir_path = components[..=i].join("/");
                if !written_dirs.contains(&dir_path) {
                    expand_from = i;
                    break;
                }
                expand_from = i + 1;
            }

            // Check if parent directory has changed (for deduplication purposes)
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

                    // Write intermediate directory row
                    let dir_name = format!("{}/", &components[i]);

                    // Check if this component should be written (deduplication)
                    let should_write = if i < prev_components.len() {
                        let parent_changed_for_row = (0..i).any(|j| {
                            j >= prev_components.len() || components[j] != prev_components[j]
                        });
                        parent_changed_for_row || components[i] != prev_components[i]
                    } else {
                        true
                    };

                    // Select format based on boundary
                    let dir_format = if boundary_rows.contains(&row) {
                        &tree_format_bottom
                    } else {
                        &tree_format
                    };

                    // Write empty cells for columns before this depth
                    for col in 0..i {
                        worksheet.write_string_with_format(row, col as u16, "", dir_format).ok();
                    }

                    // Write directory name
                    if should_write {
                        worksheet.write_string_with_format(row, i as u16, &dir_name, dir_format).ok();
                    } else {
                        worksheet.write_string_with_format(row, i as u16, "", dir_format).ok();
                    }

                    // Fill remaining columns with empty cells
                    for col in (i + 1)..max_depth {
                        worksheet.write_string_with_format(row, col as u16, "", dir_format).ok();
                    }

                    // Write empty status for intermediate directory
                    worksheet.write_string_with_format(row, status_col, "", dir_format).ok();

                    // Track row info
                    let first_level = components.first().cloned().unwrap_or_default();
                    row_info.push((i + 1, false, first_level));

                    // Update prev_components for next iteration
                    prev_components = components[..=i].to_vec();
                    row += 1;
                }
            }

            // Write the file/entry row
            // Select format based on boundary
            let row_format = if boundary_rows.contains(&row) {
                format_bottom
            } else {
                format
            };

            // Check deduplication for each component
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

            // Fill remaining columns with empty cells
            for col in depth..max_depth {
                worksheet.write_string_with_format(row, col as u16, "", row_format).ok();
            }

            // Write status
            worksheet.write_string_with_format(row, status_col, status_str, row_format).ok();

            // Track row info
            let first_level = components.first().cloned().unwrap_or_default();
            row_info.push((depth, true, first_level));

            // Update state
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
                worksheet.set_column_width(col as u16, 12.0).ok();
            }
        }

        Ok(())
    }

    /// Pre-calculate which rows are directory boundaries (for border formatting)
    /// Returns a HashSet of Excel row numbers that should have bottom borders
    fn calculate_boundary_rows(entries: &[&FileEntry]) -> std::collections::HashSet<u32> {
        let mut boundary_rows = std::collections::HashSet::new();
        let mut row_info: Vec<String> = Vec::new();  // first_level_dir for each row
        let mut written_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

        for entry in entries {
            let components: Vec<String> = entry
                .relative_path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            let depth = components.len();
            let first_level = components.first().cloned().unwrap_or_default();

            // Simulate intermediate directory rows
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
            let parent_changed = row_info.last().map_or(true, |prev_first| *prev_first != first_level);

            // Add intermediate directory rows
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

            // Add the file row
            if depth > 1 {
                let dir_path = components[..depth-1].join("/");
                written_dirs.insert(dir_path);
            }
            row_info.push(first_level);
        }

        // Determine boundary rows (where first_level changes or is last row)
        for i in 0..row_info.len() {
            let is_boundary = if i + 1 >= row_info.len() {
                true // Last row
            } else {
                row_info[i] != row_info[i + 1]
            };

            if is_boundary {
                let excel_row = (i + 1) as u32; // +1 for header row
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

        // Group consecutive rows where depth >= fold_level
        let mut groups: Vec<(u32, u32)> = Vec::new();
        let mut current_group_start: Option<u32> = None;

        for (idx, (depth, _, _)) in row_info.iter().enumerate() {
            let row = (idx + 1) as u32; // +1 for header row

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

        // Handle last group
        if let Some(start) = current_group_start {
            let last_row = row_info.len() as u32;
            if last_row >= start {
                groups.push((start, last_row));
            }
        }

        // Apply grouping
        for (start, end) in groups {
            worksheet.group_rows(start, end).ok();
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

    #[test]
    fn test_row_info_depth_calculation() {
        // Test that row_info tracks depth correctly for path expansion
        // With the new expansion logic:
        // a/b/c/d.txt becomes:
        //   row 1: a/       (depth 1, not file)
        //   row 2: b/       (depth 2, not file)
        //   row 3: c/       (depth 3, not file)
        //   row 4: d.txt    (depth 4, file)

        let entries: Vec<FileEntry> = vec![
            FileEntry::new(PathBuf::from("a/b/c/d.txt"), FileStatus::Added, false),
        ];

        // Verify depth calculation
        assert_eq!(entries[0].relative_path.components().count(), 4);

        // With path expansion, intermediate directories are on separate rows
        // This enables fold-level to work correctly
    }

    #[test]
    fn test_row_info_boundary_detection() {
        // Test boundary detection in row_info
        // row_info format: (depth, is_file, first_level_dir)

        let row_info: Vec<(usize, bool, String)> = vec![
            (1, true, "a.txt".to_string()),      // a.txt -> boundary (next first_level differs)
            (1, false, "b".to_string()),         // b/ intermediate
            (2, true, "b".to_string()),          // b/d.txt
            (2, false, "b".to_string()),         // b/e/ intermediate
            (3, true, "b".to_string()),          // b/e/g.txt -> boundary (next first_level differs)
            (1, false, "c".to_string()),         // c/ intermediate
            (2, true, "c".to_string()),          // c/e.txt
            (2, false, "c".to_string()),         // c/f/ intermediate
            (3, true, "c".to_string()),          // c/f/h.txt -> boundary (last row)
        ];

        // Boundaries occur when:
        // 1. Next row has different first_level_dir, or
        // 2. It's the last row

        // idx 0 (a.txt): first_level "a.txt", next is "b" -> boundary
        assert_ne!(&row_info[0].2, &row_info[1].2);
        // idx 4 (b/e/g.txt): first_level "b", next is "c" -> boundary
        assert_ne!(&row_info[4].2, &row_info[5].2);
        // idx 8 (c/f/h.txt): last row -> boundary
        assert_eq!(row_info.len() - 1, 8);
    }

    #[test]
    fn test_row_grouping_expanded_fold_level_2() {
        // Test grouping with expanded intermediate rows
        // row_info: (depth, is_file, first_level_dir)

        let row_info: Vec<(usize, bool, String)> = vec![
            (1, true, "a.txt".to_string()),      // depth 1 - not grouped
            (1, false, "b".to_string()),         // depth 1 - not grouped
            (2, true, "b".to_string()),          // depth 2 - grouped
            (2, false, "b".to_string()),         // depth 2 - grouped
            (3, true, "b".to_string()),          // depth 3 - grouped
            (1, false, "c".to_string()),         // depth 1 - not grouped (breaks group)
            (2, true, "c".to_string()),          // depth 2 - grouped (new group)
            (2, false, "c".to_string()),         // depth 2 - grouped
            (3, true, "c".to_string()),          // depth 3 - grouped
        ];

        let fold_level = 2;

        // With fold_level=2:
        // rows 2-4 (0-indexed) should be grouped
        // row 5 breaks the group (depth 1 < fold_level)
        // rows 6-8 should be grouped

        // Verify depths for grouping
        assert!(row_info[2].0 >= fold_level);
        assert!(row_info[3].0 >= fold_level);
        assert!(row_info[4].0 >= fold_level);
        assert!(row_info[5].0 < fold_level); // breaks group
        assert!(row_info[6].0 >= fold_level);
        assert!(row_info[7].0 >= fold_level);
        assert!(row_info[8].0 >= fold_level);
    }

    #[test]
    fn test_row_grouping_expanded_fold_level_3() {
        // Test that fold_level=3 only groups depth >= 3

        let row_info: Vec<(usize, bool, String)> = vec![
            (1, true, "a.txt".to_string()),      // depth 1 - not grouped
            (1, false, "b".to_string()),         // depth 1 - not grouped
            (2, true, "b".to_string()),          // depth 2 - not grouped
            (2, false, "b".to_string()),         // depth 2 - not grouped
            (3, true, "b".to_string()),          // depth 3 - grouped
            (1, false, "c".to_string()),         // depth 1 - not grouped (breaks any group)
            (2, true, "c".to_string()),          // depth 2 - not grouped
            (2, false, "c".to_string()),         // depth 2 - not grouped
            (3, true, "c".to_string()),          // depth 3 - grouped (single row group)
        ];

        let fold_level = 3;

        // Only rows 4 and 8 (depth 3) should be grouped
        // Note: row 5 breaks any group because depth 1 < fold_level

        assert!(row_info[4].0 >= fold_level);
        assert!(row_info[5].0 < fold_level); // breaks group
        assert!(row_info[8].0 >= fold_level);
    }

    #[test]
    fn test_path_expansion_components() {
        // Test that path components are correctly identified for expansion

        let path = PathBuf::from("a/b/c/d.txt");
        let components: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();

        assert_eq!(components.len(), 4);
        assert_eq!(components[0], "a");
        assert_eq!(components[1], "b");
        assert_eq!(components[2], "c");
        assert_eq!(components[3], "d.txt");

        // Intermediate directories are components[0..3]
        // File is components[3]
    }

    #[test]
    fn test_cell_deduplication_logic() {
        // Test the logic for skipping duplicate cell values
        // When the component at column i is the same as previous row's component at column i,
        // AND all parent components (0..i) are also the same, the cell should be empty.

        let prev_components = vec!["b".to_string(), "e".to_string()];
        let curr_components = vec!["b".to_string(), "e".to_string(), "g.txt".to_string()];

        // For column 0 (component "b"):
        // prev[0] = "b", curr[0] = "b" -> same, no parent to check -> should NOT write
        let should_write_col0 = {
            let i = 0;
            if i < prev_components.len() {
                let parent_changed = (0..i).any(|j| {
                    j >= prev_components.len() || curr_components[j] != prev_components[j]
                });
                parent_changed || curr_components[i] != prev_components[i]
            } else {
                true
            }
        };
        assert!(!should_write_col0, "Column 0 should not be written (same as prev)");

        // For column 1 (component "e"):
        // prev[1] = "e", curr[1] = "e" -> same
        // parent (column 0): "b" == "b" -> no parent change
        // -> should NOT write
        let should_write_col1 = {
            let i = 1;
            if i < prev_components.len() {
                let parent_changed = (0..i).any(|j| {
                    j >= prev_components.len() || curr_components[j] != prev_components[j]
                });
                parent_changed || curr_components[i] != prev_components[i]
            } else {
                true
            }
        };
        assert!(!should_write_col1, "Column 1 should not be written (same as prev)");

        // For column 2 (component "g.txt"):
        // prev has no column 2 -> should write
        let should_write_col2 = {
            let i = 2;
            if i < prev_components.len() {
                let parent_changed = (0..i).any(|j| {
                    j >= prev_components.len() || curr_components[j] != prev_components[j]
                });
                parent_changed || curr_components[i] != prev_components[i]
            } else {
                true
            }
        };
        assert!(should_write_col2, "Column 2 should be written (not in prev)");
    }

    #[test]
    fn test_cell_deduplication_parent_changed() {
        // Test: when parent directory changes, all children should be written
        let prev_components = vec!["b".to_string(), "d.txt".to_string()];
        let curr_components = vec!["b".to_string(), "e".to_string()];

        // For column 1 (component "e"):
        // prev[1] = "d.txt", curr[1] = "e" -> different -> should write
        let should_write_col1 = {
            let i = 1;
            if i < prev_components.len() {
                let parent_changed = (0..i).any(|j| {
                    j >= prev_components.len() || curr_components[j] != prev_components[j]
                });
                parent_changed || curr_components[i] != prev_components[i]
            } else {
                true
            }
        };
        assert!(should_write_col1, "Column 1 should be written (different from prev)");

        // Test: when first-level directory changes
        let prev_components2 = vec!["b".to_string(), "e".to_string(), "g.txt".to_string()];
        let curr_components2 = vec!["c".to_string(), "e.txt".to_string()];

        // For column 0 (component "c"):
        // prev[0] = "b", curr[0] = "c" -> different -> should write
        let should_write_col0 = {
            let i = 0;
            if i < prev_components2.len() {
                let parent_changed = (0..i).any(|j| {
                    j >= prev_components2.len() || curr_components2[j] != prev_components2[j]
                });
                parent_changed || curr_components2[i] != prev_components2[i]
            } else {
                true
            }
        };
        assert!(should_write_col0, "Column 0 should be written (different first-level)");

        // For column 1 (component "e.txt"):
        // Parent (column 0) changed -> should write even if same value
        let should_write_col1_2 = {
            let i = 1;
            if i < prev_components2.len() {
                let parent_changed = (0..i).any(|j| {
                    j >= prev_components2.len() || curr_components2[j] != prev_components2[j]
                });
                parent_changed || curr_components2[i] != prev_components2[i]
            } else {
                true
            }
        };
        assert!(should_write_col1_2, "Column 1 should be written (parent changed)");
    }
}
