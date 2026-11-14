//! Vai trò nền tảng của tài khoản (thay đổi OpenSpec platform-roles; design R1–R3):
//! Người sử dụng ứng dụng (mặc định), Lập trình viên, Quản trị viên.
//!
//! - Quản trị viên: có dòng trong `admin_members` (kèm vai trò con của bảng quản trị).
//! - Lập trình viên: có dòng `role_grants` với `role = 'developer'`.
//! - Còn lại: Người sử dụng ứng dụng.
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
    Developer,
    Admin,
}

impl PlatformRole {
    pub const ALL: [PlatformRole; 3] = [
        PlatformRole::User,
        PlatformRole::Developer,
        PlatformRole::Admin,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PlatformRole::User => "user",
            PlatformRole::Developer => "developer",
            PlatformRole::Admin => "admin",
        }
    }

    pub fn parse(s: &str) -> Option<PlatformRole> {
        PlatformRole::ALL.into_iter().find(|r| r.as_str() == s)
    }

    pub fn label(self, lang: Lang) -> &'static str {
        lang.t(match self {
            PlatformRole::User => "role.user",
            PlatformRole::Developer => "role.developer",
            PlatformRole::Admin => "role.admin",
        })
    }

    /// Được vào Cổng nhà phát triển và dùng `f-dev`.
    pub fn can_develop(self) -> bool {
        matches!(self, PlatformRole::Developer | PlatformRole::Admin)
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
    let developer: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM role_grants WHERE subject = $1 AND role = 'developer')",
    )
    .bind(subject)
    .fetch_one(db)
    .await?;
    Ok(if developer {
        PlatformRole::Developer
    } else {
        PlatformRole::User
    })
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
        PlatformRole::Developer => {
            sqlx::query("DELETE FROM admin_members WHERE subject = $1")
                .bind(target)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO role_grants (subject, role, email, granted_by, reason) VALUES ($1, 'developer', $2, $3, $4)
                 ON CONFLICT (subject) DO UPDATE SET email = $2, granted_by = $3, reason = $4, granted_at = now()",
            )
            .bind(target)
            .bind(&email)
            .bind(&by.name)
            .bind(reason)
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
    // Yêu cầu đang chờ không còn ý nghĩa khi đã có quyền.
    if role.can_develop() {
        sqlx::query(
            "UPDATE role_requests SET status = 'approved', decided_by = $2, decided_at = now(), decision_note = $3
             WHERE subject = $1 AND status = 'pending'",
        )
        .bind(target)
        .bind(&by.name)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
    }
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
    let (admin, developer): (bool, bool) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM admin_members WHERE subject = $1),
                EXISTS (SELECT 1 FROM role_grants WHERE subject = $1 AND role = 'developer')",
    )
    .bind(subject)
    .fetch_one(&mut **tx)
    .await?;
    Ok(if admin {
        PlatformRole::Admin
    } else if developer {
        PlatformRole::Developer
    } else {
        PlatformRole::User
    })
}

/// Một yêu cầu quyền Lập trình viên.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Request {
    pub id: Uuid,
    pub subject: Uuid,
    pub email: String,
    pub note: String,
    pub status: String,
    pub created_at: time::OffsetDateTime,
    pub decision_note: Option<String>,
}

/// Yêu cầu gần nhất của một người (đang chờ hoặc đã xử lý).
pub async fn latest_request(db: &Db, subject: Uuid) -> sqlx::Result<Option<Request>> {
    sqlx::query_as(
        "SELECT id, subject, email, note, status, created_at, decision_note FROM role_requests
         WHERE subject = $1 ORDER BY created_at DESC LIMIT 1",
    )
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
    if role_of(&state.db, subject).await?.can_develop() {
        return Err(RoleError::AlreadyDeveloper);
    }
    let id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO role_requests (id, subject, email, note) VALUES ($1, $2, $3, $4)
         ON CONFLICT (subject) WHERE status = 'pending' DO NOTHING",
    )
    .bind(id)
    .bind(subject)
    .bind(email.trim().to_ascii_lowercase())
    .bind(note.trim().chars().take(1000).collect::<String>())
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
        json!({ "role": "developer" }),
    )
    .await;
    crate::review::admin_alert(
        state,
        "role_request",
        &format!("{email} yêu cầu quyền Lập trình viên"),
        json!({ "email": email, "note": note.trim(), "request": id }),
    )
    .await;
    Ok(id)
}

/// Yêu cầu đang chờ, cũ nhất trước.
pub async fn pending_requests(db: &Db) -> sqlx::Result<Vec<Request>> {
    sqlx::query_as(
        "SELECT id, subject, email, note, status, created_at, decision_note FROM role_requests
         WHERE status = 'pending' ORDER BY created_at",
    )
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
    let r: Request = sqlx::query_as(
        "SELECT id, subject, email, note, status, created_at, decision_note FROM role_requests
         WHERE id = $1 AND status = 'pending'",
    )
    .bind(request)
    .fetch_optional(&state.db)
    .await?
    .ok_or(RoleError::RequestNotFound)?;
    let note = note.trim();
    if approve {
        let reason = if note.is_empty() {
            "Duyệt yêu cầu quyền Lập trình viên"
        } else {
            note
        };
        set_role(
            &state.db,
            r.subject,
            &r.email,
            PlatformRole::Developer,
            &[],
            by,
            reason,
        )
        .await?;
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
    let message = if approve {
        Note::vi(
            "Đã được cấp quyền Lập trình viên",
            "Bạn đã được cấp quyền Lập trình viên trên F-Eco. Mở Cổng nhà phát triển để tạo tài khoản nhà phát triển và app đầu tiên.",
            "/developer",
        )
        .en(
            "Developer access granted",
            "You now have Developer access on F-Eco. Open the developer portal to create your developer account and first app.",
        )
    } else {
        Note::vi(
            "Yêu cầu quyền Lập trình viên chưa được duyệt",
            format!("Yêu cầu quyền Lập trình viên của bạn chưa được duyệt. Lý do: {note}"),
            "/developer",
        )
        .en(
            "Developer access request declined",
            format!("Your request for Developer access was declined. Reason: {note}"),
        )
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
    let developers: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT subject, email FROM role_grants WHERE role = 'developer'
         AND subject NOT IN (SELECT subject FROM admin_members) ORDER BY email",
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
    out.extend(developers.into_iter().map(|(subject, email)| Holder {
        subject,
        email,
        role: PlatformRole::Developer,
        admin_roles: Vec::new(),
    }));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_parse_and_permissions() {
        for r in PlatformRole::ALL {
            assert_eq!(PlatformRole::parse(r.as_str()), Some(r));
            assert!(!r.label(Lang::Vi).is_empty() && !r.label(Lang::En).is_empty());
        }
        assert!(!PlatformRole::User.can_develop());
        assert!(PlatformRole::Developer.can_develop());
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
