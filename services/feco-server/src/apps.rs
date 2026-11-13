//! Thông tin app con và quy tắc cấp scope (spec user-data-consent).
//!
//! Mỗi app là một OAuth client trong Hydra. `metadata` của client (do cổng nhà phát
//! triển ghi, nhóm 8) khai báo các scope dữ liệu app dùng, scope nào bắt buộc và mục
//! đích sử dụng:
//!
//! ```json
//! {"developer": "Acme", "developer_verified": true,
//!  "scopes": {"profile": {"required": true, "purpose_vi": "...", "purpose_en": "..."},
//!             "email":   {"required": false, "purpose_vi": "...", "purpose_en": "..."}}}
//! ```

use std::collections::BTreeSet;

use feco_core::Scope;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::i18n::Lang;

/// Scope kỹ thuật của OIDC, không phải dữ liệu người dùng: được cấp khi app yêu cầu.
pub const PROTOCOL_SCOPES: &[&str] = &["openid", "offline_access"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredScope {
    pub scope: Scope,
    pub required: bool,
    pub purpose_vi: String,
    pub purpose_en: String,
}

#[derive(Debug, Clone)]
pub struct AppInfo {
    pub client_id: String,
    pub name: String,
    pub developer: String,
    pub developer_verified: bool,
    pub policy_uri: String,
    pub scopes: Vec<DeclaredScope>,
}

impl AppInfo {
    /// Đọc từ một OAuth client của Hydra.
    pub fn from_client(client: &Value) -> AppInfo {
        let meta = &client["metadata"];
        let mut scopes: Vec<DeclaredScope> = meta["scopes"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(name, d)| {
                Some(DeclaredScope {
                    scope: name.parse::<Scope>().ok()?,
                    required: d["required"].as_bool().unwrap_or(false),
                    purpose_vi: d["purpose_vi"].as_str().unwrap_or_default().to_owned(),
                    purpose_en: d["purpose_en"].as_str().unwrap_or_default().to_owned(),
                })
            })
            .collect();
        scopes.sort_by_key(|s| s.scope);
        AppInfo {
            client_id: client["client_id"].as_str().unwrap_or_default().to_owned(),
            name: client["client_name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| client["client_id"].as_str())
                .unwrap_or_default()
                .to_owned(),
            developer: meta["developer"].as_str().unwrap_or_default().to_owned(),
            developer_verified: meta["developer_verified"].as_bool().unwrap_or(false),
            policy_uri: client["policy_uri"].as_str().unwrap_or_default().to_owned(),
            scopes,
        }
    }

    pub fn declared(&self, scope: Scope) -> Option<&DeclaredScope> {
        self.scopes.iter().find(|s| s.scope == scope)
    }

    /// Phiên bản nội dung màn hình đồng ý: mã băm của mọi thứ người dùng được thấy
    /// (tên app, nhà phát triển, chính sách, scope, mục đích, mô tả, ngôn ngữ).
    /// Lưu cùng bản ghi đồng ý (spec "Đồng ý theo từng app và từng scope").
    pub fn content_version(&self, lang: Lang, shown: &[Scope]) -> String {
        let mut h = Sha256::new();
        for part in [
            "consent-v1",
            lang.code(),
            &self.client_id,
            &self.name,
            &self.developer,
            &self.policy_uri,
        ] {
            h.update(part.as_bytes());
            h.update([0]);
        }
        for s in shown {
            if let Some(d) = self.declared(*s) {
                for part in [
                    s.as_str(),
                    if d.required { "required" } else { "optional" },
                    &d.purpose_vi,
                    &d.purpose_en,
                    s.description_vi(),
                    s.description_en(),
                ] {
                    h.update(part.as_bytes());
                    h.update([0]);
                }
            }
        }
        let hex: String = h
            .finalize()
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect();
        format!("v1-{hex}")
    }
}

/// Scope dữ liệu app yêu cầu mà app đã khai báo (scope lạ hoặc chưa khai báo bị bỏ).
pub fn requested_data_scopes(app: &AppInfo, requested: &[String]) -> Vec<Scope> {
    let mut out: Vec<Scope> = requested
        .iter()
        .filter_map(|s| s.parse::<Scope>().ok())
        .filter(|s| app.declared(*s).is_some())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Tập scope cấp cho app: scope giao thức được yêu cầu, cộng các scope dữ liệu được yêu
/// cầu mà (đã cấp từ trước) hoặc (bắt buộc) hoặc (người dùng vừa chọn).
pub fn grant(
    app: &AppInfo,
    requested: &[String],
    previously_granted: &BTreeSet<String>,
    chosen_optional: &BTreeSet<Scope>,
) -> Vec<String> {
    let mut out: Vec<String> = requested
        .iter()
        .filter(|s| PROTOCOL_SCOPES.contains(&s.as_str()))
        .cloned()
        .collect();
    for scope in requested_data_scopes(app, requested) {
        let declared = app.declared(scope).expect("đã lọc theo khai báo");
        if previously_granted.contains(scope.as_str())
            || declared.required
            || chosen_optional.contains(&scope)
        {
            out.push(scope.as_str().to_owned());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Scope dữ liệu cần hỏi người dùng lần này (chưa từng được cấp).
pub fn scopes_to_ask(
    app: &AppInfo,
    requested: &[String],
    previously_granted: &BTreeSet<String>,
) -> Vec<Scope> {
    requested_data_scopes(app, requested)
        .into_iter()
        .filter(|s| !previously_granted.contains(s.as_str()))
        .collect()
}

/// Scope buộc phải hỏi lại người dùng trước khi app mở được bằng mã khởi chạy (không có màn
/// hình đồng ý). Người dùng đã từng đồng ý với app thì scope tùy chọn họ bỏ qua coi như đã từ
/// chối, không chặn mỗi lần mở; chỉ scope bắt buộc chưa được cấp (ví dụ app thêm ở phiên bản sau)
/// mới phải hỏi lại. Chưa từng đồng ý thì mọi scope dữ liệu đều phải hỏi.
pub fn scopes_blocking_launch(
    app: &AppInfo,
    requested: &[String],
    previously_granted: &BTreeSet<String>,
) -> Vec<Scope> {
    let consented_before = !previously_granted.is_empty();
    scopes_to_ask(app, requested, previously_granted)
        .into_iter()
        .filter(|s| !consented_before || app.declared(*s).is_some_and(|d| d.required))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn soundlab() -> AppInfo {
        AppInfo::from_client(&json!({
            "client_id": "soundlab",
            "client_name": "SoundLab",
            "policy_uri": "https://soundlab.example/privacy",
            "metadata": {
                "developer": "Acme",
                "scopes": {
                    "profile": {"required": true, "purpose_vi": "Hiển thị trên bản nhạc bạn chia sẻ", "purpose_en": "Shown on tracks you share"},
                    "email": {"required": false, "purpose_vi": "Gửi bản tin", "purpose_en": "Newsletter"},
                    "phone": {"required": false, "purpose_vi": "Gửi mã xác nhận", "purpose_en": "Confirmation codes"},
                    "contacts": {"required": true}
                }
            }
        }))
    }

    fn set(xs: &[&str]) -> BTreeSet<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    fn req(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn unknown_declared_scopes_are_ignored() {
        let app = soundlab();
        let names: Vec<Scope> = app.scopes.iter().map(|s| s.scope).collect();
        assert_eq!(names, [Scope::Profile, Scope::Email, Scope::Phone]);
    }

    /// Mở app từ launcher sau khi đã bỏ qua một scope tùy chọn: không bị đòi đồng ý lại.
    #[test]
    fn declined_optional_scope_does_not_block_launch() {
        let app = soundlab();
        let requested = req(&["openid", "profile", "email"]);
        assert_eq!(
            scopes_blocking_launch(&app, &requested, &set(&[])),
            [Scope::Profile, Scope::Email],
            "chưa từng đồng ý"
        );
        assert!(
            scopes_blocking_launch(&app, &requested, &set(&["openid", "profile"])).is_empty(),
            "email đã bị bỏ qua"
        );
        // Scope bắt buộc mới (phone chuyển thành bắt buộc ở bản sau) vẫn phải hỏi lại.
        let mut v2 = app.clone();
        for s in v2.scopes.iter_mut() {
            if s.scope == Scope::Phone {
                s.required = true;
            }
        }
        assert_eq!(
            scopes_blocking_launch(
                &v2,
                &req(&["openid", "profile", "phone"]),
                &set(&["openid", "profile"])
            ),
            [Scope::Phone]
        );
    }

    /// Scenario "Đồng ý một phần".
    #[test]
    fn partial_consent_grants_required_only() {
        let g = grant(
            &soundlab(),
            &req(&["openid", "profile", "email"]),
            &set(&[]),
            &BTreeSet::new(),
        );
        assert_eq!(g, ["openid", "profile"]);
    }

    #[test]
    fn chosen_optional_scope_is_granted() {
        let chosen: BTreeSet<Scope> = [Scope::Email].into();
        let g = grant(
            &soundlab(),
            &req(&["openid", "profile", "email"]),
            &set(&[]),
            &chosen,
        );
        assert_eq!(g, ["email", "openid", "profile"]);
    }

    #[test]
    fn undeclared_and_unknown_requests_are_never_granted() {
        let chosen: BTreeSet<Scope> = [Scope::Billing].into();
        let g = grant(
            &soundlab(),
            &req(&["openid", "profile", "billing", "admin"]),
            &set(&[]),
            &chosen,
        );
        assert_eq!(g, ["openid", "profile"]);
    }

    /// Scenario "App xin thêm scope": chỉ hỏi scope mới, giữ scope cũ.
    #[test]
    fn only_new_scopes_are_asked_and_old_ones_kept() {
        let app = soundlab();
        let prev = set(&["openid", "profile"]);
        let requested = req(&["openid", "profile", "phone"]);
        assert_eq!(scopes_to_ask(&app, &requested, &prev), [Scope::Phone]);
        let g = grant(&app, &requested, &prev, &BTreeSet::new());
        assert_eq!(
            g,
            ["openid", "profile"],
            "chưa chọn phone thì không cấp phone"
        );
    }

    #[test]
    fn content_version_changes_with_what_is_shown() {
        let app = soundlab();
        let a = app.content_version(Lang::Vi, &[Scope::Profile, Scope::Email]);
        assert_eq!(
            a,
            app.content_version(Lang::Vi, &[Scope::Profile, Scope::Email])
        );
        assert_ne!(
            a,
            app.content_version(Lang::En, &[Scope::Profile, Scope::Email])
        );
        assert_ne!(a, app.content_version(Lang::Vi, &[Scope::Profile]));
        let mut changed = app.clone();
        changed.scopes[1].purpose_vi = "Mục đích khác".into();
        assert_ne!(
            a,
            changed.content_version(Lang::Vi, &[Scope::Profile, Scope::Email])
        );
        assert!(a.starts_with("v1-") && a.len() == 19);
    }
}
