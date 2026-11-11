//! The history of a project's runs (tasks.md 11.2, spec app-builder "Tạo dự án": each project keeps
//! the history of its runs): one JSON line per run in `runs.jsonl`, added when the run ends. The
//! rebuild run is its only writer; lines are only ever appended.

use std::io::Write;
use std::path::Path;

use f_builder_evidence::ToolError;
use serde::{Deserialize, Serialize};

/// Where the history is kept in the project folder.
pub const RUNS_FILE: &str = "runs.jsonl";

/// The version of each line ([`RunRecord::version`]).
pub const RUN_RECORD_VERSION: u32 = 1;

/// The longest message kept in a line.
const MAX_MESSAGE: usize = 4000;

/// How a run began.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// A first build, or Try again on one.
    Start,
    /// Carrying on where an earlier run stopped (Continue, Rerun step).
    Continue,
    /// A change the person asked for on the existing build.
    Change,
}

/// One run, as it ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub version: u32,
    pub started_ms: u64,
    pub ended_ms: u64,
    pub kind: RunKind,
    /// What the person asked for in this run (a change), if anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub ok: bool,
    /// The person paused it.
    pub paused: bool,
    /// What the run said at its end.
    pub outcome: String,
    /// The project's status when it ended (`Ready`, `Paused`, `Needs you`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl RunRecord {
    pub fn new(started_ms: u64, kind: RunKind, message: Option<&str>) -> RunRecord {
        RunRecord {
            version: RUN_RECORD_VERSION,
            started_ms,
            ended_ms: started_ms,
            kind,
            message: message.map(|m| m.chars().take(MAX_MESSAGE).collect()),
            ok: false,
            paused: false,
            outcome: String::new(),
            status: None,
        }
    }
}

/// Add `record` to the history of the project at `root`.
pub fn append(root: &Path, record: &RunRecord) -> Result<(), ToolError> {
    let path = root.join(RUNS_FILE);
    let line = serde_json::to_string(record).map_err(|e| ToolError::internal(e.to_string()))?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| ToolError::io(path.display(), e))?;
    writeln!(f, "{line}").map_err(|e| ToolError::io(path.display(), e))
}

/// The project's runs, oldest first. Lines this version can't read (a newer F-Builder's, or a line
/// cut short by a crash) are left out, never written over.
pub fn read(root: &Path) -> Vec<RunRecord> {
    let Ok(text) = std::fs::read_to_string(root.join(RUNS_FILE)) else { return Vec::new() };
    text.lines().filter_map(|l| serde_json::from_str::<RunRecord>(l).ok()).filter(|r| r.version <= RUN_RECORD_VERSION).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_are_added_in_order_and_a_damaged_line_is_skipped() {
        let d = tempfile::tempdir().unwrap();
        assert!(read(d.path()).is_empty());
        let mut first = RunRecord::new(1000, RunKind::Start, None);
        first.ended_ms = 5000;
        first.ok = true;
        first.outcome = "Ready.".into();
        first.status = Some("Ready".into());
        append(d.path(), &first).unwrap();
        // A crash cut a line short, and a newer F-Builder wrote one this version doesn't know.
        std::fs::OpenOptions::new().append(true).open(d.path().join(RUNS_FILE)).unwrap().write_all(b"{\"version\":1,\"start\n").unwrap();
        std::fs::OpenOptions::new().append(true).open(d.path().join(RUNS_FILE)).unwrap().write_all(b"{\"version\":9}\n").unwrap();
        let mut change = RunRecord::new(6000, RunKind::Change, Some("Thêm nút xuất Excel"));
        change.paused = true;
        change.outcome = "Paused.".into();
        append(d.path(), &change).unwrap();
        let runs = read(d.path());
        assert_eq!(runs, [first, change]);
        assert_eq!(runs[1].message.as_deref(), Some("Thêm nút xuất Excel"));
    }
}
