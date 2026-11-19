//! Mã nguồn kèm phiên bản app và quét mã nguồn khi duyệt (thay đổi chat-and-source-scan, design
//! S2; spec source-scan "Quét mã nguồn khi duyệt app", app-review "Kiểm tra tự động gồm mã nguồn").
//!
//! Nộp mã nguồn (bảng `source_submissions`, chỉ khi phiên bản đang là bản nháp):
//! - tải tệp zip (web hoặc `PUT /v1/dev/versions/{id}/source`, tối đa 200 MB): `origin = upload`;
//! - repo GitHub công khai và commit (web, hoặc `[source]` trong `f-app.toml`): `origin = git`;
//!   máy chủ tải `{FECO_SOURCE_FETCH_URL}/<owner>/<repo>/zip/<commit>` lúc duyệt.
//!
//! [`review`] chạy trong `review::run_submit` sau khi gói sạch và trước `manual_reason`:
//! có phát hiện `critical` thì từ chối (`reject_code = source_scan`); có `high` hoặc OSV không trả
//! lời thì duyệt tay; không kèm mã nguồn thì ghi "Không có mã nguồn"; tải repo lỗi mạng thì xếp
//! hàng lại. Báo cáo đầy đủ ở `source_submissions.report` (bí mật đã che), tóm tắt ở
//! `app_versions.checks.source`.

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use feco_codescan::{FetchError, OsvClient, Report, ScanOptions, Severity};
use feco_manifest::Manifest;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    developer::{self as dev, problem, DevError, DevResult, Membership},
    i18n::Lang,
    store::Db,
    AppState,
};

/// Mã nguồn tối đa 200 MB (tệp tải lên và zip tải từ GitHub).
pub const MAX_SOURCE_BYTES: u64 = 200 * 1024 * 1024;
/// Số dòng phát hiện tối đa ghi trong lý do từ chối và email.
const MAX_NOTE_LINES: usize = 20;
/// Tải repo lỗi mạng: chạy lại sau 5 phút.
pub const RETRY_SECS: i64 = 300;
pub const NO_SOURCE: &str = "Không có mã nguồn";

fn invalid_source(vi: impl Into<String>, en: impl Into<String>) -> DevError {
    problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_source", vi, en)
}

/// Phiên bản nháp của tài khoản mà thành viên được sửa.
async fn draft(db: &Db, m: &Membership, version_id: Uuid) -> DevResult<dev::VersionInfo> {
    m.need_agreement()?;
    dev::need_edit(m.role)?;
    let (info, _) = dev::version(db, m, version_id).await?;
    if info.status != "draft" {
        return Err(problem(
            StatusCode::CONFLICT,
            "not_draft",
            "Chỉ nộp mã nguồn được cho phiên bản đang ở trạng thái nháp.",
            "Source code can only be attached to a draft version.",
        ));
    }
    Ok(info)
}

/// Nơi lưu mã nguồn tải lên của một phiên bản.
pub fn source_path(state: &AppState, app_id: &str, version: &str) -> PathBuf {
    state
        .package_dir
        .join(app_id)
        .join(version)
        .join("source.zip")
}

/// Ghi nhận tệp zip mã nguồn đã nhận vào file tạm: kiểm SHA-256 và cấu trúc zip, rồi chuyển vào
/// kho gói.
pub async fn store_upload(
    state: &AppState,
    m: &Membership,
    version_id: Uuid,
    upload: dev::Upload<'_>,
) -> DevResult<()> {
    let info = draft(&state.db, m, version_id).await?;
    if upload
        .declared_sha256
        .is_some_and(|d| !d.trim().eq_ignore_ascii_case(upload.sha256))
    {
        return Err(invalid_source(
            "SHA-256 của mã nguồn không khớp giá trị đã khai báo.",
            "The source code SHA-256 does not match the declared value.",
        ));
    }
    let path = upload.path.to_path_buf();
    let checked = tokio::task::spawn_blocking(move || {
        feco_codescan::archive::validate(&path, feco_codescan::MAX_FILES)
    })
    .await
    .map_err(|e| DevError::Upstream(e.to_string()))?;
    if let Err(e) = checked {
        return Err(invalid_source(
            format!("Mã nguồn phải là tệp zip hợp lệ: {e}"),
            format!("The source code must be a valid zip file: {e}"),
        ));
    }
    let dest = source_path(state, &info.app_id, &info.version);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))?;
    }
    tokio::fs::rename(upload.path, &dest)
        .await
        .map_err(|e| DevError::Upstream(e.to_string()))?;
    sqlx::query(
        "INSERT INTO source_submissions (version_id, origin, storage_path, size_bytes, sha256)
         VALUES ($1, 'upload', $2, $3, $4)
         ON CONFLICT (version_id) DO UPDATE SET origin = 'upload', repository = NULL, commit = NULL,
             storage_path = $2, size_bytes = $3, sha256 = $4, status = 'pending', report = NULL,
             scanned_at = NULL, submitted_at = now()",
    )
    .bind(version_id)
    .bind(dest.to_string_lossy().into_owned())
    .bind(upload.size as i64)
    .bind(upload.sha256)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Repo GitHub chuẩn hóa `https://github.com/<owner>/<repo>` và commit chữ thường.
