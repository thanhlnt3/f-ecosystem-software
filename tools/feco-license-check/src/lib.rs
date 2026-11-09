//! Kiểm tra license cho phần không phải crate Rust (spec `dependency-licensing`):
//! dịch vụ, ảnh container và tài nguyên khai báo trong `third_party/native.toml`.
//!
//! Quy tắc:
//! - License của mỗi thành phần phải nằm trong [`ALLOWED`], hoặc có ngoại lệ trong
//!   `third_party/license-exceptions.toml` ghi đủ người duyệt, ngày duyệt (YYYY-MM-DD)
//!   và lý do.
//! - Biểu thức `A OR B` đạt nếu một vế đạt; `A AND B` đạt nếu mọi vế đạt.
//! - Mọi ảnh container trong `infra/docker-compose.yml` phải được khai báo trong
//!   `native.toml`, để không ảnh nào lọt vào mà chưa kiểm tra license.

use std::collections::BTreeSet;

use serde::Deserialize;

/// Danh sách cho phép, khớp spec `dependency-licensing` và `deny.toml`.
pub const ALLOWED: &[&str] = &[
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "ISC",
    "Zlib",
    "BSL-1.0",
    "Unicode-3.0",
    "OFL-1.1",
    "CC0-1.0",
    "MPL-2.0",
    "PostgreSQL",
    "0BSD",
];

#[derive(Debug, Deserialize)]
pub struct NativeManifest {
    #[serde(default, rename = "component")]
    pub components: Vec<Component>,
}

#[derive(Debug, Deserialize)]
pub struct Component {
    pub name: String,
    pub license: String,
    /// Tên ảnh container đầy đủ kèm tag, nếu thành phần chạy dưới dạng container.
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExceptionFile {
    #[serde(default, rename = "exception")]
    pub exceptions: Vec<Exception>,
}

#[derive(Debug, Deserialize)]
pub struct Exception {
    pub license: String,
    pub component: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub reviewer: String,
    #[serde(default)]
    pub date: String,
}

/// Một vi phạm, dạng đọc được cho người.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation(pub String);

/// Tách biểu thức license thành các vế. Chỉ hỗ trợ `AND` hoặc `OR` ở một cấp,
/// không hỗ trợ ngoặc. Biểu thức phức tạp hơn bị coi là không hợp lệ.
fn terms(expr: &str) -> Result<(Vec<String>, bool), String> {
    if expr.contains('(') || expr.contains(')') {
        return Err(format!("biểu thức có ngoặc chưa được hỗ trợ: {expr:?}"));
    }
    let has_and = expr.contains(" AND ");
    let has_or = expr.contains(" OR ");
    if has_and && has_or {
        return Err(format!("không trộn AND và OR không có ngoặc: {expr:?}"));
    }
    let sep = if has_and { " AND " } else { " OR " };
    let parts: Vec<String> = expr.split(sep).map(|s| s.trim().to_owned()).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("biểu thức license rỗng hoặc sai: {expr:?}"));
    }
    Ok((parts, has_and))
}

fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn exception_problem(e: &Exception) -> Option<String> {
    let mut missing = Vec::new();
    if e.reviewer.trim().is_empty() {
        missing.push("người duyệt (reviewer)");
    }
    if !valid_date(e.date.trim()) {
        missing.push("ngày duyệt (date, YYYY-MM-DD)");
    }
    if e.reason.trim().is_empty() {
        missing.push("lý do (reason)");
    }
    (!missing.is_empty()).then(|| missing.join(", "))
}

/// Kiểm tra toàn bộ. `compose_images` là danh sách ảnh đọc từ docker compose.
pub fn check(
    native: &NativeManifest,
    exceptions: &ExceptionFile,
    compose_images: &[String],
) -> Vec<Violation> {
    let mut out = Vec::new();

    for e in &exceptions.exceptions {
        if let Some(p) = exception_problem(e) {
            out.push(Violation(format!(
                "ngoại lệ cho {} ({}) thiếu {p}",
                e.license, e.component
            )));
        }
    }

    let license_ok = |component: &str, license: &str| -> bool {
        ALLOWED.contains(&license)
            || exceptions.exceptions.iter().any(|e| {
                e.license == license && e.component == component && exception_problem(e).is_none()
            })
    };

    for c in &native.components {
        match terms(&c.license) {
            Err(msg) => out.push(Violation(format!("{}: {msg}", c.name))),
            Ok((parts, is_and)) => {
                let bad: Vec<&String> = parts.iter().filter(|p| !license_ok(&c.name, p)).collect();
                let failed = if is_and {
                    !bad.is_empty()
                } else {
                    bad.len() == parts.len()
                };
                if failed {
                    out.push(Violation(format!(
                        "{} [{}]: license không nằm trong danh sách cho phép và không có ngoại lệ đã duyệt: {}",
                        c.name,
                        c.license,
                        bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                    )));
                }
            }
        }
    }

    let declared: BTreeSet<&str> = native
        .components
        .iter()
        .filter_map(|c| c.image.as_deref())
        .collect();
    for image in compose_images {
        if !declared.contains(image.as_str()) {
            out.push(Violation(format!(
                "ảnh container {image:?} trong docker compose chưa được khai báo trong third_party/native.toml"
            )));
        }
    }
    out
}

