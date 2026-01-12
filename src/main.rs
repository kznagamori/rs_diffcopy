mod cli;
mod comparator;
mod config;
mod copier;
mod error;
mod excel;
mod patch;
mod progress;
mod safety;
mod scanner;
mod summary;
mod three_way;
mod three_way_excel;
mod three_way_summary;
mod types;
mod utils;

use clap::Parser;
use std::process::ExitCode;

use cli::Cli;
use comparator::Comparator;
use config::Config;
use copier::Copier;
use error::DiffCopyError;
use excel::ExcelWriter;
use patch::PatchGenerator;
use progress::ProgressManager;
use safety::{check_output_directory, validate_directories};
use summary::SummaryWriter;
use three_way::ThreeWayComparator;
use three_way_excel::ThreeWayExcelWriter;
use three_way_summary::ThreeWaySummaryWriter;

/// Windows console codepage manager
/// Sets console output codepage to UTF-8 (65001) on creation and restores on drop
#[cfg(windows)]
struct WindowsConsoleCodepage {
    original_output_cp: u32,
}

#[cfg(windows)]
impl WindowsConsoleCodepage {
    fn new() -> Self {
        use windows_sys::Win32::System::Console::{GetConsoleOutputCP, SetConsoleOutputCP};

        let original_output_cp = unsafe { GetConsoleOutputCP() };

        // Set output codepage to UTF-8 (65001)
        unsafe {
            SetConsoleOutputCP(65001);
        }

        Self { original_output_cp }
    }
}

#[cfg(windows)]
impl Drop for WindowsConsoleCodepage {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Console::SetConsoleOutputCP;

        // Restore original codepage
        unsafe {
            SetConsoleOutputCP(self.original_output_cp);
        }
    }
}

fn main() -> ExitCode {
    // Set Windows console to UTF-8 for proper Box Drawing character display
    #[cfg(windows)]
    let _codepage_guard = WindowsConsoleCodepage::new();

    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(1)
        }
    }
}

fn run() -> error::Result<ExitCode> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Build configuration
    let config = Config::from_cli(&cli)?;

    // Validate directories exist
    validate_directories(
        &config.source,
        &config.target,
        config.base.as_deref(),
    )?;

    // Check output directory
    check_output_directory(&config.output, config.force, config.dry_run)?;

    // Set up colored output
    setup_colored_output(&config);

    // Run appropriate mode
    let exit_code = if config.three_way {
        run_three_way(&config)?
    } else {
        run_two_way(&config)?
    };

    // Save config if requested
    if let Some(ref save_path) = config.save_config {
        let config_content = config.to_toml_string();
        std::fs::write(save_path, config_content).map_err(|e| {
            DiffCopyError::FileWriteError {
                path: save_path.clone(),
                message: e.to_string(),
            }
        })?;
        eprintln!("Configuration saved to: {}", save_path.display());
    }

    Ok(exit_code)
}

fn setup_colored_output(config: &Config) {
    match config.color {
        types::ColorMode::Always => {
            colored::control::set_override(true);
        }
        types::ColorMode::Never => {
            colored::control::set_override(false);
        }
        types::ColorMode::Auto => {
            // Let colored handle auto-detection
        }
    }
}

