//! Dịch vụ HTTP của F-Eco: sức khỏe, trang tài khoản (nhóm 2), đồng ý dữ liệu và
//! đăng nhập một lần cho app con qua Hydra (nhóm 3).
//! Các API khác (danh mục, đơn hàng, thanh toán) được thêm theo từng nhóm task.

pub mod account;
pub mod api;
pub mod apps;
pub mod auth;
pub mod challenge;
pub mod connected;
pub mod csrf;
pub mod export;
pub mod hydra;
pub mod i18n;
pub mod kratos;
pub mod mailer;
pub mod oauth;
pub mod pairwise;
pub mod store;
pub mod throttle;
pub mod views;
pub mod webhooks;

use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use feco_config::Config;
use feco_core::Environment;
use serde::Serialize;
use url::Url;

use crate::{
    auth::FlowQuery,
    challenge::Challenge,
    hydra::HydraClient,
    i18n::Lang,
    kratos::{FlowKind, KratosClient},
    mailer::{MailError, Mailer},
    pairwise::Pairwise,
    store::Db,
    throttle::LoginThrottle,
};

pub struct AppState {
    pub environment: Environment,
    pub public_url: Url,
    pub kratos: KratosClient,
    pub throttle: LoginThrottle,
    pub challenge: Challenge,
    pub mailer: Mailer,
    pub db: Db,
    pub hydra: HydraClient,
    pub pairwise: Pairwise,
    /// HTTP client gửi webhook cho app con.
    pub http: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error(transparent)]
    Mail(#[from] MailError),
    #[error("địa chỉ cơ sở dữ liệu không hợp lệ: {0}")]
    Database(#[from] sqlx::Error),
}

impl AppState {
    /// Dựng trạng thái từ cấu hình. Kết nối cơ sở dữ liệu được mở khi dùng lần đầu;
    /// gọi [`store::MIGRATOR`] trước khi phục vụ yêu cầu.
    pub fn from_config(config: &Config) -> Result<Self, StartError> {
        let a = &config.auth;
        let db = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_lazy(config.database_url.expose())?;
        Ok(AppState {
            environment: config.environment,
            public_url: a.public_url.clone(),
            kratos: KratosClient::new(a.kratos_public_url.clone(), a.kratos_admin_url.clone()),
            throttle: LoginThrottle::default(),
            challenge: Challenge::from_config(&a.challenge),
            mailer: Mailer::smtp(&a.smtp_url, &a.email_from)?,
            db,
            hydra: HydraClient::new(a.hydra_admin_url.clone()),
            pairwise: Pairwise::new(a.pairwise_secret.expose()),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("tạo HTTP client"),
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
            move |s: State<Arc<AppState>>, l: Lang, q: Query<FlowQuery>, h: HeaderMap| {
                auth::show_flow(kind, s, l, q, h)
            },
        )
        .post(
            move |s: State<Arc<AppState>>,
                  l: Lang,
                  q: Query<FlowQuery>,
                  c: kratos::ClientInfo,
                  h: HeaderMap,
                  b: Bytes| { auth::submit_flow(kind, s, l, q, c, h, b) },
        ),
    )
}

pub fn router(state: Arc<AppState>) -> Router {
    let mut r = Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(auth::home))
        .route("/logout", get(auth::logout))
        .route("/error", get(auth::error))
        .route("/lang/{code}", get(auth::set_lang))
        .route("/sessions", get(account::sessions))
        .route("/sessions/revoke-others", post(account::revoke_others))
        .route("/sessions/{id}/revoke", post(account::revoke))
        .route(
            "/account/delete",
            get(account::delete_form).post(account::delete_submit),
        )
        .route("/account/export", get(export::show).post(export::request))
        .route("/account/export/{token}", get(export::download))
        .route("/oauth/login", get(oauth::login))
        .route(
            "/oauth/consent",
            get(oauth::consent).post(oauth::consent_submit),
        )
        .route("/oauth/logout", get(oauth::logout))
        .route("/apps", get(connected::list))
        .route("/apps/{client_id}", get(connected::detail))
        .route("/apps/{client_id}/revoke", post(connected::revoke))
        .route("/v1/me", get(api::me));
    for kind in [
        FlowKind::Login,
        FlowKind::Registration,
        FlowKind::Verification,
        FlowKind::Recovery,
        FlowKind::Settings,
    ] {
        r = flow_routes(r, kind);
    }
    r.with_state(state)
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
            db: sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://x:y@127.0.0.1:9/x")
                .unwrap(),
            hydra: HydraClient::new(url("http://127.0.0.1:9/")),
            pairwise: Pairwise::new("test-only-pairwise-secret-0123456789"),
            http: reqwest::Client::new(),
        }
    }

    #[tokio::test]
    async fn healthz_reports_environment_and_version() {
        let app = router(Arc::new(state(Environment::Staging)));
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
        let app = router(Arc::new(state(Environment::Dev)));
        let res = app
            .oneshot(Request::get("/khong-co").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn flow_without_id_redirects_to_kratos_init() {
        let app = router(Arc::new(state(Environment::Dev)));
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
        let app = router(Arc::new(state(Environment::Dev)));
        let res = app
            .oneshot(Request::get("/login?flow=abc").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 503);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&body).contains("tạm gián đoạn"));
    }
}
