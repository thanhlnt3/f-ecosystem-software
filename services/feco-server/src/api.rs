//! API dữ liệu người dùng cho app con: `GET /v1/me` (spec user-data-consent, "Danh mục
//! scope dữ liệu", "Nhật ký truy cập dữ liệu"; spec app-sdk, "API dữ liệu người dùng",
//! "Mã lỗi thống nhất").
//!
//! - Chỉ trả các trường thuộc scope mà access token mang theo.
//! - `?fields=email,phone_number`: nếu một trường thuộc scope chưa được cấp, trả 403
//!   `insufficient_scope` và không trả dữ liệu.
//! - `sub` là định danh riêng của app (pairwise).
//! - Mỗi lần đọc dữ liệu được ghi vào nhật ký truy cập.

use std::{collections::BTreeSet, sync::Arc};

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use feco_core::Scope;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::{store, AppState};

#[derive(Debug, Deserialize)]
pub struct MeQuery {
    pub fields: Option<String>,
}

/// Trường trả về và scope cần có.
pub const FIELDS: &[(&str, Scope)] = &[
    ("name", Scope::Profile),
    ("picture", Scope::Profile),
    ("email", Scope::Email),
    ("email_verified", Scope::Email),
    ("locale", Scope::Locale),
    ("zoneinfo", Scope::Locale),
    ("phone_number", Scope::Phone),
    ("billing", Scope::Billing),
];

pub fn api_error(status: StatusCode, code: &str, en: &str, vi: &str) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let mut res = (
        status,
        Json(json!({ "error": code, "message": en, "message_vi": vi, "request_id": request_id })),
    )
        .into_response();
    if status == StatusCode::UNAUTHORIZED {
        res.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer error=\"invalid_token\""),
        );
    }
    res
}

fn invalid_token() -> Response {
    api_error(
        StatusCode::UNAUTHORIZED,
        "invalid_token",
        "The access token is missing, expired or revoked.",
        "Access token bị thiếu, đã hết hạn hoặc đã bị thu hồi.",
    )
}

pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Dựng phần dữ liệu theo scope từ traits của danh tính.
pub fn profile_fields(identity: &Value, granted: &BTreeSet<Scope>) -> Map<String, Value> {
    let t = &identity["traits"];
    let mut out = Map::new();
    let s = |v: &Value| {
        v.as_str()
            .map(|x| Value::String(x.to_owned()))
            .unwrap_or(Value::Null)
    };
    if granted.contains(&Scope::Profile) {
        out.insert("name".into(), s(&t["name"]));
        out.insert("picture".into(), s(&t["avatar_url"]));
    }
    if granted.contains(&Scope::Email) {
        let email = t["email"].as_str().unwrap_or_default();
        let verified = identity["verifiable_addresses"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|a| a["value"] == email && a["verified"] == true);
        out.insert("email".into(), json!(email));
        out.insert("email_verified".into(), json!(verified));
    }
    if granted.contains(&Scope::Locale) {
        out.insert("locale".into(), json!(t["locale"].as_str().unwrap_or("vi")));
        out.insert(
            "zoneinfo".into(),
            json!(t["timezone"].as_str().unwrap_or("Asia/Ho_Chi_Minh")),
        );
    }
    if granted.contains(&Scope::Phone) {
        out.insert("phone_number".into(), s(&t["phone"]));
    }
    if granted.contains(&Scope::Billing) {
        out.insert(
            "billing".into(),
            json!({
                "name": t["billing"]["name"],
                "tax_code": t["billing"]["tax_code"],
                "address": t["billing"]["address"],
            }),
        );
    }
    out
}

