//! Cổng nhà phát triển: tài khoản, thỏa thuận, xác minh tên miền, nhóm và vai trò, app và thông
//! tin đăng nhập, phiên bản, gói, phát hành dần, số liệu (spec developer-portal).
//!
//! Module này chứa quy tắc nghiệp vụ; `dev_web.rs` là các trang web và API gọi vào đây.

use std::path::PathBuf;

use axum::http::StatusCode;
use base64::Engine;
use feco_manifest::Manifest;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::Digest;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    catalog,
    codesign::{self, Signature},
    dns,
    store::Db,
    AppState,
};

/// Phiên bản Thỏa thuận nhà phát triển hiện hành. Đổi giá trị này thì mọi tài khoản phải chấp
/// nhận lại trước khi tạo app hay nộp phiên bản.
pub const AGREEMENT_VERSION: &str = "2026-11-01";
/// Gói tối đa 2 GB (spec developer-portal, "Nộp gói phiên bản").
pub const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Tệp chạy lớn nhất được đọc để kiểm tra chữ ký.
const MAX_ENTRY_BYTES: u64 = 1024 * 1024 * 1024;
const ALL_SCOPES: &str = "openid offline_access profile email locale phone billing purchases";

/// Lỗi có thông báo cho người dùng (hai ngôn ngữ) và mã HTTP.
#[derive(Debug, Clone)]
pub struct Problem {
    pub status: StatusCode,
    pub code: &'static str,
    pub vi: String,
    pub en: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DevError {
    #[error("{}", .0.vi)]
    Problem(Problem),
    #[error("cơ sở dữ liệu: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Upstream(String),
}

pub type DevResult<T> = Result<T, DevError>;

pub(crate) fn problem(
    status: StatusCode,
    code: &'static str,
    vi: impl Into<String>,
    en: impl Into<String>,
) -> DevError {
    DevError::Problem(Problem {
        status,
        code,
        vi: vi.into(),
        en: en.into(),
    })
}

fn invalid(vi: impl Into<String>, en: impl Into<String>) -> DevError {
    problem(StatusCode::BAD_REQUEST, "invalid_request", vi, en)
}

fn not_found() -> DevError {
    problem(
        StatusCode::NOT_FOUND,
        "not_found",
        "Không tìm thấy.",
        "Not found.",
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Admin,
    Developer,
    Viewer,
}

impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        Some(match s {
            "owner" => Role::Owner,
            "admin" => Role::Admin,
            "developer" => Role::Developer,
            "viewer" => Role::Viewer,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Developer => "developer",
            Role::Viewer => "viewer",
        }
    }

    pub const fn label_vi(self) -> &'static str {
        match self {
            Role::Owner => "Chủ sở hữu",
            Role::Admin => "Quản trị",
            Role::Developer => "Phát triển",
            Role::Viewer => "Chỉ xem",
        }
    }

    /// Tạo app, nộp manifest và gói.
    pub const fn can_edit(self) -> bool {
        !matches!(self, Role::Viewer)
    }

    /// Phát hành phiên bản, mời thành viên, tạo token.
    pub const fn can_release(self) -> bool {
        matches!(self, Role::Owner | Role::Admin)
    }

    /// Xóa app.
    pub const fn can_delete(self) -> bool {
        matches!(self, Role::Owner)
    }
}

fn need_edit(role: Role) -> DevResult<()> {
    if role.can_edit() {
        Ok(())
    } else {
        Err(problem(
            StatusCode::FORBIDDEN,
            "insufficient_role",
            "Vai trò Chỉ xem không được thay đổi app.",
            "The Viewer role cannot change apps.",
        ))
    }
}

fn need_release(role: Role) -> DevResult<()> {
    if role.can_release() {
        Ok(())
    } else {
        Err(problem(
            StatusCode::FORBIDDEN,
            "insufficient_role",
            "Cần vai trò Quản trị hoặc Chủ sở hữu.",
            "This needs the Admin or Owner role.",
        ))
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: Uuid,
    pub kind: String,
    pub display_name: String,
    pub legal_name: Option<String>,
    pub business_id: Option<String>,
    pub domain: Option<String>,
    pub domain_token: String,
    pub domain_verified: bool,
    pub agreement_version: Option<String>,
    pub agreement_accepted_at: Option<OffsetDateTime>,
    /// Bị tạm khóa chờ điều tra (ví dụ khi gói có mã độc).
    pub suspended: bool,
}

impl Account {
    pub fn agreement_current(&self) -> bool {
        self.agreement_version.as_deref() == Some(AGREEMENT_VERSION)
    }

    /// Giá trị bản ghi TXT phải thêm vào tên miền.
    pub fn txt_record(&self) -> String {
        format!("feco-domain-verification={}", self.domain_token)
    }
}

#[derive(Debug, Clone)]
pub struct Membership {
    pub account: Account,
    pub role: Role,
    pub subject: Uuid,
    pub email: String,
}

impl Membership {
    fn need_agreement(&self) -> DevResult<()> {
        if self.account.suspended {
            return Err(problem(
                StatusCode::FORBIDDEN,
                "account_suspended",
                "Tài khoản nhà phát triển đang bị tạm khóa chờ điều tra.",
                "This developer account is suspended pending investigation.",
            ));
        }
        if self.account.agreement_current() {
            Ok(())
        } else {
            Err(problem(
                StatusCode::FORBIDDEN,
                "agreement_required",
                "Bạn cần đọc và chấp nhận Thỏa thuận nhà phát triển trước.",
                "You need to read and accept the Developer Agreement first.",
            ))
        }
    }
}

type AccountRow = (
    Uuid,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<OffsetDateTime>,
    Option<String>,
    Option<OffsetDateTime>,
    bool,
);

const ACCOUNT_COLUMNS: &str =
    "a.id, a.kind, a.display_name, a.legal_name, a.business_id, a.domain, \
     a.domain_token, a.domain_verified_at, a.agreement_version, a.agreement_accepted_at, \
     a.suspended_at IS NOT NULL";

fn account_from(r: AccountRow) -> Account {
    Account {
        id: r.0,
        kind: r.1,
        display_name: r.2,
        legal_name: r.3,
        business_id: r.4,
        domain: r.5,
        domain_token: r.6,
        domain_verified: r.7.is_some(),
        agreement_version: r.8,
        agreement_accepted_at: r.9,
        suspended: r.10,
    }
}

fn random_token(prefix: &str) -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
    format!(
        "{prefix}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Tài khoản nhà phát triển mà người dùng đang là thành viên.
pub async fn membership(db: &Db, subject: Uuid) -> DevResult<Option<Membership>> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT role, email FROM developer_members WHERE subject = $1 AND status = 'active'",
    )
    .bind(subject)
    .fetch_optional(db)
    .await?;
    let Some((role, email)) = row else {
        return Ok(None);
    };
    let account: AccountRow = sqlx::query_as(&format!(
        "SELECT {ACCOUNT_COLUMNS} FROM developer_accounts a
         JOIN developer_members m ON m.account_id = a.id WHERE m.subject = $1"
    ))
    .bind(subject)
    .fetch_one(db)
    .await?;
    Ok(Some(Membership {
        account: account_from(account),
        role: Role::parse(&role).unwrap_or(Role::Viewer),
        subject,
        email,
    }))
}

