//! Vai trò nền tảng của tài khoản (thay đổi OpenSpec platform-roles; fcoin-wallet-and-marketplace
//! design R1): Người dùng phổ thông (mặc định), Khách hàng cá nhân, Khách hàng doanh nghiệp,
//! Lập trình viên cá nhân, Công ty phát triển phần mềm, Quản trị viên.
//!
//! - Quản trị viên: có dòng trong `admin_members` (kèm vai trò con của bảng quản trị).
//! - Bốn vai trò giữa: một dòng `role_grants` với `role` là mã vai trò.
//! - Còn lại: Người dùng phổ thông.
//!
//! Đổi vai trò chỉ qua [`set_role`]: bắt buộc lý do, ghi nhật ký kiểm toán, không tự đổi vai trò
//! của mình, không bỏ Quản trị hệ thống cuối cùng.

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    admin,
    audit::{self, Actor},
    i18n::Lang,
    notifications::{self, Category, Note},
    store::Db,
    AppState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformRole {
    User,
    Customer,
    BusinessCustomer,
    Developer,
    DevCompany,
    Admin,
}

impl PlatformRole {
    pub const ALL: [PlatformRole; 6] = [
        PlatformRole::User,
        PlatformRole::Customer,
        PlatformRole::BusinessCustomer,
        PlatformRole::Developer,
        PlatformRole::DevCompany,
        PlatformRole::Admin,
    ];

