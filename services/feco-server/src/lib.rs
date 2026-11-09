//! Dịch vụ HTTP của F-Eco: kiểm tra sức khỏe và các trang tài khoản (nhóm task 2).
//! Các API khác (danh mục, đơn hàng, thanh toán) được thêm theo từng nhóm task.

pub mod auth;
pub mod challenge;
pub mod kratos;
pub mod mailer;
pub mod throttle;
pub mod views;

use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use feco_config::Config;
use feco_core::Environment;
use serde::Serialize;
use url::Url;

use crate::{
    auth::FlowQuery,
    challenge::Challenge,
    kratos::{FlowKind, KratosClient},
    mailer::{MailError, Mailer},
    throttle::LoginThrottle,
};

pub struct AppState {
    pub environment: Environment,
    pub public_url: Url,
    pub kratos: KratosClient,
    pub throttle: LoginThrottle,
    pub challenge: Challenge,
    pub mailer: Mailer,
}

impl AppState {
    pub fn from_config(config: &Config) -> Result<Self, MailError> {
        let a = &config.auth;
        Ok(AppState {
            environment: config.environment,
            public_url: a.public_url.clone(),
            kratos: KratosClient::new(a.kratos_public_url.clone(), a.kratos_admin_url.clone()),
            throttle: LoginThrottle::default(),
            challenge: Challenge::from_config(&a.challenge),
            mailer: Mailer::smtp(&a.smtp_url, &a.email_from)?,
        })
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Health {
    pub status: &'static str,
    pub environment: Environment,
    pub version: &'static str,
}

async fn healthz(State(state): State<Arc<AppState>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        environment: state.environment,
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Đăng ký GET và POST cho một luồng Kratos tại `/<tên luồng>`.
fn flow_routes(router: Router<Arc<AppState>>, kind: FlowKind) -> Router<Arc<AppState>> {
    router.route(
        &format!("/{}", kind.path()),
        get(
            move |s: State<Arc<AppState>>, q: Query<FlowQuery>, h: HeaderMap| {
                auth::show_flow(kind, s, q, h)
            },
        )
        .post(
            move |s: State<Arc<AppState>>, q: Query<FlowQuery>, h: HeaderMap, b: Bytes| {
                auth::submit_flow(kind, s, q, h, b)
            },
        ),
    )
}

pub fn router(state: AppState) -> Router {
    let mut r = Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(auth::home))
        .route("/logout", get(auth::logout))
        .route("/error", get(auth::error));
    for kind in [
        FlowKind::Login,
        FlowKind::Registration,
        FlowKind::Verification,
        FlowKind::Recovery,
        FlowKind::Settings,
    ] {
        r = flow_routes(r, kind);
    }
    r.with_state(Arc::new(state))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    fn state(environment: Environment) -> AppState {
        let url = |s: &str| Url::parse(s).unwrap();
        AppState {
            environment,
            public_url: url("http://127.0.0.1:8080/"),
            // Cổng 9 (discard) để mọi lời gọi Kratos thất bại ngay trong test đơn vị.
            kratos: KratosClient::new(url("http://127.0.0.1:9/"), url("http://127.0.0.1:9/")),
            throttle: LoginThrottle::default(),
            challenge: Challenge::Test,
            mailer: Mailer::memory().0,
        }
    }

    #[tokio::test]
    async fn healthz_reports_environment_and_version() {
        let app = router(state(Environment::Staging));
        let res = app
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["environment"], "staging");
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        let app = router(state(Environment::Dev));
        let res = app
            .oneshot(Request::get("/khong-co").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn flow_without_id_redirects_to_kratos_init() {
        let app = router(state(Environment::Dev));
        let res = app
            .oneshot(Request::get("/registration").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 303);
        assert_eq!(
            res.headers()["location"],
            "http://127.0.0.1:9/self-service/registration/browser"
        );
    }

    #[tokio::test]
    async fn kratos_down_shows_friendly_error() {
        let app = router(state(Environment::Dev));
        let res = app
            .oneshot(Request::get("/login?flow=abc").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 503);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&body).contains("tạm gián đoạn"));
    }
}