#[derive(Debug, Clone, Default)]
pub struct Registration {
    pub organization: bool,
    pub display_name: String,
    pub legal_name: String,
    pub business_id: String,
    pub domain: String,
    pub accept_agreement: bool,
}

/// Điều kiện của danh tính để làm nhà phát triển: email đã xác minh và đã bật xác thực hai lớp.
pub fn identity_requirements(identity: &Value) -> DevResult<String> {
    let email = identity["traits"]["email"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let verified = identity["verifiable_addresses"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|a| a["value"] == email.as_str() && a["verified"] == true);
    if !verified {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "email_unverified",
            "Bạn cần xác minh email trước khi đăng ký nhà phát triển.",
            "Verify your email before registering as a developer.",
        ));
    }
    // Kratos tạo sẵn một mục `webauthn` rỗng cho mọi danh tính, nên chỉ mục `totp` (chỉ có khi
    // người dùng đã cấu hình xong) mới chứng tỏ đã bật xác thực hai lớp.
    if !identity["credentials"]["totp"].is_object() {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "second_factor_required",
            "Bạn cần bật xác thực hai lớp (ứng dụng xác thực TOTP) trong Cài đặt tài khoản trước khi đăng ký nhà phát triển.",
            "Turn on two-factor authentication (a TOTP authenticator app) in Account settings before registering as a developer.",
        ));
    }
    Ok(email)
}

/// Đăng ký nhà phát triển cá nhân hoặc tổ chức; người đăng ký là Chủ sở hữu.
pub async fn register(db: &Db, subject: Uuid, email: &str, r: &Registration) -> DevResult<Uuid> {
    if membership(db, subject).await?.is_some() {
        return Err(invalid(
            "Bạn đã thuộc một tài khoản nhà phát triển.",
            "You already belong to a developer account.",
        ));
    }
    let display = r.display_name.trim();
    if display.is_empty() || display.chars().count() > 80 {
        return Err(invalid(
            "Tên hiển thị là bắt buộc (tối đa 80 ký tự).",
            "A display name is required (80 characters at most).",
        ));
    }
    let domain = r.domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if r.organization {
        if r.legal_name.trim().is_empty() || r.business_id.trim().is_empty() {
            return Err(invalid(
                "Tổ chức phải có tên pháp lý và mã số doanh nghiệp.",
                "An organization needs a legal name and a business registration number.",
            ));
        }
        if !dns::valid_domain(&domain) {
            return Err(invalid(
                "Tên miền không hợp lệ.",
                "The domain is not valid.",
            ));
        }
    }
    let some = |s: &str| Some(s.trim().to_owned()).filter(|s| !s.is_empty() && r.organization);
    let id = Uuid::new_v4();
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO developer_accounts (id, kind, display_name, legal_name, business_id, domain,
             domain_token, agreement_version, agreement_accepted_at, agreement_accepted_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CASE WHEN $8 IS NULL THEN NULL ELSE now() END, $9)",
    )
    .bind(id)
    .bind(if r.organization { "organization" } else { "individual" })
    .bind(display)
    .bind(some(&r.legal_name))
    .bind(some(&r.business_id))
    .bind(some(&domain))
    .bind(random_token(""))
    .bind(r.accept_agreement.then_some(AGREEMENT_VERSION))
    .bind(r.accept_agreement.then_some(subject))
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO developer_members (account_id, email, subject, role, status, joined_at)
         VALUES ($1, $2, $3, 'owner', 'active', now())",
    )
    .bind(id)
    .bind(email)
    .bind(subject)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Ghi lại phiên bản và thời điểm chấp nhận Thỏa thuận nhà phát triển.
pub async fn accept_agreement(db: &Db, m: &Membership) -> DevResult<()> {
    need_release(m.role)?;
    sqlx::query(
        "UPDATE developer_accounts SET agreement_version = $2, agreement_accepted_at = now(),
             agreement_accepted_by = $3 WHERE id = $1",
    )
    .bind(m.account.id)
    .bind(AGREEMENT_VERSION)
    .bind(m.subject)
    .execute(db)
    .await?;
    Ok(())
}

/// Tra bản ghi TXT của tên miền; khớp thì tài khoản và mọi app của nó nhận dấu "Đã xác minh".
pub async fn verify_domain(state: &AppState, m: &Membership) -> DevResult<bool> {
    need_release(m.role)?;
    let Some(domain) = m.account.domain.as_deref() else {
        return Err(invalid(
            "Tài khoản chưa khai báo tên miền.",
            "This account has no domain.",
        ));
    };
    let records = dns::txt_records(&state.dns_servers, domain)
        .await
        .map_err(|e| DevError::Upstream(e.to_string()))?;
    let expected = m.account.txt_record();
    if !records.iter().any(|r| r.trim() == expected) {
        return Ok(false);
    }
    sqlx::query("UPDATE developer_accounts SET domain_verified_at = now() WHERE id = $1")
        .bind(m.account.id)
        .execute(&state.db)
        .await?;
    sqlx::query(
        "UPDATE catalog_apps SET developer_verified = true, updated_at = now()
         WHERE id IN (SELECT app_id FROM developer_apps WHERE account_id = $1)",
    )
    .bind(m.account.id)
    .execute(&state.db)
    .await?;
    state.catalog.clear();
    Ok(true)
}

#[derive(Debug, Clone)]
pub struct Member {
    pub email: String,
    pub role: Role,
    pub active: bool,
}

pub async fn members(db: &Db, account: Uuid) -> DevResult<Vec<Member>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT email, role, status FROM developer_members WHERE account_id = $1 ORDER BY invited_at",
    )
    .bind(account)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Member {
            email: r.0,
            role: Role::parse(&r.1).unwrap_or(Role::Viewer),
            active: r.2 == "active",
        })
        .collect())
}

/// Mời thành viên vào tài khoản tổ chức.
pub async fn invite(db: &Db, m: &Membership, email: &str, role: Role) -> DevResult<()> {
    need_release(m.role)?;
    if m.account.kind != "organization" {
        return Err(invalid(
            "Chỉ tài khoản tổ chức mới mời được thành viên.",
            "Only organization accounts can invite members.",
        ));
    }
    if role == Role::Owner && m.role != Role::Owner {
        return Err(need_release(Role::Viewer).unwrap_err());
    }
    let email = email.trim().to_ascii_lowercase();
    if !email.contains('@') || email.len() > 254 {
        return Err(invalid("Email không hợp lệ.", "The email is not valid."));
    }
    sqlx::query(
        "INSERT INTO developer_members (account_id, email, role, status) VALUES ($1, $2, $3, 'invited')
         ON CONFLICT (account_id, email) DO UPDATE SET role = $3
         WHERE developer_members.status = 'invited'",
    )
    .bind(m.account.id)
    .bind(&email)
    .bind(role.as_str())
    .execute(db)
    .await?;
    Ok(())
}