    /// Vai trò người dùng tự xin được (trang Vai trò).
    pub const REQUESTABLE: [PlatformRole; 4] = [
        PlatformRole::Customer,
        PlatformRole::BusinessCustomer,
        PlatformRole::Developer,
        PlatformRole::DevCompany,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PlatformRole::User => "user",
            PlatformRole::Customer => "customer",
            PlatformRole::BusinessCustomer => "business_customer",
            PlatformRole::Developer => "developer",
            PlatformRole::DevCompany => "dev_company",
            PlatformRole::Admin => "admin",
        }
    }

    /// Mã lưu trong `role_grants` (None: mặc định hoặc Quản trị viên).
    pub fn grant_code(self) -> Option<&'static str> {
        match self {
            PlatformRole::User | PlatformRole::Admin => None,
            r => Some(r.as_str()),
        }
    }

    /// Vai trò tổ chức: phải khai tên pháp lý và mã số thuế khi xin.
    pub fn is_organization(self) -> bool {
        matches!(
            self,
            PlatformRole::BusinessCustomer | PlatformRole::DevCompany
        )
    }

    pub fn parse(s: &str) -> Option<PlatformRole> {
        PlatformRole::ALL.into_iter().find(|r| r.as_str() == s)
    }

    pub fn label(self, lang: Lang) -> &'static str {
        lang.t(match self {
            PlatformRole::User => "role.user",
            PlatformRole::Customer => "role.customer",
            PlatformRole::BusinessCustomer => "role.business_customer",
            PlatformRole::Developer => "role.developer",
            PlatformRole::DevCompany => "role.dev_company",
            PlatformRole::Admin => "role.admin",
        })
    }

    /// Được vào Cổng nhà phát triển, gửi app mới lên kho và dùng `f-dev`.
    pub fn can_develop(self) -> bool {
        matches!(
            self,
            PlatformRole::Developer | PlatformRole::DevCompany | PlatformRole::Admin
        )
    }

    /// Nhận yêu cầu làm phần mềm (hồ sơ nhận việc).
    pub fn can_take_projects(self) -> bool {
        matches!(self, PlatformRole::Developer | PlatformRole::DevCompany)
    }

    /// Gửi yêu cầu làm phần mềm cho người nhận việc.
    pub fn can_commission(self) -> bool {
        matches!(
            self,
            PlatformRole::Customer | PlatformRole::BusinessCustomer
        )
    }

    /// Lập hợp đồng cho dự án.
    pub fn can_sign_contracts(self) -> bool {
        matches!(
            self,
            PlatformRole::BusinessCustomer | PlatformRole::DevCompany
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RoleError {
    #[error("cần nhập lý do")]
    ReasonRequired,
    #[error("không được tự đổi vai trò của chính mình")]
    SelfChange,
    #[error("quản trị viên cần ít nhất một vai trò con")]
    AdminNeedsSubrole,
    #[error("phải còn ít nhất một Quản trị hệ thống")]
    LastSysadmin,
    #[error("đã có yêu cầu đang chờ duyệt")]
    AlreadyPending,
    #[error("đã có quyền Lập trình viên")]
    AlreadyDeveloper,
    #[error("không tìm thấy yêu cầu đang chờ")]
    RequestNotFound,
    #[error("vai trò này không xin được")]
    NotRequestable,
    #[error("cần tên pháp lý và mã số thuế hợp lệ")]
    LegalInfoRequired,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl RoleError {
    /// Khóa thông báo (i18n) cho giao diện.
    pub fn key(&self) -> &'static str {
        match self {
            RoleError::ReasonRequired => "role.err.reason",
            RoleError::SelfChange => "role.err.self",
            RoleError::AdminNeedsSubrole => "role.err.subrole",
            RoleError::LastSysadmin => "role.err.last_sysadmin",
            RoleError::AlreadyPending => "role.err.pending",
            RoleError::AlreadyDeveloper => "role.err.already",
            RoleError::RequestNotFound => "role.err.request",
            RoleError::NotRequestable => "role.err.not_requestable",
            RoleError::LegalInfoRequired => "role.err.legal",
            RoleError::Db(_) => "error.generic",
        }
    }
}

/// Vai trò nền tảng của một tài khoản.
pub async fn role_of(db: &Db, subject: Uuid) -> sqlx::Result<PlatformRole> {
    let admin: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM admin_members WHERE subject = $1)")
            .bind(subject)
            .fetch_one(db)
            .await?;
    if admin {
        return Ok(PlatformRole::Admin);
    }
    let granted: Option<String> =
        sqlx::query_scalar("SELECT role FROM role_grants WHERE subject = $1")
            .bind(subject)
            .fetch_optional(db)
            .await?;
    Ok(granted
        .as_deref()
        .and_then(PlatformRole::parse)
        .unwrap_or(PlatformRole::User))
}

/// Vai trò con của quản trị viên (rỗng nếu không phải quản trị viên).
pub async fn admin_roles(db: &Db, subject: Uuid) -> sqlx::Result<Vec<String>> {
    Ok(
        sqlx::query_scalar::<_, Vec<String>>("SELECT roles FROM admin_members WHERE subject = $1")
            .bind(subject)
            .fetch_optional(db)
            .await?
            .unwrap_or_default(),
    )
}

/// Người thực hiện việc đổi vai trò.
#[derive(Debug, Clone)]
pub struct ChangedBy {
    /// Tài khoản quản trị viên (None: công cụ dòng lệnh của người vận hành).
    pub subject: Option<Uuid>,
    /// Tên hoặc email ghi vào nhật ký.
    pub name: String,
}

/// Đổi vai trò nền tảng (design R2). `subroles` chỉ dùng khi đặt Quản trị viên.
pub async fn set_role(
    db: &Db,
    target: Uuid,
    email: &str,
    role: PlatformRole,
    subroles: &[admin::Role],
    by: &ChangedBy,
    reason: &str,
) -> Result<PlatformRole, RoleError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(RoleError::ReasonRequired);
    }
    if by.subject == Some(target) {
        return Err(RoleError::SelfChange);
    }
    if role == PlatformRole::Admin && subroles.is_empty() {
        return Err(RoleError::AdminNeedsSubrole);
    }
    let mut tx = db.begin().await?;
    // Khóa bảng để kiểm "Quản trị hệ thống cuối cùng" không bị hai thao tác song song vượt qua.
    sqlx::query("LOCK TABLE admin_members IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let before = role_of_tx(&mut tx, target).await?;
    let loses_sysadmin = {
        let current: Vec<String> = sqlx::query_scalar::<_, Vec<String>>(
            "SELECT roles FROM admin_members WHERE subject = $1",
        )
        .bind(target)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or_default();
        current.iter().any(|r| r == admin::Role::Sysadmin.as_str())
            && !(role == PlatformRole::Admin && subroles.contains(&admin::Role::Sysadmin))
    };
    if loses_sysadmin {
        let others: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM admin_members WHERE subject <> $1 AND $2 = ANY(roles)",
        )
        .bind(target)
        .bind(admin::Role::Sysadmin.as_str())
        .fetch_one(&mut *tx)
        .await?;
        if others == 0 {
            return Err(RoleError::LastSysadmin);
        }
    }
    let email = email.trim().to_ascii_lowercase();
    match role {
        PlatformRole::User => {
            sqlx::query("DELETE FROM admin_members WHERE subject = $1")
                .bind(target)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM role_grants WHERE subject = $1")
                .bind(target)
                .execute(&mut *tx)
                .await?;
        }
        PlatformRole::Customer
        | PlatformRole::BusinessCustomer
        | PlatformRole::Developer
        | PlatformRole::DevCompany => {
            sqlx::query("DELETE FROM admin_members WHERE subject = $1")
                .bind(target)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO role_grants (subject, role, email, granted_by, reason) VALUES ($1, $5, $2, $3, $4)
                 ON CONFLICT (subject) DO UPDATE SET role = $5, email = $2, granted_by = $3, reason = $4, granted_at = now()",
            )
            .bind(target)
            .bind(&email)
            .bind(&by.name)
            .bind(reason)
            .bind(role.as_str())
            .execute(&mut *tx)
            .await?;
        }
        PlatformRole::Admin => {
            let roles: Vec<String> = subroles.iter().map(|r| r.as_str().to_owned()).collect();
            sqlx::query(
                "INSERT INTO admin_members (subject, email, roles, added_by) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (subject) DO UPDATE SET email = $2, roles = $3",
            )
            .bind(target)
            .bind(&email)
            .bind(&roles)
            .bind(&by.name)
            .execute(&mut *tx)
            .await?;
        }
    }
    // Yêu cầu đang chờ đúng vai trò vừa cấp không còn ý nghĩa (Quản trị viên có mọi quyền
    // Lập trình viên nên cũng đóng yêu cầu Lập trình viên).
    sqlx::query(
        "UPDATE role_requests SET status = 'approved', decided_by = $2, decided_at = now(), decision_note = $3
         WHERE subject = $1 AND status = 'pending' AND (role = $4 OR ($4 = 'admin' AND role = 'developer'))",
    )
    .bind(target)
    .bind(&by.name)
    .bind(reason)
    .bind(role.as_str())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::record(
        db,
        Actor::Admin(by.name.clone()),
        "role.set",
        Some(&target.to_string()),
        None,
        json!({
            "email": email,
            "from": before.as_str(),
            "to": role.as_str(),
            "admin_roles": subroles.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
            "reason": reason,
        }),
    )
    .await;
    Ok(before)
}

