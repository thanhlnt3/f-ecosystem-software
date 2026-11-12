//! Dịch vụ HTTP của F-Eco: sức khỏe, trang tài khoản (nhóm 2), đồng ý dữ liệu và
//! đăng nhập một lần cho app con qua Hydra (nhóm 3).
//! Các API khác (danh mục, đơn hàng, thanh toán) được thêm theo từng nhóm task.

pub mod account;
pub mod alerts;
pub mod api;
pub mod apps;
pub mod audit;
pub mod auth;
pub mod catalog;
pub mod challenge;
pub mod codesign;
pub mod connected;
pub mod crypto;
pub mod csrf;
pub mod dev_web;
pub mod developer;
pub mod dns;
pub mod entitlements;
pub mod errors;
pub mod export;
pub mod gateways;
pub mod hydra;
pub mod i18n;
pub mod kratos;
pub mod launch;
pub mod mailer;
pub mod momo;
pub mod oauth;
pub mod orders;
pub mod orders_web;
pub mod pairwise;
pub mod payments;
pub mod payments_web;
pub mod ratelimit;

pub mod review;
pub mod scan;
pub mod security;
pub mod store;
pub mod store_web;
pub mod throttle;
pub mod views;
pub mod vnpay;
pub mod webhooks;
pub mod zalopay;

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
    /// API công khai của Hydra (feco-server tự đi luồng ủy quyền khi đổi mã khởi chạy).
    pub hydra_public: Url,
    /// Client ID của F-Launcher (xem `feco_config::AuthConfig::launcher_client_id`).
    pub launcher_client_id: Option<String>,
    pub pairwise: Pairwise,
    /// HTTP client gửi webhook cho app con.
    pub http: reqwest::Client,
    /// Khóa mã hóa dữ liệu nhạy cảm khi lưu.
    pub data_key: crypto::DataKey,
    pub ratelimit: ratelimit::RateLimiter,
    pub limits: Limits,
    /// Bộ nhớ đệm kết quả tìm kiếm danh mục.
    pub catalog: catalog::Cache,
    /// Máy chủ DNS để xác minh tên miền của nhà phát triển.
    pub dns_servers: Vec<std::net::SocketAddr>,
    /// Thư mục lưu gói nhà phát triển tải lên.
    pub package_dir: std::path::PathBuf,
    /// Kho TUF để ký target khi phát hành (không cấu hình thì bỏ qua bước ký).
    pub tuf: Option<feco_config::TufConfig>,
    pub tuf_lock: tokio::sync::Mutex<()>,
    /// ClamAV, engine quét thứ hai của khâu kiểm duyệt.
    pub clamd: Option<std::net::SocketAddr>,
    /// Địa chỉ nhận cảnh báo kiểm duyệt.
    pub admin_emails: Vec<String>,
    /// Cổng thanh toán ZaloPay (không cấu hình thì trang thanh toán báo chưa sẵn sàng).
    pub zalopay: Option<zalopay::ZaloPay>,
    /// Cổng VNPAY (QR, thẻ ATM, thẻ quốc tế) và MoMo (ví MoMo).
    pub vnpay: Option<vnpay::Vnpay>,
    pub momo: Option<momo::Momo>,
    /// Gốc địa chỉ cổng thanh toán gọi callback (mặc định `public_url`).
    pub payment_callback_base: Url,
}

/// Ngưỡng giới hạn tần suất (yêu cầu mỗi phút).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub ip_per_min: u32,
    pub app_per_min: u32,
    pub user_per_min: u32,
}