fn git_source(repository: &str, commit: &str) -> DevResult<(String, String)> {
    let Some((owner, repo)) = feco_manifest::github_repo(repository) else {
        return Err(invalid_source(
            "Repo phải là repo GitHub công khai dạng https://github.com/<chủ>/<repo>.",
            "The repository must be a public GitHub repository like https://github.com/<owner>/<repo>.",
        ));
    };
    let commit = commit.trim();
    if !feco_manifest::valid_commit(commit) {
        return Err(invalid_source(
            "Commit phải là mã commit đủ 40 ký tự hex.",
            "The commit must be a full 40-character hex commit id.",
        ));
    }
    Ok((
        format!("https://github.com/{owner}/{repo}"),
        commit.to_ascii_lowercase(),
    ))
}

async fn upsert_git(db: &Db, version_id: Uuid, repository: &str, commit: &str) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO source_submissions (version_id, origin, repository, commit)
         VALUES ($1, 'git', $2, $3)
         ON CONFLICT (version_id) DO UPDATE SET origin = 'git', repository = $2, commit = $3,
             storage_path = NULL, size_bytes = NULL, sha256 = NULL, status = 'pending',
             report = NULL, scanned_at = NULL, submitted_at = now()",
    )
    .bind(version_id)
    .bind(repository)
    .bind(commit)
    .execute(db)
    .await?;
    Ok(())
}

/// Nộp mã nguồn dạng repo GitHub và commit (trang phiên bản).
pub async fn set_git(
    db: &Db,
    m: &Membership,
    version_id: Uuid,
    repository: &str,
    commit: &str,
) -> DevResult<()> {
    draft(db, m, version_id).await?;
    let (repository, commit) = git_source(repository, commit)?;
    upsert_git(db, version_id, &repository, &commit).await?;
    Ok(())
}

/// Khi lưu manifest: `[source]` thành mã nguồn `origin = git`; manifest không còn `[source]` thì
/// bỏ mã nguồn git đã ghi từ manifest trước (tệp đã tải lên được giữ).
pub async fn record_manifest(db: &Db, version_id: Uuid, manifest: &Manifest) -> sqlx::Result<()> {
    match &manifest.source_git {
        Some(git) => upsert_git(db, version_id, &git.repository, &git.commit).await,
        None => {
            sqlx::query("DELETE FROM source_submissions WHERE version_id = $1 AND origin = 'git'")
                .bind(version_id)
                .execute(db)
                .await?;
            Ok(())
        }
    }
}

/// Gửi duyệt (lại): kết quả quét cũ bị xóa để mã nguồn được quét lại.
pub async fn reset(db: &Db, version_id: Uuid) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE source_submissions SET status = 'pending', report = NULL, scanned_at = NULL
         WHERE version_id = $1",
    )
    .bind(version_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Kết quả quét mã nguồn trong khâu kiểm tra tự động.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Không kèm mã nguồn: ghi "Không có mã nguồn", không đổi cách duyệt.
    NoSource(Value),
    /// Không có phát hiện `critical` hay `high`, OSV đã tra hoặc bị tắt.
    Clean(Value),
    /// Phải duyệt tay: lý do "Quét mã nguồn: N phát hiện cần xem".
    Manual { reason: String, summary: Value },
    /// Từ chối theo điều khoản kỹ thuật (`reject_code = source_scan`).
    Reject { note: String, summary: Value },
    /// Lỗi tạm thời (tải repo lỗi mạng): xếp hàng lại.
    Retry(String),
}