async fn role_of_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subject: Uuid,
) -> sqlx::Result<PlatformRole> {
    let (admin, granted): (bool, Option<String>) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM admin_members WHERE subject = $1),
                (SELECT role FROM role_grants WHERE subject = $1)",
    )
    .bind(subject)
    .fetch_one(&mut **tx)
    .await?;
    Ok(if admin {
        PlatformRole::Admin
    } else {
        granted
            .as_deref()
            .and_then(PlatformRole::parse)
            .unwrap_or(PlatformRole::User)
    })
}

/// Ghi chú của yêu cầu vai trò tạo lúc đăng ký (trang Phân quyền hiện nguồn "Chọn khi đăng ký").
pub const REGISTRATION_NOTE: &str = "Chọn khi đăng ký";

/// Vai trò chọn được trên trang đăng ký (thay đổi role-at-registration).
pub const REGISTRATION_ROLES: [PlatformRole; 5] = [
    PlatformRole::User,
    PlatformRole::Customer,
    PlatformRole::BusinessCustomer,
    PlatformRole::Developer,
    PlatformRole::DevCompany,
];

/// Lỗi của lựa chọn vai trò trên trang đăng ký.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleChoiceError {
    UnknownRole,
    LegalNameRequired,
    TaxCodeInvalid,
}