/// Lời mời đang chờ cho một địa chỉ email: (id tài khoản, tên tài khoản, vai trò).
pub async fn invitations(db: &Db, email: &str) -> DevResult<Vec<(Uuid, String, Role)>> {
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT a.id, a.display_name, m.role FROM developer_members m
         JOIN developer_accounts a ON a.id = m.account_id
         WHERE m.email = $1 AND m.status = 'invited'",
    )
    .bind(email.to_ascii_lowercase())
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.0, r.1, Role::parse(&r.2).unwrap_or(Role::Viewer)))
        .collect())
}

pub async fn accept_invitation(
    db: &Db,
    account: Uuid,
    subject: Uuid,
    email: &str,
) -> DevResult<()> {
    if membership(db, subject).await?.is_some() {
        return Err(invalid(
            "Bạn đã thuộc một tài khoản nhà phát triển.",
            "You already belong to a developer account.",
        ));
    }
    let n = sqlx::query(
        "UPDATE developer_members SET subject = $3, status = 'active', joined_at = now()
         WHERE account_id = $1 AND email = $2 AND status = 'invited'",
    )
    .bind(account)
    .bind(email.to_ascii_lowercase())
    .bind(subject)
    .execute(db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(not_found());
    }
    Ok(())
}

/// Tạo token cho CLI `f-dev`. Giá trị token chỉ trả về một lần.
pub async fn create_token(db: &Db, m: &Membership, name: &str) -> DevResult<String> {
    need_edit(m.role)?;
    let token = random_token("fdev_");
    sqlx::query(
        "INSERT INTO developer_tokens (id, account_id, subject, name, token_hash)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(m.account.id)
    .bind(m.subject)
    .bind(name.trim())
    .bind(sha256_hex(token.as_bytes()))
    .execute(db)
    .await?;
    Ok(token)
}

/// Thành viên ứng với một token nhà phát triển còn hiệu lực (vai trò lấy theo thời điểm dùng).
pub async fn token_membership(db: &Db, token: &str) -> DevResult<Option<Membership>> {
    let subject: Option<Uuid> = sqlx::query_scalar(
        "UPDATE developer_tokens SET last_used_at = now()
         WHERE token_hash = $1 AND revoked_at IS NULL RETURNING subject",
    )
    .bind(sha256_hex(token.trim().as_bytes()))
    .fetch_optional(db)
    .await?;
    match subject {
        Some(s) => membership(db, s).await,
        None => Ok(None),
    }
}

/// Địa chỉ chuyển hướng hợp lệ: `https://`, loopback `http://127.0.0.1` hoặc `http://localhost`
/// (app desktop), hoặc custom URI scheme của app.
pub fn valid_redirect(uri: &str) -> bool {
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    match url.scheme() {
        "https" => url.host_str().is_some(),
        "http" => matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")),
        "javascript" | "data" | "file" | "about" | "blob" => false,
        _ => url.fragment().is_none(),
    }
}

fn parse_redirects(text: &str) -> DevResult<Vec<String>> {
    let uris: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if uris.is_empty() || uris.len() > 10 {
        return Err(invalid(
            "Cần từ 1 đến 10 địa chỉ chuyển hướng.",
            "Between 1 and 10 redirect URIs are required.",
        ));
    }
    if let Some(bad) = uris.iter().find(|u| !valid_redirect(u)) {
        return Err(invalid(
            format!("Địa chỉ chuyển hướng không hợp lệ: {bad} (dùng https, loopback http://127.0.0.1 hoặc scheme riêng của app)."),
            format!("Invalid redirect URI: {bad} (use https, loopback http://127.0.0.1 or the app's own scheme)."),
        ));
    }
    Ok(uris)
}

#[derive(Debug, Clone)]
pub struct DevApp {
    pub app_id: String,
    pub client_id: String,
    pub name: String,
    pub kind: String,
}

pub struct CreatedApp {
    pub app: DevApp,
    /// Chỉ có với app có máy chủ riêng; hiện một lần.
    pub client_secret: Option<String>,
}

fn hydra(e: crate::hydra::HydraError) -> DevError {
    DevError::Upstream(e.to_string())
}

/// Tạo app và OAuth client của nó (spec "Thông tin đăng nhập cho app").
pub async fn create_app(
    state: &AppState,
    m: &Membership,
    app_id: &str,
    name: &str,
    server_side: bool,
    redirects: &str,
) -> DevResult<CreatedApp> {
    m.need_agreement()?;
    need_edit(m.role)?;
    let app_id = app_id.trim();
    if !feco_manifest::valid_app_id(app_id) {
        return Err(invalid(
            "Mã app chỉ dùng chữ thường, số, '.', '-', '_' (tối đa 63 ký tự).",
            "The app ID may only use lowercase letters, digits, '.', '-', '_' (63 characters at most).",
        ));
    }
    if name.trim().is_empty() {
        return Err(invalid("Tên app là bắt buộc.", "An app name is required."));
    }
    let redirects = parse_redirects(redirects)?;
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM developer_apps WHERE app_id = $1)
             OR EXISTS (SELECT 1 FROM catalog_apps WHERE id = $1)",
    )
    .bind(app_id)
    .fetch_one(&state.db)
    .await?;
    if taken {
        return Err(invalid(
            "Mã app này đã có người dùng.",
            "This app ID is already taken.",
        ));
    }
    let client = state
        .hydra
        .create_client(&json!({
            "client_name": name.trim(),
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "scope": ALL_SCOPES,
            "redirect_uris": redirects,
            // App desktop: client công khai, bắt buộc PKCE, không có secret trong gói.
            "token_endpoint_auth_method": if server_side { "client_secret_basic" } else { "none" },
            "subject_type": "pairwise",
            "metadata": {
                "developer": m.account.display_name,
                "developer_verified": m.account.domain_verified,
                "scopes": {},
            },
        }))
        .await
        .map_err(hydra)?;
    let client_id = client["client_id"]
        .as_str()
        .ok_or_else(|| DevError::Upstream("Hydra không trả client_id".into()))?
        .to_owned();
    let kind = if server_side { "server" } else { "desktop" };
    sqlx::query(
        "INSERT INTO developer_apps (app_id, account_id, client_id, name, kind) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(app_id)
    .bind(m.account.id)
    .bind(&client_id)
    .bind(name.trim())
    .bind(kind)
    .execute(&state.db)
    .await?;
    Ok(CreatedApp {
        app: DevApp {
            app_id: app_id.to_owned(),
            client_id,
            name: name.trim().to_owned(),
            kind: kind.to_owned(),
        },
        client_secret: client["client_secret"].as_str().map(str::to_owned),
    })
}

