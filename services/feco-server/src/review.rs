//! Kiểm duyệt app (spec app-review; design D6).
//!
//! 1. Nhà phát triển gửi duyệt → phiên bản vào hàng đợi `review_jobs` (`review_stage = checks`).
//! 2. [`process_due`] chạy kiểm tra tự động cho từng gói ([`crate::scan`]): mã độc (ít nhất hai
//!    engine), chữ ký mã, tệp chạy và kiến trúc khớp manifest, packer. Không qua thì bị từ chối;
//!    phát hiện mã độc thì tài khoản nhà phát triển bị tạm khóa và quản trị viên nhận cảnh báo.
//! 3. Qua kiểm tra: phiên bản đầu của app, hoặc thêm scope dữ liệu hay quyền hệ thống, vào hàng
//!    chờ duyệt thủ công ([`manual_reason`]); các phiên bản khác được duyệt tự động.
//! 4. Người duyệt duyệt hoặc từ chối theo một điều khoản của chính sách nội dung ([`CLAUSES`]);
//!    nhà phát triển nhận email nêu điều khoản và cách sửa, và kháng nghị được một lần.
//! 5. Gỡ khẩn cấp ([`revoke`]) và quét lại định kỳ 7 ngày ([`enqueue_rescans`]).
//!
//! Bảng quản trị (nhóm 14) sẽ gọi các hàm ở đây; tạm thời dùng công cụ `feco-catalog`.

use std::collections::BTreeSet;

use axum::http::StatusCode;
use feco_manifest::Manifest;
use serde::Serialize;
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::{self, Actor},
    catalog,
    developer::{self, problem, DevError, DevResult},
    scan::{self, Engine},
    store::Db,
    AppState,
};

/// Một điều khoản của chính sách nội dung, dùng làm mẫu lý do từ chối.
pub struct Clause {
    pub code: &'static str,
    pub number: &'static str,
    pub title_vi: &'static str,
    pub title_en: &'static str,
    pub rule_vi: &'static str,
    pub rule_en: &'static str,
    pub fix_vi: &'static str,
    pub fix_en: &'static str,
}

/// Chính sách nội dung công bố ở `/policy/content` (spec app-review, "Chính sách nội dung").
pub const CLAUSES: [Clause; 8] = [
    Clause {
        code: "malware",
        number: "1",
        title_vi: "Mã độc",
        title_en: "Malware",
        rule_vi: "App không được chứa mã độc, phần mềm gián điệp hay mã tải về và chạy mã độc.",
        rule_en: "Apps must not contain malware, spyware, or code that downloads and runs malware.",
        fix_vi: "Gỡ thành phần độc hại, quét lại toàn bộ gói và chuỗi build, rồi liên hệ F-Eco để mở khóa tài khoản.",
        fix_en: "Remove the malicious component, rescan the package and your build chain, then contact F-Eco to unlock the account.",
    },
    Clause {
        code: "undeclared_data",
        number: "2",
        title_vi: "Thu thập hoặc dùng dữ liệu trái khai báo",
        title_en: "Collecting or using data beyond what was declared",
        rule_vi: "App chỉ được thu thập và dùng dữ liệu người dùng đúng scope và đúng mục đích đã khai báo trong f-app.toml.",
        rule_en: "Apps may only collect and use user data within the scopes and for the purposes declared in f-app.toml.",
        fix_vi: "Ngừng dùng dữ liệu ngoài mục đích đã khai báo, hoặc khai báo đúng mục đích (người dùng sẽ được hỏi đồng ý lại), rồi nộp phiên bản mới.",
        fix_en: "Stop using the data beyond the declared purpose, or declare the real purpose (users will be asked to consent again), then submit a new version.",
    },
    Clause {
        code: "hidden_mining",
        number: "3",
        title_vi: "Đào tiền mã hóa ẩn",
        title_en: "Hidden cryptocurrency mining",
        rule_vi: "App không được dùng máy của người dùng để đào tiền mã hóa mà không nói rõ và không được đồng ý.",
        rule_en: "Apps must not use the user's computer to mine cryptocurrency without clear disclosure and consent.",
        fix_vi: "Gỡ thành phần đào tiền mã hóa.",
        fix_en: "Remove the mining component.",
    },
    Clause {
        code: "drm_copyright",
        number: "4",
        title_vi: "Vượt DRM hoặc vi phạm bản quyền",
        title_en: "Circumventing DRM or infringing copyright",
        rule_vi: "App không được vượt cơ chế bảo vệ bản quyền hay phân phối nội dung, mã nguồn của người khác trái license.",
        rule_en: "Apps must not circumvent copy protection or distribute other people's content or code against its license.",
        fix_vi: "Gỡ phần vi phạm, hoặc gửi bằng chứng quyền sử dụng khi kháng nghị.",
        fix_en: "Remove the infringing part, or provide proof of your right to use it in an appeal.",
    },
    Clause {
        code: "brand_impersonation",
        number: "5",
        title_vi: "Giả mạo thương hiệu",
        title_en: "Brand impersonation",
        rule_vi: "App không được dùng tên, biểu tượng hay giao diện gây nhầm với sản phẩm hoặc thương hiệu của người khác. App dựng lại từ phần mềm khác không được dùng tên, logo hay tài sản của sản phẩm gốc.",
        rule_en: "Apps must not use names, icons or designs that can be confused with someone else's product or brand. Apps rebuilt from other software must not use the original product's name, logo or assets.",
        fix_vi: "Đổi tên, biểu tượng, ảnh chụp và mô tả sang nhận diện của riêng bạn.",
        fix_en: "Change the name, icon, screenshots and description to your own identity.",
    },
    Clause {
        code: "illegal_content",
        number: "6",
        title_vi: "Nội dung vi phạm pháp luật Việt Nam",
        title_en: "Content that breaks Vietnamese law",
        rule_vi: "App không được chứa hay phát tán nội dung vi phạm pháp luật Việt Nam.",
        rule_en: "Apps must not contain or distribute content that breaks Vietnamese law.",
        fix_vi: "Gỡ nội dung vi phạm.",
        fix_en: "Remove the offending content.",
    },
    Clause {
        code: "misleading",
        number: "7",
        title_vi: "Mô tả sai sự thật",
        title_en: "Misleading description",
        rule_vi: "Tên, mô tả và ảnh chụp phải đúng với những gì app làm.",
        rule_en: "The name, description and screenshots must match what the app does.",
        fix_vi: "Sửa mô tả và ảnh chụp cho đúng với app.",
        fix_en: "Correct the description and screenshots.",
    },
    Clause {
        code: "technical",
        number: "8",
        title_vi: "Không qua kiểm tra kỹ thuật",
        title_en: "Failed technical checks",
        rule_vi: "Gói phải đúng với f-app.toml (tệp chạy, kiến trúc), được ký mã bằng chứng thư của nhà phát triển, và không bị nén hay làm rối.",
        rule_en: "Packages must match f-app.toml (entry file, architecture), be code-signed with the developer's certificate, and not be packed or obfuscated.",
        fix_vi: "Sửa các lỗi được liệt kê rồi nộp phiên bản mới.",
        fix_en: "Fix the listed problems and submit a new version.",
    },
];

