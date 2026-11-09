//! Lớp bảo vệ chung cho mọi yêu cầu (spec platform-security):
//! - "Mã hóa khi truyền": ở production, yêu cầu đi qua HTTP thường (proxy báo
//!   `X-Forwarded-Proto: http`) được chuyển sang HTTPS; mọi phản hồi có HSTS khi địa chỉ
//!   công khai là https.
//! - Header chống nhúng khung, đoán kiểu nội dung, lộ referrer.
//! - "Giới hạn tần suất" theo địa chỉ IP, trả 429 kèm `Retry-After`.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{api::api_error, kratos::ClientInfo, ratelimit::retry_after_secs, AppState};

pub const HSTS: &str = "max-age=31536000; includeSubDomains";

fn set(res: &mut Response, name: header::HeaderName, value: &'static str) {
    res.headers_mut()
        .insert(name, HeaderValue::from_static(value));
}

pub async fn layer(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let https_public = state.public_url.scheme() == "https";

    // Chuyển HTTP sang HTTPS (chỉ khi chạy sau proxy TLS ở production).
    if state.environment.is_production()
        && req
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            == Some("http")
    {
        let path = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/");
        let to = format!(
            "{}{}",
            state.public_url.as_str().trim_end_matches('/'),
            path
        );
        let mut res = StatusCode::PERMANENT_REDIRECT.into_response();
        if let Ok(v) = HeaderValue::from_str(&to) {
            res.headers_mut().insert(header::LOCATION, v);
        }
        set(&mut res, header::STRICT_TRANSPORT_SECURITY, HSTS);
        return res;
    }

    // Giới hạn tần suất theo IP (trừ kiểm tra sức khỏe).
    if req.uri().path() != "/healthz" {
        let (mut parts, body) = req.into_parts();
        let client = <ClientInfo as axum::extract::FromRequestParts<()>>::from_request_parts(
            &mut parts,
            &(),
        )
        .await
        .unwrap_or_default();
        let req = Request::from_parts(parts, body);
        let key = format!("ip:{}", client.ip.as_deref().unwrap_or("unknown"));
        if let Err(wait) = state.ratelimit.check(
            &key,
            state.limits.ip_per_min,
            Duration::from_secs(60),
            Instant::now(),
        ) {
            return too_many(req.uri().path(), wait);
        }
        return finish(next.run(req).await, https_public);
    }
    finish(next.run(req).await, https_public)
}

fn finish(mut res: Response, https_public: bool) -> Response {
    set(&mut res, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(&mut res, header::X_FRAME_OPTIONS, "DENY");
    set(
        &mut res,
        header::REFERRER_POLICY,
        "strict-origin-when-cross-origin",
    );
    set(
        &mut res,
        header::CONTENT_SECURITY_POLICY,
        "frame-ancestors 'none'; base-uri 'none'; object-src 'none'",
    );
    if https_public {
        set(&mut res, header::STRICT_TRANSPORT_SECURITY, HSTS);
    }
    res
}

/// Phản hồi 429: JSON cho API (`/v1/...`), chữ thường cho trang web.
pub fn too_many(path: &str, wait: Duration) -> Response {
    let mut res = if path.starts_with("/v1/") {
        api_error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many requests. Retry after the time in the Retry-After header.",
            "Quá nhiều yêu cầu. Hãy thử lại sau khoảng thời gian trong header Retry-After.",
        )
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Quá nhiều yêu cầu, hãy thử lại sau ít phút. / Too many requests, please try again shortly.",
        )
            .into_response()
    };
    if let Ok(v) = HeaderValue::from_str(&retry_after_secs(wait).to_string()) {
        res.headers_mut().insert(header::RETRY_AFTER, v);
    }
    res
}