pub async fn apps(db: &Db, account: Uuid) -> DevResult<Vec<DevApp>> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT app_id, client_id, name, kind FROM developer_apps
         WHERE account_id = $1 AND deleted_at IS NULL ORDER BY created_at",
    )
    .bind(account)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| DevApp {
            app_id: r.0,
            client_id: r.1,
            name: r.2,
            kind: r.3,
        })
        .collect())
}

/// App của tài khoản; app của tài khoản khác coi như không tồn tại.
pub async fn app(db: &Db, m: &Membership, app_id: &str) -> DevResult<DevApp> {
    apps(db, m.account.id)
        .await?
        .into_iter()
        .find(|a| a.app_id == app_id)
        .ok_or_else(not_found)
}

async fn client_json(state: &AppState, client_id: &str) -> DevResult<Value> {
    state
        .hydra
        .get_client(client_id)
        .await
        .map_err(hydra)?
        .ok_or_else(not_found)
}

pub async fn redirect_uris(state: &AppState, app: &DevApp) -> DevResult<Vec<String>> {
    let client = client_json(state, &app.client_id).await?;
    Ok(client["redirect_uris"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect())
}

pub async fn set_redirects(
    state: &AppState,
    m: &Membership,
    app: &DevApp,
    text: &str,
) -> DevResult<()> {
    need_release(m.role)?;
    let mut client = client_json(state, &app.client_id).await?;
    client["redirect_uris"] = json!(parse_redirects(text)?);
    state
        .hydra
        .update_client(&app.client_id, &client)
        .await
        .map_err(hydra)?;
    Ok(())
}

/// Xoay client secret của app có máy chủ riêng; secret mới chỉ hiện một lần.
pub async fn rotate_secret(state: &AppState, m: &Membership, app: &DevApp) -> DevResult<String> {
    need_release(m.role)?;
    if app.kind != "server" {
        return Err(invalid(
            "App desktop dùng client công khai với PKCE, không có client secret.",
            "Desktop apps use a public client with PKCE and have no client secret.",
        ));
    }
    let mut client = client_json(state, &app.client_id).await?;
    let secret = random_token("");
    client["client_secret"] = json!(secret);
    state
        .hydra
        .update_client(&app.client_id, &client)
        .await
        .map_err(hydra)?;
    Ok(secret)
}

/// Đăng ký địa chỉ nhận webhook (https) và tạo khóa ký mới; khóa chỉ hiện một lần.
pub async fn register_webhook(
    state: &AppState,
    m: &Membership,
    app: &DevApp,
    url: &str,
) -> DevResult<String> {
    need_release(m.role)?;
    let url = url.trim();
    let ok = url::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            || (u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1" | "localhost")))
    });
    if !ok {
        return Err(invalid(
            "Địa chỉ webhook phải là https:// (http chỉ cho 127.0.0.1 khi dev).",
            "The webhook URL must use https:// (http only for 127.0.0.1 during development).",
        ));
    }
    Ok(crate::store::register_webhook(&state.db, &state.data_key, &app.client_id, url).await?)
}

/// Xóa app: chỉ Chủ sở hữu. App biến khỏi danh mục; OAuth client bị xóa.
pub async fn delete_app(state: &AppState, m: &Membership, app: &DevApp) -> DevResult<()> {
    if !m.role.can_delete() {
        return Err(problem(
            StatusCode::FORBIDDEN,
            "insufficient_role",
            "Chỉ Chủ sở hữu mới xóa được app.",
            "Only the Owner can delete an app.",
        ));
    }
    sqlx::query("UPDATE developer_apps SET deleted_at = now() WHERE app_id = $1")
        .bind(&app.app_id)
        .execute(&state.db)
        .await?;
    catalog::remove_app(
        &state.db,
        &state.catalog,
        &app.app_id,
        "Nhà phát triển xóa app",
    )
    .await?;
    state
        .hydra
        .delete_client(&app.client_id)
        .await
        .map_err(hydra)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub id: Uuid,
    pub app_id: String,
    pub version: String,
    pub status: String,
    pub channel: Option<String>,
    pub rollout_percent: i32,
    /// Nền tảng khai báo trong manifest.
    pub platforms: Vec<String>,
    /// Nền tảng đã có gói.
    pub uploaded: Vec<String>,
    pub review_note: Option<String>,
    /// `checks`: đang kiểm tra tự động; `manual`: chờ duyệt thủ công; `done`.
    pub review_stage: Option<String>,
    /// Lý do phải duyệt thủ công.
    pub manual_reason: Option<String>,
    /// Đang bị từ chối và lần từ chối này chưa được kháng nghị.
    pub appealable: bool,
}

type VersionRow = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    i32,
    Value,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
);

const VERSION_COLUMNS: &str =
    "v.id, v.app_id, v.version, v.status, v.channel, v.rollout_percent, v.manifest, v.review_note, \
     v.review_stage, v.manual_reason, \
     v.status = 'rejected' AND NOT EXISTS (SELECT 1 FROM review_appeals ra \
         WHERE ra.version_id = v.id AND ra.reject_no = v.reject_count)";

async fn version_info(db: &Db, r: VersionRow) -> DevResult<VersionInfo> {
    let uploaded: Vec<String> = sqlx::query_scalar(
        "SELECT platform FROM app_packages WHERE version_id = $1 ORDER BY platform",
    )
    .bind(r.0)
    .fetch_all(db)
    .await?;
    let manifest: Manifest =
        serde_json::from_value(r.6).map_err(|e| DevError::Upstream(e.to_string()))?;
    Ok(VersionInfo {
        id: r.0,
        app_id: r.1,
        version: r.2,
        status: r.3,
        channel: r.4,
        rollout_percent: r.5,
        platforms: manifest.platforms.iter().map(|p| p.key()).collect(),
        uploaded,
        review_note: r.7,
        review_stage: r.8,
        manual_reason: r.9,
        appealable: r.10,
    })
}

pub async fn versions(db: &Db, app_id: &str) -> DevResult<Vec<VersionInfo>> {
    let rows: Vec<VersionRow> = sqlx::query_as(&format!(
        "SELECT {VERSION_COLUMNS} FROM app_versions v WHERE v.app_id = $1 ORDER BY v.created_at DESC"
    ))
    .bind(app_id)
    .fetch_all(db)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        out.push(version_info(db, r).await?);
    }
    Ok(out)
}