pub fn clause(code: &str) -> Option<&'static Clause> {
    CLAUSES.iter().find(|c| c.code == code)
}

/// Đưa phiên bản vào hàng đợi kiểm tra.
pub async fn enqueue(db: &Db, version_id: Uuid, kind: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO review_jobs (version_id, kind) VALUES ($1, $2)")
        .bind(version_id)
        .bind(kind)
        .execute(db)
        .await?;
    Ok(())
}

/// Xếp lịch quét lại các gói đang phát hành mà lần quét gần nhất đã quá 7 ngày.
pub async fn enqueue_rescans(db: &Db) -> sqlx::Result<u64> {
    Ok(sqlx::query(
        "INSERT INTO review_jobs (version_id, kind)
         SELECT v.id, 'rescan' FROM app_versions v
         WHERE v.status IN ('released', 'paused')
           AND (v.last_scanned_at IS NULL OR v.last_scanned_at < now() - interval '7 days')
           AND NOT EXISTS (SELECT 1 FROM review_jobs j
                           WHERE j.version_id = v.id AND j.status IN ('queued', 'running'))",
    )
    .execute(db)
    .await?
    .rows_affected())
}

async fn engines(state: &AppState) -> sqlx::Result<Vec<Engine>> {
    let signatures: Vec<(String, String)> =
        sqlx::query_as("SELECT sha256, name FROM malware_signatures")
            .fetch_all(&state.db)
            .await?;
    let mut out = vec![Engine::Signatures(signatures)];
    if let Some(addr) = state.clamd {
        out.push(Engine::Clamd(addr));
    }
    Ok(out)
}

/// Gửi cảnh báo cho quản trị viên: ghi vào `admin_alerts` và gửi email.
pub async fn admin_alert(state: &AppState, kind: &str, message: &str, details: Value) {
    let res = sqlx::query("INSERT INTO admin_alerts (kind, message, details) VALUES ($1, $2, $3)")
        .bind(kind)
        .bind(message)
        .bind(&details)
        .execute(&state.db)
        .await;
    if let Err(e) = res {
        tracing::error!(error = %e, "không ghi được cảnh báo quản trị");
    }
    for to in &state.admin_emails {
        let subject = format!("[F-Eco kiểm duyệt] {message}");
        let body = format!(
            "{message}\n\n{}\n",
            serde_json::to_string_pretty(&details).unwrap_or_default()
        );
        if let Err(e) = state.mailer.send(to, &subject, &body).await {
            tracing::warn!(error = %e, "không gửi được email cảnh báo quản trị");
        }
    }
}

/// Email của Chủ sở hữu và Quản trị của tài khoản sở hữu app.
async fn developer_emails(db: &Db, app_id: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT m.email FROM developer_members m JOIN developer_apps a ON a.account_id = m.account_id
         WHERE a.app_id = $1 AND m.status = 'active' AND m.role IN ('owner', 'admin')",
    )
    .bind(app_id)
    .fetch_all(db)
    .await
}

async fn notify_developer(state: &AppState, app_id: &str, subject: &str, body: &str) {
    match developer_emails(&state.db, app_id).await {
        Ok(emails) => {
            for to in emails {
                if let Err(e) = state.mailer.send(&to, subject, body).await {
                    tracing::warn!(error = %e, "không gửi được email cho nhà phát triển");
                }
            }
        }
        Err(e) => tracing::error!(error = %e, "không đọc được email nhà phát triển"),
    }
}