type SubmissionRow = (String, Option<String>, Option<String>, Option<String>);

/// Tóm tắt cho `app_versions.checks.source`.
pub fn summary(origin: &str, report: &Report) -> Value {
    json!({
        "status": "scanned",
        "origin": origin,
        "counts": report.counts(),
        "osv": report.osv.as_str(),
    })
}

/// Lý do từ chối: luật, tệp và dòng của từng phát hiện `critical` (không chép trích đoạn).
pub fn rejection_note(report: &Report) -> String {
    let critical: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Critical)
        .collect();
    let mut note = format!(
        "Quét mã nguồn: {} phát hiện mức critical (bí mật lộ hoặc mã độc hại)",
        critical.len()
    );
    for f in critical.iter().take(MAX_NOTE_LINES) {
        note.push_str(&format!(
            "\n- {}: {} dòng {} ({})",
            f.rule, f.path, f.line, f.message
        ));
    }
    if critical.len() > MAX_NOTE_LINES {
        note.push_str(&format!(
            "\n- … và {} phát hiện khác",
            critical.len() - MAX_NOTE_LINES
        ));
    }
    note
}

/// Quyết định theo bảng của design S2.
pub fn decide(origin: &str, report: &Report) -> Outcome {
    let summary = summary(origin, report);
    if report.count(Severity::Critical) > 0 {
        return Outcome::Reject {
            note: rejection_note(report),
            summary,
        };
    }
    let high = report.count(Severity::High);
    let osv_failed = matches!(report.osv, feco_codescan::OsvStatus::Failed(_));
    match (high, osv_failed) {
        (0, false) => Outcome::Clean(summary),
        (0, true) => Outcome::Manual {
            reason: "Quét mã nguồn: không tra được lỗ hổng thư viện (OSV.dev không trả lời)".into(),
            summary,
        },
        (n, failed) => Outcome::Manual {
            reason: format!(
                "Quét mã nguồn: {n} phát hiện cần xem{}",
                if failed {
                    "; không tra được OSV.dev"
                } else {
                    ""
                }
            ),
            summary,
        },
    }
}

/// Quét mã nguồn của phiên bản (nếu có) và lưu báo cáo.
pub async fn review(
    state: &AppState,
    version_id: Uuid,
    manifest: &Manifest,
) -> Result<Outcome, String> {
    let db_err = |e: sqlx::Error| e.to_string();
    let row: Option<SubmissionRow> = sqlx::query_as(
        "SELECT origin, repository, commit, storage_path FROM source_submissions WHERE version_id = $1",
    )
    .bind(version_id)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err)?;
    let Some((origin, repository, commit, storage_path)) = row else {
        return Ok(Outcome::NoSource(
            json!({ "status": "none", "note": NO_SOURCE }),
        ));
    };
    // Mã nguồn git được tải vào file tạm, xóa sau khi quét.
    let mut fetched: Option<PathBuf> = None;
    let zip = match origin.as_str() {
        "git" => {
            let (repository, commit) = (repository.unwrap_or_default(), commit.unwrap_or_default());
            let Some((owner, repo)) = feco_manifest::github_repo(&repository) else {
                return fail(
                    state,
                    version_id,
                    &format!("Repo mã nguồn không hợp lệ: {repository}"),
                )
                .await;
            };
            let dir = state.package_dir.join(".incoming");
            tokio::fs::create_dir_all(&dir)
                .await
                .map_err(|e| e.to_string())?;
            let dest = dir.join(format!("source-{}.zip", Uuid::new_v4()));
            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|e| e.to_string())?;
            let base = state.source_fetch_url.as_str();
            match feco_codescan::download_github_zip(
                &http,
                base,
                &owner,
                &repo,
                &commit,
                &dest,
                MAX_SOURCE_BYTES,
            )
            .await
            {
                Ok(_) => {}
                Err(FetchError::Network(e)) => {
                    let _ = tokio::fs::remove_file(&dest).await;
                    return Ok(Outcome::Retry(format!(
                        "không tải được mã nguồn {repository}@{commit}: {e}"
                    )));
                }
                Err(FetchError::Status(code)) => {
                    let _ = tokio::fs::remove_file(&dest).await;
                    return fail(
                        state,
                        version_id,
                        &format!(
                            "Không tải được mã nguồn từ {repository} tại commit {commit} (HTTP {code}): repo phải công khai trên GitHub và commit phải tồn tại."
                        ),
                    )
                    .await;
                }
                Err(FetchError::TooLarge(_)) => {
                    let _ = tokio::fs::remove_file(&dest).await;
                    return fail(
                        state,
                        version_id,
                        &format!("Mã nguồn {repository}@{commit} lớn hơn 200 MB."),
                    )
                    .await;
                }
                Err(FetchError::Io(e)) => {
                    let _ = tokio::fs::remove_file(&dest).await;
                    return Err(e);
                }
            }
            fetched = Some(dest.clone());
            dest
        }
        _ => PathBuf::from(storage_path.unwrap_or_default()),
    };
    let opts = ScanOptions {
        osv: state.osv_url.as_ref().map(|u| OsvClient::new(u.as_str())),
        manifest_license: Some(manifest.license.clone()),
        ..ScanOptions::default()
    };
    let scanned = feco_codescan::scan(Path::new(&zip), &opts).await;
    if let Some(f) = fetched {
        let _ = tokio::fs::remove_file(f).await;
    }
    let report = match scanned {
        Ok(r) => r,
        Err(e) => {
            return fail(
                state,
                version_id,
                &format!("Tệp mã nguồn không quét được: {e}"),
            )
            .await;
        }
    };
    sqlx::query(
        "UPDATE source_submissions SET status = 'scanned', report = $2, scanned_at = now()
         WHERE version_id = $1",
    )
    .bind(version_id)
    .bind(serde_json::to_value(&report).unwrap_or(Value::Null))
    .execute(&state.db)
    .await
    .map_err(db_err)?;
    Ok(decide(&origin, &report))
}