/// Phiên bản thuộc một app của tài khoản.
pub async fn version(db: &Db, m: &Membership, id: Uuid) -> DevResult<(VersionInfo, Manifest)> {
    let row: Option<VersionRow> = sqlx::query_as(&format!(
        "SELECT {VERSION_COLUMNS} FROM app_versions v
         JOIN developer_apps a ON a.app_id = v.app_id
         WHERE v.id = $1 AND a.account_id = $2 AND a.deleted_at IS NULL"
    ))
    .bind(id)
    .bind(m.account.id)
    .fetch_optional(db)
    .await?;
    let row = row.ok_or_else(not_found)?;
    let manifest: Manifest =
        serde_json::from_value(row.6.clone()).map_err(|e| DevError::Upstream(e.to_string()))?;
    Ok((version_info(db, row).await?, manifest))
}

/// Nộp `f-app.toml` cho một phiên bản mới (hoặc sửa bản nháp). Manifest sai bị từ chối kèm danh
/// sách lỗi chỉ rõ từng trường; phiên bản phải lớn hơn bản đã phát hành.
pub async fn save_manifest(
    db: &Db,
    m: &Membership,
    app: &DevApp,
    toml: &str,
) -> DevResult<VersionInfo> {
    m.need_agreement()?;
    need_edit(m.role)?;
    let manifest = feco_manifest::parse(toml).map_err(|errors| {
        problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_manifest",
            errors.join("\n"),
            errors.join("\n"),
        )
    })?;
    if manifest.id != app.app_id {
        return Err(problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_manifest",
            format!(
                "id trong manifest là {:?} nhưng app này là {:?}",
                manifest.id, app.app_id
            ),
            format!(
                "the manifest id is {:?} but this app is {:?}",
                manifest.id, app.app_id
            ),
        ));
    }
    let released: Vec<String> = sqlx::query_scalar(
        "SELECT version FROM app_versions WHERE app_id = $1 AND status IN ('released', 'paused')",
    )
    .bind(&app.app_id)
    .fetch_all(db)
    .await?;
    let latest = released
        .iter()
        .filter_map(|v| semver::Version::parse(v).ok())
        .max();
    if let Some(latest) = latest.filter(|l| manifest.semver() <= *l) {
        return Err(problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "version_not_increasing",
            format!(
                "version {} phải lớn hơn bản đã phát hành {latest}",
                manifest.version
            ),
            format!(
                "version {} must be greater than the released version {latest}",
                manifest.version
            ),
        ));
    }
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, status FROM app_versions WHERE app_id = $1 AND version = $2")
            .bind(&app.app_id)
            .bind(&manifest.version)
            .fetch_optional(db)
            .await?;
    let json = serde_json::to_value(&manifest).map_err(|e| DevError::Upstream(e.to_string()))?;
    let id = match existing {
        Some((id, status)) if status == "draft" || status == "rejected" => {
            sqlx::query(
                "UPDATE app_versions SET manifest = $2, manifest_toml = $3, status = 'draft' WHERE id = $1",
            )
            .bind(id)
            .bind(&json)
            .bind(toml)
            .execute(db)
            .await?;
            // Manifest đổi thì các gói đã tải lên phải nộp lại.
            sqlx::query("DELETE FROM app_packages WHERE version_id = $1")
                .bind(id)
                .execute(db)
                .await?;
            id
        }
        Some(_) => {
            return Err(problem(
                StatusCode::CONFLICT,
                "version_exists",
                format!(
                    "Phiên bản {} đã được nộp; hãy tăng số phiên bản.",
                    manifest.version
                ),
                format!(
                    "Version {} was already submitted; bump the version number.",
                    manifest.version
                ),
            ))
        }
        None => {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO app_versions (id, app_id, version, manifest, manifest_toml, created_by)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(id)
            .bind(&app.app_id)
            .bind(&manifest.version)
            .bind(&json)
            .bind(toml)
            .bind(m.subject)
            .execute(db)
            .await?;
            id
        }
    };
    Ok(version(db, m, id).await?.0)
}

/// Nơi lưu gói của một phiên bản cho một nền tảng.
pub fn package_path(state: &AppState, app_id: &str, version: &str, platform: &str) -> PathBuf {
    state
        .package_dir
        .join(app_id)
        .join(version)
        .join(format!("{platform}.zip"))
}

/// Tên target TUF và tên gói trong danh mục.
pub fn target_name(app_id: &str, version: &str, platform: &str) -> String {
    format!("{app_id}-{version}-{platform}.zip")
}

fn package_problem(vi: impl Into<String>, en: impl Into<String>) -> DevError {
    problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_package", vi, en)
}