/// Lý do phải duyệt thủ công so với bản đã duyệt gần nhất; `None` thì được duyệt tự động.
pub fn manual_reason(previous: Option<&Manifest>, next: &Manifest) -> Option<String> {
    let Some(previous) = previous else {
        return Some("Phiên bản đầu tiên của app".into());
    };
    let scopes =
        |m: &Manifest| -> BTreeSet<&str> { m.scopes.iter().map(|s| s.scope.as_str()).collect() };
    let added: Vec<&str> = scopes(next)
        .difference(&scopes(previous))
        .copied()
        .collect();
    if !added.is_empty() {
        return Some(format!("Thêm scope dữ liệu: {}", added.join(", ")));
    }
    let mut permissions = Vec::new();
    let (a, b) = (&previous.permissions, &next.permissions);
    if b.network && !a.network {
        permissions.push("network".to_owned());
    }
    if b.camera && !a.camera {
        permissions.push("camera".to_owned());
    }
    if b.microphone && !a.microphone {
        permissions.push("microphone".to_owned());
    }
    for folder in &b.user_folders {
        if !a.user_folders.contains(folder) {
            permissions.push(format!("user_folders.{folder}"));
        }
    }
    if !permissions.is_empty() {
        return Some(format!("Thêm quyền hệ thống: {}", permissions.join(", ")));
    }
    None
}

#[derive(Debug, Clone, Serialize)]
pub struct JobResult {
    pub outcome: String,
    pub reports: Vec<scan::Report>,
}

/// Chạy các job đang chờ trong hàng đợi; trả số job đã xử lý.
pub async fn process_due(state: &AppState) -> sqlx::Result<usize> {
    let mut done = 0;
    loop {
        let job: Option<(i64, Uuid, String)> = sqlx::query_as(
            "UPDATE review_jobs SET status = 'running', started_at = now(), attempts = attempts + 1
             WHERE id = (SELECT id FROM review_jobs WHERE status = 'queued'
                         ORDER BY id FOR UPDATE SKIP LOCKED LIMIT 1)
             RETURNING id, version_id, kind",
        )
        .fetch_optional(&state.db)
        .await?;
        let Some((id, version_id, kind)) = job else {
            return Ok(done);
        };
        let result = match kind.as_str() {
            "submit" => run_submit(state, version_id).await,
            _ => run_rescan(state, version_id).await,
        };
        let (status, value) = match result {
            Ok(r) => ("done", serde_json::to_value(r).unwrap_or(Value::Null)),
            Err(e) => {
                tracing::error!(error = %e, job = id, "job kiểm duyệt lỗi");
                ("failed", json!({ "error": e }))
            }
        };
        sqlx::query(
            "UPDATE review_jobs SET status = $2, finished_at = now(), result = $3 WHERE id = $1",
        )
        .bind(id)
        .bind(status)
        .bind(value)
        .execute(&state.db)
        .await?;
        done += 1;
    }
}

struct Loaded {
    app_id: String,
    version: String,
    status: String,
    stage: Option<String>,
    manifest: Manifest,
    packages: Vec<(String, String)>,
}

async fn load(db: &Db, version_id: Uuid) -> Result<Loaded, String> {
    let row: Option<(String, String, String, Option<String>, Value)> = sqlx::query_as(
        "SELECT app_id, version, status, review_stage, manifest FROM app_versions WHERE id = $1",
    )
    .bind(version_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;
    let (app_id, version, status, stage, manifest) = row.ok_or("phiên bản không còn")?;
    let manifest: Manifest = serde_json::from_value(manifest).map_err(|e| e.to_string())?;
    let packages: Vec<(String, String)> = sqlx::query_as(
        "SELECT platform, storage_path FROM app_packages WHERE version_id = $1 ORDER BY platform",
    )
    .bind(version_id)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(Loaded {
        app_id,
        version,
        status,
        stage,
        manifest,
        packages,
    })
}

async fn scan_all(state: &AppState, v: &Loaded, full: bool) -> Result<Vec<scan::Report>, String> {
    let engines = engines(state).await.map_err(|e| e.to_string())?;
    let mut reports = Vec::new();
    for (platform, path) in &v.packages {
        let declared = v.manifest.platforms.iter().find(|p| p.key() == *platform);
        let (entry, arch) = declared
            .map(|p| (p.entry.as_str(), p.arch.as_str()))
            .unwrap_or_default();
        let mut report = scan::check_package(
            std::path::Path::new(path),
            platform,
            entry,
            arch,
            &engines,
            full,
        )
        .await;
        if full && report.malware.is_none() {
            // Chữ ký mã: kiểm lại ở khâu duyệt (cùng quy tắc với lúc nộp).
            if let Some(p) = declared {
                let (path, os, entry) = (path.clone(), p.os.clone(), p.entry.clone());
                let signed = tokio::task::spawn_blocking(move || {
                    developer::inspect_package(std::path::Path::new(&path), &os, &entry)
                })
                .await
                .map_err(|e| e.to_string())?;
                if let Err(DevError::Problem(p)) = signed {
                    if !report
                        .problems
                        .iter()
                        .any(|x| x.contains("không có tệp chạy"))
                    {
                        report.problems.push(p.vi);
                    }
                }
            }
        }
        reports.push(report);
    }
    Ok(reports)
}

async fn reject_version(
    state: &AppState,
    version_id: Uuid,
    code: &str,
    note: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE app_versions SET status = 'rejected', review_stage = 'done', reviewed_at = now(),
             reject_count = reject_count + 1, reject_code = $2, review_note = $3
         WHERE id = $1",
    )
    .bind(version_id)
    .bind(code)
    .bind(note)
    .execute(&state.db)
    .await?;
    Ok(())
}