impl RoleChoiceError {
    /// Khóa chuỗi giao diện của lỗi.
    pub fn key(self) -> &'static str {
        match self {
            RoleChoiceError::UnknownRole => "reg.role_unknown",
            RoleChoiceError::LegalNameRequired => "reg.legal_name_required",
            RoleChoiceError::TaxCodeInvalid => "reg.tax_code_invalid",
        }
    }

    /// Trường của biểu mẫu đăng ký bị sai.
    pub fn field(self) -> &'static str {
        match self {
            RoleChoiceError::UnknownRole => "feco_role",
            RoleChoiceError::LegalNameRequired => "feco_legal_name",
            RoleChoiceError::TaxCodeInvalid => "feco_tax_code",
        }
    }
}

/// Vai trò, tên pháp lý, mã số thuế người dùng nhập trên trang đăng ký. Các trường `feco_*` không
/// được chuyển tới Kratos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoleChoice {
    pub role: String,
    pub legal_name: String,
    pub tax_code: String,
}

impl RoleChoice {
    pub fn from_form(form: &[(String, String)]) -> Self {
        let get = |name: &str| {
            form.iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.trim().to_owned())
                .unwrap_or_default()
        };
        RoleChoice {
            role: get("feco_role"),
            legal_name: get("feco_legal_name"),
            tax_code: get("feco_tax_code"),
        }
    }

    /// Vai trò đã chọn và kiểm: trống là Người dùng phổ thông; vai trò tổ chức cần tên pháp lý
    /// và mã số thuế hợp lệ.
    pub fn validate(&self) -> Result<PlatformRole, RoleChoiceError> {
        let role = if self.role.is_empty() {
            PlatformRole::User
        } else {
            PlatformRole::parse(&self.role)
                .filter(|r| REGISTRATION_ROLES.contains(r))
                .ok_or(RoleChoiceError::UnknownRole)?
        };
        if role.is_organization() {
            if self.legal_name.is_empty() {
                return Err(RoleChoiceError::LegalNameRequired);
            }
            if !crate::payments::valid_tax_code(&self.tax_code) {
                return Err(RoleChoiceError::TaxCodeInvalid);
            }
        }
        Ok(role)
    }

    /// Vai trò cần quản trị viên duyệt (khác Người dùng phổ thông), nếu lựa chọn hợp lệ.
    pub fn needs_approval(&self) -> Option<PlatformRole> {
        self.validate().ok().filter(|r| *r != PlatformRole::User)
    }

    pub fn organization(&self) -> Option<Organization> {
        self.validate()
            .ok()
            .filter(|r| r.is_organization())
            .map(|_| Organization {
                legal_name: self.legal_name.clone(),
                tax_code: self.tax_code.clone(),
            })
    }
}

/// Một yêu cầu vai trò.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Request {
    pub id: Uuid,
    pub subject: Uuid,
    pub email: String,
    pub note: String,
    pub status: String,
    pub created_at: time::OffsetDateTime,
    pub decision_note: Option<String>,
    pub role: String,
    pub legal_name: Option<String>,
    pub tax_code: Option<String>,
}

impl Request {
    pub fn platform_role(&self) -> PlatformRole {
        PlatformRole::parse(&self.role).unwrap_or(PlatformRole::Developer)
    }
}

const REQUEST_COLUMNS: &str =
    "id, subject, email, note, status, created_at, decision_note, role, legal_name, tax_code";