/// Mã nguồn không quét được do lỗi của nhà phát triển (repo riêng tư, commit không có, zip hỏng):
/// từ chối kèm lý do.
async fn fail(state: &AppState, version_id: Uuid, why: &str) -> Result<Outcome, String> {
    sqlx::query(
        "UPDATE source_submissions SET status = 'failed', report = $2, scanned_at = now()
         WHERE version_id = $1",
    )
    .bind(version_id)
    .bind(json!({ "error": why }))
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(Outcome::Reject {
        note: format!("Quét mã nguồn: {why}"),
        summary: json!({ "status": "failed", "error": why }),
    })
}

// ---------- Hiển thị ----------

pub struct FindingRow {
    pub severity: String,
    pub rule: String,
    pub place: String,
    pub message: String,
    pub excerpt: String,
}

/// Báo cáo quét mã nguồn trên trang phiên bản (nhà phát triển) và trang duyệt (người duyệt).
pub struct SourceView {
    /// Nguồn: `source.zip (12 KB)` hoặc `https://github.com/o/r @ abcdef0`.
    pub origin: String,
    /// "critical 0 · high 1 · medium 0 · info 0 · OSV: đã tra", hoặc trạng thái.
    pub summary: String,
    pub findings: Vec<FindingRow>,
    pub scanned: bool,
}

type ViewRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<Value>,
    Option<Value>,
);