/// Kiểm tra gói zip đã tải lên: có tệp chạy như manifest khai báo, và tệp chạy được ký mã.
pub fn inspect_package(zip_path: &std::path::Path, os: &str, entry: &str) -> DevResult<()> {
    use std::io::Read;
    let file = std::fs::File::open(zip_path).map_err(|e| DevError::Upstream(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| {
        package_problem(
            format!("Gói không phải file zip hợp lệ: {e}"),
            format!("The package is not a valid zip file: {e}"),
        )
    })?;
    let names: Vec<String> = zip
        .file_names()
        .filter_map(|n| n.ok().map(|c| c.into_owned()))
        .collect();
    let mut bytes = Vec::new();
    {
        let item = zip.by_name(entry).map_err(|_| {
            package_problem(
                format!("Gói không có tệp chạy {entry} như khai báo trong f-app.toml."),
                format!(
                    "The package does not contain the entry file {entry} declared in f-app.toml."
                ),
            )
        })?;
        item.take(MAX_ENTRY_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|e| {
                package_problem(
                    format!("Không đọc được {entry}: {e}"),
                    format!("Cannot read {entry}: {e}"),
                )
            })?;
    }
    match os {
        "windows" => {
            if codesign::authenticode(&bytes) != Signature::Signed {
                return Err(package_problem(
                    "Gói Windows phải được ký Authenticode",
                    "Windows packages must be Authenticode-signed",
                ));
            }
        }
        "macos" => {
            if codesign::macho(&bytes) != Signature::Signed {
                return Err(package_problem(
                    "Gói macOS phải được ký Developer ID",
                    "macOS packages must be signed with a Developer ID",
                ));
            }
            // Vé notarize được đính vào bundle (.app/Contents/CodeResources).
            let stapled = entry
                .split_once(".app/")
                .map(|(bundle, _)| format!("{bundle}.app/Contents/CodeResources"))
                .is_some_and(|ticket| names.contains(&ticket));
            if !stapled {
                return Err(package_problem(
                    "Gói macOS phải được notarize và đính vé notarize vào bundle .app",
                    "macOS packages must be notarized with the ticket stapled to the .app bundle",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Một gói vừa nhận vào file tạm.
pub struct Upload<'a> {
    pub path: &'a std::path::Path,
    /// SHA-256 do người nộp khai báo (CLI); cổng web không khai báo.
    pub declared_sha256: Option<&'a str>,
    /// SHA-256 máy chủ tính được khi nhận.
    pub sha256: &'a str,
    pub size: u64,
}

/// Ghi nhận gói đã tải lên đĩa (`uploaded` là file tạm): kiểm tra SHA-256, tệp chạy và chữ ký
/// mã, rồi chuyển vào kho gói.
pub async fn store_package(
    state: &AppState,
    m: &Membership,
    version_id: Uuid,
    platform: &str,
    upload: Upload<'_>,
) -> DevResult<()> {
    let Upload {
        path: uploaded,
        declared_sha256,
        sha256: actual_sha256,
        size,
    } = upload;
    m.need_agreement()?;
    need_edit(m.role)?;
    let (info, manifest) = version(&state.db, m, version_id).await?;
    if info.status != "draft" {
        return Err(problem(
            StatusCode::CONFLICT,
            "not_draft",
            "Chỉ tải gói lên được cho phiên bản đang ở trạng thái nháp.",
            "Packages can only be uploaded to a draft version.",
        ));
    }
    let declared = manifest
        .platforms
        .iter()
        .find(|p| p.key() == platform)
        .ok_or_else(|| {
            package_problem(
                format!("f-app.toml không khai báo nền tảng {platform}."),
                format!("f-app.toml does not declare the platform {platform}."),
            )
        })?;
    if declared_sha256.is_some_and(|d| !d.trim().eq_ignore_ascii_case(actual_sha256)) {
        return Err(package_problem(
            "SHA-256 của gói không khớp giá trị đã khai báo.",
            "The package SHA-256 does not match the declared value.",
        ));
    }
    let (path, os, entry) = (
        uploaded.to_path_buf(),
        declared.os.clone(),
        declared.entry.clone(),
    );
    tokio::task::spawn_blocking(move || inspect_package(&path, &os, &entry))
        .await
        .map_err(|e| DevError::Upstream(e.to_string()))??;
    let dest = package_path(state, &info.app_id, &info.version, platform);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| DevError::Upstream(e.to_string()))?;
    }
    tokio::fs::rename(uploaded, &dest)
        .await
        .map_err(|e| DevError::Upstream(e.to_string()))?;
    sqlx::query(
        "INSERT INTO app_packages (version_id, platform, sha256, size_bytes, storage_path)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (version_id, platform) DO UPDATE SET sha256 = $3, size_bytes = $4,
             storage_path = $5, uploaded_at = now()",
    )
    .bind(version_id)
    .bind(platform)
    .bind(actual_sha256)
    .bind(size as i64)
    .bind(dest.to_string_lossy().into_owned())
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Gửi phiên bản đi kiểm duyệt: mọi nền tảng trong manifest phải có gói.
pub async fn submit(db: &Db, m: &Membership, version_id: Uuid) -> DevResult<VersionInfo> {
    m.need_agreement()?;
    need_edit(m.role)?;
    let (info, _) = version(db, m, version_id).await?;
    let missing: Vec<&String> = info
        .platforms
        .iter()
        .filter(|p| !info.uploaded.contains(p))
        .collect();
    if info.status != "draft" || !missing.is_empty() {
        let list = missing
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(problem(
            StatusCode::CONFLICT,
            "not_ready",
            format!("Chưa gửi duyệt được: phiên bản phải ở trạng thái nháp và có đủ gói (thiếu: {list})."),
            format!("Cannot submit: the version must be a draft with every package uploaded (missing: {list})."),
        ));
    }
    sqlx::query(
        "UPDATE app_versions SET status = 'pending_review', review_stage = 'checks',
             submitted_at = now() WHERE id = $1",
    )
    .bind(version_id)
    .execute(db)
    .await?;
    // Kiểm tra tự động (nhóm 9) chạy ở worker nền; xem `review::process_due`.
    crate::review::enqueue(db, version_id, "submit").await?;
    Ok(version(db, m, version_id).await?.0)
}

/// Phát hành một phiên bản đã duyệt lên kênh `stable` hoặc `beta` cho `percent`% người dùng, hoặc
/// đổi phần trăm của bản đang phát hành. Chỉ Quản trị và Chủ sở hữu.
pub async fn release(
    state: &AppState,
    m: &Membership,
    version_id: Uuid,
    channel: &str,
    percent: i32,
) -> DevResult<()> {
    need_release(m.role)?;
    m.need_agreement()?;
    if !["stable", "beta"].contains(&channel) || !(1..=100).contains(&percent) {
        return Err(invalid(
            "Kênh phải là stable hoặc beta, phần trăm từ 1 đến 100.",
            "The channel must be stable or beta, and the percentage between 1 and 100.",
        ));
    }
    let (info, manifest) = version(&state.db, m, version_id).await?;
    if !["approved", "released", "paused"].contains(&info.status.as_str()) {
        return Err(problem(
            StatusCode::CONFLICT,
            "not_approved",
            "Chỉ phát hành được phiên bản đã qua kiểm duyệt.",
            "Only versions that passed review can be released.",
        ));
    }
    let dev_app = app(&state.db, m, &info.app_id).await?;

    // OAuth client: chính sách quyền riêng tư và scope kèm mục đích lấy từ manifest, để màn hình
    // đồng ý và trang F-Store hiện đúng điều app đã khai báo.
    let mut client = client_json(state, &dev_app.client_id).await?;
    let scopes: serde_json::Map<String, Value> = manifest
        .scopes
        .iter()
        .map(|s| {
            (
                s.scope.as_str().to_owned(),
                json!({
                    "required": s.required,
                    "purpose_vi": s.purpose,
                    "purpose_en": s.purpose_en.clone().unwrap_or_else(|| s.purpose.clone()),
                }),
            )
        })
        .collect();
    client["policy_uri"] = json!(manifest.privacy_policy);
    client["client_name"] = json!(manifest.name);
    client["metadata"] = json!({
        "developer": m.account.display_name,
        "developer_verified": m.account.domain_verified,
        "scopes": scopes,
    });
    state
        .hydra
        .update_client(&dev_app.client_id, &client)
        .await
        .map_err(hydra)?;

    catalog::upsert_app(
        &state.db,
        &state.catalog,
        &catalog::AppInput {
            id: info.app_id.clone(),
            client_id: dev_app.client_id.clone(),
            name: manifest.name.clone(),
            developer: m.account.display_name.clone(),
            developer_verified: m.account.domain_verified,
            owner_subject: owner_subject(&state.db, m.account.id).await?,
            first_party: false,
            summary: manifest.summary.clone(),
            description: manifest.description.clone(),
            category: manifest.category.clone(),
            price_vnd: 0,
            license: manifest.license.clone(),
            screenshots: Vec::new(),
        },
    )
    .await?;
    let packages: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT platform, size_bytes, storage_path FROM app_packages WHERE version_id = $1",
    )
    .bind(version_id)
    .fetch_all(&state.db)
    .await?;
    for (platform, size, path) in &packages {
        let target = target_name(&info.app_id, &info.version, platform);
        sign_target(state, &manifest, &dev_app, platform, &target, path).await?;
        catalog::release_version(
            &state.db,
            &state.catalog,
            &info.app_id,
            &catalog::VersionInput {
                version: info.version.clone(),
                platform: platform.clone(),
                size_bytes: (*size).max(0) as u64,
                tuf_target: target,
                changelog: String::new(),
            },
        )
        .await?;
    }
    sqlx::query(
        "UPDATE catalog_versions SET channel = $3, rollout_percent = $4 WHERE app_id = $1 AND version = $2",
    )
    .bind(&info.app_id)
    .bind(&info.version)
    .bind(channel)
    .bind(percent)
    .execute(&state.db)
    .await?;
    sqlx::query(
        "UPDATE app_versions SET status = 'released', channel = $2, rollout_percent = $3,
             released_at = COALESCE(released_at, now()) WHERE id = $1",
    )
    .bind(version_id)
    .bind(channel)
    .bind(percent)
    .execute(&state.db)
    .await?;
    state.catalog.clear();
    Ok(())
}

async fn owner_subject(db: &Db, account: Uuid) -> DevResult<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT subject FROM developer_members WHERE account_id = $1 AND role = 'owner' AND status = 'active'
         ORDER BY joined_at LIMIT 1",
    )
    .bind(account)
    .fetch_optional(db)
    .await?
    .flatten())
}

/// Ký gói thành target TUF (design D3), nếu máy chủ được cấu hình kho TUF.
async fn sign_target(
    state: &AppState,
    manifest: &Manifest,
    app: &DevApp,
    platform: &str,
    target: &str,
    path: &str,
) -> DevResult<()> {
    let Some(tuf) = &state.tuf else {
        return Ok(());
    };
    let entry = manifest
        .platforms
        .iter()
        .find(|p| p.key() == platform)
        .map(|p| p.entry.clone());
    // App riêng tư (spec custom-build-orders): đánh dấu trong target để launcher chỉ hiện cho
    // khách hàng của đơn.
    let private: bool =
        sqlx::query_scalar("SELECT private_to IS NOT NULL FROM catalog_apps WHERE id = $1")
            .bind(&app.app_id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or(false);
    let meta = feco_tuf::TargetMeta {
        app: app.app_id.clone(),
        version: manifest.version.clone(),
        platform: platform.to_owned(),
        title: Some(manifest.name.clone()),
        entry,
        client_id: Some(app.client_id.clone()),
        private,
    };
    let signers: Vec<_> = tuf.sign_keys.iter().map(feco_tuf::keys::source).collect();
    let repo = feco_tuf::Repo::open(&tuf.repo);
    // Một lần ký tại một thời điểm: metadata TUF được đọc, sửa rồi ghi lại.
    let _guard = state.tuf_lock.lock().await;
    match repo
        .publish(
            std::path::Path::new(path),
            target,
            &meta,
            &signers,
            feco_tuf::Expiry::default(),
            jiff::Timestamp::now(),
        )
        .await
    {
        Ok(()) => Ok(()),
        // Đã ký từ lần phát hành trước (ví dụ sau khi tạm dừng rồi phát hành lại).
        Err(feco_tuf::Error::Invalid(msg)) if msg.contains("đã tồn tại") => Ok(()),
        Err(e) => Err(DevError::Upstream(format!("không ký được target TUF: {e}"))),
    }
}

/// Dừng phát hành dần: launcher chưa nhận bản này sẽ không nhận nữa.
pub async fn pause(state: &AppState, m: &Membership, version_id: Uuid) -> DevResult<()> {
    need_release(m.role)?;
    let (info, _) = version(&state.db, m, version_id).await?;
    if info.status != "released" {
        return Err(problem(
            StatusCode::CONFLICT,
            "not_released",
            "Phiên bản này chưa được phát hành.",
            "This version has not been released.",
        ));
    }
    sqlx::query("UPDATE app_versions SET status = 'paused', rollout_percent = 0 WHERE id = $1")
        .bind(version_id)
        .execute(&state.db)
        .await?;
    sqlx::query(
        "UPDATE catalog_versions SET rollout_percent = 0 WHERE app_id = $1 AND version = $2",
    )
    .bind(&info.app_id)
    .bind(&info.version)
    .execute(&state.db)
    .await?;
    state.catalog.clear();
    Ok(())
}

/// Nhóm (0–99) của một thiết bị cho một phiên bản: ổn định giữa các lần hỏi, độc lập giữa các
/// phiên bản.
pub fn rollout_bucket(app_id: &str, version: &str, device: &str) -> u32 {
    let digest = sha2::Sha256::digest(format!("{app_id}\n{version}\n{device}").as_bytes());
    u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 100
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    pub tuf_target: String,
}

/// Phiên bản mà một launcher nên dùng: bản mới nhất trên kênh mà thiết bị nằm trong phần trăm
/// phát hành (kênh `beta` nhận cả bản `stable`).
pub async fn update_for(
    db: &Db,
    app_id: &str,
    platform: &str,
    channel: &str,
    device: &str,
) -> DevResult<Option<Update>> {
    let rows: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT v.version, v.tuf_target, v.rollout_percent FROM catalog_versions v
         JOIN catalog_apps a ON a.id = v.app_id
         WHERE v.app_id = $1 AND v.platform = $2 AND v.status = 'released' AND a.status = 'listed'
           AND (v.channel = 'stable' OR v.channel = $3)",
    )
    .bind(app_id)
    .bind(platform)
    .bind(channel)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter(|(version, _, percent)| {
            *percent >= 100 || (rollout_bucket(app_id, version, device) as i32) < *percent
        })
        .filter_map(|(version, target, _)| {
            Some((semver::Version::parse(&version).ok()?, version, target))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, version, tuf_target)| Update {
            version,
            tuf_target,
        }))
}