fn rejection_email(
    state: &AppState,
    app_id: &str,
    version: &str,
    c: &Clause,
    note: &str,
) -> (String, String) {
    let url = format!("{}developer/apps/{app_id}", state.public_url);
    let policy = format!("{}policy/content", state.public_url);
    (
        format!("{app_id} {version} bị từ chối / was rejected"),
        format!(
            "{app_id} {version} bị từ chối.\n\nĐiều {num} chính sách nội dung: {title}\n{rule}\n\nChi tiết: {note}\n\n\
             Cách khắc phục: {fix}\n\nKháng nghị (một lần cho mỗi lần từ chối): {url}\nChính sách nội dung: {policy}\n\n\
             ---\n{app_id} {version} was rejected.\nClause {num}: {title_en}\n{rule_en}\nDetails: {note}\nHow to fix: {fix_en}\n\
             Appeal (once per rejection): {url}\nContent policy: {policy}\n",
            num = c.number,
            title = c.title_vi,
            rule = c.rule_vi,
            fix = c.fix_vi,
            title_en = c.title_en,
            rule_en = c.rule_en,
            fix_en = c.fix_en,
        ),
    )
}

async fn run_submit(state: &AppState, version_id: Uuid) -> Result<JobResult, String> {
    let v = load(&state.db, version_id).await?;
    if v.status != "pending_review" || v.stage.as_deref() != Some("checks") {
        return Ok(JobResult {
            outcome: format!("bỏ qua: phiên bản đang ở {}", v.status),
            reports: Vec::new(),
        });
    }
    let reports = scan_all(state, &v, true).await?;
    let db_err = |e: sqlx::Error| e.to_string();
    sqlx::query("UPDATE app_versions SET checks = $2, last_scanned_at = now() WHERE id = $1")
        .bind(version_id)
        .bind(serde_json::to_value(&reports).unwrap_or(Value::Null))
        .execute(&state.db)
        .await
        .map_err(db_err)?;

    // Scenario "Phát hiện mã độc".
    if let Some((engine, malware, file)) = reports.iter().find_map(|r| r.malware.clone()) {
        let note = format!("Bị từ chối: mã độc ({malware}, phát hiện bởi {engine} trong {file})");
        reject_version(state, version_id, "malware", &note)
            .await
            .map_err(db_err)?;
        let account: Option<(Uuid, String)> = sqlx::query_as(
            "UPDATE developer_accounts SET suspended_at = now(), suspended_reason = $2
             WHERE id = (SELECT account_id FROM developer_apps WHERE app_id = $1)
             RETURNING id, display_name",
        )
        .bind(&v.app_id)
        .bind(&note)
        .fetch_optional(&state.db)
        .await
        .map_err(db_err)?;
        audit::record(
            &state.db,
            Actor::System,
            "review.malware",
            Some(&v.app_id),
            None,
            json!({ "version": v.version, "engine": engine, "malware": malware }),
        )
        .await;
        admin_alert(
            state,
            "malware",
            &format!("Phát hiện mã độc trong {} {}", v.app_id, v.version),
            json!({
                "app": v.app_id, "version": v.version, "engine": engine, "malware": malware,
                "file": file, "account": account.map(|a| a.1),
                "action": "Tài khoản nhà phát triển đã bị tạm khóa chờ điều tra",
            }),
        )
        .await;
        let c = clause("malware").expect("điều khoản mã độc");
        let (subject, body) = rejection_email(state, &v.app_id, &v.version, c, &note);
        notify_developer(
            state,
            &v.app_id,
            &subject,
            &format!(
                "{body}\nTài khoản nhà phát triển của bạn đang bị tạm khóa chờ điều tra.\n\
                 Your developer account is suspended pending investigation.\n"
            ),
        )
        .await;
        return Ok(JobResult {
            outcome: "malware".into(),
            reports,
        });
    }

    // Không đủ engine chạy được: chưa kết luận, chờ chạy lại (không chuyển sang duyệt thủ công).
    let errors: Vec<String> = reports
        .iter()
        .flat_map(|r| r.engine_errors.clone())
        .collect();
    if !errors.is_empty() {
        enqueue(&state.db, version_id, "submit")
            .await
            .map_err(db_err)?;
        return Err(format!(
            "engine quét lỗi, sẽ chạy lại: {}",
            errors.join("; ")
        ));
    }

    let problems: Vec<String> = reports.iter().flat_map(|r| r.problems.clone()).collect();
    if !problems.is_empty() {
        let note = problems.join("\n");
        reject_version(state, version_id, "technical", &note)
            .await
            .map_err(db_err)?;
        let c = clause("technical").expect("điều khoản kỹ thuật");
        let (subject, body) = rejection_email(state, &v.app_id, &v.version, c, &note);
        notify_developer(state, &v.app_id, &subject, &body).await;
        return Ok(JobResult {
            outcome: "rejected".into(),
            reports,
        });
    }

    // Quy tắc duyệt thủ công hay tự động.
    let previous: Option<Value> = sqlx::query_scalar(
        "SELECT manifest FROM app_versions
         WHERE app_id = $1 AND id <> $2 AND status IN ('approved', 'released', 'paused')
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&v.app_id)
    .bind(version_id)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err)?;
    let previous: Option<Manifest> = previous.and_then(|m| serde_json::from_value(m).ok());
    match manual_reason(previous.as_ref(), &v.manifest) {
        Some(reason) => {
            sqlx::query(
                "UPDATE app_versions SET review_stage = 'manual', manual_reason = $2 WHERE id = $1",
            )
            .bind(version_id)
            .bind(&reason)
            .execute(&state.db)
            .await
            .map_err(db_err)?;
            Ok(JobResult {
                outcome: format!("manual: {reason}"),
                reports,
            })
        }
        None => {
            sqlx::query(
                "UPDATE app_versions SET status = 'approved', review_stage = 'done', reviewed_at = now(),
                     review_note = 'Duyệt tự động: qua kiểm tra tự động, không đổi scope hay quyền'
                 WHERE id = $1",
            )
            .bind(version_id)
            .execute(&state.db)
            .await
            .map_err(db_err)?;
            Ok(JobResult {
                outcome: "auto_approved".into(),
                reports,
            })
        }
    }
}