/// Yêu cầu gần nhất của một người (đang chờ hoặc đã xử lý).
pub async fn latest_request(db: &Db, subject: Uuid) -> sqlx::Result<Option<Request>> {
    sqlx::query_as(&format!(
        "SELECT {REQUEST_COLUMNS} FROM role_requests
         WHERE subject = $1 ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(subject)
    .fetch_optional(db)
    .await
}

/// Gửi yêu cầu quyền Lập trình viên (scenario "Gửi trùng": chỉ một yêu cầu đang chờ).
pub async fn request_developer(
    state: &AppState,
    subject: Uuid,
    email: &str,
    note: &str,
) -> Result<Uuid, RoleError> {
    request_role(state, subject, email, PlatformRole::Developer, note, None).await
}

/// Thông tin tổ chức khi xin vai trò doanh nghiệp hoặc công ty.
#[derive(Debug, Clone, Default)]
pub struct Organization {
    pub legal_name: String,
    pub tax_code: String,
}

/// Gửi yêu cầu một vai trò (trang Vai trò). Vai trò tổ chức bắt buộc tên pháp lý và mã số thuế.
pub async fn request_role(
    state: &AppState,
    subject: Uuid,
    email: &str,
    role: PlatformRole,
    note: &str,
    org: Option<&Organization>,
) -> Result<Uuid, RoleError> {
    if !PlatformRole::REQUESTABLE.contains(&role) {
        return Err(RoleError::NotRequestable);
    }
    let current = role_of(&state.db, subject).await?;
    if current == role || (role == PlatformRole::Developer && current.can_develop()) {
        return Err(RoleError::AlreadyDeveloper);
    }
    let (legal_name, tax_code) = if role.is_organization() {
        let org = org.ok_or(RoleError::LegalInfoRequired)?;
        let name = org.legal_name.trim();
        let tax = org.tax_code.trim();
        if name.is_empty() || !crate::payments::valid_tax_code(tax) {
            return Err(RoleError::LegalInfoRequired);
        }
        (
            Some(name.chars().take(200).collect::<String>()),
            Some(tax.to_owned()),
        )
    } else {
        (None, None)
    };
    let id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO role_requests (id, subject, email, note, role, legal_name, tax_code) VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (subject) WHERE status = 'pending' DO NOTHING",
    )
    .bind(id)
    .bind(subject)
    .bind(email.trim().to_ascii_lowercase())
    .bind(note.trim().chars().take(1000).collect::<String>())
    .bind(role.as_str())
    .bind(&legal_name)
    .bind(&tax_code)
    .execute(&state.db)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Err(RoleError::AlreadyPending);
    }
    audit::record(
        &state.db,
        Actor::User(subject),
        "role.request",
        Some(&subject.to_string()),
        None,
        json!({ "role": role.as_str(), "legal_name": legal_name, "tax_code": tax_code }),
    )
    .await;
    crate::review::admin_alert(
        state,
        "role_request",
        &format!("{email} yêu cầu vai trò {}", role.label(Lang::Vi)),
        json!({ "email": email, "role": role.as_str(), "note": note.trim(), "request": id }),
    )
    .await;
    Ok(id)
}

/// Yêu cầu đang chờ, cũ nhất trước.
pub async fn pending_requests(db: &Db) -> sqlx::Result<Vec<Request>> {
    sqlx::query_as(&format!(
        "SELECT {REQUEST_COLUMNS} FROM role_requests
         WHERE status = 'pending' ORDER BY created_at"
    ))
    .fetch_all(db)
    .await
}