/// Ghi nhận một người dùng đã mở app hôm nay (để đếm người dùng hoạt động).
pub async fn record_activity(db: &Db, client_id: &str, subject: Uuid) {
    let res = sqlx::query(
        "INSERT INTO app_activity (app_id, subject, day)
         SELECT id, $2, current_date FROM catalog_apps WHERE client_id = $1
         ON CONFLICT DO NOTHING",
    )
    .bind(client_id)
    .bind(subject)
    .execute(db)
    .await;
    if let Err(e) = res {
        tracing::warn!(error = %e, "không ghi được hoạt động của app");
    }
}

/// App báo một lần lỗi khi chạy (qua F-SDK).
pub async fn record_crash(db: &Db, client_id: &str, version: &str) -> DevResult<bool> {
    let n = sqlx::query(
        "INSERT INTO app_crashes (app_id, version, day, count)
         SELECT id, $2, current_date, 1 FROM catalog_apps WHERE client_id = $1
         ON CONFLICT (app_id, version, day) DO UPDATE SET count = app_crashes.count + 1",
    )
    .bind(client_id)
    .bind(version)
    .execute(db)
    .await?
    .rows_affected();
    Ok(n > 0)
}

/// Số liệu tổng hợp của một app; không có danh tính người dùng.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Metrics {
    pub installs: i64,
    pub active_7d: i64,
    pub active_30d: i64,
    /// (phiên bản, số lượt cài).
    pub versions: Vec<(String, i64)>,
    pub crashes_30d: i64,
    pub rating_avg: Option<f64>,
    pub rating_count: i64,
}