async fn run_rescan(state: &AppState, version_id: Uuid) -> Result<JobResult, String> {
    let v = load(&state.db, version_id).await?;
    let reports = scan_all(state, &v, false).await?;
    let errors: Vec<String> = reports
        .iter()
        .flat_map(|r| r.engine_errors.clone())
        .collect();
    if errors.is_empty() {
        sqlx::query("UPDATE app_versions SET last_scanned_at = now() WHERE id = $1")
            .bind(version_id)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    }
    // Scenario "Phát hiện muộn": cảnh báo kèm số người đang cài, đề xuất thu hồi.
    if let Some((engine, malware, file)) = reports.iter().find_map(|r| r.malware.clone()) {
        let installs: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM catalog_installs WHERE app_id = $1 AND version = $2",
        )
        .bind(&v.app_id)
        .bind(&v.version)
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        admin_alert(
            state,
            "late_detection",
            &format!(
                "Quét lại phát hiện mã độc trong {} {} đang phát hành; {installs} người dùng đang cài",
                v.app_id, v.version
            ),
            json!({
                "app": v.app_id, "version": v.version, "engine": engine, "malware": malware,
                "file": file, "installs": installs,
                "suggestion": format!("Thu hồi khẩn cấp: feco-catalog emergency {} {} \"<lý do>\"", v.app_id, v.version),
            }),
        )
        .await;
        return Ok(JobResult {
            outcome: "late_detection".into(),
            reports,
        });
    }
    Ok(JobResult {
        outcome: "clean".into(),
        reports,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueItem {
    pub version_id: Uuid,
    pub app_id: String,
    pub version: String,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub submitted_at: Option<OffsetDateTime>,
}

type QueueRow = (Uuid, String, String, Option<String>, Option<OffsetDateTime>);

/// Hàng chờ duyệt thủ công, cũ nhất trước.
pub async fn manual_queue(db: &Db) -> sqlx::Result<Vec<QueueItem>> {
    let rows: Vec<QueueRow> = sqlx::query_as(
        "SELECT id, app_id, version, manual_reason, submitted_at FROM app_versions
         WHERE status = 'pending_review' AND review_stage = 'manual' ORDER BY submitted_at",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| QueueItem {
            version_id: r.0,
            app_id: r.1,
            version: r.2,
            reason: r.3.unwrap_or_default(),
            submitted_at: r.4,
        })
        .collect())
}

/// Phiên bản theo mã app và số phiên bản.
pub async fn version_id(db: &Db, app_id: &str, version: &str) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar("SELECT id FROM app_versions WHERE app_id = $1 AND version = $2")
        .bind(app_id)
        .bind(version)
        .fetch_optional(db)
        .await
}

fn not_in_manual_queue() -> DevError {
    problem(
        StatusCode::CONFLICT,
        "not_in_manual_review",
        "Phiên bản không ở hàng chờ duyệt thủ công (phải qua kiểm tra tự động trước).",
        "The version is not in the manual review queue (it must pass the automated checks first).",
    )
}

/// Người duyệt chấp nhận một phiên bản trong hàng chờ duyệt thủ công.
pub async fn approve(
    state: &AppState,
    version_id: Uuid,
    reviewer: &str,
    note: &str,
) -> DevResult<()> {
    let row: Option<(String, String)> = sqlx::query_as(
        "UPDATE app_versions SET status = 'approved', review_stage = 'done', reviewed_at = now(),
             review_note = $2
         WHERE id = $1 AND status = 'pending_review' AND review_stage = 'manual'
         RETURNING app_id, version",
    )
    .bind(version_id)
    .bind(if note.trim().is_empty() {
        "Đã duyệt"
    } else {
        note.trim()
    })
    .fetch_optional(&state.db)
    .await?;
    let (app_id, version) = row.ok_or_else(not_in_manual_queue)?;
    audit::record(
        &state.db,
        Actor::Admin(reviewer.to_owned()),
        "review.approve",
        Some(&app_id),
        None,
        json!({ "version": version }),
    )
    .await;
    notify_developer(
        state,
        &app_id,
        &format!("{app_id} {version} đã được duyệt / was approved"),
        &format!(
            "{app_id} {version} đã qua kiểm duyệt. Bạn có thể phát hành ở {url}\n\n---\n\
             {app_id} {version} passed review. You can release it at {url}\n",
            url = format!("{}developer/apps/{app_id}", state.public_url)
        ),
    )
    .await;
    Ok(())
}