/// Duyệt hoặc từ chối một yêu cầu; người gửi nhận thông báo (loại bắt buộc).
pub async fn decide(
    state: &AppState,
    request: Uuid,
    approve: bool,
    by: &ChangedBy,
    note: &str,
) -> Result<(), RoleError> {
    let r: Request = sqlx::query_as(&format!(
        "SELECT {REQUEST_COLUMNS} FROM role_requests
         WHERE id = $1 AND status = 'pending'"
    ))
    .bind(request)
    .fetch_optional(&state.db)
    .await?
    .ok_or(RoleError::RequestNotFound)?;
    let note = note.trim();
    let role = r.platform_role();
    if approve {
        let default_reason = format!("Duyệt yêu cầu vai trò {}", role.label(Lang::Vi));
        let reason = if note.is_empty() {
            default_reason.as_str()
        } else {
            note
        };
        set_role(&state.db, r.subject, &r.email, role, &[], by, reason).await?;
    } else {
        if note.is_empty() {
            return Err(RoleError::ReasonRequired);
        }
        sqlx::query(
            "UPDATE role_requests SET status = 'rejected', decided_by = $2, decided_at = now(), decision_note = $3
             WHERE id = $1 AND status = 'pending'",
        )
        .bind(r.id)
        .bind(&by.name)
        .bind(note)
        .execute(&state.db)
        .await?;
        audit::record(
            &state.db,
            Actor::Admin(by.name.clone()),
            "role.request_rejected",
            Some(&r.subject.to_string()),
            None,
            json!({ "request": r.id, "note": note }),
        )
        .await;
    }
    let (vi_name, en_name) = (role.label(Lang::Vi), role.label(Lang::En));
    let link = if role.can_develop() {
        "/developer"
    } else {
        "/account/role"
    };
    let message = match (approve, role) {
        (true, PlatformRole::Developer) => Note::vi(
            "Đã được cấp quyền Lập trình viên",
            "Bạn đã được cấp quyền Lập trình viên trên F-Eco. Mở Cổng nhà phát triển để tạo tài khoản nhà phát triển và app đầu tiên.",
            link,
        )
        .en(
            "Developer access granted",
            "You now have Developer access on F-Eco. Open the developer portal to create your developer account and first app.",
        ),
        (true, _) => Note::vi(
            format!("Đã được cấp vai trò {vi_name}"),
            format!("Bạn đã được cấp vai trò {vi_name} trên F-Eco."),
            link,
        )
        .en(
            format!("{en_name} role granted"),
            format!("You now have the {en_name} role on F-Eco."),
        ),
        (false, _) => Note::vi(
            format!("Yêu cầu vai trò {vi_name} chưa được duyệt"),
            format!("Yêu cầu vai trò {vi_name} của bạn chưa được duyệt. Lý do: {note}"),
            link,
        )
        .en(
            format!("{en_name} role request declined"),
            format!("Your request for the {en_name} role was declined. Reason: {note}"),
        ),
    };
    notifications::notify(
        state,
        r.subject,
        Some(&r.email),
        Category::Security,
        &message,
    )
    .await;
    Ok(())
}

/// Người có vai trò khác Người sử dụng (để trang Phân quyền liệt kê).
#[derive(Debug, Clone)]
pub struct Holder {
    pub subject: Uuid,
    pub email: String,
    pub role: PlatformRole,
    pub admin_roles: Vec<String>,
}