pub async fn metrics(db: &Db, app_id: &str) -> DevResult<Metrics> {
    let installs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM catalog_installs WHERE app_id = $1")
            .bind(app_id)
            .fetch_one(db)
            .await?;
    let active = |days: i32| {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(DISTINCT subject) FROM app_activity
             WHERE app_id = $1 AND day > current_date - $2::int",
        )
        .bind(app_id)
        .bind(days)
        .fetch_one(db)
    };
    let (active_7d, active_30d) = (active(7).await?, active(30).await?);
    let versions: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(version, '?'), count(*) FROM catalog_installs WHERE app_id = $1
         GROUP BY 1 ORDER BY 2 DESC",
    )
    .bind(app_id)
    .fetch_all(db)
    .await?;
    let crashes_30d: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(count), 0)::bigint FROM app_crashes
         WHERE app_id = $1 AND day > current_date - 30",
    )
    .bind(app_id)
    .fetch_one(db)
    .await?;
    let (rating_avg, rating_count): (Option<f64>, i64) = sqlx::query_as(
        "SELECT avg(rating)::float8, count(*) FROM catalog_reviews WHERE app_id = $1 AND NOT hidden",
    )
    .bind(app_id)
    .fetch_one(db)
    .await?;
    Ok(Metrics {
        installs,
        active_7d,
        active_30d,
        versions,
        crashes_30d,
        rating_avg,
        rating_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn roles() {
        for (role, edit, release, delete) in [
            (Role::Owner, true, true, true),
            (Role::Admin, true, true, false),
            (Role::Developer, true, false, false),
            (Role::Viewer, false, false, false),
        ] {
            assert_eq!(
                (role.can_edit(), role.can_release(), role.can_delete()),
                (edit, release, delete),
                "{role:?}"
            );
            assert_eq!(Role::parse(role.as_str()), Some(role));
        }
        // Scenario "Phát triển viên phát hành".
        let DevError::Problem(p) = need_release(Role::Developer).unwrap_err() else {
            panic!("phải là lỗi quyền");
        };
        assert_eq!(p.vi, "Cần vai trò Quản trị hoặc Chủ sở hữu.");
        assert_eq!(p.status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn redirect_uris() {
        for ok in [
            "https://soundlab.example/cb",
            "http://127.0.0.1:9999/cb",
            "http://localhost/cb",
            "com.example.soundlab://callback",
        ] {
            assert!(valid_redirect(ok), "{ok}");
        }
        for bad in [
            "http://soundlab.example/cb",
            "javascript:alert(1)",
            "data:text/html,x",
            "khong phai url",
            "file:///etc/passwd",
        ] {
            assert!(!valid_redirect(bad), "{bad}");
        }
        assert!(parse_redirects("").is_err());
        assert_eq!(
            parse_redirects("https://a.example/cb\n http://127.0.0.1:1/cb")
                .unwrap()
                .len(),
            2
        );
    }

    /// Scenario "Phát hành 10%": khoảng 10% thiết bị nhận bản mới; một thiết bị luôn cùng nhóm.
    #[test]
    fn rollout_buckets_are_stable_and_roughly_uniform() {
        let devices: Vec<String> = (0..5000).map(|i| format!("device-{i}")).collect();
        let in_ten = devices
            .iter()
            .filter(|d| rollout_bucket("soundlab", "1.3.0", d) < 10)
            .count();
        assert!(
            (400..=600).contains(&in_ten),
            "{in_ten}/5000 thiết bị nằm trong 10%"
        );
        let in_fifty = devices
            .iter()
            .filter(|d| rollout_bucket("soundlab", "1.3.0", d) < 50)
            .count();
        assert!((2300..=2700).contains(&in_fifty), "{in_fifty}");
        assert_eq!(
            rollout_bucket("soundlab", "1.3.0", "device-7"),
            rollout_bucket("soundlab", "1.3.0", "device-7")
        );
        // Mở rộng từ 10% lên 50% giữ nguyên những thiết bị đã nhận.
        assert!(devices
            .iter()
            .filter(|d| rollout_bucket("soundlab", "1.3.0", d) < 10)
            .all(|d| rollout_bucket("soundlab", "1.3.0", d) < 50));
    }

    fn zip_with(files: &[(&str, Vec<u8>)]) -> tempfile::NamedTempFile {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut zip = zip::ZipWriter::new(tmp.reopen().unwrap());
        for (name, data) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        tmp
    }

    fn message(r: DevResult<()>) -> String {
        match r {
            Err(DevError::Problem(p)) => p.vi,
            other => panic!("phải bị từ chối: {other:?}"),
        }
    }

    /// Scenario "Gói Windows chưa ký" và các trường hợp macOS.
    #[test]
    fn packages_must_be_code_signed() {
        use crate::codesign::fixtures;
        let signed = zip_with(&[("SoundLab.exe", fixtures::pe(true))]);
        inspect_package(signed.path(), "windows", "SoundLab.exe").expect("gói đã ký");
        let unsigned = zip_with(&[("SoundLab.exe", fixtures::pe(false))]);
        assert_eq!(
            message(inspect_package(unsigned.path(), "windows", "SoundLab.exe")),
            "Gói Windows phải được ký Authenticode"
        );
        assert!(
            message(inspect_package(signed.path(), "windows", "Other.exe"))
                .contains("không có tệp chạy Other.exe")
        );

        let entry = "SoundLab.app/Contents/MacOS/SoundLab";
        let mac = zip_with(&[
            (entry, fixtures::macho(Some(4096))),
            ("SoundLab.app/Contents/CodeResources", b"ticket".to_vec()),
        ]);
        inspect_package(mac.path(), "macos", entry).expect("gói macOS đã ký và notarize");
        let adhoc = zip_with(&[(entry, fixtures::macho(Some(8)))]);
        assert_eq!(
            message(inspect_package(adhoc.path(), "macos", entry)),
            "Gói macOS phải được ký Developer ID"
        );
        let not_notarized = zip_with(&[(entry, fixtures::macho(Some(4096)))]);
        assert!(
            message(inspect_package(not_notarized.path(), "macos", entry)).contains("notarize")
        );
    }
}