/// Người duyệt từ chối theo một điều khoản chính sách; nhà phát triển nhận email kèm nút kháng nghị.
pub async fn reject(
    state: &AppState,
    version_id: Uuid,
    reviewer: &str,
    clause_code: &str,
    note: &str,
) -> DevResult<()> {
    let c = clause(clause_code).ok_or_else(|| {
        problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            format!("Không có điều khoản {clause_code:?}"),
            format!("No policy clause {clause_code:?}"),
        )
    })?;
    let summary = format!(
        "Bị từ chối theo điều {}: {}. {}\nCách khắc phục: {}",
        c.number,
        c.title_vi,
        note.trim(),
        c.fix_vi
    );
    let row: Option<(String, String)> = sqlx::query_as(
        "UPDATE app_versions SET status = 'rejected', review_stage = 'done', reviewed_at = now(),
             reject_count = reject_count + 1, reject_code = $2, review_note = $3
         WHERE id = $1 AND status = 'pending_review' AND review_stage = 'manual'
         RETURNING app_id, version",
    )
    .bind(version_id)
    .bind(c.code)
    .bind(&summary)
    .fetch_optional(&state.db)
    .await?;
    let (app_id, version) = row.ok_or_else(not_in_manual_queue)?;
    audit::record(
        &state.db,
        Actor::Admin(reviewer.to_owned()),
        "review.reject",
        Some(&app_id),
        None,
        json!({ "version": version, "clause": c.code }),
    )
    .await;
    let (subject, body) = rejection_email(state, &app_id, &version, c, note.trim());
    notify_developer(state, &app_id, &subject, &body).await;
    Ok(())
}