pub async fn holders(db: &Db) -> sqlx::Result<Vec<Holder>> {
    let admins: Vec<(Uuid, String, Vec<String>)> =
        sqlx::query_as("SELECT subject, email, roles FROM admin_members ORDER BY email")
            .fetch_all(db)
            .await?;
    let developers: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT subject, email, role FROM role_grants
         WHERE subject NOT IN (SELECT subject FROM admin_members) ORDER BY role, email",
    )
    .fetch_all(db)
    .await?;
    let mut out: Vec<Holder> = admins
        .into_iter()
        .map(|(subject, email, roles)| Holder {
            subject,
            email,
            role: PlatformRole::Admin,
            admin_roles: roles,
        })
        .collect();
    out.extend(developers.into_iter().map(|(subject, email, role)| Holder {
        subject,
        email,
        role: PlatformRole::parse(&role).unwrap_or(PlatformRole::Developer),
        admin_roles: Vec::new(),
    }));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// role-at-registration, scenario "Công ty thiếu mã số thuế", "Vai trò lạ".
    #[test]
    fn role_choice_at_registration() {
        let form = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        // Mặc định: Người dùng phổ thông, không cần duyệt.
        let c = RoleChoice::from_form(&form(&[("traits.email", "a@b.vn")]));
        assert_eq!(c.validate(), Ok(PlatformRole::User));
        assert_eq!(c.needs_approval(), None);
        let c = RoleChoice::from_form(&form(&[("feco_role", "user")]));
        assert_eq!(c.validate(), Ok(PlatformRole::User));
        // Khách hàng cá nhân, Lập trình viên cá nhân: không cần thông tin tổ chức.
        for code in ["customer", "developer"] {
            let c = RoleChoice::from_form(&form(&[("feco_role", code)]));
            assert_eq!(c.needs_approval().map(PlatformRole::as_str), Some(code));
            assert!(c.organization().is_none());
        }
        // Vai trò lạ, kể cả Quản trị viên.
        for code in ["admin", "root", "Developer"] {
            let c = RoleChoice::from_form(&form(&[("feco_role", code)]));
            assert_eq!(c.validate(), Err(RoleChoiceError::UnknownRole), "{code}");
        }
        // Vai trò tổ chức.
        let c = RoleChoice::from_form(&form(&[
            ("feco_role", "dev_company"),
            ("feco_legal_name", "Công ty CP Phần mềm Sao Mai"),
        ]));
        assert_eq!(c.validate(), Err(RoleChoiceError::TaxCodeInvalid));
        assert_eq!(c.validate().unwrap_err().field(), "feco_tax_code");
        let c = RoleChoice::from_form(&form(&[
            ("feco_role", "business_customer"),
            ("feco_tax_code", "0312345678"),
        ]));
        assert_eq!(c.validate(), Err(RoleChoiceError::LegalNameRequired));
        let c = RoleChoice::from_form(&form(&[
            ("feco_role", "business_customer"),
            ("feco_legal_name", " Công ty TNHH Hoa Mai "),
            ("feco_tax_code", "0312345678-001"),
        ]));
        assert_eq!(c.validate(), Ok(PlatformRole::BusinessCustomer));
        let org = c.organization().unwrap();
        assert_eq!(org.legal_name, "Công ty TNHH Hoa Mai");
        assert_eq!(org.tax_code, "0312345678-001");
    }

    #[test]
    fn roles_parse_and_permissions() {
        for r in PlatformRole::ALL {
            assert_eq!(PlatformRole::parse(r.as_str()), Some(r));
            assert!(!r.label(Lang::Vi).is_empty() && !r.label(Lang::En).is_empty());
        }
        assert!(!PlatformRole::User.can_develop());
        assert!(PlatformRole::Developer.can_develop());
        assert!(PlatformRole::DevCompany.can_develop());
        assert!(!PlatformRole::Customer.can_develop());
        // Ma trận quyền (design R1).
        let pick = |f: fn(PlatformRole) -> bool| -> Vec<PlatformRole> {
            PlatformRole::ALL.into_iter().filter(|r| f(*r)).collect()
        };
        assert_eq!(
            pick(PlatformRole::can_take_projects),
            [PlatformRole::Developer, PlatformRole::DevCompany]
        );
        assert_eq!(
            pick(PlatformRole::can_commission),
            [PlatformRole::Customer, PlatformRole::BusinessCustomer]
        );
        assert_eq!(
            pick(PlatformRole::can_sign_contracts),
            [PlatformRole::BusinessCustomer, PlatformRole::DevCompany]
        );
        for r in PlatformRole::REQUESTABLE {
            assert!(r.grant_code().is_some());
        }
        assert_eq!(PlatformRole::User.grant_code(), None);
        assert_eq!(PlatformRole::Admin.grant_code(), None);
        assert!(
            PlatformRole::Admin.can_develop(),
            "quản trị viên có quyền Lập trình viên"
        );
        assert_eq!(PlatformRole::parse("root"), None);
        assert_eq!(
            serde_json::to_string(&PlatformRole::Developer).unwrap(),
            "\"developer\""
        );
    }
}