/// Lấy các giá trị `image:` từ một file docker compose (không cần thư viện YAML).
pub fn compose_images(compose: &str) -> Vec<String> {
    compose
        .lines()
        .filter_map(|l| l.trim().strip_prefix("image:"))
        .map(|v| v.trim().trim_matches(['"', '\'']).to_owned())
        .filter(|v| !v.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native(src: &str) -> NativeManifest {
        toml::from_str(src).unwrap()
    }
    fn exc(src: &str) -> ExceptionFile {
        toml::from_str(src).unwrap()
    }

    #[test]
    fn allowed_licenses_pass() {
        let n = native(
            r#"
            [[component]]
            name = "Ory Kratos"
            license = "Apache-2.0"
            image = "oryd/kratos:v1.3.1"
            [[component]]
            name = "PostgreSQL"
            license = "PostgreSQL"
            image = "postgres:17-alpine"
            "#,
        );
        let v = check(
            &n,
            &exc(""),
            &["oryd/kratos:v1.3.1".into(), "postgres:17-alpine".into()],
        );
        assert!(v.is_empty(), "{v:?}");
    }

    /// Spec dependency-licensing, scenario "Thêm thư viện AGPL".
    #[test]
    fn agpl_component_fails_and_names_it() {
        let n = native(
            r#"
            [[component]]
            name = "Zitadel"
            license = "AGPL-3.0-only"
            "#,
        );
        let v = check(&n, &exc(""), &[]);
        assert_eq!(v.len(), 1);
        assert!(
            v[0].0.contains("Zitadel") && v[0].0.contains("AGPL-3.0-only"),
            "{v:?}"
        );
    }

    /// Spec dependency-licensing, scenario "Ngoại lệ thiếu người duyệt".
    #[test]
    fn exception_without_reviewer_fails() {
        let n = native(
            r#"
            [[component]]
            name = "ClamAV"
            license = "GPL-2.0-only"
            "#,
        );
        let e = exc(r#"
            [[exception]]
            license = "GPL-2.0-only"
            component = "ClamAV"
            reason = "Chạy thành dịch vụ quét riêng, không phân phối cho người dùng"
            date = "2026-11-09"
            "#);
        let v = check(&n, &e, &[]);
        assert!(v.iter().any(|x| x.0.contains("thiếu người duyệt")), "{v:?}");
        assert!(
            v.iter().any(|x| x.0.contains("ClamAV [GPL-2.0-only]")),
            "{v:?}"
        );
    }

    #[test]
    fn complete_exception_is_accepted() {
        let n = native(
            r#"
            [[component]]
            name = "ClamAV"
            license = "GPL-2.0-only"
            "#,
        );
        let e = exc(r#"
            [[exception]]
            license = "GPL-2.0-only"
            component = "ClamAV"
            reason = "Chạy thành dịch vụ quét riêng, không phân phối cho người dùng"
            reviewer = "Le Thanh"
            date = "2026-11-09"
            "#);
        assert!(check(&n, &e, &[]).is_empty());
    }

    #[test]
    fn exception_only_covers_its_own_component() {
        let n = native(
            r#"
            [[component]]
            name = "OtherTool"
            license = "GPL-2.0-only"
            "#,
        );
        let e = exc(r#"
            [[exception]]
            license = "GPL-2.0-only"
            component = "ClamAV"
            reason = "r"
            reviewer = "Le Thanh"
            date = "2026-11-09"
            "#);
        assert_eq!(check(&n, &e, &[]).len(), 1);
    }

    #[test]
    fn or_needs_one_allowed_and_needs_all() {
        let n = native(
            r#"
            [[component]]
            name = "DualLicensed"
            license = "GPL-3.0-only OR MIT"
            [[component]]
            name = "Mixed"
            license = "MIT AND SSPL-1.0"
            "#,
        );
        let v = check(&n, &exc(""), &[]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].0.contains("Mixed") && v[0].0.contains("SSPL-1.0"));
    }

    #[test]
    fn undeclared_compose_image_fails() {
        let v = check(&native(""), &exc(""), &["redis:7.4".into()]);
        assert_eq!(v.len(), 1);
        assert!(v[0].0.contains("redis:7.4"));
    }

    #[test]
    fn parses_compose_images() {
        let compose = "services:\n  db:\n    image: postgres:17-alpine\n  kv:\n    image: \"valkey/valkey:8.0-alpine\"\n    # image: commented\n";
        assert_eq!(
            compose_images(compose),
            vec![
                "postgres:17-alpine".to_string(),
                "valkey/valkey:8.0-alpine".to_string()
            ]
        );
    }

    #[test]
    fn malformed_expressions_are_violations() {
        let n = native(
            r#"
            [[component]]
            name = "Weird"
            license = "(MIT OR Apache-2.0) AND Zlib"
            "#,
        );
        assert!(check(&n, &exc(""), &[])[0].0.contains("ngoặc"));
    }
}
