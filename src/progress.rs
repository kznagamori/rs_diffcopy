use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::utils::{eprintln_cp932, is_piped};

/// Progress display manager
pub struct ProgressManager {
    verbose: bool,
    enabled: bool,
    current_phase: usize,
    total_phases: usize,
}

impl ProgressManager {
    pub fn new(verbose: bool, has_patch: bool) -> Self {
        let total_phases = if has_patch { 5 } else { 4 };
        let enabled = !is_piped();

        Self {
            verbose,
            enabled,
            current_phase: 0,
            total_phases,
        }
    }

    /// Update total phases (call before starting if patch mode changes)
    #[allow(dead_code)]
    pub fn set_has_patch(&mut self, has_patch: bool) {
        self.total_phases = if has_patch { 5 } else { 4 };
    }

    /// Start scanning phase
    pub fn start_scanning(&mut self) -> ProgressPhase {
        self.current_phase = 1;
        let pb = self.create_spinner("Scanning Directories...");
        ProgressPhase::new(pb, self.verbose)
    }

    /// Finish scanning phase
    pub fn finish_scanning(&self, count: usize) {
        if self.enabled {
            eprintln_cp932(&format!("Found {} items.", count));
        }
    }

    /// Start comparing phase
    pub fn start_comparing(&mut self, total: usize) -> ProgressPhase {
        self.current_phase = 2;
        let pb = self.create_progress_bar(total, "Comparing Files");
        ProgressPhase::new(pb, self.verbose)
    }

    /// Finish comparing phase
    pub fn finish_comparing(&self, count: usize) {
        if self.enabled {
            eprintln_cp932(&format!("Compared {} items.", count));
        }
    }

    /// Start copying phase
    pub fn start_copying(&mut self, total: usize) -> ProgressPhase {
        self.current_phase = 3;
        let pb = self.create_progress_bar(total, "Copying Files");
        ProgressPhase::new(pb, self.verbose)
    }

    /// Finish copying phase
    pub fn finish_copying(&self, count: usize) {
        if self.enabled {
            eprintln_cp932(&format!("Copied {} files.", count));
        }
    }

    /// Start patch generation phase
    pub fn start_patching(&mut self, total: usize) -> ProgressPhase {
        self.current_phase = 4;
        let pb = self.create_progress_bar(total, "Generating Patches");
        ProgressPhase::new(pb, self.verbose)
    }

    /// Finish patch generation phase
    pub fn finish_patching(&self, count: usize) {
        if self.enabled {
            eprintln_cp932(&format!("Generated {} patches.", count));
        }
    }

    /// Start summary writing phase
    pub fn start_summary(&mut self) -> ProgressPhase {
        self.current_phase = if self.total_phases == 5 { 5 } else { 4 };
        let pb = self.create_spinner("Writing Summary...");
        ProgressPhase::new(pb, self.verbose)
    }

    /// Finish summary phase
    pub fn finish_summary(&self) {
        if self.enabled {
            eprintln_cp932("Completed.");
        }
    }

    fn create_spinner(&self, message: &str) -> Option<ProgressBar> {
        if !self.enabled {
            return None;
        }

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template(&format!(
                    "[{}/{}] {{spinner}} {}",
                    self.current_phase, self.total_phases, message
                ))
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(pb)
    }

    fn create_progress_bar(&self, total: usize, message: &str) -> Option<ProgressBar> {
        if !self.enabled {
            return None;
        }

        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "[{}/{}] {}: [{{bar:40}}] {{percent}}% ({{pos}}/{{len}})",
                    self.current_phase, self.total_phases, message
                ))
                .unwrap()
                .progress_chars("=>-"),
        );
        Some(pb)
    }
}

/// A single progress phase
pub struct ProgressPhase {
    bar: Option<ProgressBar>,
    verbose: bool,
}

impl ProgressPhase {
    fn new(bar: Option<ProgressBar>, verbose: bool) -> Self {
        Self { bar, verbose }
    }

    /// Increment progress by 1
    pub fn inc(&self) {
        if let Some(ref pb) = self.bar {
            pb.inc(1);
        }
    }

    /// Set current progress message (for verbose mode)
    pub fn set_message(&self, msg: &str) {
        if self.verbose {
            if let Some(ref pb) = self.bar {
                pb.set_message(msg.to_string());
            } else {
                eprintln_cp932(&format!("  {}", msg));
            }
        }
    }

    /// Finish this phase
    pub fn finish(&self) {
        if let Some(ref pb) = self.bar {
            pb.finish_and_clear();
        }
    }
}

/// Shared cancellation flag for parallel operations
#[allow(dead_code)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn clone_inner(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}
