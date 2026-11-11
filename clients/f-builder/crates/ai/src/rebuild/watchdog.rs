//! The no-progress watchdog (tasks.md 11.3, spec app-builder "Không bị treo im lặng"): when a step
//! has made no progress for [`STALL_AFTER`] — nothing said in the run's log, no file changed in the
//! project, and no CPU used by the programs F-Builder started — the person is told and offered
//! Keep waiting, Rerun step or Stop. A step that is busy in any of those ways is never reported.

use std::path::Path;
use std::time::{Duration, Instant};

/// How long a step may make no progress before the person is told.
pub const STALL_AFTER: Duration = Duration::from_secs(10 * 60);

/// How often the run is looked at.
pub const SAMPLE_EVERY: Duration = Duration::from_secs(15);

/// CPU time the started programs must add between two looks to count as working. An idle agent
/// or tool server still wakes now and then; a compile or a test run uses seconds.
const CPU_WORKING: Duration = Duration::from_millis(500);

/// The most files and folders looked at in one pass over the project.
const MAX_ENTRIES: usize = 50_000;

/// Folders whose contents say nothing about the person's project moving on (build outputs,
/// downloaded packages, version control).
const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", ".venv", "__pycache__", "dist", "build", ".gradle"];

/// Decides when a run's step has stalled, from what is sampled every [`SAMPLE_EVERY`].
#[derive(Debug)]
pub struct Watchdog {
    after: Duration,
    files: Option<u64>,
    cpu: Option<Duration>,
    last_progress: Instant,
    last_warned: Option<Instant>,
}

impl Watchdog {
    pub fn new(after: Duration, now: Instant) -> Watchdog {
        Watchdog { after, files: None, cpu: None, last_progress: now, last_warned: None }
    }

    /// Take in one sample: `quiet` is how long since the run last had anything to say, `files` a
    /// fingerprint of the project's files, `cpu` the CPU time used so far by the started programs
    /// (`None` when it can't be read). Returns true when the person should be told now: once per
    /// [`STALL_AFTER`] without progress, again after each further stretch ("Keep waiting").
    pub fn observe(&mut self, now: Instant, quiet: Duration, files: u64, cpu: Option<Duration>) -> bool {
        let files_moved = self.files.is_some_and(|f| f != files);
        let cpu_moved = matches!((self.cpu, cpu), (Some(was), Some(is)) if is.saturating_sub(was) >= CPU_WORKING);
        self.files = Some(files);
        if cpu.is_some() {
            self.cpu = cpu;
        }
        if files_moved || cpu_moved {
            self.last_progress = now;
        }
        let idle = quiet.min(now.saturating_duration_since(self.last_progress));
        if idle < self.after {
            self.last_warned = None;
            return false;
        }
        match self.last_warned {
            Some(t) if now.saturating_duration_since(t) < self.after => false,
            _ => {
                self.last_warned = Some(now);
                true
            }
        }
    }
}

/// A fingerprint of the files under `root` (names, sizes and change times): it changes when any
/// file is written, added or removed. Build outputs and package folders are left out, and at most
/// [`MAX_ENTRIES`] entries are read.
pub fn files_fingerprint(root: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            seen += 1;
            if seen > MAX_ENTRIES {
                return hasher.finish();
            }
            let name = e.file_name();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                if !SKIP_DIRS.iter().any(|s| name == *s) {
                    stack.push(e.path());
                }
                continue;
            }
            e.path().hash(&mut hasher);
            meta.len().hash(&mut hasher);
            if let Ok(t) = meta.modified().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).map_err(std::io::Error::other)) {
                t.as_nanos().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    /// Spec scenario "Bước kiểm tra bị kẹt": nothing said, nothing written, no CPU for 10 minutes.
    #[test]
    fn a_step_with_no_progress_for_ten_minutes_is_reported_once_then_after_each_further_stretch() {
        let t0 = Instant::now();
        let mut dog = Watchdog::new(STALL_AFTER, t0);
        let cpu = Some(Duration::from_secs(5));
        for m in 0..10 {
            assert!(!dog.observe(t0 + MIN * m, MIN * m, 7, cpu), "minute {m}");
        }
        assert!(dog.observe(t0 + MIN * 10, MIN * 10, 7, cpu), "told at 10 minutes");
        assert!(!dog.observe(t0 + MIN * 11, MIN * 11, 7, cpu), "told once");
        // "Keep waiting": still stuck ten minutes later, told again.
        assert!(!dog.observe(t0 + MIN * 19, MIN * 19, 7, cpu));
        assert!(dog.observe(t0 + MIN * 20, MIN * 20, 7, cpu));
    }

    #[test]
    fn any_kind_of_progress_keeps_a_long_step_from_being_reported() {
        let t0 = Instant::now();
        // The log moves: the run's `quiet` stays short.
        let mut dog = Watchdog::new(STALL_AFTER, t0);
        for m in 0..30 {
            assert!(!dog.observe(t0 + MIN * m, Duration::from_secs(20), 7, None), "log, minute {m}");
        }
        // Files change, though nothing is said.
        let mut dog = Watchdog::new(STALL_AFTER, t0);
        for m in 0..30 {
            assert!(!dog.observe(t0 + MIN * m, MIN * m, m as u64, None), "files, minute {m}");
        }
        // A compile uses CPU, though nothing is said or written.
        let mut dog = Watchdog::new(STALL_AFTER, t0);
        for m in 0..30u32 {
            assert!(!dog.observe(t0 + MIN * m, MIN * m, 7, Some(Duration::from_secs(u64::from(m) * 30))), "cpu, minute {m}");
        }
    }

    #[test]
    fn an_idle_program_waking_now_and_then_is_not_progress() {
        let t0 = Instant::now();
        let mut dog = Watchdog::new(STALL_AFTER, t0);
        let mut told = false;
        for m in 0..=10u32 {
            told = dog.observe(t0 + MIN * m, MIN * m, 7, Some(Duration::from_millis(u64::from(m) * 50)));
        }
        assert!(told, "50 ms of CPU a minute is an idle program");
    }

    #[test]
    fn progress_after_a_warning_starts_the_count_again() {
        let t0 = Instant::now();
        let mut dog = Watchdog::new(STALL_AFTER, t0);
        assert!(dog.observe(t0 + MIN * 10, MIN * 10, 7, None));
        assert!(!dog.observe(t0 + MIN * 11, MIN * 11, 8, None), "a file changed");
        assert!(!dog.observe(t0 + MIN * 20, MIN * 20, 8, None), "9 minutes since then");
        assert!(dog.observe(t0 + MIN * 21, MIN * 21, 8, None));
    }

    #[test]
    fn the_fingerprint_changes_with_the_projects_files_but_not_its_build_output() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app/src")).unwrap();
        std::fs::write(dir.path().join("app/src/main.rs"), "fn main() {}").unwrap();
        let a = files_fingerprint(dir.path());
        assert_eq!(a, files_fingerprint(dir.path()), "stable");
        std::fs::create_dir_all(dir.path().join("app/target/debug")).unwrap();
        std::fs::write(dir.path().join("app/target/debug/x"), "build output").unwrap();
        assert_eq!(a, files_fingerprint(dir.path()), "build output is left out");
        std::fs::write(dir.path().join("app/src/main.rs"), "fn main() { println!(); }").unwrap();
        let b = files_fingerprint(dir.path());
        assert_ne!(a, b, "a file was written");
        std::fs::write(dir.path().join("app/src/lib.rs"), "").unwrap();
        assert_ne!(b, files_fingerprint(dir.path()), "a file was added");
    }
}
