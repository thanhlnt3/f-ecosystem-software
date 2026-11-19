//! Báo cáo Markdown (tiếng Việt) cho người xem xét.

use std::{collections::BTreeMap, fmt::Write};

use crate::{OsvStatus, Report, Severity};

/// Ngày UTC hiện tại dạng `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    civil(secs / 86_400)
}

/// Ngày lịch từ số ngày kể từ 1970-01-01 (thuật toán của Howard Hinnant).
fn civil(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

pub fn render(report: &Report, target: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Báo cáo quét mã nguồn\n");
    let _ = writeln!(out, "- Mục tiêu: `{target}`");
    let _ = writeln!(out, "- Ngày quét: {}", today());
    let _ = writeln!(
        out,
        "- Tệp đã quét: {} ({} KB){}",
        report.files_scanned,
        report.bytes_scanned / 1024,
        if report.truncated {
            " — **dừng sớm vì vượt giới hạn**"
        } else {
            ""
        }
    );
    if report.skipped_large > 0 {
        let _ = writeln!(out, "- Bỏ qua {} tệp lớn hơn 2 MB", report.skipped_large);
    }
    let osv = match &report.osv {
        OsvStatus::Checked => "đã tra".to_owned(),
        OsvStatus::Skipped => "bỏ qua".to_owned(),
        OsvStatus::Failed(r) => format!("**thất bại**: {r}"),
    };
    let _ = writeln!(out, "- OSV.dev: {osv}");
    let mut by_eco: BTreeMap<&str, usize> = BTreeMap::new();
    for d in &report.dependencies {
        *by_eco.entry(d.ecosystem.as_str()).or_default() += 1;
    }
    let deps = by_eco
        .iter()
        .map(|(e, n)| format!("{e} {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        out,
        "- Thư viện đọc từ lockfile: {} {}",
        report.dependencies.len(),
        if deps.is_empty() {
            String::new()
        } else {
            format!("({deps})")
        }
    );
    if !report.licenses.is_empty() {
        let list = report
            .licenses
            .iter()
            .map(|l| format!("{} (`{}`)", l.spdx, l.path))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "- Giấy phép: {list}");
    }
    let _ = writeln!(out, "\n## Số phát hiện\n");
    let _ = writeln!(out, "| Mức | Còn lại | Đã cho phép |\n|---|---|---|");
    for s in Severity::ALL {
        let allowed = report
            .suppressed
            .iter()
            .filter(|x| x.finding.severity == s)
            .count();
        let _ = writeln!(out, "| {} | {} | {allowed} |", s.as_str(), report.count(s));
    }
    let _ = writeln!(out, "\n## Phát hiện\n");
    if report.findings.is_empty() {
        let _ = writeln!(out, "Không có phát hiện.");
    } else {
        let _ = writeln!(
            out,
            "| Mức | Luật | Tệp | Mô tả | Trích đoạn |\n|---|---|---|---|---|"
        );
        for f in &report.findings {
            let place = if f.line > 0 {
                format!("{}:{}", f.path, f.line)
            } else {
                f.path.clone()
            };
            let _ = writeln!(
                out,
                "| {} | `{}` | `{}` | {} | {} |",
                f.severity.as_str(),
                f.rule,
                cell(&place),
                cell(&f.message),
                f.excerpt
                    .as_deref()
                    .map(|e| format!("`{}`", cell(e)))
                    .unwrap_or_default()
            );
        }
    }
    if !report.suppressed.is_empty() {
        let _ = writeln!(out, "\n## Đã xem xét và cho phép\n");
        let _ = writeln!(out, "| Mức | Luật | Tệp | Lý do |\n|---|---|---|---|");
        for s in &report.suppressed {
            let f = &s.finding;
            let _ = writeln!(
                out,
                "| {} | `{}` | `{}:{}` | {} |",
                f.severity.as_str(),
                f.rule,
                cell(&f.path),
                f.line,
                cell(&s.reason)
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_411), "2025-11-19");
        assert_eq!(civil(19_782), "2024-02-29");
    }

    #[test]
    fn renders_tables() {
        let r = Report {
            findings: vec![crate::Finding {
                rule: "secret.github_token".into(),
                category: crate::Category::Secret,
                severity: Severity::Critical,
                path: "src/a|b.ts".into(),
                line: 3,
                message: "Token GitHub".into(),
                excerpt: Some("ghp_****".into()),
            }],
            ..Report::default()
        };
        let md = render(&r, "github:o/r@abc");
        assert!(md.contains("| critical | 1 | 0 |"), "{md}");
        assert!(md.contains("`src/a\\|b.ts:3`"), "{md}");
        assert!(md.contains("`ghp_****`"));
    }
}
