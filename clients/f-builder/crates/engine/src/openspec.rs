//! OpenSpec in a folder a project starts from (tasks.md 11.2, spec app-builder "Tạo từ OpenSpec"):
//! `openspec/changes/<name>/` with `proposal.md`, `design.md`, `tasks.md` and
//! `specs/<capability>/spec.md`. Only the layout is read here; the builder reads the files.

use std::path::{Path, PathBuf};

/// One change under `openspec/changes/` (archived ones are left out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The change's folder name.
    pub name: String,
    /// Its folder, relative to the project (`openspec/changes/<name>`), with `/` separators.
    pub dir: String,
    /// Its capabilities: the folders under `specs/` that hold a `spec.md`.
    pub capabilities: Vec<String>,
    pub has_proposal: bool,
    pub has_design: bool,
    pub has_tasks: bool,
    /// How many `### Requirement` and `#### Scenario` headings its specs have.
    pub requirements: usize,
    pub scenarios: usize,
}

/// The most changes and capabilities read from one folder.
const MAX: usize = 200;

/// The OpenSpec changes in `root`, by name. Empty when it has none.
pub fn changes(root: &Path) -> Vec<Change> {
    let dir = root.join("openspec").join("changes");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<Change> = entries
        .flatten()
        .take(MAX)
        .filter(|e| e.path().is_dir() && e.file_name() != "archive")
        .filter_map(|e| change(&e.path(), &e.file_name().to_string_lossy()))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn change(path: &Path, name: &str) -> Option<Change> {
    let has = |f: &str| path.join(f).is_file();
    let mut capabilities = Vec::new();
    let (mut requirements, mut scenarios) = (0, 0);
    if let Ok(specs) = std::fs::read_dir(path.join("specs")) {
        for e in specs.flatten().take(MAX) {
            let spec: PathBuf = e.path().join("spec.md");
            let Ok(text) = std::fs::read_to_string(&spec) else { continue };
            capabilities.push(e.file_name().to_string_lossy().into_owned());
            requirements += text.lines().filter(|l| l.trim_start().starts_with("### Requirement")).count();
            scenarios += text.lines().filter(|l| l.trim_start().starts_with("#### Scenario")).count();
        }
    }
    capabilities.sort();
    let (has_proposal, has_design, has_tasks) = (has("proposal.md"), has("design.md"), has("tasks.md"));
    // A folder with none of a change's parts isn't one.
    if capabilities.is_empty() && !has_proposal && !has_tasks {
        return None;
    }
    Some(Change { name: name.to_string(), dir: format!("openspec/changes/{name}"), capabilities, has_proposal, has_design, has_tasks, requirements, scenarios })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn changes_are_found_with_their_capabilities_and_counts() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "openspec/changes/add-chi-tieu/proposal.md", "# Proposal");
        write(r, "openspec/changes/add-chi-tieu/tasks.md", "- [ ] 1.1 Làm");
        write(
            r,
            "openspec/changes/add-chi-tieu/specs/budget/spec.md",
            "## ADDED Requirements\n### Requirement: Ghi khoản chi\n#### Scenario: Thêm\n#### Scenario: Xóa\n### Requirement: Biểu đồ\n#### Scenario: Theo tháng\n",
        );
        write(r, "openspec/changes/add-chi-tieu/specs/sync/spec.md", "### Requirement: Đồng bộ\n#### Scenario: Ngoại tuyến\n");
        write(r, "openspec/changes/archive/2026-01-01-old/proposal.md", "# Old");
        std::fs::create_dir_all(r.join("openspec/changes/empty")).unwrap();
        let found = changes(r);
        assert_eq!(found.len(), 1, "archive and empty folders are not changes: {found:?}");
        let c = &found[0];
        assert_eq!((c.name.as_str(), c.dir.as_str()), ("add-chi-tieu", "openspec/changes/add-chi-tieu"));
        assert_eq!(c.capabilities, ["budget", "sync"]);
        assert_eq!((c.requirements, c.scenarios), (3, 4));
        assert!(c.has_proposal && c.has_tasks && !c.has_design);
    }

    #[test]
    fn a_folder_without_openspec_has_no_changes() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "notes.md", "x");
        assert!(changes(d.path()).is_empty());
    }
}
