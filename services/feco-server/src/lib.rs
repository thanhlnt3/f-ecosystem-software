//! Dịch vụ HTTP của F-Eco: sức khỏe, trang tài khoản (nhóm 2), đồng ý dữ liệu và
//! đăng nhập một lần cho app con qua Hydra (nhóm 3).
//! Các API khác (danh mục, đơn hàng, thanh toán) được thêm theo từng nhóm task.

pub mod account;
pub mod admin;
pub mod alerts;
pub mod api;
pub mod apps;
pub mod audit;
pub mod auth;
pub mod bootstrap;
pub mod catalog;
pub mod challenge;
pub mod chat;
pub mod chat_api;
pub mod chat_web;
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
pub mod i18n_fr;
pub mod i18n_ja;
pub mod kratos;
pub mod launch;
pub mod loadtest;
pub mod maildns;
pub mod mailer;
pub mod misa;
pub mod momo;
pub mod notifications;
pub mod oauth;
pub mod orders;
pub mod orders_web;
pub mod pairwise;
pub mod payments;
pub mod payments_web;
pub mod profile;
pub mod projects;
pub mod projects_web;
pub mod ratelimit;
pub mod status;
pub mod ui_prefs;

pub mod review;
pub mod roles;
pub mod scan;
pub mod security;
pub mod source_scan;
pub mod store;
pub mod store_web;
pub mod throttle;
pub mod token_proxy;
pub mod views;
pub mod vnpay;
pub mod wallet;
pub mod wallet_web;
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
    /// Client ID của F-Launcher di động (`feco_config::AuthConfig::mobile_client_id`).
    pub mobile_client_id: Option<String>,
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
    /// OSV.dev để tra lỗ hổng thư viện khi quét mã nguồn; `None` khi `FECO_OSV=off`.
    pub osv_url: Option<Url>,
    /// Nơi tải mã nguồn GitHub tại một commit (codeload.github.com).
    pub source_fetch_url: Url,
    /// Bảng quản trị không bắt buộc passkey (chỉ dev; feco-config đã kiểm).
    pub admin_passkey_optional: bool,
    /// Số người dùng đếm gần nhất cho dashboard quản trị (QUESTIONS 80): (lúc đếm, số).
    pub user_count: std::sync::Mutex<Option<(std::time::Instant, usize)>>,
    /// Cổng thanh toán ZaloPay (không cấu hình thì trang thanh toán báo chưa sẵn sàng).
    pub zalopay: Option<zalopay::ZaloPay>,
    /// Cổng VNPAY (QR, thẻ ATM, thẻ quốc tế) và MoMo (ví MoMo).
    pub vnpay: Option<vnpay::Vnpay>,
    pub momo: Option<momo::Momo>,
    /// Nhà cung cấp hóa đơn điện tử; không cấu hình thì phát hành hóa đơn nháp nội bộ.
    pub misa: Option<misa::Misa>,
    /// Gốc địa chỉ cổng thanh toán gọi callback (mặc định `public_url`).
    pub payment_callback_base: Url,
    /// Cho phép tạo lệnh rút F-Coin mới (`FECO_WALLET_WITHDRAWALS`).
    pub withdrawals_enabled: bool,
    /// Proxy được tin `X-Forwarded-For` (`FECO_TRUSTED_PROXIES`).
    pub trusted_proxies: Vec<std::net::IpAddr>,
    /// Chat: sự kiện tin mới (từ `LISTEN feco_chat`) và người đang mở hội thoại.
    pub chat: chat::Hub,
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
    /// Client là F-Launcher (desktop hoặc di động): app bên thứ nhất, được gọi API dành cho
    /// launcher (thay đổi mobile-launcher).
    pub fn is_launcher_client(&self, client_id: &str) -> bool {
        [&self.launcher_client_id, &self.mobile_client_id]
            .iter()
            .any(|c| c.as_deref() == Some(client_id))
    }

    /// Dựng trạng thái từ cấu hình. Kết nối cơ sở dữ liệu được mở khi dùng lần đầu;
    /// gọi [`store::MIGRATOR`] trước khi phục vụ yêu cầu.
    pub fn from_config(config: &Config) -> Result<Self, StartError> {
        let a = &config.auth;
        // Production chỉ gửi email qua SMTP có mã hóa (spec notifications).
        if config.environment.is_production() && !Mailer::smtp_url_is_encrypted(&a.smtp_url) {
            return Err(StartError::Mail(MailError::Config(
                "production phải dùng smtps:// hoặc smtp://...?tls=starttls".into(),
            )));
        }
        let db = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_lazy(config.database_url.expose())?;
        // Tên cookie CSRF theo địa chỉ công khai (`__Host-` khi https).
        csrf::init(a.public_url.scheme() == "https");
        Ok(AppState {
            environment: config.environment,
            public_url: a.public_url.clone(),
            kratos: KratosClient::new(a.kratos_public_url.clone(), a.kratos_admin_url.clone())
                .with_after_login(
                    a.public_url
                        .join("auth/after-login")
                        .expect("đường dẫn hợp lệ"),
                ),
            throttle: LoginThrottle::default(),
            challenge: Challenge::from_config(&a.challenge),
            mailer: Mailer::smtp(&a.smtp_url, &a.email_from)?,
            db,
            hydra: HydraClient::new(a.hydra_admin_url.clone()),
            hydra_public: a.hydra_public_url.clone(),
            launcher_client_id: a.launcher_client_id.clone(),
            mobile_client_id: a.mobile_client_id.clone(),
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
            osv_url: config.developer.osv_url.clone(),
            source_fetch_url: config.developer.source_fetch_url.clone(),
            admin_passkey_optional: config.security.admin_passkey_optional,
            user_count: std::sync::Mutex::new(None),
            zalopay: config.payments.zalopay.as_ref().map(zalopay::ZaloPay::new),
            vnpay: config.payments.vnpay.as_ref().map(vnpay::Vnpay::new),
            momo: config.payments.momo.as_ref().map(momo::Momo::new),
            misa: config.payments.misa.as_ref().map(misa::Misa::new),
            payment_callback_base: config
                .payments
                .callback_base
                .clone()
                .unwrap_or_else(|| a.public_url.clone()),
            withdrawals_enabled: !config.payments.withdrawals_disabled,
            trusted_proxies: config.security.trusted_proxies.clone(),
            chat: chat::Hub::default(),
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
        .route("/auth/after-login", get(auth::after_login))
        .route("/oauth2/token", post(token_proxy::token))
        .route("/error", get(auth::error))
        .route("/lang/{code}", get(auth::set_lang))
        .route("/ui.css", get(ui_prefs::stylesheet))
        .route("/status", get(status::page))
        .route("/status.json", get(status::json))
        .route("/preferences", get(ui_prefs::show).post(ui_prefs::save))
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
        .route(
            "/v1/catalog/installs/{app}",
            axum::routing::delete(store_web::api_uninstall),
        )
        .route("/v1/catalog/{id}", get(store_web::api_detail))
        .route("/v1/catalog/{id}/update", get(dev_web::api_update))
        .route("/v1/apps/crashes", post(dev_web::api_crash))
        .route("/v1/dev/apps/{app}/versions", post(dev_web::api_manifest))
        .route(
            "/v1/dev/versions/{id}/packages/{platform}",
            axum::routing::put(dev_web::api_package),
        )
        .route(
            "/v1/dev/versions/{id}/source",
            axum::routing::put(dev_web::api_source),
        )
        .route("/v1/dev/versions/{id}/submit", post(dev_web::api_submit))
        .route("/developer", get(dev_web::home))
        .route("/developer/docs", get(dev_web::docs))
        .route("/policy/content", get(dev_web::content_policy))
        .route("/developer/register", post(dev_web::register))
        .route("/developer/role-request", post(dev_web::request_role))
        .route("/developer/apps/{id}", get(dev_web::app_page))
        .route("/developer/apps/{id}/{action}", post(dev_web::app_action))
        .route(
            "/developer/versions/{id}/packages",
            post(dev_web::upload_package).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route(
            "/developer/versions/{id}/source",
            post(dev_web::upload_source).layer(axum::extract::DefaultBodyLimit::disable()),
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
        .route("/admin", get(admin::home))
        .route("/admin/reviews", get(admin::reviews))
        .route(
            "/admin/reviews/{version}/{action}",
            post(admin::review_action),
        )
        .route("/admin/orders", get(admin::orders_list))
        .route("/admin/orders/{id}", get(admin::order_detail))
        .route("/admin/orders/{id}/{action}", post(admin::order_action))
        .route(
            "/admin/refunds",
            get(admin::refunds).post(admin::create_refund),
        )
        .route("/admin/refunds/{id}/approve", post(admin::approve_refund))
        .route("/admin/users", get(admin::users))
        .route("/admin/users/{id}/{action}", post(admin::user_action))
        .route(
            "/admin/members",
            get(admin::members).post(admin::save_member),
        )
        .route(
            "/admin/members/requests/{id}/{action}",
            post(admin::decide_role_request),
        )
        .route("/notifications", get(notifications::center))
        .route(
            "/notifications/settings",
            post(notifications::save_settings),
        )
        .route(
            "/notifications/unsubscribe/{token}",
            get(notifications::unsubscribe_page).post(notifications::unsubscribe),
        )
        .route("/v1/notifications", get(notifications::api_list))
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
        .route("/wallet", get(wallet_web::page))
        .route("/wallet/topup", post(wallet_web::topup))
        .route("/wallet/withdraw", post(wallet_web::withdraw))
        .route(
            "/wallet/withdrawals/{id}/cancel",
            post(wallet_web::cancel_withdrawal),
        )
        .route("/wallet/stats", get(wallet_web::stats))
        .route("/v1/library", get(store_web::api_library))
        .route("/v1/wallet", get(wallet_web::api_wallet))
        .route("/v1/wallet/purchases", post(wallet_web::api_purchase))
        .route("/store/{id}/buy", post(wallet_web::buy))
        .route("/store/{id}/icon.png", get(store_web::icon))
        .route("/library", get(store_web::library))
        .route("/favicon.svg", get(ui_prefs::favicon))
        .route("/admin/wallet", get(wallet_web::admin_page))
        .route("/admin/wallet/{action}", post(wallet_web::admin_action))
        .route(
            "/admin/wallet/withdrawals/{id}/{action}",
            post(wallet_web::admin_withdrawal_action),
        )
        .route(
            "/admin/wallet/grants/{id}/{action}",
            post(wallet_web::admin_grant_action),
        )
        .route(
            "/account/role",
            get(projects_web::role_page).post(projects_web::role_request),
        )
        .route("/projects", get(projects_web::list))
        .route("/projects/providers", get(projects_web::providers))
        .route(
            "/projects/new",
            get(projects_web::new_form).post(projects_web::create),
        )
        .route("/projects/profile", post(projects_web::save_profile))
        .route("/projects/{id}", get(projects_web::show))
        .route("/projects/{id}/{action}", post(projects_web::action))
        .route("/admin/projects", get(projects_web::admin_list))
        .route("/admin/projects/{id}", get(projects_web::admin_detail))
        .route(
            "/admin/projects/{id}/resolve",
            post(projects_web::admin_resolve),
        )
        .route("/store", get(store_web::list))
        .route("/store/{id}", get(store_web::show))
        .route("/store/{id}/reviews", post(store_web::submit_review))
        .route(
            "/store/{id}/reviews/{rid}/reply",
            post(store_web::submit_reply),
        )
        .route("/store/{id}/report", post(store_web::submit_report))
        // Chat (thay đổi chat-and-source-scan).
        .route("/messages", get(chat_web::list))
        .route("/messages/start", post(chat_web::start))
        .route("/messages/events", get(chat_web::events))
        .route("/messages/chat.js", get(chat_web::script))
        .route(
            "/messages/{id}",
            get(chat_web::show)
                .post(chat_web::send)
                // Một tệp đính kèm tối đa 20 MB (chat::send chặn tệp lớn hơn) và phần còn lại.
                .layer(axum::extract::DefaultBodyLimit::max(
                    chat::MAX_ATTACHMENT + 1024 * 1024,
                )),
        )
        .route("/messages/{id}/after/{last}", get(chat_web::after))
        .route("/messages/{id}/files/{att}", get(chat_web::download))
        .route("/messages/{id}/{action}", post(chat_web::action))
        .route("/admin/chat-reports", get(chat_web::admin_reports))
        .route("/admin/chat-reports/{id}", get(chat_web::admin_report))
        .route(
            "/admin/chat-reports/{id}/handled",
            post(chat_web::admin_handled),
        )
        .route(
            "/admin/projects/{id}/message",
            post(projects_web::admin_message),
        )
        .route("/v1/chat/unread", get(chat_api::unread))
        .route(
            "/v1/chat/conversations",
            get(chat_api::conversations).post(chat_api::start),
        )
        .route(
            "/v1/chat/conversations/{id}/messages",
            get(chat_api::messages).post(chat_api::send),
        )
        .route("/v1/chat/conversations/{id}/read", post(chat_api::read))
        .route("/v1/launch-codes", post(launch::issue))
        .route("/v1/account/role", get(launch::account_role))
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
        chat_web::nav_badge,
    ))
    .layer(axum::middleware::from_fn(ui_prefs::apply))
    .layer(axum::middleware::from_fn_with_state(
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
            mobile_client_id: None,
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
            osv_url: None,
            source_fetch_url: url("http://127.0.0.1:9/"),
            admin_passkey_optional: false,
            user_count: std::sync::Mutex::new(None),
            zalopay: None,
            vnpay: None,
            momo: None,
            misa: None,
            payment_callback_base: Url::parse("http://127.0.0.1:8080/").unwrap(),
            withdrawals_enabled: true,
            trusted_proxies: Vec::new(),
            chat: chat::Hub::default(),
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
        // Yêu cầu đi qua proxy tin cậy 10.0.0.1, IP thật nằm trong X-Forwarded-For.
        s.trusted_proxies = vec!["10.0.0.1".parse().unwrap()];
        let app = router(Arc::new(s));
        let call = |ip: &'static str| {
            let app = app.clone();
            async move {
                app.oneshot(
                    Request::get("/khong-co")
                        .header("x-forwarded-for", ip)
                        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
                            [10, 0, 0, 1],
                            40000,
                        ))))
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

    /// X-Forwarded-For tự khai từ kết nối không qua proxy tin cậy không được dùng để né giới hạn.
    #[tokio::test]
    async fn spoofed_forwarded_for_does_not_bypass_ip_rate_limit() {
        let mut s = state(Environment::Dev);
        s.limits.ip_per_min = 2;
        let app = router(Arc::new(s));
        let mut statuses = Vec::new();
        for fake in ["203.0.113.1", "203.0.113.2", "203.0.113.3"] {
            let res = app
                .clone()
                .oneshot(
                    Request::get("/khong-co")
                        .header("x-forwarded-for", fake)
                        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
                            [198, 51, 100, 20],
                            40000,
                        ))))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            statuses.push(res.status().as_u16());
        }
        assert_eq!(statuses, [404, 404, 429]);
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
