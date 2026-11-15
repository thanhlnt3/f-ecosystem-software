//! Trang hồ sơ của người dùng đã đăng nhập (`/`; thay đổi launcher-windows-and-web-profile,
//! spec account-identity "Trang hồ sơ").
//!
//! Phần đầu: ảnh đại diện chữ cái, tên, email và trạng thái xác minh, vai trò nền tảng, ngày tham
//! gia. Dưới là thẻ số liệu (ứng dụng đã kết nối, thiết bị, đơn hàng, thông báo chưa đọc) và các
//! nhóm lối tắt: bảo mật, dữ liệu và quyền riêng tư, mua sắm, công cụ theo vai trò. Số liệu nào
//! không lấy được thì hiện "—", trang vẫn mở.

use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{i18n::Lang, roles::PlatformRole, AppState};

pub struct Stat {
    pub id: &'static str,
    pub label: &'static str,
    pub value: String,
    pub href: &'static str,
}

pub struct Profile {
    pub name: String,
    pub email: String,
    pub verified: bool,
    pub initials: String,
    /// Màu nền ảnh đại diện, `#rrggbb` (cố định theo mã người dùng).
    pub avatar: String,
    pub role: &'static str,
    pub role_code: &'static str,
    pub member_since: String,
    pub two_factor: Option<bool>,
    pub stats: Vec<Stat>,
    pub developer: bool,
    pub admin: bool,
}

/// Hai chữ cái đầu của tên (hoặc email) viết hoa, giữ dấu tiếng Việt.
pub fn initials(name: &str, email: &str) -> String {
    let source = if name.trim().is_empty() {
        email.split('@').next().unwrap_or_default()
    } else {
        name
    };
    let words: Vec<&str> = source.split_whitespace().collect();
    let pick: String = match words.as_slice() {
        [] => String::new(),
        [one] => one.chars().take(2).collect(),
        [first, .., last] => first.chars().take(1).chain(last.chars().take(1)).collect(),
    };
    let up = pick.to_uppercase();
    if up.is_empty() {
        "?".into()
    } else {
        up
    }
}

/// Màu ảnh đại diện: một trong các màu đọc rõ chữ trắng, chọn theo mã người dùng.
pub fn avatar_color(subject: Uuid) -> String {
    const COLORS: [&str; 8] = [
        "#0f6b5c", "#2563eb", "#7c3aed", "#b45309", "#be185d", "#15803d", "#0e7490", "#9333ea",
    ];
    let sum: u32 = subject.as_bytes().iter().map(|b| u32::from(*b)).sum();
    COLORS[(sum as usize) % COLORS.len()].into()
}

fn date(t: &str) -> String {
    OffsetDateTime::parse(t, &time::format_description::well_known::Rfc3339)
        .ok()
        .and_then(|d| {
            d.format(&time::macros::format_description!("[day]/[month]/[year]"))
                .ok()
        })
        .unwrap_or_default()
}

fn count<E: std::fmt::Display>(r: Result<usize, E>, what: &str) -> String {
    match r {
        Ok(n) => n.to_string(),
        Err(e) => {
            tracing::warn!(error = %e, what, "không lấy được số liệu hồ sơ");
            "—".into()
        }
    }
}

/// Dựng hồ sơ từ phiên Kratos; các số liệu được hỏi song song.
pub async fn load(state: &AppState, lang: Lang, session: &Value) -> Profile {
    let identity = &session["identity"];
    let email = identity["traits"]["email"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let name = identity["traits"]["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let verified = identity["verifiable_addresses"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|a| a["value"].as_str() == Some(email.as_str()) && a["verified"] == true);
    let id = identity["id"].as_str().unwrap_or_default().to_owned();
    let subject: Uuid = id.parse().unwrap_or_default();

    let (role, apps, sessions, orders, unread, full, wallet) = tokio::join!(
        crate::roles::role_of(&state.db, subject),
        state.hydra.consent_sessions(&id),
        state.kratos.identity_sessions(&id),
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM orders WHERE customer = $1")
            .bind(subject)
            .fetch_one(&state.db),
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notifications WHERE subject = $1 AND read_at IS NULL"
        )
        .bind(subject)
        .fetch_one(&state.db),
        state.kratos.has_second_factor(&id),
        crate::wallet::summary(&state.db, subject),
    );
    let role = role.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "không đọc được vai trò");
        PlatformRole::User
    });
    // Xác thực hai lớp: danh tính có TOTP, khóa bảo mật hoặc mã khôi phục.
    let two_factor = full.ok().flatten();
    let stats = vec![
        Stat {
            id: "stat-wallet",
            label: lang.t("profile.stat.wallet"),
            value: match wallet {
                Ok(w) => crate::wallet::coin(w.balance, lang),
                Err(e) => {
                    tracing::warn!(error = %e, "không đọc được ví");
                    "—".into()
                }
            },
            href: "/wallet",
        },
        Stat {
            id: "stat-apps",
            label: lang.t("profile.stat.apps"),
            value: count(apps.map(|v| v.len()), "apps"),
            href: "/apps",
        },
        Stat {
            id: "stat-sessions",
            label: lang.t("profile.stat.sessions"),
            value: count(sessions.map(|v| v.len()), "sessions"),
            href: "/sessions",
        },
        Stat {
            id: "stat-orders",
            label: lang.t("profile.stat.orders"),
            value: count(orders.map(|n| n as usize), "orders"),
            href: "/orders",
        },
        Stat {
            id: "stat-unread",
            label: lang.t("profile.stat.unread"),
            value: count(unread.map(|n| n as usize), "unread"),
            href: "/notifications",
        },
    ];
    Profile {
        initials: initials(&name, &email),
        avatar: avatar_color(subject),
        name,
        email,
        verified,
        role: role.label(lang),
        role_code: role.as_str(),
        member_since: identity["created_at"]
            .as_str()
            .map(date)
            .unwrap_or_default(),
        two_factor,
        stats,
        developer: role.can_develop(),
        admin: role == PlatformRole::Admin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_from_name_or_email() {
        assert_eq!(initials("Lê Ngọc Thanh", "x@y.vn"), "LT");
        assert_eq!(initials("an", "x@y.vn"), "AN");
        assert_eq!(initials("  ", "minh.anh@example.com"), "MI");
        assert_eq!(initials("", "@"), "?");
        assert_eq!(initials("Đặng Ánh", ""), "ĐÁ");
    }

    #[test]
    fn avatar_color_is_stable() {
        let id = Uuid::from_u128(42);
        assert_eq!(avatar_color(id), avatar_color(id));
        assert!(avatar_color(Uuid::from_u128(7)).starts_with('#'));
        assert_eq!(date("2026-11-15T01:02:03Z"), "15/11/2026");
        assert_eq!(date("không phải ngày"), "");
    }
}
