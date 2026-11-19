//! Kiểm thử đầu-cuối của chat (thay đổi OpenSpec chat-and-source-scan, tasks 1.1–1.4) với Ory
//! Kratos, Hydra, Mailpit và PostgreSQL thật.
//!
//! Chỉ chạy khi đặt `FECO_E2E=1`, cần hạ tầng dev (Kratos 4433/4434, Hydra 4444/4445, Mailpit
//! 1025/8025, PostgreSQL 5432) và cổng 8080 trống. Máy chủ kiểm thử dùng cơ sở dữ liệu riêng
//! `feco_chat_e2e` (tự tạo) để không đụng lược đồ của các nhánh khác trong `feco`.
//!
//! Mỗi test ghi scenario của `specs/messaging` và `specs/notifications` mà nó kiểm.

use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use feco_config::Config;
use feco_server::{kratos::KratosClient, router, store, AppState};
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Value};
use uuid::Uuid;

const BASE: &str = "http://127.0.0.1:8080";
const MAILPIT: &str = "http://127.0.0.1:8025";
const HYDRA_PUBLIC: &str = "http://127.0.0.1:4444";
const HYDRA_ADMIN: &str = "http://127.0.0.1:4445";
const APP_CALLBACK: &str = "http://127.0.0.1:9999/cb";
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";
const ADMIN_EMAIL: &str = "kiem-duyet-chat@f-eco.test";
const DB_NAME: &str = "feco_chat_e2e";

/// Kratos dev dùng SQLite trong bộ nhớ: các test chạy lần lượt.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static LAUNCHER: OnceLock<String> = OnceLock::new();
static MOBILE: OnceLock<String> = OnceLock::new();
static SERVER_VARS: OnceLock<HashMap<String, String>> = OnceLock::new();
static ZALOPAY: OnceLock<feco_server::zalopay::testing::FakeZaloPay> = OnceLock::new();

fn db_url(name: &str) -> String {
    let base = std::env::var("FECO_E2E_PG")
        .unwrap_or_else(|_| "postgres://feco:feco-dev-only@127.0.0.1:5432".into());
    format!("{}/{name}", base.trim_end_matches('/'))
}

/// Tạo cơ sở dữ liệu `name` nếu chưa có (kết nối qua cơ sở dữ liệu `feco` của dev).
async fn ensure_database(name: &str) {
    use sqlx::{Connection, Executor};
    let mut c = sqlx::PgConnection::connect(&db_url("feco"))
        .await
        .expect("kết nối PostgreSQL dev");
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(name)
            .fetch_one(&mut c)
            .await
            .unwrap();
    if !exists {
        c.execute(format!("CREATE DATABASE {name}").as_str())
            .await
            .expect("tạo cơ sở dữ liệu kiểm thử");
    }
}

async fn drop_database(name: &str) {
    use sqlx::{Connection, Executor};
    let mut c = sqlx::PgConnection::connect(&db_url("feco")).await.unwrap();
    let _ = c
        .execute(format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)").as_str())
        .await;
}

async fn create_public_client(name: &str) -> String {
    let body = json!({
        "client_name": name,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": "openid offline_access profile",
        "redirect_uris": [APP_CALLBACK],
        "token_endpoint_auth_method": "none",
    });
    let res: Value = reqwest::Client::new()
        .post(format!("{HYDRA_ADMIN}/admin/clients"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    res["client_id"].as_str().expect("client_id").to_owned()
}

fn e2e_ready() -> bool {
    if std::env::var("FECO_E2E").is_err() {
        eprintln!("bỏ qua: đặt FECO_E2E=1 và chạy hạ tầng dev để kiểm thử đầu-cuối");
        return false;
    }
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "warn".into()),
            )
            .with_ansi(false)
            .with_test_writer()
            .try_init();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                ensure_database(DB_NAME).await;
                let launcher = create_public_client("F-Launcher").await;
                LAUNCHER.set(launcher.clone()).unwrap();
                let mobile = create_public_client("F-Launcher di động").await;
                MOBILE.set(mobile.clone()).unwrap();
                let work =
                    std::env::temp_dir().join(format!("feco-e2e-chat-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&work);
                let zalopay = feco_server::zalopay::testing::FakeZaloPay::serve(
                    2553,
                    "e2e-chat-key1",
                    "e2e-chat-key2",
                )
                .await
                .unwrap();
                let zalopay_endpoint = zalopay.endpoint();
                let _ = ZALOPAY.set(zalopay);
                let vars: HashMap<String, String> = [
                    ("FECO_ENV", "dev".to_owned()),
                    ("FECO_DATABASE_URL", db_url(DB_NAME)),
                    ("FECO_LAUNCHER_CLIENT_ID", launcher),
                    ("FECO_MOBILE_CLIENT_ID", mobile),
                    (
                        "FECO_PACKAGE_DIR",
                        work.join("packages").to_string_lossy().into_owned(),
                    ),
                    ("FECO_ADMIN_EMAILS", ADMIN_EMAIL.to_owned()),
                    ("FECO_ADMIN_PASSKEY_OPTIONAL", "true".to_owned()),
                    // Cổng ZaloPay giả để nạp F-Coin ký quỹ dự án (khóa chỉ dùng cho cổng giả).
                    ("FECO_ZALOPAY_APP_ID", "2553".to_owned()),
                    ("FECO_ZALOPAY_KEY1", "e2e-chat-key1".to_owned()),
                    ("FECO_ZALOPAY_KEY2", "e2e-chat-key2".to_owned()),
                    ("FECO_ZALOPAY_ENDPOINT", zalopay_endpoint),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
                let _ = SERVER_VARS.set(vars.clone());
                let config = Config::from_vars(&vars).unwrap();
                let state = std::sync::Arc::new(AppState::from_config(&config).unwrap());
                store::MIGRATOR
                    .run(&state.db)
                    .await
                    .expect("chạy migration");
                // Như main.rs: nghe LISTEN feco_chat từ đầu.
                feco_server::chat::start_listener(&state);
                let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
                    .await
                    .expect("cổng 8080 phải trống cho kiểm thử E2E");
                tx.send(()).unwrap();
                axum::serve(
                    listener,
                    router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .await
                .unwrap();
            });
        });
        rx.recv().unwrap();
    });
    true
}

/// AppState riêng cho runtime của từng test.
fn local_state() -> AppState {
    let vars = SERVER_VARS.get().cloned().unwrap();
    AppState::from_config(&Config::from_vars(&vars).unwrap()).unwrap()
}

fn kratos() -> KratosClient {
    KratosClient::new(
        url::Url::parse("http://127.0.0.1:4433/").unwrap(),
        url::Url::parse("http://127.0.0.1:4434/").unwrap(),
    )
}

/// Trình duyệt dừng khi được chuyển về địa chỉ callback của app (như app thật nhận code).
fn app_browser() -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "accept-language",
        reqwest::header::HeaderValue::from_static("vi-VN"),
    );
    reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(UA)
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::custom(|a| {
            if a.url().port() == Some(9999) {
                a.stop()
            } else {
                a.follow()
            }
        }))
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap()
}

fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn unique(prefix: &str) -> (String, String) {
    let s = stamp();
    (
        format!("{prefix}-{s}@example.com"),
        format!("Tra-sua-tran-chau-{:x}-Q", s.wrapping_mul(7919)),
    )
}