impl Limits {
    pub fn from_config(c: &feco_config::SecurityConfig) -> Self {
        Limits {
            ip_per_min: c.rate_ip_per_min,
            app_per_min: c.rate_app_per_min,
            user_per_min: c.rate_user_per_min,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error(transparent)]
    Mail(#[from] MailError),
    #[error("địa chỉ cơ sở dữ liệu không hợp lệ: {0}")]
    Database(#[from] sqlx::Error),
    #[error("khóa mã hóa dữ liệu không hợp lệ: {0}")]
    DataKey(#[from] crypto::CryptoError),
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
            hydra_public: a.hydra_public_url.clone(),
            launcher_client_id: a.launcher_client_id.clone(),
            pairwise: Pairwise::new(a.pairwise_secret.expose()),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("tạo HTTP client"),
            data_key: crypto::DataKey::from_base64(config.security.data_key.expose())?,
            ratelimit: ratelimit::RateLimiter::default(),
            limits: Limits::from_config(&config.security),
            catalog: catalog::Cache::default(),
            dns_servers: config.developer.dns_servers.clone(),
            package_dir: config.developer.package_dir.clone(),
            tuf: config.developer.tuf.clone(),
            tuf_lock: tokio::sync::Mutex::new(()),
            clamd: config.developer.clamd,
            admin_emails: config.developer.admin_emails.clone(),
            zalopay: config.payments.zalopay.as_ref().map(zalopay::ZaloPay::new),
            vnpay: config.payments.vnpay.as_ref().map(vnpay::Vnpay::new),
            momo: config.payments.momo.as_ref().map(momo::Momo::new),
            payment_callback_base: config
                .payments
                .callback_base
                .clone()
                .unwrap_or_else(|| a.public_url.clone()),
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
        .route("/v1/me", get(api::me))
        .route("/v1/entitlements", get(entitlements::api_list))
        .route("/v1/catalog", get(store_web::api_list))
        .route("/v1/catalog/installs", post(store_web::api_install))
        .route("/v1/catalog/{id}", get(store_web::api_detail))
        .route("/v1/catalog/{id}/update", get(dev_web::api_update))
        .route("/v1/apps/crashes", post(dev_web::api_crash))
        .route("/v1/dev/apps/{app}/versions", post(dev_web::api_manifest))
        .route(
            "/v1/dev/versions/{id}/packages/{platform}",
            axum::routing::put(dev_web::api_package),
        )
        .route("/v1/dev/versions/{id}/submit", post(dev_web::api_submit))
        .route("/developer", get(dev_web::home))
        .route("/developer/docs", get(dev_web::docs))
        .route("/policy/content", get(dev_web::content_policy))
        .route("/developer/register", post(dev_web::register))
        .route("/developer/apps/{id}", get(dev_web::app_page))
        .route("/developer/apps/{id}/{action}", post(dev_web::app_action))
        .route(
            "/developer/versions/{id}/packages",
            post(dev_web::upload_package).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route(
            "/developer/versions/{id}/{action}",
            post(dev_web::version_action),
        )
        .route("/developer/{action}", post(dev_web::account_action))
        .route(
            "/orders",
            get(orders_web::list)
                .post(orders_web::create)
                // Tệp đính kèm tối đa 200 MB (orders_web tự đếm và chặn).
                .layer(axum::extract::DefaultBodyLimit::max(210 * 1024 * 1024)),
        )
        .route("/orders/{id}", get(orders_web::show))
        .route(
            "/orders/{id}/messages",
            post(orders_web::post_message)
                .layer(axum::extract::DefaultBodyLimit::max(60 * 1024 * 1024)),
        )
        .route("/orders/{id}/files/{file}", get(orders_web::download_file))
        .route("/orders/{id}/handover", get(orders_web::handover))
        .route("/orders/{id}/{action}", post(orders_web::action))
        .route("/policy/custom-build", get(orders_web::terms))
        .route("/payments", get(payments_web::history))
        .route("/payments/billing", post(payments_web::save_billing))
        .route("/payments/return", get(payments_web::return_page))
        .route("/payments/invoices/{id}", get(payments_web::invoice))
        .route(
            "/payments/{request}/pay",
            get(payments_web::pay_page).post(payments_web::pay),
        )
        .route(
            "/v1/payments/zalopay/callback",
            post(payments_web::zalopay_callback),
        )
        .route("/v1/payments/vnpay/ipn", get(payments_web::vnpay_ipn))
        .route("/v1/payments/momo/ipn", post(payments_web::momo_ipn))
        .route("/v1/catalog/private", get(store_web::api_private))
        .route("/store", get(store_web::list))
        .route("/store/{id}", get(store_web::show))
        .route("/store/{id}/reviews", post(store_web::submit_review))
        .route(
            "/store/{id}/reviews/{rid}/reply",
            post(store_web::submit_reply),
        )
        .route("/store/{id}/report", post(store_web::submit_report))
        .route("/v1/launch-codes", post(launch::issue))
        .route("/v1/launch-codes/exchange", post(launch::exchange))
        .route(
            "/security/not-me/{token}",
            get(alerts::not_me).post(alerts::not_me_confirm),
        );
    for kind in [
        FlowKind::Login,
        FlowKind::Registration,
        FlowKind::Verification,
        FlowKind::Recovery,
        FlowKind::Settings,
    ] {
        r = flow_routes(r, kind);
    }
    r.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        security::layer,
    ))
    .with_state(state)
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
            hydra_public: url("http://127.0.0.1:9/"),
            launcher_client_id: Some("launcher".into()),
            pairwise: Pairwise::new("test-only-pairwise-secret-0123456789"),
            http: reqwest::Client::new(),
            data_key: crypto::DataKey::from_bytes(&[9u8; 32]).unwrap(),
            ratelimit: ratelimit::RateLimiter::default(),
            limits: Limits {
                ip_per_min: 1000,
                app_per_min: 1000,
                user_per_min: 1000,
            },
            catalog: catalog::Cache::default(),
            dns_servers: Vec::new(),
            package_dir: std::env::temp_dir(),
            tuf: None,
            tuf_lock: tokio::sync::Mutex::new(()),
            clamd: None,
            admin_emails: Vec::new(),
            zalopay: None,
            vnpay: None,
            momo: None,
            payment_callback_base: Url::parse("http://127.0.0.1:8080/").unwrap(),
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

    fn production(mut s: AppState) -> AppState {
        s.environment = Environment::Production;
        s.public_url = Url::parse("https://f-eco.example/").unwrap();
        s
    }

    /// Spec platform-security, scenario "Kết nối HTTP thường".
    #[tokio::test]
    async fn production_redirects_plain_http_and_sets_hsts() {
        let app = router(Arc::new(production(state(Environment::Dev))));
        let res = app
            .oneshot(
                Request::get("/login?x=1")
                    .header("x-forwarded-proto", "http")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), 308);
        assert_eq!(res.headers()["location"], "https://f-eco.example/login?x=1");
        assert!(res.headers()["strict-transport-security"]
            .to_str()
            .unwrap()
            .starts_with("max-age=31536000"));
    }

    #[tokio::test]
    async fn security_headers_on_every_response() {
        let app = router(Arc::new(production(state(Environment::Dev))));
        let res = app
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let h = res.headers();
        assert_eq!(h["x-content-type-options"], "nosniff");
        assert_eq!(h["x-frame-options"], "DENY");
        assert!(h["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'"));
        assert!(h.contains_key("strict-transport-security"));
        // Ở dev (http) không gửi HSTS.
        let dev = router(Arc::new(state(Environment::Dev)));
        let res = dev
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(!res.headers().contains_key("strict-transport-security"));
    }

    /// Spec platform-security, "Giới hạn tần suất" theo IP.
    #[tokio::test]
    async fn ip_rate_limit_returns_429_with_retry_after() {
        let mut s = state(Environment::Dev);
        s.limits.ip_per_min = 3;
        let app = router(Arc::new(s));
        let call = |ip: &'static str| {
            let app = app.clone();
            async move {
                app.oneshot(
                    Request::get("/khong-co")
                        .header("x-forwarded-for", ip)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
            }
        };
        for _ in 0..3 {
            assert_eq!(call("203.0.113.7").await.status(), 404);
        }
        let res = call("203.0.113.7").await;
        assert_eq!(res.status(), 429);
        let retry: u64 = res.headers()["retry-after"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=60).contains(&retry));
        assert_eq!(
            call("198.51.100.9").await.status(),
            404,
            "IP khác không bị ảnh hưởng"
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