/// Nhà phát triển kháng nghị lần từ chối gần nhất (một lần cho mỗi lần từ chối).
pub async fn appeal(
    db: &Db,
    m: &developer::Membership,
    version_id: Uuid,
    message: &str,
) -> DevResult<()> {
    if !m.role.can_edit() {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "insufficient_role",
            "Vai trò Chỉ xem không kháng nghị được.",
            "The Viewer role cannot appeal.",
        ));
    }
    let message = message.trim();
    if message.is_empty() || message.chars().count() > 4000 {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Hãy nêu lý do kháng nghị (tối đa 4000 ký tự).",
            "Give a reason for the appeal (4000 characters at most).",
        ));
    }
    let (info, _) = developer::version(db, m, version_id).await?;
    let reject_no: Option<i32> = sqlx::query_scalar(
        "SELECT reject_count FROM app_versions WHERE id = $1 AND status = 'rejected'",
    )
    .bind(info.id)
    .fetch_optional(db)
    .await?;
    let Some(reject_no) = reject_no else {
        return Err(problem(
            StatusCode::CONFLICT,
            "not_rejected",
            "Chỉ kháng nghị được phiên bản đang bị từ chối.",
            "Only rejected versions can be appealed.",
        ));
    };
    let inserted = sqlx::query(
        "INSERT INTO review_appeals (version_id, reject_no, message) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(info.id)
    .bind(reject_no)
    .bind(message)
    .execute(db)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Err(problem(
            StatusCode::CONFLICT,
            "already_appealed",
            "Mỗi lần từ chối chỉ được kháng nghị một lần.",
            "Each rejection can be appealed only once.",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct Appeal {
    pub version_id: Uuid,
    pub app_id: String,
    pub version: String,
    pub reject_code: Option<String>,
    pub message: String,
}

pub async fn open_appeals(db: &Db) -> sqlx::Result<Vec<Appeal>> {
    let rows: Vec<(Uuid, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT v.id, v.app_id, v.version, v.reject_code, a.message FROM review_appeals a
         JOIN app_versions v ON v.id = a.version_id WHERE a.status = 'open' ORDER BY a.created_at",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Appeal {
            version_id: r.0,
            app_id: r.1,
            version: r.2,
            reject_code: r.3,
            message: r.4,
        })
        .collect())
}

/// Xử lý kháng nghị: `overturn` thì phiên bản được duyệt, không thì giữ nguyên quyết định.
pub async fn resolve_appeal(
    state: &AppState,
    version_id: Uuid,
    reviewer: &str,
    overturn: bool,
    note: &str,
) -> DevResult<()> {
    let row: Option<(String, String)> = sqlx::query_as(
        "UPDATE review_appeals a SET status = $2, resolution = $3, resolved_at = now()
         FROM app_versions v
         WHERE a.version_id = $1 AND v.id = a.version_id AND a.reject_no = v.reject_count
           AND a.status = 'open' AND v.status = 'rejected'
         RETURNING v.app_id, v.version",
    )
    .bind(version_id)
    .bind(if overturn { "overturned" } else { "upheld" })
    .bind(note)
    .fetch_optional(&state.db)
    .await?;
    let (app_id, version) = row.ok_or_else(|| {
        problem(
            StatusCode::CONFLICT,
            "no_open_appeal",
            "Không có kháng nghị đang mở cho phiên bản này.",
            "There is no open appeal for this version.",
        )
    })?;
    if overturn {
        sqlx::query(
            "UPDATE app_versions SET status = 'approved', review_note = $2, reviewed_at = now() WHERE id = $1",
        )
        .bind(version_id)
        .bind(format!("Duyệt sau kháng nghị: {note}"))
        .execute(&state.db)
        .await?;
    }
    audit::record(
        &state.db,
        Actor::Admin(reviewer.to_owned()),
        "review.appeal",
        Some(&app_id),
        None,
        json!({ "version": version, "overturned": overturn }),
    )
    .await;
    let (outcome, outcome_en) = if overturn {
        (
            "được chấp nhận; phiên bản đã được duyệt",
            "was accepted; the version is now approved",
        )
    } else {
        (
            "không được chấp nhận; quyết định từ chối giữ nguyên",
            "was not accepted; the rejection stands",
        )
    };
    notify_developer(
        state,
        &app_id,
        &format!("Kết quả kháng nghị {app_id} {version} / Appeal result"),
        &format!(
            "Kháng nghị cho {app_id} {version} {outcome}.\nGhi chú: {note}\n\n---\n\
             The appeal for {app_id} {version} {outcome_en}.\nNote: {note}\n"
        ),
    )
    .await;
    Ok(())
}

/// Gỡ khẩn cấp một phiên bản (`Some`) hoặc toàn bộ app (`None`), kèm lý do: biến khỏi danh mục
/// ngay, launcher chặn mở qua metadata TUF mới, nhà phát triển nhận email.
pub async fn revoke(
    state: &AppState,
    app_id: &str,
    version: Option<&str>,
    admin: &str,
    reason: &str,
) -> DevResult<usize> {
    if reason.trim().is_empty() {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Thu hồi phải có lý do.",
            "A revocation needs a reason.",
        ));
    }
    let versions: Vec<(Uuid, String, Vec<String>)> = sqlx::query_as(
        "SELECT v.id, v.version, COALESCE(array_agg(p.platform) FILTER (WHERE p.platform IS NOT NULL), '{}')
         FROM app_versions v LEFT JOIN app_packages p ON p.version_id = v.id
         WHERE v.app_id = $1 AND ($2::text IS NULL OR v.version = $2)
           AND v.status IN ('approved', 'released', 'paused', 'pending_review')
         GROUP BY v.id, v.version",
    )
    .bind(app_id)
    .bind(version)
    .fetch_all(&state.db)
    .await?;
    // Phiên bản chỉ có trong danh mục (thêm bằng công cụ, không qua cổng) vẫn được thu hồi.
    let catalog_only: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT version, platform, tuf_target FROM catalog_versions
         WHERE app_id = $1 AND ($2::text IS NULL OR version = $2) AND status = 'released'",
    )
    .bind(app_id)
    .bind(version)
    .fetch_all(&state.db)
    .await?;
    if versions.is_empty() && catalog_only.is_empty() {
        return Err(problem(
            StatusCode::NOT_FOUND,
            "not_found",
            "Không có phiên bản nào để thu hồi.",
            "There is nothing to revoke.",
        ));
    }
    let mut targets: Vec<String> = catalog_only.iter().map(|c| c.2.clone()).collect();
    for (id, ver, platforms) in &versions {
        sqlx::query("UPDATE app_versions SET status = 'revoked', review_note = $2 WHERE id = $1")
            .bind(id)
            .bind(format!("Thu hồi khẩn cấp: {}", reason.trim()))
            .execute(&state.db)
            .await?;
        for p in platforms {
            targets.push(developer::target_name(app_id, ver, p));
        }
    }
    let mut revoked_versions: BTreeSet<String> = versions.iter().map(|v| v.1.clone()).collect();
    revoked_versions.extend(catalog_only.iter().map(|c| c.0.clone()));
    for ver in &revoked_versions {
        catalog::revoke_version(&state.db, &state.catalog, app_id, ver).await?;
    }
    if version.is_none() {
        catalog::remove_app(&state.db, &state.catalog, app_id, reason.trim()).await?;
    }
    // Launcher chặn mở các phiên bản này sau lần đồng bộ kế tiếp (capability launcher).
    if let Some(tuf) = &state.tuf {
        let signers: Vec<_> = tuf.sign_keys.iter().map(feco_tuf::keys::source).collect();
        let repo = feco_tuf::Repo::open(&tuf.repo);
        let _guard = state.tuf_lock.lock().await;
        targets.sort();
        targets.dedup();
        for target in &targets {
            match repo
                .revoke(
                    target,
                    reason.trim(),
                    &signers,
                    feco_tuf::Expiry::default(),
                    jiff::Timestamp::now(),
                )
                .await
            {
                Ok(_) => {}
                Err(feco_tuf::Error::Invalid(msg)) if msg.contains("không có target") => {}
                Err(e) => {
                    return Err(DevError::Upstream(format!(
                        "không thu hồi được target TUF {target}: {e}"
                    )))
                }
            }
        }
    }
    // Báo cho chính app (spec app-sdk, "Sự kiện cho app").
    let client_id: Option<String> = sqlx::query_scalar(
        "SELECT client_id FROM catalog_apps WHERE id = $1
         UNION SELECT client_id FROM developer_apps WHERE app_id = $1 LIMIT 1",
    )
    .bind(app_id)
    .fetch_optional(&state.db)
    .await?;
    if let Some(client_id) = client_id {
        let payload = json!({
            "event": "app.version.revoked",
            "client_id": client_id,
            "app_id": app_id,
            "versions": revoked_versions,
            "reason": reason.trim(),
            "occurred_at": OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        });
        crate::store::enqueue_event(&state.db, &client_id, "app.version.revoked", &payload).await?;
    }
    audit::record(
        &state.db,
        Actor::Admin(admin.to_owned()),
        "review.revoke",
        Some(app_id),
        None,
        json!({ "versions": revoked_versions, "reason": reason.trim() }),
    )
    .await;
    let (scope, scope_en) = match version {
        Some(v) => (format!("phiên bản {v}"), format!("version {v}")),
        None => ("toàn bộ app".to_owned(), "every version".to_owned()),
    };
    notify_developer(
        state,
        app_id,
        &format!("{app_id}: thu hồi khẩn cấp / emergency revocation"),
        &format!(
            "F-Eco đã thu hồi {scope} của {app_id}.\nLý do: {}\n\nApp biến khỏi F-Store và F-Launcher chặn mở các phiên bản bị thu hồi.\n\
             Bạn có thể phản hồi qua {}developer/apps/{app_id}\n\n---\n\
             F-Eco revoked {scope_en} of {app_id}.\nReason: {0}\n\nThe app is removed from F-Store and F-Launcher blocks revoked versions.\n\
             You can respond at {1}developer/apps/{app_id}\n",
            reason.trim(),
            state.public_url
        ),
    )
    .await;
    Ok(revoked_versions.len())
}