fn run_two_way(config: &Config) -> error::Result<ExitCode> {
    let has_patch = config.patch || config.patch_file.is_some();
    let mut progress = ProgressManager::new(config.verbose, has_patch);

    // Phase 1: Scanning
    let scanning_phase = progress.start_scanning();
    let comparator = Comparator::new(config)?;
    scanning_phase.finish();

    // Phase 2: Comparing
    let source_count = count_items(&config.source);
    let target_count = count_items(&config.target);
    let estimated_items = source_count.max(target_count);
    progress.finish_scanning(estimated_items);

    let comparing_phase = progress.start_comparing(estimated_items);
    let mut result = comparator.compare(&comparing_phase)?;
    comparing_phase.finish();
    progress.finish_comparing(result.entries.len());

    // Phase 3: Copying
    let copy_count = result
        .entries
        .iter()
        .filter(|e| {
            matches!(
                e.status,
                types::FileStatus::Added
                    | types::FileStatus::Modified
                    | types::FileStatus::Permission
            ) || (e.status == types::FileStatus::Deleted && config.copy_deleted)
        })
        .count();

    let copying_phase = progress.start_copying(copy_count);
    let copier = Copier::new(config);
    copier.copy(&mut result, &copying_phase)?;
    copying_phase.finish();

    let copied_count = result.copy_results.iter().filter(|r| r.success).count();
    progress.finish_copying(copied_count);

    // Phase 4: Patching (if enabled)
    if has_patch && !config.dry_run {
        let patch_count = result
            .entries
            .iter()
            .filter(|e| e.status == types::FileStatus::Modified && !e.is_directory)
            .count();

        let patching_phase = progress.start_patching(patch_count);
        let patcher = PatchGenerator::new(config);
        patcher.generate(&mut result, &patching_phase)?;
        patching_phase.finish();

        let generated_count = result.patch_results.iter().filter(|r| r.generated).count();
        progress.finish_patching(generated_count);
    }

    // Phase 5: Summary
    let summary_phase = progress.start_summary();

    // Write summary
    let summary_writer = SummaryWriter::new(config);
    summary_writer.write(&result)?;

    // Write Excel report
    let excel_writer = ExcelWriter::new(config);
    excel_writer.write(&result)?;

    summary_phase.finish();
    progress.finish_summary();

    // Determine exit code
    let exit_code = if result.stats.has_differences() {
        ExitCode::from(0) // Differences found
    } else {
        ExitCode::from(2) // No differences
    };

    Ok(exit_code)
}

fn run_three_way(config: &Config) -> error::Result<ExitCode> {
    // Validate base directory
    if config.base.is_none() {
        return Err(DiffCopyError::ThreeWayRequiresBase);
    }

    let mut progress = ProgressManager::new(config.verbose, false);

    // Phase 1: Scanning
    let scanning_phase = progress.start_scanning();
    let comparator = ThreeWayComparator::new(config)?;
    scanning_phase.finish();

    // Phase 2: Comparing
    let base_count = config.base.as_ref().map(|p| count_items(p)).unwrap_or(0);
    let source_count = count_items(&config.source);
    let target_count = count_items(&config.target);
    let estimated_items = base_count.max(source_count).max(target_count);
    progress.finish_scanning(estimated_items);

    let comparing_phase = progress.start_comparing(estimated_items);
    let mut result = comparator.compare(&comparing_phase)?;
    comparing_phase.finish();
    progress.finish_comparing(result.entries.len());

    // Phase 3: Copying
    let copy_count = result.entries.len();
    let copying_phase = progress.start_copying(copy_count);
    comparator.copy(&mut result, &copying_phase)?;
    copying_phase.finish();

    let copied_count = result.copy_results.iter().filter(|r| r.success).count();
    progress.finish_copying(copied_count);

    // Phase 4: Summary
    let summary_phase = progress.start_summary();

    // Write summary
    let summary_writer = ThreeWaySummaryWriter::new(config);
    summary_writer.write(&result)?;

    // Write Excel report
    let excel_writer = ThreeWayExcelWriter::new(config);
    excel_writer.write(&result)?;

    summary_phase.finish();
    progress.finish_summary();

    // Determine exit code
    let exit_code = if result.stats.has_conflicts() {
        ExitCode::from(3) // Conflicts found
    } else if result.stats.has_differences() {
        ExitCode::from(0) // Differences found
    } else {
        ExitCode::from(2) // No differences
    };

    Ok(exit_code)
}

fn count_items(path: &std::path::Path) -> usize {
    walkdir::WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .count()
}