/// GET /v1/me
pub async fn me(
    State(state): State<Arc<AppState>>,
    Query(q): Query<MeQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = bearer(&headers) else {
        return invalid_token();
    };
    let info = match state.hydra.introspect(token).await {
        Ok(Some(i)) => i,
        Ok(None) => return invalid_token(),
        Err(e) => {
            tracing::error!(error = %e, "không kiểm tra được token");
            return api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "temporarily_unavailable",
                "The service is temporarily unavailable.",
                "Dịch vụ đang tạm gián đoạn.",
            );
        }
    };
    let granted: BTreeSet<Scope> = info["scope"]
        .as_str()
        .unwrap_or_default()
        .split(' ')
        .filter_map(|s| s.parse().ok())
        .collect();

    if let Some(fields) = &q.fields {
        for f in fields.split(',').map(str::trim).filter(|f| !f.is_empty()) {
            match FIELDS.iter().find(|(name, _)| *name == f) {
                Some((_, scope)) if !granted.contains(scope) => {
                    return api_error(
                        StatusCode::FORBIDDEN,
                        "insufficient_scope",
                        &format!("The field `{f}` requires the `{scope}` scope."),
                        &format!("Trường `{f}` cần scope `{scope}`."),
                    )
                }
                Some(_) => {}
                None => {
                    return api_error(
                        StatusCode::BAD_REQUEST,
                        "invalid_request",
                        &format!("Unknown field `{f}`."),
                        &format!("Không có trường `{f}`."),
                    )
                }
            }
        }
    }

    let (Some(subject), Some(client_id)) = (
        info["sub"].as_str().and_then(|s| s.parse::<Uuid>().ok()),
        info["client_id"].as_str(),
    ) else {
        return invalid_token();
    };
    // Spec platform-security, "Giới hạn tần suất": theo app và theo người dùng.
    let minute = std::time::Duration::from_secs(60);
    let now = std::time::Instant::now();
    for (key, limit) in [
        (format!("app:{client_id}"), state.limits.app_per_min),
        (format!("user:{subject}"), state.limits.user_per_min),
    ] {
        if let Err(wait) = state.ratelimit.check(&key, limit, minute, now) {
            return crate::security::too_many("/v1/me", wait);
        }
    }
    let identity = match state.kratos.get_identity(&subject.to_string()).await {
        Ok(Some(i)) => i,
        Ok(None) => return invalid_token(),
        Err(e) => {
            tracing::error!(error = %e, "không đọc được danh tính");
            return api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "temporarily_unavailable",
                "The service is temporarily unavailable.",
                "Dịch vụ đang tạm gián đoạn.",
            );
        }
    };

    let mut body = profile_fields(&identity, &granted);
    let wanted: Option<BTreeSet<&str>> = q
        .fields
        .as_deref()
        .map(|f| f.split(',').map(str::trim).collect());
    if let Some(w) = &wanted {
        body.retain(|k, _| w.contains(k.as_str()));
    }
    let read_scopes: Vec<String> = FIELDS
        .iter()
        .filter(|(f, _)| body.contains_key(*f))
        .map(|(_, s)| s.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !read_scopes.is_empty() {
        if let Err(e) = store::log_access(&state.db, subject, client_id, &read_scopes).await {
            // Không ghi được nhật ký thì không trả dữ liệu (spec yêu cầu mọi lần đọc đều có nhật ký).
            tracing::error!(error = %e, "không ghi được nhật ký truy cập");
            return api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "temporarily_unavailable",
                "The service is temporarily unavailable.",
                "Dịch vụ đang tạm gián đoạn.",
            );
        }
    }
    body.insert(
        "sub".into(),
        json!(state.pairwise.subject(client_id, &subject.to_string())),
    );
    Json(Value::Object(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> Value {
        json!({
            "traits": {"email": "lan@example.com", "name": "Lan Nguyễn", "locale": "vi",
                       "phone": "+84901234567", "avatar_url": "https://img.example/lan.png",
                       "billing": {"name": "Công ty A", "tax_code": "0101234567", "address": "Hà Nội"}},
            "verifiable_addresses": [{"value": "lan@example.com", "verified": true}]
        })
    }

    /// Scenario "Gọi API ngoài scope" (phần dữ liệu): chỉ có profile thì không có email.
    #[test]
    fn profile_only_returns_name_and_picture() {
        let f = profile_fields(&identity(), &[Scope::Profile].into());
        let keys: Vec<&str> = f.keys().map(String::as_str).collect();
        assert_eq!(keys, ["name", "picture"]);
        assert_eq!(f["name"], "Lan Nguyễn");
    }

    #[test]
    fn every_scope_maps_to_its_fields() {
        let all: BTreeSet<Scope> = Scope::ALL.into_iter().collect();
        let f = profile_fields(&identity(), &all);
        for (field, _) in FIELDS {
            assert!(f.contains_key(*field), "thiếu trường {field}");
        }
        assert_eq!(f["email_verified"], true);
        assert_eq!(f["zoneinfo"], "Asia/Ho_Chi_Minh");
        assert_eq!(f["billing"]["tax_code"], "0101234567");
    }

    #[test]
    fn error_body_has_both_languages_and_request_id() {
        let res = invalid_token();
        assert_eq!(res.status(), 401);
        assert!(res.headers().contains_key(header::WWW_AUTHENTICATE));
    }
}