#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    pub id: i64,
    pub kind: String,
    pub message: String,
    pub details: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

pub async fn open_alerts(db: &Db) -> sqlx::Result<Vec<Alert>> {
    let rows: Vec<(i64, String, String, Value, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, kind, message, details, created_at FROM admin_alerts
         WHERE acknowledged_at IS NULL ORDER BY id",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Alert {
            id: r.0,
            kind: r.1,
            message: r.2,
            details: r.3,
            created_at: r.4,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(scopes: &[&str], network: bool, folders: &[&str]) -> Manifest {
        let mut text = String::from(
            "id = \"soundlab\"\nname = \"SoundLab\"\nversion = \"1.2.0\"\nlicense = \"MIT\"\n\
             privacy_policy = \"https://s.example/p\"\n[[platforms]]\nos = \"windows\"\narch = \"x64\"\nentry = \"S.exe\"\n",
        );
        for s in scopes {
            text.push_str(&format!("[scopes.{s}]\npurpose = \"x\"\n"));
        }
        text.push_str(&format!(
            "[permissions]\nnetwork = {network}\nuser_folders = [{}]\n",
            folders
                .iter()
                .map(|f| format!("\"{f}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        feco_manifest::parse(&text).unwrap()
    }

    /// Scenario "Bản cập nhật không đổi quyền" và "Thêm scope".
    #[test]
    fn manual_or_automatic() {
        let base = manifest(&["profile"], true, &["music"]);
        assert_eq!(
            manual_reason(None, &base).as_deref(),
            Some("Phiên bản đầu tiên của app")
        );
        assert_eq!(
            manual_reason(Some(&base), &base),
            None,
            "không đổi scope và quyền"
        );
        let fewer = manifest(&[], false, &[]);
        assert_eq!(
            manual_reason(Some(&base), &fewer),
            None,
            "bớt quyền không cần duyệt lại"
        );
        let phone = manifest(&["profile", "phone"], true, &["music"]);
        assert_eq!(
            manual_reason(Some(&base), &phone).as_deref(),
            Some("Thêm scope dữ liệu: phone")
        );
        let folders = manifest(&["profile"], true, &["music", "documents"]);
        assert_eq!(
            manual_reason(Some(&base), &folders).as_deref(),
            Some("Thêm quyền hệ thống: user_folders.documents")
        );
        let offline = manifest(&["profile"], false, &["music"]);
        assert_eq!(
            manual_reason(Some(&offline), &base).as_deref(),
            Some("Thêm quyền hệ thống: network")
        );
    }

    #[test]
    fn clauses_are_complete() {
        let codes: Vec<&str> = CLAUSES.iter().map(|c| c.code).collect();
        for code in [
            "malware",
            "undeclared_data",
            "hidden_mining",
            "drm_copyright",
            "brand_impersonation",
            "illegal_content",
        ] {
            assert!(codes.contains(&code), "{code}");
        }
        assert_eq!(
            clause("brand_impersonation").unwrap().title_vi,
            "Giả mạo thương hiệu"
        );
        for c in &CLAUSES {
            assert!(!c.rule_en.is_empty() && !c.fix_en.is_empty() && !c.fix_vi.is_empty());
        }
    }
}
