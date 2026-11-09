//! Chống CSRF cho các biểu mẫu do chính feco-server xử lý (đăng xuất thiết bị, xóa
//! tài khoản). Biểu mẫu của luồng Kratos đã có token CSRF riêng của Kratos.
//!
//! Cách làm "double submit": một token ngẫu nhiên nằm trong cookie `feco_csrf`
//! (HttpOnly, SameSite=Strict) và trong trường ẩn của biểu mẫu; hai giá trị phải khớp.

use axum::http::{header, HeaderMap, HeaderValue};

pub const COOKIE: &str = "feco_csrf";
pub const FIELD: &str = "feco_csrf";

fn from_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|c| c.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, v)| *k == COOKIE && v.len() == 64)
        .map(|(_, v)| v.to_owned())
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
    let cookie = format!(
        "{COOKIE}={t}; Path=/; HttpOnly; SameSite=Strict{}",
        if secure { "; Secure" } else { "" }
    );
    (t, HeaderValue::from_str(&cookie).ok())
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
}
