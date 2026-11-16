//! Chống CSRF cho các biểu mẫu do chính feco-server xử lý (đăng xuất thiết bị, xóa
//! tài khoản). Biểu mẫu của luồng Kratos đã có token CSRF riêng của Kratos.
//!
//! Cách làm "double submit": một token ngẫu nhiên nằm trong cookie (HttpOnly,
//! SameSite=Strict) và trong trường ẩn của biểu mẫu; hai giá trị phải khớp.
//!
//! Địa chỉ công khai là https thì cookie tên `__Host-feco_csrf` (Secure, Path=/, không
//! Domain) để tên miền con không ghi đè được; dev chạy http dùng `feco_csrf`.

use std::sync::OnceLock;

use axum::http::{header, HeaderMap, HeaderValue};

pub const COOKIE: &str = "feco_csrf";
/// Tên cookie khi địa chỉ công khai là https.
pub const HOST_COOKIE: &str = "__Host-feco_csrf";
pub const FIELD: &str = "feco_csrf";

static HTTPS: OnceLock<bool> = OnceLock::new();

/// Gọi một lần lúc khởi động: địa chỉ công khai có là https không.
pub fn init(https_public: bool) {
    let _ = HTTPS.set(https_public);
}

fn https() -> bool {
    HTTPS.get().copied().unwrap_or(false)
}

/// Tên cookie theo chế độ: https dùng tiền tố `__Host-`.
pub fn cookie_name(https: bool) -> &'static str {
    if https {
        HOST_COOKIE
    } else {
        COOKIE
    }
}

/// Token trong cookie đúng tên (cookie tên khác, kể cả `feco_csrf` khi đang https, bị bỏ qua).
fn token_in(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|c| c.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, v)| *k == name && v.len() == 64)
        .map(|(_, v)| v.to_owned())
}

fn from_cookie(headers: &HeaderMap) -> Option<String> {
    token_in(headers, cookie_name(https()))
}

/// Giá trị Set-Cookie cho token.
fn set_cookie(token: &str, https: bool, secure: bool) -> String {
    format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Strict{}",
        cookie_name(https),
        if https || secure { "; Secure" } else { "" }
    )
}

fn random_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên của hệ điều hành");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Token cho biểu mẫu sắp hiển thị, cùng cookie cần đặt nếu trình duyệt chưa có.
pub fn issue(headers: &HeaderMap, secure: bool) -> (String, Option<HeaderValue>) {
    if let Some(t) = from_cookie(headers) {
        return (t, None);
    }
    let t = random_token();
    let cookie = HeaderValue::from_str(&set_cookie(&t, https(), secure)).ok();
    (t, cookie)
}

/// So khớp token trong biểu mẫu với cookie, thời gian so sánh không phụ thuộc nội dung.
pub fn verify(headers: &HeaderMap, form_token: Option<&str>) -> bool {
    let (Some(a), Some(b)) = (from_cookie(headers), form_token) else {
        return false;
    };
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_cookie(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_str(v).unwrap());
        h
    }

    #[test]
    fn issues_new_token_and_cookie_when_missing() {
        let (t, cookie) = issue(&HeaderMap::new(), true);
        assert_eq!(t.len(), 64);
        let c = cookie.unwrap();
        let c = c.to_str().unwrap();
        assert!(c.contains("HttpOnly") && c.contains("SameSite=Strict") && c.contains("Secure"));
    }

    #[test]
    fn reuses_existing_cookie() {
        let tok = "a".repeat(64);
        let (t, cookie) = issue(&with_cookie(&format!("x=1; feco_csrf={tok}")), false);
        assert_eq!(t, tok);
        assert!(cookie.is_none());
    }

    #[test]
    fn verify_requires_matching_values() {
        let tok = "b".repeat(64);
        let h = with_cookie(&format!("feco_csrf={tok}"));
        assert!(verify(&h, Some(&tok)));
        assert!(!verify(&h, Some(&"c".repeat(64))));
        assert!(!verify(&h, None));
        assert!(!verify(&HeaderMap::new(), Some(&tok)));
    }

    #[test]
    fn csrf_cookie_uses_host_prefix_on_https() {
        let tok = "d".repeat(64);
        let c = set_cookie(&tok, true, false);
        assert!(c.starts_with("__Host-feco_csrf="), "{c}");
        assert!(c.contains("; Secure") && c.contains("Path=/") && !c.contains("Domain"));
        assert!(set_cookie(&tok, false, false).starts_with("feco_csrf="));
        // Ở https chỉ đọc cookie __Host-; cookie feco_csrf (tên miền con ghi được) bị bỏ qua.
        let h = with_cookie(&format!(
            "feco_csrf={}; __Host-feco_csrf={tok}",
            "e".repeat(64)
        ));
        assert_eq!(token_in(&h, cookie_name(true)), Some(tok.clone()));
        let h = with_cookie(&format!("feco_csrf={tok}"));
        assert_eq!(token_in(&h, cookie_name(true)), None);
        assert_eq!(token_in(&h, cookie_name(false)), Some(tok));
    }
}