pub async fn view(db: &Db, lang: Lang, version_id: Uuid) -> sqlx::Result<Option<SourceView>> {
    let row: Option<ViewRow> = sqlx::query_as(
        "SELECT s.origin, s.repository, s.commit, s.size_bytes, s.status, s.report, v.checks->'source'
         FROM app_versions v LEFT JOIN source_submissions s ON s.version_id = v.id WHERE v.id = $1",
    )
    .bind(version_id)
    .fetch_optional(db)
    .await?;
    let Some((origin, repository, commit, size, status, report, checks)) = row else {
        return Ok(None);
    };
    let Some(origin) = origin else {
        // Không kèm mã nguồn: chỉ hiện sau khi khâu kiểm tra đã ghi nhận.
        let none = checks.as_ref().and_then(|c| c["status"].as_str()) == Some("none");
        return Ok(none.then(|| SourceView {
            origin: String::new(),
            summary: lang.t("source.none").to_owned(),
            findings: Vec::new(),
            scanned: false,
        }));
    };
    let origin = if origin == "git" {
        let commit = commit.unwrap_or_default();
        format!(
            "{} @ {}",
            repository.unwrap_or_default(),
            commit.get(..10).unwrap_or(&commit)
        )
    } else {
        format!("source.zip ({} KB)", size.unwrap_or(0) / 1024)
    };
    let status = status.unwrap_or_default();
    let parsed: Option<Report> = report
        .as_ref()
        .and_then(|r| serde_json::from_value(r.clone()).ok());
    let (summary, findings, scanned) = match (status.as_str(), parsed) {
        ("scanned", Some(r)) => {
            let counts = r.counts();
            let osv = lang.t(&format!("source.osv.{}", r.osv.as_str()));
            let summary = format!(
                "critical {} · high {} · medium {} · info {} · {}: {osv}",
                counts["critical"],
                counts["high"],
                counts["medium"],
                counts["info"],
                lang.t("source.osv")
            );
            let rows = r
                .findings
                .iter()
                .map(|f| FindingRow {
                    severity: f.severity.as_str().to_owned(),
                    rule: f.rule.clone(),
                    place: if f.line > 0 {
                        format!("{}:{}", f.path, f.line)
                    } else {
                        f.path.clone()
                    },
                    message: f.message.clone(),
                    excerpt: f.excerpt.clone().unwrap_or_default(),
                })
                .collect();
            (summary, rows, true)
        }
        ("failed", _) => {
            let why = report
                .as_ref()
                .and_then(|r| r["error"].as_str())
                .unwrap_or_default();
            (
                format!("{}: {why}", lang.t("source.failed")),
                Vec::new(),
                false,
            )
        }
        _ => (lang.t("source.pending").to_owned(), Vec::new(), false),
    };
    Ok(Some(SourceView {
        origin,
        summary,
        findings,
        scanned,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use feco_codescan::{Category, Finding, OsvStatus};

    fn finding(rule: &str, severity: Severity) -> Finding {
        Finding {
            rule: rule.into(),
            category: Category::Secret,
            severity,
            path: "src/config.ts".into(),
            line: 3,
            message: "Token GitHub".into(),
            excerpt: Some("ghp_****".into()),
        }
    }

    #[test]
    fn decision_table() {
        let mut r = Report {
            osv: OsvStatus::Checked,
            ..Report::default()
        };
        assert!(matches!(decide("upload", &r), Outcome::Clean(_)));
        r.findings
            .push(finding("vulnerability.osv", Severity::Medium));
        assert!(matches!(decide("upload", &r), Outcome::Clean(_)));
        r.findings
            .push(finding("vulnerability.osv", Severity::High));
        match decide("git", &r) {
            Outcome::Manual { reason, summary } => {
                assert_eq!(reason, "Quét mã nguồn: 1 phát hiện cần xem");
                assert_eq!(summary["counts"]["high"], 1);
                assert_eq!(summary["osv"], "checked");
                assert_eq!(summary["origin"], "git");
            }
            other => panic!("{other:?}"),
        }
        r.osv = OsvStatus::Failed("hết giờ".into());
        match decide("git", &r) {
            Outcome::Manual { reason, .. } => {
                assert!(reason.ends_with("không tra được OSV.dev"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        r.findings.clear();
        assert!(matches!(decide("git", &r), Outcome::Manual { .. }));
        r.findings
            .push(finding("secret.github_token", Severity::Critical));
        match decide("upload", &r) {
            Outcome::Reject { note, summary } => {
                assert!(
                    note.contains("secret.github_token: src/config.ts dòng 3"),
                    "{note}"
                );
                assert!(
                    !note.contains("ghp_"),
                    "lý do không chép trích đoạn: {note}"
                );
                assert_eq!(summary["counts"]["critical"], 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn git_sources_are_normalized() {
        let commit = "ABCDEF0123456789ABCDEF0123456789ABCDEF01";
        assert_eq!(
            git_source("https://github.com/a/b.git", commit).unwrap(),
            ("https://github.com/a/b".into(), commit.to_ascii_lowercase())
        );
        assert!(git_source("https://gitlab.com/a/b", commit).is_err());
        assert!(git_source("https://github.com/a/b", "main").is_err());
    }
}
