//! CPU time used by the programs a process started, for telling a step that is working quietly
//! (a compile, a test run) from one that has stopped (tasks.md 11.3, "Không bị treo im lặng").

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// The CPU time used so far by every process below `root` (its children, theirs, and so on), not
/// counting `root` itself. `None` when the system's process list can't be read.
///
/// Reads the whole process list: call it every few seconds at most, never while drawing.
pub fn descendants_cpu_time(root: u32) -> Option<Duration> {
    let mut sys = System::new().ok()?;
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cpu());
    let tree: Vec<(u32, Option<u32>, u64)> =
        sys.processes().iter().map(|(pid, p)| (pid.as_u32(), p.parent().map(Pid::as_u32), p.accumulated_cpu_time())).collect();
    Some(Duration::from_millis(descendants_ms(&tree, root)))
}

/// Sum the CPU milliseconds of everything below `root` in a `(pid, parent, cpu ms)` list.
fn descendants_ms(tree: &[(u32, Option<u32>, u64)], root: u32) -> u64 {
    let mut children: HashMap<u32, Vec<(u32, u64)>> = HashMap::new();
    for &(pid, parent, ms) in tree {
        if let Some(parent) = parent.filter(|p| *p != pid) {
            children.entry(parent).or_default().push((pid, ms));
        }
    }
    let mut total = 0u64;
    let mut seen = HashSet::from([root]);
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        for &(child, ms) in children.get(&pid).map(Vec::as_slice).unwrap_or_default() {
            // A reused pid can make a loop: each process is counted once.
            if seen.insert(child) {
                total = total.saturating_add(ms);
                stack.push(child);
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_processes_below_the_root_are_counted_each_once() {
        // 1 ─ 10 ─ 11 ─ 12, 1 ─ 20; 99 is someone else's; 13 and 14 point at each other.
        let tree = [
            (1, None, 1000),
            (10, Some(1), 5),
            (11, Some(10), 7),
            (12, Some(11), 11),
            (20, Some(1), 13),
            (99, None, 500),
            (13, Some(14), 1),
            (14, Some(13), 1),
        ];
        assert_eq!(descendants_ms(&tree, 1), 5 + 7 + 11 + 13);
        assert_eq!(descendants_ms(&tree, 11), 11);
        assert_eq!(descendants_ms(&tree, 12), 0);
        assert_eq!(descendants_ms(&tree, 13), 1, "a loop ends");
    }

    #[test]
    fn a_child_that_works_shows_up() {
        let me = std::process::id();
        let before = descendants_cpu_time(me).unwrap_or_default();
        // A child that spins for a moment.
        let exe = std::env::current_exe().unwrap_or_default();
        let child =
            std::process::Command::new(exe).args(["--exact", "cpu::tests::spin_when_asked", "--ignored", "--nocapture"]).env("F_BUILDER_SPIN", "1").spawn();
        let Ok(mut child) = child else { return };
        let mut most = before;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(50));
            most = most.max(descendants_cpu_time(me).unwrap_or_default());
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(most > before, "a busy child adds CPU time: {before:?} -> {most:?}");
    }

    /// Run by `a_child_that_works_shows_up` in a child process.
    #[test]
    #[ignore = "started by a_child_that_works_shows_up"]
    fn spin_when_asked() {
        if std::env::var("F_BUILDER_SPIN").is_err() {
            return;
        }
        let until = std::time::Instant::now() + Duration::from_secs(3);
        let mut x = 0u64;
        while std::time::Instant::now() < until {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
        assert!(x != 1);
    }
}