struct Form {
    action: String,
    fields: Vec<(String, String)>,
}

fn parse_form(html: &str) -> Form {
    let doc = Html::parse_document(html);
    let form = doc
        .select(&Selector::parse("form").unwrap())
        .next()
        .unwrap_or_else(|| panic!("trang không có biểu mẫu:\n{html}"));
    Form {
        action: form.value().attr("action").unwrap().to_owned(),
        fields: form
            .select(&Selector::parse("input[type=hidden]").unwrap())
            .map(|i| {
                (
                    i.value().attr("name").unwrap().to_owned(),
                    i.value().attr("value").unwrap_or_default().to_owned(),
                )
            })
            .collect(),
    }
}

async fn get(c: &reqwest::Client, path: &str) -> String {
    let url = if path.starts_with("http") {
        path.to_owned()
    } else {
        format!("{BASE}{path}")
    };
    c.get(url).send().await.unwrap().text().await.unwrap()
}

async fn get_status(c: &reqwest::Client, path: &str) -> (u16, String) {
    let res = c.get(format!("{BASE}{path}")).send().await.unwrap();
    (res.status().as_u16(), res.text().await.unwrap())
}

async fn submit(c: &reqwest::Client, form: &Form, extra: &[(&str, &str)]) -> String {
    let mut pairs = form.fields.clone();
    pairs.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .finish();
    c.post(format!("{BASE}{}", form.action))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

async fn login(c: &reqwest::Client, email: &str, password: &str) -> String {
    let form = parse_form(&get(c, "/login").await);
    submit(
        c,
        &form,
        &[
            ("identifier", email),
            ("password", password),
            ("method", "password"),
        ],
    )
    .await
}

async fn register_in(c: &reqwest::Client, email: &str, password: &str, name: &str) {
    let form = parse_form(&get(c, "/registration").await);
    submit(
        c,
        &form,
        &[
            ("traits.email", email),
            ("traits.name", name),
            ("password", password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(login(c, email, password).await.contains(email));
}

async fn mailpit_messages() -> Vec<Value> {
    let list: Value = reqwest::Client::new()
        .get(format!("{MAILPIT}/api/v1/messages?limit=500"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    list["messages"].as_array().cloned().unwrap_or_default()
}

fn mail_matches(m: &Value, to: &str, subject: &str) -> bool {
    m["To"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|t| t["Address"] == to)
        && m["Subject"].as_str().is_some_and(|s| s.contains(subject))
}

/// Chờ email tới `to` có tiêu đề chứa `subject`; trả (tiêu đề, nội dung chữ).
async fn wait_mail(to: &str, subject: &str) -> (String, String) {
    let http = reqwest::Client::new();
    for _ in 0..60 {
        if let Some(m) = mailpit_messages()
            .await
            .into_iter()
            .find(|m| mail_matches(m, to, subject))
        {
            let id = m["ID"].as_str().unwrap();
            let msg: Value = http
                .get(format!("{MAILPIT}/api/v1/message/{id}"))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            return (
                m["Subject"].as_str().unwrap_or_default().to_owned(),
                msg["Text"].as_str().unwrap_or_default().to_owned(),
            );
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("không thấy email tới {to} với tiêu đề chứa {subject:?}");
}

async fn mail_count(to: &str, subject: &str) -> usize {
    mailpit_messages()
        .await
        .iter()
        .filter(|m| mail_matches(m, to, subject))
        .count()
}

async fn verify_email(c: &reqwest::Client, email: &str) {
    let (_, mail) = wait_mail(email, "Xác minh email").await;
    let link = Regex::new(r"https?://\S+/self-service/verification\?\S+")
        .unwrap()
        .find(&mail)
        .expect("email có liên kết xác minh")
        .as_str()
        .to_owned();
    let page = get(c, &link).await;
    assert!(page.contains("Bạn đã xác minh email thành công"), "{page}");
}

async fn subject_of(email: &str) -> Uuid {
    kratos()
        .list_identities()
        .await
        .unwrap()
        .into_iter()
        .find(|i| i["traits"]["email"] == email)
        .and_then(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
        .expect("có danh tính")
}

fn csrf_of(page: &str) -> String {
    Regex::new(r#"name="feco_csrf" value="([^"]+)""#)
        .unwrap()
        .captures(page)
        .unwrap_or_else(|| panic!("trang không có token CSRF:\n{page}"))[1]
        .to_owned()
}

/// Gửi biểu mẫu thường; trả mã HTTP, trang cuối cùng và địa chỉ cuối cùng.
async fn post(
    c: &reqwest::Client,
    path: &str,
    csrf: &str,
    fields: &[(&str, &str)],
) -> (u16, String, String) {
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("feco_csrf", csrf)
        .extend_pairs(fields.iter().copied())
        .finish();
    let res = c
        .post(format!("{BASE}{path}"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();
    let url = res.url().to_string();
    (res.status().as_u16(), res.text().await.unwrap(), url)
}

/// Gửi biểu mẫu multipart (gửi tin kèm tệp).
async fn post_multipart(
    c: &reqwest::Client,
    path: &str,
    csrf: &str,
    fields: &[(&str, &str)],
    file: Option<(&str, Vec<u8>)>,
) -> (u16, String, String) {
    let mut form = reqwest::multipart::Form::new().text("feco_csrf", csrf.to_owned());
    for (k, v) in fields {
        form = form.text(k.to_string(), v.to_string());
    }
    if let Some((name, bytes)) = file {
        form = form.part(
            "file",
            reqwest::multipart::Part::bytes(bytes).file_name(name.to_owned()),
        );
    }
    let res = c
        .post(format!("{BASE}{path}"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let url = res.url().to_string();
    (res.status().as_u16(), res.text().await.unwrap(), url)
}

fn pkce() -> (String, String) {
    use base64::Engine;
    use sha2::Digest;
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

/// Access token của F-Launcher (máy tính hoặc di động) cho người đang đăng nhập trong `user`.
async fn token_for(user: &reqwest::Client, client_id: &str) -> String {
    let (verifier, challenge) = pkce();
    let mut url = url::Url::parse(&format!("{HYDRA_PUBLIC}/oauth2/auth")).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid offline_access profile")
        .append_pair("redirect_uri", APP_CALLBACK)
        .append_pair("state", "trang-thai-du-dai")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    let res = user.get(url).send().await.unwrap();
    assert!(res.status().is_redirection(), "launcher không cần đồng ý");
    let cb = url::Url::parse(res.headers()["location"].to_str().unwrap()).unwrap();
    let code = cb
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| panic!("không có code: {cb}"));
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", &code)
        .append_pair("redirect_uri", APP_CALLBACK)
        .append_pair("client_id", client_id)
        .append_pair("code_verifier", &verifier)
        .finish();
    let tokens: Value = reqwest::Client::new()
        .post(format!("{HYDRA_PUBLIC}/oauth2/token"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    tokens["access_token"].as_str().unwrap().to_owned()
}

async fn launcher_token(user: &reqwest::Client) -> String {
    token_for(user, LAUNCHER.get().unwrap()).await
}

async fn mobile_token(user: &reqwest::Client) -> String {
    token_for(user, MOBILE.get().unwrap()).await
}

/// Gọi `/v1/chat/...`; trả mã HTTP và JSON (Null nếu không có thân).
async fn api(token: &str, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let http = reqwest::Client::new();
    let url = format!("{BASE}{path}");
    let req = match method {
        "POST" => http.post(url),
        _ => http.get(url),
    }
    .bearer_auth(token);
    let req = match body {
        Some(b) => req.json(&b),
        None => req,
    };
    let res = req.send().await.unwrap();
    let status = res.status().as_u16();
    let text = res.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

struct User {
    c: reqwest::Client,
    email: String,
    subject: Uuid,
}

async fn user(prefix: &str, name: &str, verified: bool) -> User {
    let (email, password) = unique(prefix);
    let c = app_browser();
    register_in(&c, &email, &password, name).await;
    if verified {
        verify_email(&c, &email).await;
    }
    let subject = subject_of(&email).await;
    User { c, email, subject }
}

async fn set_role(state: &AppState, u: &User, role: feco_server::roles::PlatformRole) {
    feco_server::roles::set_role(
        &state.db,
        u.subject,
        &u.email,
        role,
        &[],
        &feco_server::roles::ChangedBy {
            subject: None,
            name: "e2e".into(),
        },
        "Kiểm thử E2E chat",
    )
    .await
    .unwrap();
}

/// Hội thoại trực tiếp giữa hai người (tạo trong tiến trình test).
async fn direct(state: &AppState, customer: &User, developer: &User) -> Uuid {
    feco_server::chat::start_direct(
        state,
        customer.subject,
        feco_server::chat::Target::Developer(developer.subject),
    )
    .await
    .unwrap()
    .0
    .id
}

async fn unread(token: &str) -> i64 {
    let (status, v) = api(token, "GET", "/v1/chat/unread", None).await;
    assert_eq!(status, 200, "{v}");
    v["unread"].as_i64().unwrap()
}

async fn message_notes(db: &store::Db, subject: Uuid, link: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT title, body FROM notifications WHERE subject = $1 AND category = 'messages' AND link = $2",
    )
    .bind(subject)
    .bind(link)
    .fetch_all(db)
    .await
    .unwrap()
}

/// App trong F-Store do tài khoản nhà phát triển của `owner` sở hữu.
async fn owned_app(state: &AppState, owner: &User, account_name: &str) -> String {
    let tag = format!("{:x}", stamp() % 0xffff_ffff);
    let app_id = format!("mentor-{tag}");
    let account = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO developer_accounts (id, kind, display_name, domain_token) VALUES ($1, 'individual', $2, $3)",
    )
    .bind(account)
    .bind(account_name)
    .bind(&tag)
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO developer_members (account_id, email, subject, role, status, joined_at)
         VALUES ($1, $2, $3, 'owner', 'active', now())",
    )
    .bind(account)
    .bind(&owner.email)
    .bind(owner.subject)
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO developer_apps (app_id, account_id, client_id, name, kind) VALUES ($1, $2, $3, 'Mentor', 'server')",
    )
    .bind(&app_id)
    .bind(account)
    .bind(format!("client-{app_id}"))
    .execute(&state.db)
    .await
    .unwrap();
    let input = feco_server::catalog::AppInput {
        id: app_id.clone(),
        client_id: format!("client-{app_id}"),
        name: "Mentor".into(),
        developer: account_name.into(),
        license: "MIT".into(),
        category: "Giáo dục".into(),
        summary: "Học cùng người hướng dẫn".into(),
        description: "Mô tả dài".into(),
        ..Default::default()
    };
    feco_server::catalog::upsert_app(&state.db, &state.catalog, &input)
        .await
        .unwrap();
    feco_server::catalog::release_version(
        &state.db,
        &state.catalog,
        &app_id,
        &feco_server::catalog::VersionInput {
            version: "1.0.0".into(),
            platform: "windows-x64".into(),
            size_bytes: 1_048_576,
            tuf_target: format!("{app_id}-1.0.0.zip"),
            changelog: "Bản đầu".into(),
        },
    )
    .await
    .unwrap();
    app_id
}

fn conversation_of(url: &str) -> Uuid {
    url.split("/messages/")
        .nth(1)
        .and_then(|s| s.split(['#', '?', '/']).next())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("không phải trang hội thoại: {url}"))
}

/// Spec messaging: "Nhắn từ trang app", "Không phải nhà phát triển", "Quá nhiều hội thoại mới",
/// "Người ngoài", "Tin quá dài", "Đã đọc", "Bị chặn", "Launcher gửi tin"; báo cáo hội thoại tới
/// quản trị hỗ trợ (chỉ đọc, đánh dấu đã xử lý); số chưa đọc trên thanh trên cùng.
#[tokio::test]
async fn direct_conversations_web_and_launcher_api() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let state = local_state();
    let db = &state.db;

    // Chủ tài khoản nhà phát triển của "Mentor" (không cần vai trò Lập trình viên).
    let dev = user("chu-mentor", "Chủ Mentor", true).await;
    let app_id = owned_app(&state, &dev, "Mentor Studio").await;
    let cus = user("khach-chat", "Khách Chat", true).await;

    // Scenario "Nhắn từ trang app".
    let page = get(&cus.c, &format!("/store/{app_id}")).await;
    assert!(page.contains(r#"id="message-developer""#), "{page}");
    let (status, page, url) = post(
        &cus.c,
        "/messages/start",
        &csrf_of(&page),
        &[("app_id", &app_id)],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let conv = conversation_of(&url);
    assert!(
        page.contains("Mentor Studio") && page.contains(&format!("/store/{app_id}")),
        "{page}"
    );
    let row: (String, Uuid, Uuid, Option<String>) =
        sqlx::query_as("SELECT kind, customer, developer, app_id FROM conversations WHERE id = $1")
            .bind(conv)
            .fetch_one(db)
            .await
            .unwrap();
    assert_eq!(
        row,
        (
            "direct".to_owned(),
            cus.subject,
            dev.subject,
            Some(app_id.clone())
        )
    );
    // Bấm lại thì mở hội thoại đã có.
    let page = get(&cus.c, &format!("/store/{app_id}")).await;
    let (_, _, url) = post(
        &cus.c,
        "/messages/start",
        &csrf_of(&page),
        &[("app_id", &app_id)],
    )
    .await;
    assert_eq!(conversation_of(&url), conv);
    // Chủ app không thấy nút nhắn cho chính mình.
    let page = get(&dev.c, &format!("/store/{app_id}")).await;
    assert!(!page.contains(r#"id="message-developer""#));
    // Email chưa xác minh thì không bắt đầu được.
    let unverified = user("chua-xac-minh", "Chưa Xác Minh", false).await;
    let page = get(&unverified.c, &format!("/store/{app_id}")).await;
    let (status, page, _) = post(
        &unverified.c,
        "/messages/start",
        &csrf_of(&page),
        &[("app_id", &app_id)],
    )
    .await;
    assert_eq!(status, 403);
    assert!(page.contains("Hãy xác minh email"), "{page}");

    // API cho launcher: danh sách hội thoại theo design C4.
    let cus_token = launcher_token(&cus.c).await;
    let (status, v) = api(&cus_token, "GET", "/v1/chat/conversations", None).await;
    assert_eq!(status, 200, "{v}");
    let c = v["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == conv.to_string())
        .expect("hội thoại có trong danh sách")
        .clone();
    assert_eq!(c["kind"], "direct");
    assert_eq!(c["role"], "customer");
    assert_eq!(c["peer_name"], "Mentor Studio");
    assert_eq!(c["title"], "Mentor");
    assert_eq!(c["app_id"], app_id.as_str());
    assert_eq!(c["project_id"], Value::Null);
    assert_eq!(c["blocked"], false);
    assert_eq!(c["last_message"], Value::Null);
    assert_eq!(c["unread"], 0);
    assert_eq!(c["web_url"], format!("{BASE}/messages/{conv}"));
    assert!(time::OffsetDateTime::parse(
        c["last_message_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339
    )
    .is_ok());
    // Mở lại qua API: 200 khi đã có.
    let (status, v) = api(
        &cus_token,
        "POST",
        "/v1/chat/conversations",
        Some(json!({"app_id": app_id})),
    )
    .await;
    assert_eq!(
        (status, v["id"].as_str()),
        (200, Some(conv.to_string().as_str()))
    );

    // Scenario "Không phải nhà phát triển".
    let plain = user("nguoi-thuong", "Người Thường", true).await;
    let (status, v) = api(
        &cus_token,
        "POST",
        "/v1/chat/conversations",
        Some(json!({"developer": plain.subject})),
    )
    .await;
    assert_eq!(
        (status, v["error"].as_str()),
        (422, Some("chat_not_developer"))
    );
    let (status, v) = api(
        &cus_token,
        "POST",
        "/v1/chat/conversations",
        Some(json!({"developer": cus.subject})),
    )
    .await;
    assert_eq!((status, v["error"].as_str()), (422, Some("chat_self")));

    // Scenario "Đã đọc": nhà phát triển có 2 tin chưa đọc, mở hội thoại thì unread giảm 2.
    for body in ["Tin thứ nhất", "Tin thứ hai"] {
        let (status, v) = api(
            &cus_token,
            "POST",
            &format!("/v1/chat/conversations/{conv}/messages"),
            Some(json!({ "body": body })),
        )
        .await;
        assert_eq!(status, 201, "{v}");
    }
    let dev_token = launcher_token(&dev.c).await;
    let before = unread(&dev_token).await;
    assert_eq!(before, 2);
    let list = get(&dev.c, "/messages").await;
    assert!(
        list.contains(r#"<span class="badge brand" id="nav-unread">2</span>"#),
        "{list}"
    );
    assert!(list.contains("Khách Chat") && list.contains("Tin thứ hai"));
    let page = get(&dev.c, &format!("/messages/{conv}")).await;
    let first = page.find("Tin thứ nhất").expect("có tin thứ nhất");
    assert!(page[first..].contains("Tin thứ hai"), "đúng thứ tự");
    assert_eq!(unread(&dev_token).await, before - 2);
    assert!(!get(&dev.c, "/messages").await.contains("nav-unread"));

    // Scenario "Launcher gửi tin" (token của F-Launcher di động cũng được nhận).
    let mobile = mobile_token(&cus.c).await;
    let (status, m) = api(
        &mobile,
        "POST",
        &format!("/v1/chat/conversations/{conv}/messages"),
        Some(json!({"body": "Chào anh"})),
    )
    .await;
    assert_eq!(status, 201, "{m}");
    assert_eq!(m["mine"], true);
    assert_eq!(m["side"], "customer");
    assert_eq!(m["body"], "Chào anh");
    assert_eq!(m["attachment"], Value::Null);
    assert!(m["id"].as_i64().unwrap() > 0 && m["at"].is_string());
    let page = get(&dev.c, &format!("/messages/{conv}")).await;
    assert!(page.contains("Chào anh"), "{page}");
    // Phía nhà phát triển: tin của khách hàng có mine = false; `after` và `limit`.
    let (status, v) = api(
        &dev_token,
        "GET",
        &format!("/v1/chat/conversations/{conv}/messages"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    let msgs = v["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 3);
    assert!(msgs.iter().all(|m| m["mine"] == false));
    let first_id = msgs[0]["id"].as_i64().unwrap();
    let (_, v) = api(
        &dev_token,
        "GET",
        &format!("/v1/chat/conversations/{conv}/messages?after={first_id}&limit=1"),
        None,
    )
    .await;
    assert_eq!(v["messages"].as_array().unwrap().len(), 1);
    assert_eq!(v["messages"][0]["body"], "Tin thứ hai");
    // Nhà phát triển trả lời; khách hàng đánh dấu đã đọc bằng `POST …/read`.
    let (status, reply) = api(
        &dev_token,
        "POST",
        &format!("/v1/chat/conversations/{conv}/messages"),
        Some(json!({"body": "Chào bạn, mình giúp được gì?"})),
    )
    .await;
    assert_eq!((status, reply["side"].as_str()), (201, Some("developer")));
    assert_eq!(unread(&cus_token).await, 1);
    let (status, _) = api(
        &cus_token,
        "POST",
        &format!("/v1/chat/conversations/{conv}/read"),
        Some(json!({"up_to": reply["id"]})),
    )
    .await;
    assert_eq!(status, 204);
    assert_eq!(unread(&cus_token).await, 0);

    // Scenario "Người ngoài".
    let outsider = launcher_token(&plain.c).await;
    let (status, v) = api(
        &outsider,
        "GET",
        &format!("/v1/chat/conversations/{conv}/messages"),
        None,
    )
    .await;
    assert_eq!((status, v["error"].as_str()), (403, Some("chat_forbidden")));
    let (status, _) = get_status(&plain.c, &format!("/messages/{conv}")).await;
    assert_eq!(status, 403);
    let (status, v) = api(
        &outsider,
        "GET",
        &format!("/v1/chat/conversations/{}/messages", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!((status, v["error"].as_str()), (404, Some("chat_not_found")));
    let (status, _) = api(&"sai".repeat(10), "GET", "/v1/chat/unread", None).await;
    assert_eq!(status, 401);

    // Scenario "Tin quá dài".
    let count = |db: store::Db| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM chat_messages WHERE conversation_id = $1",
        )
        .bind(conv)
        .fetch_one(&db)
        .await
        .unwrap()
    };
    let n = count(db.clone()).await;
    let (status, v) = api(
        &cus_token,
        "POST",
        &format!("/v1/chat/conversations/{conv}/messages"),
        Some(json!({"body": "a".repeat(5_001)})),
    )
    .await;
    assert_eq!((status, v["error"].as_str()), (422, Some("chat_too_long")));
    let (status, v) = api(
        &cus_token,
        "POST",
        &format!("/v1/chat/conversations/{conv}/messages"),
        Some(json!({"body": "   "})),
    )
    .await;
    assert_eq!((status, v["error"].as_str()), (422, Some("chat_empty")));
    assert_eq!(count(db.clone()).await, n, "không lưu tin");

    // Scenario "Bị chặn".
    let page = get(&cus.c, &format!("/messages/{conv}")).await;
    assert!(!page.contains("block-form"), "khách hàng không chặn được");
    let page = get(&dev.c, &format!("/messages/{conv}")).await;
    let (status, page, _) = post(
        &dev.c,
        &format!("/messages/{conv}/block"),
        &csrf_of(&page),
        &[],
    )
    .await;
    assert_eq!(status, 200);
    assert!(page.contains("Bạn đã chặn hội thoại này"), "{page}");
    let (status, v) = api(
        &cus_token,
        "POST",
        &format!("/v1/chat/conversations/{conv}/messages"),
        Some(json!({"body": "Anh ơi?"})),
    )
    .await;
    assert_eq!((status, v["error"].as_str()), (409, Some("chat_blocked")));
    let page = get(&cus.c, &format!("/messages/{conv}")).await;
    assert!(
        page.contains("Nhà phát triển đã chặn hội thoại này") && !page.contains("chat-form"),
        "{page}"
    );
    let (_, v) = api(&cus_token, "GET", "/v1/chat/conversations", None).await;
    assert!(v["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == conv.to_string() && c["blocked"] == true));
    let page = get(&dev.c, &format!("/messages/{conv}")).await;
    post(
        &dev.c,
        &format!("/messages/{conv}/unblock"),
        &csrf_of(&page),
        &[],
    )
    .await;
    let (status, _) = api(
        &cus_token,
        "POST",
        &format!("/v1/chat/conversations/{conv}/messages"),
        Some(json!({"body": "Cảm ơn anh"})),
    )
    .await;
    assert_eq!(status, 201);

    // Báo cáo hội thoại; quản trị hỗ trợ xem ở chế độ chỉ đọc rồi đánh dấu đã xử lý.
    let page = get(&cus.c, &format!("/messages/{conv}")).await;
    let (status, page, _) = post(
        &cus.c,
        &format!("/messages/{conv}/report"),
        &csrf_of(&page),
        &[("reason", "Quảng cáo không mời")],
    )
    .await;
    assert_eq!(status, 200);
    assert!(page.contains("Đã gửi báo cáo"), "{page}");
    let report: i64 = sqlx::query_scalar("SELECT id FROM chat_reports WHERE conversation_id = $1")
        .bind(conv)
        .fetch_one(db)
        .await
        .unwrap();
    let support = user("ho-tro-chat", "Hỗ Trợ", true).await;
    feco_server::admin::set_member(
        &state,
        "e2e",
        &support.email,
        &[feco_server::admin::Role::Support],
    )
    .await
    .unwrap();
    let page = get(&support.c, "/admin/chat-reports").await;
    assert!(page.contains("Quảng cáo không mời"), "{page}");
    let page = get(&support.c, &format!("/admin/chat-reports/{report}")).await;
    assert!(
        page.contains("Chào anh") && page.contains("Khách Chat ↔ Mentor Studio"),
        "{page}"
    );
    assert!(!page.contains("chat-form") && !page.contains("/messages/"));
    let (status, page, _) = post(
        &support.c,
        &format!("/admin/chat-reports/{report}/handled"),
        &csrf_of(&page),
        &[],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let handled: Option<String> =
        sqlx::query_scalar("SELECT handled_by FROM chat_reports WHERE id = $1")
            .bind(report)
            .fetch_one(db)
            .await
            .unwrap();
    assert_eq!(handled.as_deref(), Some(support.email.as_str()));
    // Người không có vai trò hỗ trợ không vào được.
    let (status, _) = get_status(&cus.c, "/admin/chat-reports").await;
    assert_eq!(status, 403);

    // Scenario "Quá nhiều hội thoại mới": 20 hội thoại trực tiếp trong 24 giờ, cái thứ 21 bị chặn.
    let dev2 = user("lap-trinh-vien-2", "Lập Trình Viên Hai", true).await;
    set_role(&state, &dev2, feco_server::roles::PlatformRole::Developer).await;
    let spammer = user("nhieu-hoi-thoai", "Nhiều Hội Thoại", true).await;
    sqlx::query(
        "INSERT INTO conversations (kind, customer, developer)
         SELECT 'direct', $1, gen_random_uuid() FROM generate_series(1, 20)",
    )
    .bind(spammer.subject)
    .execute(db)
    .await
    .unwrap();
    let spam_token = launcher_token(&spammer.c).await;
    let (status, v) = api(
        &spam_token,
        "POST",
        "/v1/chat/conversations",
        Some(json!({"developer": dev2.subject})),
    )
    .await;
    assert_eq!(
        (status, v["error"].as_str()),
        (429, Some("chat_rate_limited"))
    );
    sqlx::query(
        "UPDATE conversations SET created_at = now() - interval '25 hours' WHERE customer = $1",
    )
    .bind(spammer.subject)
    .execute(db)
    .await
    .unwrap();
    let (status, v) = api(
        &spam_token,
        "POST",
        "/v1/chat/conversations",
        Some(json!({"developer": dev2.subject})),
    )
    .await;
    assert_eq!(status, 201, "{v}");
    assert_eq!(v["peer_name"], "Lập Trình Viên Hai");

    // Tối đa 30 tin mỗi phút.
    let spam_conv: Uuid = v["id"].as_str().unwrap().parse().unwrap();
    for i in 0..30 {
        feco_server::chat::send(
            &state,
            spam_conv,
            spammer.subject,
            &format!("Tin {i}"),
            None,
        )
        .await
        .unwrap();
    }
    let (status, v) = api(
        &spam_token,
        "POST",
        &format!("/v1/chat/conversations/{spam_conv}/messages"),
        Some(json!({"body": "Tin 31"})),
    )
    .await;
    assert_eq!(
        (status, v["error"].as_str()),
        (429, Some("chat_rate_limited"))
    );
}

/// Spec messaging, "Tệp có mã độc"; nút "Nhắn nhà phát triển" ở danh sách nhà phát triển trong
/// Dự án; tệp sạch tải được, tệp đang quét chưa tải được, tệp quá 20 MB bị từ chối.
#[tokio::test]
async fn attachments_are_scanned_before_download() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let state = local_state();
    let db = &state.db;
    let dev = user("dev-tep", "Dev Tệp", true).await;
    set_role(&state, &dev, feco_server::roles::PlatformRole::Developer).await;
    let tag = &dev.subject.simple().to_string()[..6];
    let display = format!("Lan Tệp {tag}");
    feco_server::projects::save_profile(
        &state,
        dev.subject,
        &dev.email,
        &display,
        "Nhận làm app",
        "Rust",
        true,
    )
    .await
    .unwrap();
    let cus = user("khach-tep", "Khách Tệp", true).await;

    // Nút "Nhắn nhà phát triển" ở danh sách nhà phát triển.
    let page = get(&cus.c, &format!("/projects/providers?q={tag}")).await;
    assert!(
        page.contains(&display) && page.contains("message-developer"),
        "{page}"
    );
    let (status, page, url) = post(
        &cus.c,
        "/messages/start",
        &csrf_of(&page),
        &[("developer", &dev.subject.to_string())],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let conv = conversation_of(&url);
    assert!(page.contains(&display));

    // Nhà phát triển gửi tệp chứa chuỗi thử EICAR.
    let page = get(&dev.c, &format!("/messages/{conv}")).await;
    let (status, page, _) = post_multipart(
        &dev.c,
        &format!("/messages/{conv}"),
        &csrf_of(&page),
        &[("body", "")],
        Some(("hoa-don.exe", feco_server::scan::eicar())),
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let att: (Uuid, String) = sqlx::query_as(
        "SELECT a.id, m.body FROM chat_attachments a JOIN chat_messages m ON m.id = a.message_id
         WHERE m.conversation_id = $1 ORDER BY m.id DESC LIMIT 1",
    )
    .bind(conv)
    .fetch_one(db)
    .await
    .unwrap();
    assert_eq!(
        att.1, "hoa-don.exe",
        "tin chỉ có tệp thì nội dung là tên tệp"
    );
    let status_of = |id: Uuid| {
        let db = db.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT scan_status FROM chat_attachments WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    for _ in 0..40 {
        if status_of(att.0).await != "pending" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(status_of(att.0).await, "infected");
    let res = cus
        .c
        .get(format!("{BASE}/messages/{conv}/files/{}", att.0))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 403);
    let v: Value = res.json().await.unwrap();
    assert_eq!(v["error"], "chat_attachment_blocked");
    let alerts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM admin_alerts WHERE kind = 'chat_malware' AND details->>'attachment' = $1",
    )
    .bind(att.0.to_string())
    .fetch_one(db)
    .await
    .unwrap();
    assert_eq!(alerts, 1);
    wait_mail(ADMIN_EMAIL, "Tệp đính kèm trong chat có mã độc").await;
    let page = get(&cus.c, &format!("/messages/{conv}")).await;
    assert!(page.contains("Có mã độc, đã chặn tải"), "{page}");
    let cus_token = launcher_token(&cus.c).await;
    let (_, v) = api(
        &cus_token,
        "GET",
        &format!("/v1/chat/conversations/{conv}/messages"),
        None,
    )
    .await;
    let m = v["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(m["attachment"]["status"], "infected");
    assert_eq!(m["attachment"]["name"], "hoa-don.exe");
    assert_eq!(
        m["attachment"]["url"],
        format!("{BASE}/messages/{conv}/files/{}", att.0)
    );

    // Tệp sạch: tải được sau khi quét; đang quét thì hiện "Đang kiểm tra" và chưa tải được.
    let pdf = b"%PDF-1.4 bao gia du an".to_vec();
    let page = get(&cus.c, &format!("/messages/{conv}")).await;
    let (status, _, _) = post_multipart(
        &cus.c,
        &format!("/messages/{conv}"),
        &csrf_of(&page),
        &[("body", "Báo giá đây ạ")],
        Some(("bao-gia.pdf", pdf.clone())),
    )
    .await;
    assert_eq!(status, 200);
    let clean: (Uuid, String) = sqlx::query_as(
        "SELECT a.id, a.name FROM chat_attachments a JOIN chat_messages m ON m.id = a.message_id
         WHERE m.conversation_id = $1 ORDER BY m.id DESC LIMIT 1",
    )
    .bind(conv)
    .fetch_one(db)
    .await
    .unwrap();
    assert_eq!(clean.1, "bao-gia.pdf");
    for _ in 0..40 {
        if status_of(clean.0).await != "pending" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(status_of(clean.0).await, "clean");
    let res = dev
        .c
        .get(format!("{BASE}/messages/{conv}/files/{}", clean.0))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert!(res.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment;"));
    assert_eq!(res.bytes().await.unwrap().to_vec(), pdf);
    sqlx::query("UPDATE chat_attachments SET scan_status = 'pending' WHERE id = $1")
        .bind(clean.0)
        .execute(db)
        .await
        .unwrap();
    let page = get(&dev.c, &format!("/messages/{conv}")).await;
    assert!(page.contains("Đang kiểm tra"), "{page}");
    let res = dev
        .c
        .get(format!("{BASE}/messages/{conv}/files/{}", clean.0))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 409);
    // Quét lại định kỳ đưa tệp về trạng thái đúng.
    sqlx::query("UPDATE chat_attachments SET scan_status = 'error' WHERE id = $1")
        .bind(clean.0)
        .execute(db)
        .await
        .unwrap();
    feco_server::chat::rescan_due(&state).await.unwrap();
    assert_eq!(status_of(clean.0).await, "clean");
    // Người ngoài không tải được.
    let outsider = user("ngoai-tep", "Người Ngoài", true).await;
    let res = outsider
        .c
        .get(format!("{BASE}/messages/{conv}/files/{}", clean.0))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 403);

    // Tệp lớn hơn 20 MB bị từ chối, không lưu tin.
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_messages WHERE conversation_id = $1")
            .bind(conv)
            .fetch_one(db)
            .await
            .unwrap();
    let page = get(&cus.c, &format!("/messages/{conv}")).await;
    let (status, page, _) = post_multipart(
        &cus.c,
        &format!("/messages/{conv}"),
        &csrf_of(&page),
        &[("body", "Tệp lớn")],
        Some(("lon.bin", vec![0u8; 20 * 1024 * 1024 + 1])),
    )
    .await;
    assert_eq!(status, 413);
    assert!(page.contains("lớn hơn 20 MB"), "{page}");
    let after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_messages WHERE conversation_id = $1")
            .bind(conv)
            .fetch_one(db)
            .await
            .unwrap();
    assert_eq!(before, after);
}

/// Đọc luồng SSE tới khi có chuỗi `needle` (tối đa `secs` giây).
async fn read_until(res: &mut reqwest::Response, buf: &mut String, needle: &str, secs: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while !buf.contains(needle) {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, res.chunk()).await {
            Ok(Ok(Some(c))) => buf.push_str(&String::from_utf8_lossy(&c)),
            Ok(Ok(None)) => panic!("luồng SSE đóng: {buf}"),
            Ok(Err(e)) => panic!("lỗi đọc SSE: {e}"),
            Err(_) => panic!("không thấy {needle:?} trong {secs} giây: {buf}"),
        }
    }
}

/// Spec messaging, "Hai trình duyệt"; spec notifications, "Gộp thông báo" và không báo khi đang
/// mở hội thoại; nhóm "Tin nhắn" trong cài đặt thông báo.
#[tokio::test]
async fn realtime_and_grouped_notifications() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let state = local_state();
    let db = &state.db;
    let dev = user("dev-sse", "Dev SSE", true).await;
    set_role(&state, &dev, feco_server::roles::PlatformRole::Developer).await;
    let cus = user("khach-sse", "Khách SSE", true).await;
    let conv = direct(&state, &cus, &dev).await;
    let link = format!("/messages/{conv}");

    // Scenario "Hai trình duyệt": cả hai mở hội thoại; khách hàng gửi "Xin chào".
    let dev_page = get(&dev.c, &link).await;
    assert!(dev_page.contains(r#"src="/messages/chat.js""#));
    let last: i64 = Regex::new(r#"data-last="(\d+)""#)
        .unwrap()
        .captures(&dev_page)
        .unwrap()[1]
        .parse()
        .unwrap();
    let mut sse = dev
        .c
        .get(format!("{BASE}/messages/events?viewing={conv}"))
        .send()
        .await
        .unwrap();
    assert_eq!(sse.status().as_u16(), 200);
    assert!(sse.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let mut buf = String::new();
    read_until(&mut sse, &mut buf, "event: ready", 10).await;
    let cus_page = get(&cus.c, &link).await;
    let (status, _, _) = post_multipart(
        &cus.c,
        &link,
        &csrf_of(&cus_page),
        &[("body", "Xin chào")],
        None,
    )
    .await;
    assert_eq!(status, 200);
    read_until(
        &mut sse,
        &mut buf,
        &format!("\"conversation\":\"{conv}\""),
        10,
    )
    .await;
    assert!(buf.contains("event: message"), "{buf}");
    let fragment = get(&dev.c, &format!("/messages/{conv}/after/{last}")).await;
    assert!(
        fragment.contains("Xin chào") && !fragment.contains("<html"),
        "{fragment}"
    );
    // Đang mở hội thoại thì không báo; tin đã hiện cũng là đã đọc.
    assert!(message_notes(db, dev.subject, &link).await.is_empty());
    let dev_token = launcher_token(&dev.c).await;
    assert_eq!(unread(&dev_token).await, 0);
    // Sự kiện chỉ tới người tham gia: hội thoại khác không lọt vào luồng của nhà phát triển.
    let other_dev = user("dev-khac", "Dev Khác", true).await;
    set_role(
        &state,
        &other_dev,
        feco_server::roles::PlatformRole::Developer,
    )
    .await;
    let other = direct(&state, &cus, &other_dev).await;
    feco_server::chat::send(&state, other, cus.subject, "Tin riêng", None)
        .await
        .unwrap();
    feco_server::chat::send(&state, conv, cus.subject, "Tin thứ hai", None)
        .await
        .unwrap();
    buf.clear();
    read_until(
        &mut sse,
        &mut buf,
        &format!("\"conversation\":\"{conv}\""),
        10,
    )
    .await;
    assert!(!buf.contains(&other.to_string()), "{buf}");
    drop(sse);
    // Không đăng nhập thì không mở được luồng sự kiện.
    let (status, _) = get_status(&app_browser(), "/messages/events").await;
    assert_eq!(status, 401);
    let js = reqwest::get(format!("{BASE}/messages/chat.js"))
        .await
        .unwrap();
    assert!(js.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/javascript"));
    assert!(js.text().await.unwrap().contains("EventSource"));

    // Scenario "Gộp thông báo": nhà phát triển chưa mở hội thoại, khách hàng gửi 5 tin.
    let dev5 = user("dev-gop", "Dev Gộp", true).await;
    set_role(&state, &dev5, feco_server::roles::PlatformRole::Developer).await;
    let cus5 = user("khach-gop", "Khách Gộp", true).await;
    let conv5 = direct(&state, &cus5, &dev5).await;
    let token5 = launcher_token(&cus5.c).await;
    for i in 1..=5 {
        let (status, _) = api(
            &token5,
            "POST",
            &format!("/v1/chat/conversations/{conv5}/messages"),
            Some(json!({ "body": format!("Tin số {i}") })),
        )
        .await;
        assert_eq!(status, 201);
    }
    let link5 = format!("/messages/{conv5}");
    let notes = message_notes(db, dev5.subject, &link5).await;
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].0, "Tin nhắn mới từ Khách Gộp");
    let (_, text) = wait_mail(&dev5.email, "Tin nhắn mới từ Khách Gộp").await;
    assert!(text.contains(&link5) && !text.contains("Tin số"), "{text}");
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(mail_count(&dev5.email, "Tin nhắn mới từ").await, 1);
    // Sau 15 phút mà vẫn còn tin chưa đọc thì báo lại.
    sqlx::query(
        "UPDATE conversations SET developer_notified_at = now() - interval '16 minutes' WHERE id = $1",
    )
    .bind(conv5)
    .execute(db)
    .await
    .unwrap();
    api(
        &token5,
        "POST",
        &format!("/v1/chat/conversations/{conv5}/messages"),
        Some(json!({"body": "Tin số 6"})),
    )
    .await;
    assert_eq!(message_notes(db, dev5.subject, &link5).await.len(), 2);

    // Nhóm "Tin nhắn" trong cài đặt: bật mặc định, tắt được (tắt thì không gửi email).
    let page = get(&dev5.c, "/notifications").await;
    assert!(
        page.contains(r#"value="messages" id="n-messages" checked"#),
        "{page}"
    );
    let fields: Vec<(&str, &str)> = vec![("on", "app_updates"), ("on", "recommendations")];
    let (status, _, _) = post(&dev5.c, "/notifications/settings", &csrf_of(&page), &fields).await;
    assert_eq!(status, 200);
    let page = get(&dev5.c, "/notifications").await;
    assert!(
        page.contains(r#"value="messages" id="n-messages">"#),
        "{page}"
    );
    assert!(
        !feco_server::notifications::enabled(
            &state,
            dev5.subject,
            feco_server::notifications::Category::Messages
        )
        .await
    );
}

/// Spec messaging, "Tin cũ được giữ": migration chuyển `project_messages` sang hội thoại dự án.
/// Chạy trên một cơ sở dữ liệu tạm: dựng lược đồ tới 0018, ghi dự án có ba tin, rồi chạy 0019.
#[tokio::test]
async fn migration_keeps_old_project_messages() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let name = format!("feco_chat_mig_{}", std::process::id());
    drop_database(&name).await;
    ensure_database(&name).await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url(&name))
        .await
        .unwrap();
    let mut old = sqlx::migrate::Migrator::new(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations"
    )))
    .await
    .unwrap();
    let before: Vec<_> = old
        .migrations
        .iter()
        .filter(|m| m.version <= 18)
        .cloned()
        .collect();
    old.migrations = before.into();
    old.run(&pool).await.unwrap();
    let (project, customer, provider) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    sqlx::query(
        "INSERT INTO projects (id, code, customer, customer_email, provider, provider_email, title, description)
         VALUES ($1, 'DA-000001', $2, 'khach@example.com', $3, 'lan@example.com', 'App cũ', 'Dự án trước 1.12.0')",
    )
    .bind(project)
    .bind(customer)
    .bind(provider)
    .execute(&pool)
    .await
    .unwrap();
    for (side, body) in [
        ("customer", "Một: khách hỏi"),
        ("provider", "Hai: người nhận việc trả lời"),
        ("admin", "Ba: quản trị ghi chú"),
    ] {
        sqlx::query("INSERT INTO project_messages (project_id, side, body) VALUES ($1, $2, $3)")
            .bind(project)
            .bind(side)
            .bind(body)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO notification_preferences (subject, category, enabled) VALUES ($1, 'newsletter', true)",
    )
    .bind(customer)
    .execute(&pool)
    .await
    .unwrap();
    store::MIGRATOR.run(&pool).await.unwrap();

    let c = feco_server::chat::of_project(&pool, project)
        .await
        .unwrap()
        .expect("dự án có hội thoại");
    assert_eq!((c.customer, c.developer), (customer, provider));
    assert_eq!(c.customer_name, "khach@example.com");
    let msgs = feco_server::chat::messages(&pool, c.id, 0, 200)
        .await
        .unwrap();
    let got: Vec<(&str, &str, Option<Uuid>)> = msgs
        .iter()
        .map(|m| (m.side.as_str(), m.body.as_str(), m.sender))
        .collect();
    assert_eq!(
        got,
        [
            ("customer", "Một: khách hỏi", Some(customer)),
            ("developer", "Hai: người nhận việc trả lời", Some(provider)),
            ("admin", "Ba: quản trị ghi chú", None),
        ]
    );
    // Số chưa đọc của dự án là 0 cho cả hai bên.
    for who in [customer, provider] {
        assert_eq!(
            feco_server::chat::unread_total(&pool, who).await.unwrap(),
            0
        );
        let s = feco_server::chat::summary(&pool, c.id, who)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(s.unread, 0);
        assert_eq!(s.title(c.role_of(who).unwrap()), "App cũ");
    }
    // Nhóm thông báo "messages" được nhận trong bảng tùy chọn.
    sqlx::query(
        "INSERT INTO notification_preferences (subject, category, enabled) VALUES ($1, 'messages', false)",
    )
    .bind(customer)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    drop_database(&name).await;
}

/// Spec messaging, "Quản trị giải quyết tranh chấp"; spec notifications, "Không lộ nội dung"; ô
/// chat trong trang dự án (form không JavaScript chuyển về trang dự án).
#[tokio::test]
async fn project_conversation_and_dispute() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    use feco_server::{projects, roles::PlatformRole, wallet};
    let state = local_state();
    let db = &state.db;
    wallet::set_fees(&state, "e2e", wallet::Fees::default())
        .await
        .unwrap();
    let pro = user("nhan-viec-chat", "Lan Nhận Việc", true).await;
    set_role(&state, &pro, PlatformRole::Developer).await;
    projects::save_profile(
        &state,
        pro.subject,
        &pro.email,
        "Lan Dự Án",
        "App di động",
        "Flutter",
        true,
    )
    .await
    .unwrap();
    let cus = user("khach-du-an", "Minh Khách", true).await;
    set_role(&state, &cus, PlatformRole::Customer).await;
    let pid = projects::create(
        &state,
        cus.subject,
        &cus.email,
        pro.subject,
        "App đặt lịch",
        "Mô tả đủ dài cho dự án đặt lịch",
        None,
        None,
    )
    .await
    .unwrap();
    let c = feco_server::chat::of_project(db, pid)
        .await
        .unwrap()
        .expect("dự án mới có hội thoại");
    assert_eq!(
        (c.customer_name.as_str(), c.developer_name.as_str()),
        ("Minh Khách", "Lan Dự Án")
    );
    let link = format!("/messages/{}", c.id);

    // Scenario "Không lộ nội dung": khách hàng gửi mật khẩu trong ô chat của trang dự án.
    let page = get(&cus.c, &format!("/projects/{pid}")).await;
    assert!(
        page.contains(&format!(r#"action="/messages/{}""#, c.id)),
        "{page}"
    );
    let (status, page, url) = post_multipart(
        &cus.c,
        &link,
        &csrf_of(&page),
        &[("body", "Mật khẩu máy chủ là abc123"), ("back", "project")],
        None,
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert!(url.contains(&format!("/projects/{pid}")), "{url}");
    assert!(page.contains("Mật khẩu máy chủ là abc123"));
    let notes = message_notes(db, pro.subject, &link).await;
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].0, "Tin nhắn mới từ Minh Khách");
    assert!(!notes[0].0.contains("abc123") && !notes[0].1.contains("abc123"));
    let (subject, text) = wait_mail(&pro.email, "Tin nhắn mới từ Minh Khách").await;
    assert!(
        !subject.contains("abc123") && !text.contains("abc123"),
        "{text}"
    );
    assert!(text.contains(&link), "{text}");
    // Tin nhắn qua hành động cũ của trang dự án cũng vào hội thoại và không lộ nội dung.
    let page = get(&pro.c, &format!("/projects/{pid}")).await;
    let (status, page, _) = post(
        &pro.c,
        &format!("/projects/{pid}/message"),
        &csrf_of(&page),
        &[("body", "Mã OTP là 998877")],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let all: Vec<String> =
        sqlx::query_scalar("SELECT title || ' ' || body FROM notifications WHERE subject = $1")
            .bind(cus.subject)
            .fetch_all(db)
            .await
            .unwrap();
    assert!(all.iter().all(|n| !n.contains("998877")), "{all:?}");

    // Dự án vào tranh chấp, quản lý dự án xử lý trên bảng quản trị.
    projects::offer(&state, pid, pro.subject, 300, 10, "")
        .await
        .unwrap();
    let offer: Uuid = sqlx::query_scalar(
        "SELECT id FROM project_offers WHERE project_id = $1 AND status = 'open'",
    )
    .bind(pid)
    .fetch_one(db)
    .await
    .unwrap();
    projects::accept_offer(&state, pid, cus.subject, offer)
        .await
        .unwrap();
    let t = wallet::create_topup(&state, cus.subject, &cus.email, 300)
        .await
        .unwrap();
    feco_server::payments::record_manual(&state, t.request, time::OffsetDateTime::now_utc())
        .await
        .unwrap();
    projects::fund(&state, pid, cus.subject).await.unwrap();
    projects::dispute(&state, pid, cus.subject, "Chậm tiến độ")
        .await
        .unwrap();
    let pm = user("quan-ly-du-an-chat", "Quản Lý Dự Án", true).await;
    feco_server::admin::set_member(
        &state,
        "e2e",
        &pm.email,
        &[feco_server::admin::Role::ProjectManager],
    )
    .await
    .unwrap();
    let page = get(&pm.c, &format!("/admin/projects/{pid}")).await;
    assert!(
        page.contains("resolve-form") && page.contains("Chậm tiến độ"),
        "{page}"
    );
    let (status, page, _) = post(
        &pm.c,
        &format!("/admin/projects/{pid}/resolve"),
        &csrf_of(&page),
        &[
            ("mode", "refund"),
            ("note", "Quyết định: hoàn tiền cho khách"),
        ],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    // Scenario "Quản trị giải quyết tranh chấp": cả hai bên thấy tin của quản trị.
    for (who, token) in [
        (&cus, launcher_token(&cus.c).await),
        (&pro, launcher_token(&pro.c).await),
    ] {
        let page = get(&who.c, &format!("/projects/{pid}")).await;
        let i = page
            .find("Quyết định: hoàn tiền cho khách")
            .expect("thấy tin quản trị");
        assert!(page[..i].contains(r#"data-side="admin""#), "{page}");
        let (_, v) = api(
            &token,
            "GET",
            &format!("/v1/chat/conversations/{}/messages", c.id),
            None,
        )
        .await;
        assert!(v["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["side"] == "admin"
                && m["body"] == "Quyết định: hoàn tiền cho khách"
                && m["mine"] == false));
        let (_, v) = api(&token, "GET", "/v1/chat/conversations", None).await;
        let conv = v["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == c.id.to_string())
            .unwrap()
            .clone();
        assert_eq!(conv["kind"], "project");
        assert_eq!(conv["project_id"], pid.to_string());
        assert_eq!(conv["title"], "App đặt lịch");
    }
    // Quản trị dự án nhắn thêm trong hội thoại dự án; hai bên nhận thông báo không có nội dung.
    let page = get(&pm.c, &format!("/admin/projects/{pid}")).await;
    let (status, page, _) = post(
        &pm.c,
        &format!("/admin/projects/{pid}/message"),
        &csrf_of(&page),
        &[("body", "Tranh chấp đã đóng, cảm ơn hai bên")],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert!(page.contains("Tranh chấp đã đóng, cảm ơn hai bên"));
    let side: (String, Option<Uuid>) = sqlx::query_as(
        "SELECT side, sender FROM chat_messages WHERE conversation_id = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(c.id)
    .fetch_one(db)
    .await
    .unwrap();
    assert_eq!(side, ("admin".to_owned(), Some(pm.subject)));
    let page = get(&pro.c, &link).await;
    assert!(page.contains("Tranh chấp đã đóng, cảm ơn hai bên"));
    // Người ngoài dự án không gửi được vào hội thoại dự án qua trang quản trị.
    let (status, _) = get_status(&cus.c, &format!("/admin/projects/{pid}")).await;
    assert_eq!(status, 403);
}
