//! Danh sách phát hiện đã xem xét (`scan-allow.toml`):
//!
//! ```toml
//! [[allow]]
//! rule = "secret.private_key"        # tên luật, có thể dùng glob (`secret.*`)
//! path = "tests/fixtures/*.pem"      # glob theo đường dẫn tương đối; `*` không vượt qua `/`
//! reason = "Khóa tự sinh cho test, không dùng ở đâu khác"   # bắt buộc
//! contains = "GHSA-xxxx"             # không bắt buộc: chỉ khớp phát hiện có chuỗi này trong message
//! ```

use globset::{GlobBuilder, GlobMatcher};
use serde::Deserialize;

use crate::{Finding, Report, Suppressed};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileFormat {
    #[serde(default)]
    allow: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    rule: String,
    path: String,
    reason: String,
    #[serde(default)]
    contains: Option<String>,
}

#[derive(Debug, Clone)]
struct Compiled {
    rule: GlobMatcher,
    path: GlobMatcher,
    reason: String,
    contains: Option<String>,
    label: String,
}

#[derive(Debug, Clone, Default)]
pub struct AllowList {
    entries: Vec<Compiled>,
}

fn glob(pattern: &str, separator: bool) -> Result<GlobMatcher, String> {
    GlobBuilder::new(pattern)
        .literal_separator(separator)
        .build()
        .map(|g| g.compile_matcher())
        .map_err(|e| format!("glob {pattern:?} sai: {e}"))
}

impl AllowList {
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: FileFormat = toml::from_str(text)
            .map_err(|e| format!("scan-allow.toml sai: {}", e.to_string().trim()))?;
        let mut entries = Vec::new();
        for (i, e) in file.allow.into_iter().enumerate() {
            let n = i + 1;
            if e.reason.trim().is_empty() {
                return Err(format!("mục allow thứ {n} ({}) thiếu reason", e.rule));
            }
            entries.push(Compiled {
                rule: glob(&e.rule, false).map_err(|x| format!("mục allow thứ {n}: {x}"))?,
                path: glob(&e.path, true).map_err(|x| format!("mục allow thứ {n}: {x}"))?,
                label: format!("#{n} {} {}", e.rule, e.path),
                reason: e.reason.trim().to_owned(),
                contains: e.contains.filter(|c| !c.is_empty()),
            });
        }
        Ok(AllowList { entries })
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("không đọc được {}: {e}", path.display()))?;
        Self::parse(&text)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn find(&self, f: &Finding) -> Option<usize> {
        self.entries.iter().position(|e| {
            e.rule.is_match(&f.rule)
                && e.path.is_match(&f.path)
                && e.contains.as_ref().is_none_or(|c| f.message.contains(c))
        })
    }

    /// Lý do cho phép phát hiện `f`, nếu có.
    pub fn reason(&self, f: &Finding) -> Option<&str> {
        self.find(f).map(|i| self.entries[i].reason.as_str())
    }

    /// Chuyển phát hiện được cho phép sang `report.suppressed`.
    pub fn apply(&self, report: &mut Report) {
        if self.entries.is_empty() {
            return;
        }
        let findings = std::mem::take(&mut report.findings);
        for f in findings {
            match self.find(&f) {
                Some(i) => report.suppressed.push(Suppressed {
                    finding: f,
                    reason: self.entries[i].reason.clone(),
                }),
                None => report.findings.push(f),
            }
        }
    }

    /// Mục không khớp phát hiện nào trong báo cáo (có thể đã cũ).
    pub fn unused(&self, report: &Report) -> Vec<String> {
        let mut used = vec![false; self.entries.len()];
        for s in &report.suppressed {
            if let Some(i) = self.find(&s.finding) {
                used[i] = true;
            }
        }
        self.entries
            .iter()
            .zip(used)
            .filter(|(_, u)| !u)
            .map(|(e, _)| e.label.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Category, Severity};

    fn finding(rule: &str, path: &str, message: &str) -> Finding {
        Finding {
            rule: rule.into(),
            category: Category::Secret,
            severity: Severity::Critical,
            path: path.into(),
            line: 1,
            message: message.into(),
            excerpt: None,
        }
    }

    #[test]
    fn allow_needs_reason_and_matches_globs() {
        assert!(
            AllowList::parse("[[allow]]\nrule = \"x\"\npath = \"a\"\nreason = \" \"\n")
                .unwrap_err()
                .contains("thiếu reason")
        );
        assert!(AllowList::parse("[[allow]]\nrule = \"x\"\npath = \"a\"\n").is_err());
        assert!(AllowList::parse(
            "[[allow]]\nrule = \"x\"\npath = \"a\"\nreason = \"r\"\nextra = 1\n"
        )
        .is_err());
        let list = AllowList::parse(
            "[[allow]]\nrule = \"secret.*\"\npath = \"docs/*.md\"\nreason = \"Ví dụ trong tài liệu\"\n\n\
             [[allow]]\nrule = \"vulnerability.osv\"\npath = \"**/package-lock.json\"\ncontains = \"GHSA-1\"\nreason = \"Chỉ dùng khi dựng\"\n",
        )
        .unwrap();
        let mut report = Report {
            findings: vec![
                finding("secret.github_token", "docs/a.md", "x"),
                finding("secret.github_token", "docs/sub/a.md", "x"),
                finding(
                    "vulnerability.osv",
                    "web/package-lock.json",
                    "lodash GHSA-1",
                ),
                finding(
                    "vulnerability.osv",
                    "web/package-lock.json",
                    "lodash GHSA-2",
                ),
            ],
            ..Report::default()
        };
        list.apply(&mut report);
        assert_eq!(report.suppressed.len(), 2);
        assert_eq!(report.suppressed[0].reason, "Ví dụ trong tài liệu");
        let left: Vec<_> = report.findings.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(left, ["docs/sub/a.md", "web/package-lock.json"]);
        assert!(list.unused(&report).is_empty());
        assert_eq!(list.unused(&Report::default()).len(), 2);
    }
}
