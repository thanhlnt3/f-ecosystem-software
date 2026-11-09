//! Kiểm thử đầu-cuối với Ory Kratos và Mailpit thật (tasks.md 2.1, 2.2).
//!
//! Chỉ chạy khi đặt `FECO_E2E=1`, với Kratos ở 127.0.0.1:4433/4434 và Mailpit ở
//! 127.0.0.1:1025/8025 (docker compose trong `infra/`, hoặc `scripts/dev-local.sh`).
//! feco-server được chạy ngay trong test ở 127.0.0.1:8080, vì Kratos chuyển hướng
//! trình duyệt về địa chỉ này.
//!
//! Mỗi bước ứng với một scenario trong specs/account-identity/spec.md.

use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use feco_config::Config;
use feco_server::{account, kratos::KratosClient, router, store, AppState};
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value;

const BASE: &str = "http://127.0.0.1:8080";
const MAILPIT: &str = "http://127.0.0.1:8025";
const UA_WINDOWS_CHROME: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";
const UA_LINUX_FIREFOX: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0";

/// Kratos dev dùng SQLite trong bộ nhớ (`DSN=memory`), vốn báo "database table is locked"
/// khi nhiều luồng cùng ghi. Các test E2E vì vậy chạy lần lượt qua khóa này.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Client ID của F-Launcher, tạo trong Hydra khi khởi động máy chủ kiểm thử.
static LAUNCHER: OnceLock<String> = OnceLock::new();

/// Kiểm thử E2E chỉ chạy khi đặt FECO_E2E=1. Lần đầu gọi sẽ dựng feco-server ở cổng
/// 8080 trên một luồng riêng, dùng chung cho mọi test trong file.
fn e2e_ready() -> bool {
    if std::env::var("FECO_E2E").is_err() {
        eprintln!("bỏ qua: đặt FECO_E2E=1 và chạy Kratos + Mailpit để kiểm thử đầu-cuối");
        return false;
    }
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "warn".into()),
            )
            .try_init();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let launcher = create_launcher().await;
                LAUNCHER.set(launcher.clone()).unwrap();
                let vars: HashMap<String, String> = [
                    ("FECO_ENV", "dev"),
                    ("FECO_LAUNCHER_CLIENT_ID", launcher.as_str()),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
                let config = Config::from_vars(&vars).unwrap();
                let state = std::sync::Arc::new(AppState::from_config(&config).unwrap());
                store::MIGRATOR
                    .run(&state.db)
                    .await
                    .expect("chạy migration");
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

/// AppState riêng cho runtime của từng test (pool kết nối CSDL gắn với runtime tạo ra nó).
pub fn local_state() -> AppState {
    let vars: HashMap<String, String> = [("FECO_ENV", "dev")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    AppState::from_config(&Config::from_vars(&vars).unwrap()).unwrap()
}

fn kratos() -> KratosClient {
    KratosClient::new(
        url::Url::parse("http://127.0.0.1:4433/").unwrap(),
        url::Url::parse("http://127.0.0.1:4434/").unwrap(),
    )
}

fn browser() -> reqwest::Client {
    browser_with(UA_WINDOWS_CHROME, "vi-VN")
}

fn browser_with(user_agent: &str, accept_language: &str) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "accept-language",
        reqwest::header::HeaderValue::from_str(accept_language).unwrap(),
    );
    reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(user_agent)
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

fn unique(prefix: &str) -> (String, String) {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (
        format!("{prefix}-{stamp}@example.com"),
        // Khác hẳn email, vì Kratos từ chối mật khẩu quá giống định danh.
        format!("Tra-sua-tran-chau-{:x}-Q", stamp.wrapping_mul(7919)),
    )
}

/// Đăng ký rồi đăng nhập trong một trình duyệt mới.
async fn register_and_login(email: &str, password: &str, ua: &str) -> reqwest::Client {
    let c = browser_with(ua, "vi-VN");
    let form = parse_form(&get(&c, "/registration").await);
    let page = submit(
        &c,
        &form,
        &[
            ("traits.email", email),
            ("password", password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(page.contains("Hãy kiểm tra email"), "{page}");
    let home = login(&c, email, password).await;
    assert!(home.contains(email), "{home}");
    c
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

/// Biểu mẫu đầu tiên trên trang: địa chỉ gửi và các trường ẩn.
struct Form {
    action: String,
    fields: Vec<(String, String)>,
}

fn parse_form(html: &str) -> Form {
    parse_form_where(html, "form")
}

/// Biểu mẫu của một nhóm phương thức (thuộc tính `data-group`).
fn parse_group_form(html: &str, group: &str) -> Form {
    parse_form_where(html, &format!("form[data-group=\"{group}\"]"))
}

fn parse_form_where(html: &str, selector: &str) -> Form {
    let doc = Html::parse_document(html);
    let form = doc
        .select(&Selector::parse(selector).unwrap())
        .next()
        .unwrap_or_else(|| panic!("trang không có biểu mẫu {selector}:\n{html}"));
    let action = form.value().attr("action").unwrap().to_owned();
    let fields = form
        .select(&Selector::parse("input[type=hidden]").unwrap())
        .map(|i| {
            (
                i.value().attr("name").unwrap().to_owned(),
                i.value().attr("value").unwrap_or_default().to_owned(),
            )
        })
        .collect();
    Form { action, fields }
}

async fn get(c: &reqwest::Client, path: &str) -> String {
    let url = if path.starts_with("http") {
        path.to_owned()
    } else {
        format!("{BASE}{path}")
    };
    c.get(url).send().await.unwrap().text().await.unwrap()
}

async fn submit(c: &reqwest::Client, form: &Form, extra: &[(&str, &str)]) -> String {
    let mut pairs: Vec<(String, String)> = form.fields.clone();
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

/// Chờ email gửi tới `to` có tiêu đề chứa `subject`, trả về nội dung chữ.
async fn wait_mail(to: &str, subject: &str) -> String {
    let http = reqwest::Client::new();
    for _ in 0..60 {
        let list: Value = http
            .get(format!("{MAILPIT}/api/v1/messages"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let found = list["messages"].as_array().into_iter().flatten().find(|m| {
            m["To"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| t["Address"] == to)
                && m["Subject"].as_str().is_some_and(|s| s.contains(subject))
        });
        if let Some(m) = found {
            let id = m["ID"].as_str().unwrap();
            let msg: Value = http
                .get(format!("{MAILPIT}/api/v1/message/{id}"))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            return msg["Text"].as_str().unwrap_or_default().to_owned();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("không thấy email tới {to} với tiêu đề chứa {subject:?}");
}

#[tokio::test]
async fn registration_verification_login_and_throttling() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("e2e");

    // --- Scenario "Mật khẩu đã bị lộ"
    let lan = browser();
    let form = parse_form(&get(&lan, "/registration").await);
    let page = submit(
        &lan,
        &form,
        &[
            ("traits.email", &email),
            ("password", "Password123!"),
            ("method", "password"),
        ],
    )
    .await;
    assert!(
        page.contains("Mật khẩu này đã xuất hiện trong các vụ rò rỉ dữ liệu"),
        "{page}"
    );

    // --- Scenario "Đăng ký thành công"
    let form = parse_form(&page);
    let registered = submit(
        &lan,
        &form,
        &[
            ("traits.email", &email),
            ("traits.name", "Lan Nguyễn"),
            ("traits.locale", "vi"),
            ("password", &password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(registered.contains("Hãy kiểm tra email"), "{registered}");
    let verification_mail = wait_mail(&email, "Xác minh email").await;

    // --- Scenario "Email đã được dùng": cùng thông báo, và chủ tài khoản nhận email.
    let attacker = browser();
    let form = parse_form(&get(&attacker, "/registration").await);
    let duplicate = submit(
        &attacker,
        &form,
        &[
            ("traits.email", &email),
            ("password", "mot-mat-khau-khac-2026-Z"),
            ("method", "password"),
        ],
    )
    .await;
    let message = |html: &str| {
        let doc = Html::parse_document(html);
        doc.select(&Selector::parse(".msg").unwrap())
            .map(|e| e.text().collect::<String>())
            .collect::<Vec<_>>()
    };
    assert_eq!(message(&duplicate), message(&registered));
    wait_mail(&email, "Có người thử đăng ký").await;

    // --- Đăng nhập khi chưa xác minh: được, nhưng trang chủ báo chưa xác minh.
    let form = parse_form(&get(&lan, "/login").await);
    let home = submit(
        &lan,
        &form,
        &[
            ("identifier", &email),
            ("password", &password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(
        home.contains(&email) && home.contains("Email chưa xác minh"),
        "{home}"
    );

    // --- Mở liên kết xác minh trong email.
    let link = Regex::new(r"https?://\S+/self-service/verification\?\S+")
        .unwrap()
        .find(&verification_mail)
        .expect("email có liên kết xác minh")
        .as_str()
        .to_owned();
    let verified = get(&lan, &link).await;
    assert!(
        verified.contains("Bạn đã xác minh email thành công"),
        "{verified}"
    );
    let home = get(&lan, "/").await;
    assert!(home.contains("Email đã xác minh"), "{home}");

    // --- Scenario "Sai nhiều lần" (từ một trình duyệt khác).
    let bot = browser();
    let mut page = get(&bot, "/login").await;
    for i in 1..=5 {
        let form = parse_form(&page);
        page = submit(
            &bot,
            &form,
            &[
                ("identifier", &email),
                ("password", "sai-mat-khau-hoan-toan"),
                ("method", "password"),
            ],
        )
        .await;
        assert!(
            page.contains("Email hoặc mật khẩu không đúng"),
            "lần {i}: {page}"
        );
        assert_eq!(
            page.contains("name=\"feco_challenge\""),
            i == 5,
            "thử thách phải xuất hiện từ lần sai thứ 5 (lần {i})"
        );
    }
    let alert = wait_mail(&email, "Cảnh báo đăng nhập sai").await;
    assert!(alert.contains("5 lần đăng nhập sai"), "{alert}");

    // Thiếu thử thách: không được chuyển tới Kratos, dù mật khẩu đúng.
    let form = parse_form(&page);
    page = submit(
        &bot,
        &form,
        &[
            ("identifier", &email),
            ("password", &password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(page.contains("không phải robot"), "{page}");
    assert!(!page.contains("Xin chào"), "{page}");

    // Có thử thách và mật khẩu đúng: đăng nhập được (không khóa cứng tài khoản).
    let form = parse_form(&page);
    let home = submit(
        &bot,
        &form,
        &[
            ("identifier", &email),
            ("password", &password),
            ("method", "password"),
            ("feco_challenge", "pass"),
        ],
    )
    .await;
    assert!(home.contains("Xin chào") && home.contains(&email), "{home}");
}

/// Spec platform-support, "Ngôn ngữ": giao diện theo trình duyệt và đổi được.
#[tokio::test]
async fn english_ui_and_language_switch() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let c = browser_with(UA_WINDOWS_CHROME, "en-US,en;q=0.9");
    let page = get(&c, "/registration").await;
    assert!(page.contains("Create your F-Eco account"), "{page}");
    assert!(
        page.contains(">Password<") && page.contains(">Sign up<"),
        "{page}"
    );
    assert!(get(&c, "/").await.contains("Welcome to F-Eco"));

    get(&c, "/lang/vi").await;
    let page = get(&c, "/registration").await;
    assert!(page.contains("Đăng ký tài khoản F-Eco"), "{page}");
    assert!(page.contains(">Mật khẩu<"), "{page}");
}

fn other_session_id(sessions_page: &str) -> String {
    let doc = Html::parse_document(sessions_page);
    doc.select(&Selector::parse("tr[data-session]:not([data-current])").unwrap())
        .next()
        .unwrap_or_else(|| panic!("không có thiết bị khác:\n{sessions_page}"))
        .value()
        .attr("data-session")
        .unwrap()
        .to_owned()
}

/// Spec account-identity, "Phiên đăng nhập": danh sách thiết bị, đăng xuất từ xa.
#[tokio::test]
async fn device_list_and_remote_logout() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("sess");
    let laptop = register_and_login(&email, &password, UA_WINDOWS_CHROME).await;
    let office = browser_with(UA_LINUX_FIREFOX, "vi-VN");
    assert!(login(&office, &email, &password).await.contains(&email));

    let page = get(&laptop, "/sessions").await;
    assert!(
        page.contains("Chrome · Windows") && page.contains("Thiết bị này"),
        "{page}"
    );
    assert!(page.contains("Firefox · Linux"), "{page}");

    // Scenario "Đăng xuất thiết bị khác".
    let office_id = other_session_id(&page);
    let form = Form {
        action: format!("/sessions/{office_id}/revoke"),
        fields: parse_form_where(&page, "tr[data-session] form").fields,
    };
    let page = submit(&laptop, &form, &[]).await;
    assert!(page.contains("Đã đăng xuất thiết bị."), "{page}");
    assert!(
        get(&office, "/").await.contains("Chào mừng tới F-Eco"),
        "máy văn phòng phải bị đăng xuất"
    );
    assert!(
        get(&laptop, "/").await.contains(&email),
        "laptop vẫn đăng nhập"
    );

    // Đăng xuất mọi thiết bị khác.
    let phone = browser_with(UA_LINUX_FIREFOX, "vi-VN");
    assert!(login(&phone, &email, &password).await.contains(&email));
    let page = get(&laptop, "/sessions").await;
    let form = parse_form_where(&page, "form[action=\"/sessions/revoke-others\"]");
    let page = submit(&laptop, &form, &[]).await;
    assert!(page.contains("Đã đăng xuất mọi thiết bị khác."), "{page}");
    assert!(get(&phone, "/").await.contains("Chào mừng tới F-Eco"));

    // Biểu mẫu thiếu token CSRF bị từ chối.
    let page = submit(
        &laptop,
        &Form {
            action: "/sessions/revoke-others".into(),
            fields: vec![],
        },
        &[],
    )
    .await;
    assert!(
        page.contains("Biểu mẫu đã hết hạn hoặc không hợp lệ"),
        "{page}"
    );
}

fn totp_now(secret_b32: &str) -> String {
    let secret = totp_rs::Secret::Encoded(secret_b32.to_owned())
        .to_bytes()
        .unwrap();
    totp_rs::TOTP::new_unchecked(totp_rs::Algorithm::SHA1, 6, 1, 30, secret)
        .generate_current()
        .unwrap()
}

fn text_of(html: &str, selector: &str) -> String {
    Html::parse_document(html)
        .select(&Selector::parse(selector).unwrap())
        .next()
        .unwrap_or_else(|| panic!("không thấy {selector}:\n{html}"))
        .text()
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Spec account-identity, "Xác thực hai lớp": bật TOTP, mã khôi phục, và đăng nhập
/// phải qua bước thứ hai.
#[tokio::test]
async fn totp_recovery_codes_and_second_factor_login() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("totp");
    let c = register_and_login(&email, &password, UA_WINDOWS_CHROME).await;

    // Bật TOTP.
    let settings = get(&c, "/settings").await;
    let secret = text_of(&settings, "code#totp_secret_key");
    let form = parse_group_form(&settings, "totp");
    let page = submit(
        &c,
        &form,
        &[("totp_code", &totp_now(&secret)), ("method", "totp")],
    )
    .await;
    assert!(page.contains("Đã lưu thay đổi."), "{page}");

    // Tạo và xác nhận mã khôi phục.
    let form = parse_group_form(&page, "lookup_secret");
    let page = submit(&c, &form, &[("lookup_secret_regenerate", "true")]).await;
    let codes: Vec<String> = Html::parse_document(&page)
        .select(&Selector::parse("ul#lookup_secret_codes li").unwrap())
        .map(|li| li.text().collect::<String>().trim().to_owned())
        .collect();
    assert!(
        codes.len() >= 10,
        "phải có ít nhất 10 mã khôi phục:\n{page}"
    );
    let form = parse_group_form(&page, "lookup_secret");
    let page = submit(&c, &form, &[("lookup_secret_confirm", "true")]).await;
    assert!(page.contains("Đã lưu thay đổi."), "{page}");

    // Trình duyệt mới: mật khẩu xong phải qua bước TOTP.
    let other = browser();
    let page = login(&other, &email, &password).await;
    assert!(page.contains("Xác thực hai lớp"), "{page}");
    assert!(
        !page.contains("<strong id=\"email\">"),
        "chưa được vào trang chủ"
    );
    let form = parse_group_form(&page, "totp");
    let home = submit(
        &other,
        &form,
        &[("totp_code", &totp_now(&secret)), ("method", "totp")],
    )
    .await;
    assert!(home.contains(&email) && home.contains("Xin chào"), "{home}");

    // Trình duyệt khác: dùng mã khôi phục thay cho TOTP.
    let third = browser();
    let page = login(&third, &email, &password).await;
    let form = parse_group_form(&page, "lookup_secret");
    let home = submit(
        &third,
        &form,
        &[("lookup_secret", &codes[0]), ("method", "lookup_secret")],
    )
    .await;
    assert!(home.contains(&email), "{home}");

    // Mã khôi phục chỉ dùng được một lần.
    let fourth = browser();
    let page = login(&fourth, &email, &password).await;
    let form = parse_group_form(&page, "lookup_secret");
    let page = submit(
        &fourth,
        &form,
        &[("lookup_secret", &codes[0]), ("method", "lookup_secret")],
    )
    .await;
    assert!(
        !page.contains("Xin chào"),
        "mã khôi phục đã dùng không được chấp nhận:\n{page}"
    );
}

async fn deletion_mark(email: &str) -> Option<String> {
    kratos()
        .list_identities()
        .await
        .unwrap()
        .into_iter()
        .find(|i| i["traits"]["email"] == email)
        .and_then(|i| {
            i["metadata_admin"]["deletion_requested_at"]
                .as_str()
                .map(str::to_owned)
        })
}

async fn request_deletion(c: &reqwest::Client) -> String {
    let page = get(c, "/account/delete").await;
    let form = parse_form_where(&page, "form[action=\"/account/delete\"]");
    let rejected = submit(c, &form, &[]).await;
    assert!(rejected.contains("Hãy đánh dấu ô xác nhận."), "{rejected}");
    submit(c, &form, &[("confirm", "yes")]).await
}

/// Spec account-identity, "Xóa tài khoản": chờ 30 ngày, đăng nhập lại để hủy, rồi xóa hẳn.
#[tokio::test]
async fn account_deletion_cancel_and_purge() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("del");
    let c = register_and_login(&email, &password, UA_WINDOWS_CHROME).await;

    let page = request_deletion(&c).await;
    assert!(page.contains("đang chờ xóa"), "{page}");
    assert!(
        get(&c, "/").await.contains("Chào mừng tới F-Eco"),
        "mọi phiên phải bị đăng xuất"
    );
    assert!(deletion_mark(&email).await.is_some());

    // Đăng nhập lại trong 30 ngày: yêu cầu xóa bị hủy.
    let home = login(&c, &email, &password).await;
    assert!(home.contains("yêu cầu xóa tài khoản đã được hủy"), "{home}");
    assert!(deletion_mark(&email).await.is_none());

    // Yêu cầu lại; sau 30 ngày tài khoản bị xóa hẳn.
    let page = request_deletion(&c).await;
    assert!(page.contains("đang chờ xóa"), "{page}");
    assert!(deletion_mark(&email).await.is_some());
    let k = kratos();
    let after_29_days = SystemTime::now() + Duration::from_secs(29 * 24 * 3600);
    let st = local_state();
    account::purge_due(&st, after_29_days).await.unwrap();
    assert!(
        k.identity_exists(&email).await.unwrap(),
        "chưa đủ 30 ngày thì chưa xóa"
    );
    let after_31_days = SystemTime::now() + Duration::from_secs(31 * 24 * 3600);
    assert!(account::purge_due(&st, after_31_days).await.unwrap() >= 1);
    assert!(!k.identity_exists(&email).await.unwrap());
    let page = login(&browser(), &email, &password).await;
    assert!(page.contains("Email hoặc mật khẩu không đúng"), "{page}");
}

/// Spec account-identity, "Khôi phục tài khoản": mã qua email, đặt mật khẩu mới, mọi
/// phiên khác bị đăng xuất.
#[tokio::test]
async fn password_recovery_signs_out_other_sessions() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("rec");
    let laptop = register_and_login(&email, &password, UA_WINDOWS_CHROME).await;

    // Trình duyệt khác yêu cầu mã khôi phục.
    let phone = browser();
    let page = get(&phone, "/recovery").await;
    let form = parse_group_form(&page, "code");
    let page = submit(&phone, &form, &[("email", &email), ("method", "code")]).await;
    assert!(page.contains("chúng tôi đã gửi mã khôi phục"), "{page}");
    let mail = wait_mail(&email, "Mã khôi phục").await;
    let code = Regex::new(r"\b\d{6}\b")
        .unwrap()
        .find(&mail)
        .expect("email có mã 6 số")
        .as_str()
        .to_owned();

    // Nhập mã: được chuyển sang trang cài đặt để đặt mật khẩu mới.
    let form = parse_group_form(&page, "code");
    let settings = submit(&phone, &form, &[("code", &code), ("method", "code")]).await;
    assert!(settings.contains("Cài đặt tài khoản"), "{settings}");
    let (_, new_password) = unique("rec-new");
    let form = parse_group_form(&settings, "password");
    let page = submit(
        &phone,
        &form,
        &[("password", &new_password), ("method", "password")],
    )
    .await;
    assert!(page.contains("Đã lưu thay đổi."), "{page}");

    // Thiết bị khác bị đăng xuất; mật khẩu cũ hết hiệu lực, mật khẩu mới dùng được.
    assert!(
        get(&laptop, "/").await.contains("Chào mừng tới F-Eco"),
        "laptop phải bị đăng xuất"
    );
    assert!(login(&browser(), &email, &password)
        .await
        .contains("Email hoặc mật khẩu không đúng"));
    assert!(login(&browser(), &email, &new_password)
        .await
        .contains(&email));
}

// ====================== Nhóm 3: đồng ý dữ liệu và đăng nhập một lần ======================

const HYDRA_PUBLIC: &str = "http://127.0.0.1:4444";
const HYDRA_ADMIN: &str = "http://127.0.0.1:4445";
const APP_CALLBACK: &str = "http://127.0.0.1:9999/cb";

/// Tạo một app con (OAuth client) với các scope khai báo như cổng nhà phát triển sẽ ghi.
async fn create_app(name: &str, scopes: Value) -> String {
    let body = serde_json::json!({
        "client_name": name,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": "openid offline_access profile email locale phone billing",
        "redirect_uris": [APP_CALLBACK],
        "token_endpoint_auth_method": "none",
        "subject_type": "pairwise",
        "policy_uri": "https://app.example/privacy",
        "metadata": {"developer": "Acme", "scopes": scopes},
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

/// F-Launcher: client công khai bên thứ nhất (không hiện màn hình đồng ý).
async fn create_launcher() -> String {
    let body = serde_json::json!({
        "client_name": "F-Launcher",
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": "openid offline_access profile",
        "redirect_uris": [APP_CALLBACK, "http://127.0.0.1:47201/callback"],
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

/// Trình duyệt dừng khi được chuyển về địa chỉ callback của app (như app thật nhận code).
fn app_browser() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(UA_WINDOWS_CHROME)
        .redirect(reqwest::redirect::Policy::custom(|a| {
            if a.url().port() == Some(9999) {
                a.stop()
            } else {
                a.follow()
            }
        }))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

struct Authorize {
    verifier: String,
    /// Trang đồng ý (nếu có) hoặc `None` khi đã về thẳng callback.
    consent_page: Option<String>,
    callback: Option<url::Url>,
}

fn pkce() -> (String, String) {
    use base64::Engine;
    use sha2::Digest;
    // RFC 7636: verifier dài 43–128 ký tự.
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

async fn authorize(c: &reqwest::Client, client_id: &str, scope: &str) -> Authorize {
    let (verifier, challenge) = pkce();
    let mut url = url::Url::parse(&format!("{HYDRA_PUBLIC}/oauth2/auth")).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", scope)
        .append_pair("redirect_uri", APP_CALLBACK)
        .append_pair("state", "trang-thai-du-dai")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    let res = c.get(url).send().await.unwrap();
    finish(res, verifier).await
}

async fn finish(res: reqwest::Response, verifier: String) -> Authorize {
    if res.status().is_redirection() {
        let loc = res.headers()["location"].to_str().unwrap();
        Authorize {
            verifier,
            consent_page: None,
            callback: Some(url::Url::parse(loc).unwrap()),
        }
    } else {
        Authorize {
            verifier,
            consent_page: Some(res.text().await.unwrap()),
            callback: None,
        }
    }
}

/// Gửi màn hình đồng ý: `action` là "accept" hoặc "deny"; `optional` là scope không
/// bắt buộc người dùng tích chọn.
async fn answer_consent(
    c: &reqwest::Client,
    a: Authorize,
    action: &str,
    optional: &[&str],
) -> Authorize {
    let page = a.consent_page.expect("phải có màn hình đồng ý");
    let form = parse_form_where(&page, "form[action=\"/oauth/consent\"]");
    let mut pairs = form.fields.clone();
    pairs.push(("action".into(), action.into()));
    for s in optional {
        pairs.push((format!("scope_{s}"), "on".into()));
    }
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .finish();
    let res = c
        .post(format!("{BASE}/oauth/consent"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();
    finish(res, a.verifier).await
}

fn query(u: &url::Url, k: &str) -> Option<String> {
    u.query_pairs()
        .find(|(n, _)| n == k)
        .map(|(_, v)| v.into_owned())
}

async fn exchange(client_id: &str, a: &Authorize) -> Value {
    let cb = a.callback.as_ref().expect("phải về callback");
    let code = query(cb, "code").unwrap_or_else(|| panic!("không có code: {cb}"));
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", &code)
        .append_pair("redirect_uri", APP_CALLBACK)
        .append_pair("client_id", client_id)
        .append_pair("code_verifier", &a.verifier)
        .finish();
    reqwest::Client::new()
        .post(format!("{HYDRA_PUBLIC}/oauth2/token"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn id_token_sub(tokens: &Value) -> String {
    use base64::Engine;
    let payload = tokens["id_token"]
        .as_str()
        .unwrap()
        .split('.')
        .nth(1)
        .unwrap();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["sub"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn me(token: &str, fields: Option<&str>) -> (u16, Value) {
    let mut url = format!("{BASE}/v1/me");
    if let Some(f) = fields {
        url.push_str(&format!("?fields={f}"));
    }
    let res = reqwest::Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    (res.status().as_u16(), res.json().await.unwrap())
}

fn scope_rows(consent_page: &str) -> Vec<String> {
    Html::parse_document(consent_page)
        .select(&Selector::parse("tr[data-scope]").unwrap())
        .map(|r| r.value().attr("data-scope").unwrap().to_owned())
        .collect()
}

/// Bộ nhận webhook của "app": ghi lại header chữ ký và thân yêu cầu.
/// Các yêu cầu webhook đã nhận: (sự kiện, chữ ký, thân).
type Received = std::sync::Arc<std::sync::Mutex<Vec<(String, String, Vec<u8>)>>>;

async fn webhook_receiver() -> (String, Received) {
    use axum::{body::Bytes, http::HeaderMap, routing::post, Router};
    let got: Received = Default::default();
    let sink = got.clone();
    let app = Router::new().route(
        "/hook",
        post(move |h: HeaderMap, b: Bytes| {
            let sink = sink.clone();
            async move {
                let header = |n: &str| {
                    h.get(n)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_owned()
                };
                sink.lock().unwrap().push((
                    header("f-eco-event"),
                    header("f-eco-signature"),
                    b.to_vec(),
                ));
                "ok"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/hook", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, got)
}

/// Spec user-data-consent: đồng ý một phần, ngoài scope, pairwise, xin thêm scope, từ chối,
/// sửa hồ sơ, ứng dụng đã kết nối, nhật ký, thu hồi và webhook `user.revoked`.
#[tokio::test]
async fn consent_scopes_pairwise_and_revocation() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let st = local_state();
    let (email, password) = unique("consent");
    // Người dùng đăng ký và đăng nhập trên chính trình duyệt sẽ mở app.
    let user = app_browser();
    let form = parse_form(&get(&user, "/registration").await);
    submit(
        &user,
        &form,
        &[
            ("traits.email", &email),
            ("traits.name", "Lan Nguyễn"),
            ("password", &password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(login(&user, &email, &password).await.contains(&email));

    let soundlab = create_app(
        "SoundLab",
        serde_json::json!({
            "profile": {"required": true, "purpose_vi": "Hiển thị trên bản nhạc bạn chia sẻ", "purpose_en": "Shown on tracks you share"},
            "email": {"required": false, "purpose_vi": "Gửi bản tin", "purpose_en": "Newsletter"},
            "phone": {"required": false, "purpose_vi": "Gửi mã xác nhận", "purpose_en": "Confirmation codes"}
        }),
    )
    .await;
    let (hook_url, hooks) = webhook_receiver().await;
    let hook_secret = store::register_webhook(&st.db, &st.data_key, &soundlab, &hook_url)
        .await
        .unwrap();

    // --- Scenario "Đồng ý một phần": chỉ đồng ý profile (email không tích).
    let a = authorize(&user, &soundlab, "openid profile email").await;
    let page = a.consent_page.clone().expect("màn hình đồng ý");
    assert!(page.contains("SoundLab") && page.contains("Acme"), "{page}");
    assert!(
        page.contains("Hiển thị trên bản nhạc bạn chia sẻ") && page.contains("Gửi bản tin"),
        "{page}"
    );
    assert!(page.contains("https://app.example/privacy"), "{page}");
    assert_eq!(scope_rows(&page), ["profile", "email"]);
    let a = answer_consent(&user, a, "accept", &[]).await;
    let tokens = exchange(&soundlab, &a).await;
    assert_eq!(tokens["scope"], "openid profile", "{tokens}");
    let token_a = tokens["access_token"].as_str().unwrap().to_owned();

    let (status, body) = me(&token_a, None).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["name"], "Lan Nguyễn");
    assert!(body.get("email").is_none(), "không được có email: {body}");
    // --- Scenario "Gọi API ngoài scope".
    let (status, body) = me(&token_a, Some("email")).await;
    assert_eq!(status, 403);
    assert_eq!(body["error"], "insufficient_scope");
    assert!(
        body["message_vi"].as_str().unwrap().contains("email") && body["request_id"].is_string()
    );

    // --- Bản ghi đồng ý có phiên bản nội dung.
    let subject: uuid::Uuid = kratos()
        .list_identities()
        .await
        .unwrap()
        .into_iter()
        .find(|i| i["traits"]["email"] == email.as_str())
        .and_then(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
        .unwrap();
    let history = store::consent_history(&st.db, subject).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].action, "grant");
    assert_eq!(history[0].granted_scopes, ["openid", "profile"]);
    assert!(
        history[0].content_version.starts_with("v1-"),
        "{:?}",
        history[0]
    );

    // --- Scenario "Hai app khác nhau": `sub` riêng cho từng app, khớp giữa ID token và /v1/me.
    let noteapp = create_app(
        "NoteApp",
        serde_json::json!({"profile": {"required": true}}),
    )
    .await;
    let b = authorize(&user, &noteapp, "openid profile").await;
    let b = answer_consent(&user, b, "accept", &[]).await;
    let tokens_b = exchange(&noteapp, &b).await;
    let token_b = tokens_b["access_token"].as_str().unwrap().to_owned();
    let sub_a = me(&token_a, None).await.1["sub"]
        .as_str()
        .unwrap()
        .to_owned();
    let sub_b = me(&token_b, None).await.1["sub"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(sub_a, sub_b);
    assert_eq!(id_token_sub(&tokens), sub_a);
    assert_ne!(sub_a, subject.to_string(), "không lộ id danh tính thật");

    // --- Scenario "App xin thêm scope": chỉ hỏi phone, profile vẫn được giữ.
    let a2 = authorize(&user, &soundlab, "openid profile phone").await;
    let page = a2.consent_page.clone().expect("phải hỏi scope mới");
    assert_eq!(scope_rows(&page), ["phone"]);
    let a2 = answer_consent(&user, a2, "accept", &["phone"]).await;
    let t2 = exchange(&soundlab, &a2).await;
    assert_eq!(t2["scope"], "openid phone profile", "{t2}");

    // --- Scenario "Từ chối": app không nhận code.
    let mapapp = create_app("MapApp", serde_json::json!({"locale": {"required": true}})).await;
    let d = authorize(&user, &mapapp, "openid locale").await;
    let d = answer_consent(&user, d, "deny", &[]).await;
    let cb = d.callback.expect("về callback với lỗi");
    assert_eq!(query(&cb, "error").as_deref(), Some("access_denied"));
    assert!(query(&cb, "code").is_none());

    // --- Scenario "Sửa hồ sơ": đổi tên, app thấy tên mới ở lần gọi tiếp theo.
    let settings = get(&user, "/settings").await;
    let form = parse_group_form(&settings, "profile");
    let page = submit(
        &user,
        &form,
        &[
            ("traits.email", &email),
            ("traits.name", "Lan Nguyễn Thị"),
            ("traits.locale", "vi"),
            ("method", "profile"),
        ],
    )
    .await;
    assert!(page.contains("Đã lưu thay đổi."), "{page}");
    assert_eq!(me(&token_b, None).await.1["name"], "Lan Nguyễn Thị");

    // --- Trang "Ứng dụng đã kết nối" và nhật ký truy cập.
    let apps = get(&user, "/apps").await;
    assert!(
        apps.contains("SoundLab") && apps.contains("NoteApp"),
        "{apps}"
    );
    assert!(
        !apps.contains("MapApp"),
        "app bị từ chối không được liệt kê"
    );
    let detail = get(&user, &format!("/apps/{soundlab}")).await;
    assert!(detail.contains("Tên hiển thị và ảnh đại diện"), "{detail}");

    // --- Scenario "Thu hồi quyền".
    let form = parse_form_where(&apps, &format!("form[action=\"/apps/{soundlab}/revoke\"]"));
    let page = submit(&user, &form, &[]).await;
    assert!(page.contains("Đã thu hồi quyền."), "{page}");
    let (status, body) = me(&token_a, None).await;
    assert_eq!(status, 401);
    assert_eq!(body["error"], "invalid_token");
    assert_eq!(
        me(&token_b, None).await.0,
        200,
        "app khác không bị ảnh hưởng"
    );
    let again = authorize(&user, &soundlab, "openid profile").await;
    assert!(
        again.consent_page.is_some(),
        "lần mở sau phải hiện lại màn hình đồng ý"
    );
    let history = store::consent_history(&st.db, subject).await.unwrap();
    assert!(history
        .iter()
        .any(|r| r.action == "revoke" && r.client_id == soundlab));

    // --- Webhook `user.revoked` có chữ ký hợp lệ, chỉ mang `sub` riêng của app.
    feco_server::webhooks::deliver_due(&st.db, &st.http, &st.data_key)
        .await
        .unwrap();
    let got = hooks.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "{got:?}");
    let (event, signature, body) = &got[0];
    assert_eq!(event, "user.revoked");
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    assert!(feco_server::webhooks::verify(
        &hook_secret,
        signature,
        body,
        now
    ));
    let payload: Value = serde_json::from_slice(body).unwrap();
    assert_eq!(payload["sub"], sub_a.as_str());
    assert!(!String::from_utf8_lossy(body).contains(&email));
}

/// Spec user-data-consent, "Xuất dữ liệu cá nhân": ZIP đủ nhóm dữ liệu, chỉ chủ tài khoản tải được.
#[tokio::test]
async fn personal_data_export() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("export");
    let user = app_browser();
    let form = parse_form(&get(&user, "/registration").await);
    submit(
        &user,
        &form,
        &[
            ("traits.email", &email),
            ("password", &password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(login(&user, &email, &password).await.contains(&email));
    let app = create_app(
        "ExportApp",
        serde_json::json!({"profile": {"required": true}}),
    )
    .await;
    let a = authorize(&user, &app, "openid profile").await;
    let a = answer_consent(&user, a, "accept", &[]).await;
    let token = exchange(&app, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(me(&token, None).await.0, 200);

    let page = get(&user, "/account/export").await;
    let form = parse_form_where(&page, "form[action=\"/account/export\"]");
    let page = submit(&user, &form, &[]).await;
    assert!(page.contains("Đã nhận yêu cầu"), "{page}");
    let mail = wait_mail(&email, "Dữ liệu cá nhân").await;
    let link = Regex::new(r"http://127\.0\.0\.1:8080/account/export/[0-9a-f]{64}")
        .unwrap()
        .find(&mail)
        .expect("email có liên kết tải")
        .as_str()
        .to_owned();

    // Người khác (chưa đăng nhập) không tải được.
    let stranger = browser();
    let res = stranger.get(&link).send().await.unwrap();
    assert_ne!(
        res.headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap()),
        Some("application/zip")
    );

    let res = user.get(&link).send().await.unwrap();
    assert_eq!(res.headers()["content-type"], "application/zip");
    let bytes = res.bytes().await.unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let mut names: Vec<String> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "README.txt",
            "access_log_90_days.json",
            "builder_projects.json",
            "connected_apps.json",
            "consents.json",
            "invoices.json",
            "orders.json",
            "profile.json",
            "sessions.json"
        ]
    );
    let mut read = |n: &str| -> Value {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(n).unwrap(), &mut s).unwrap();
        serde_json::from_str(&s).unwrap()
    };
    assert_eq!(read("profile.json")["traits"]["email"], email.as_str());
    assert_eq!(read("consents.json")[0]["client_id"], app.as_str());
    assert_eq!(
        read("access_log_90_days.json")[0]["client_id"],
        app.as_str()
    );
    assert_eq!(read("connected_apps.json")[0]["app_name"], "ExportApp");

    // File xuất được mã hóa khi lưu (spec platform-security, "Mã hóa khi truyền và khi lưu").
    let st = local_state();
    let (encrypted, head): (bool, Vec<u8>) = sqlx::query_as(
        "SELECT encrypted, substring(file from 1 for 4) FROM data_exports ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&st.db)
    .await
    .unwrap();
    assert!(encrypted);
    assert_ne!(&head[..2], b"PK", "file lưu không được là ZIP dạng rõ");
}

// ====================== Nhóm 4: bảo mật nền tảng ======================

async fn subject_of(email: &str) -> uuid::Uuid {
    kratos()
        .list_identities()
        .await
        .unwrap()
        .into_iter()
        .find(|i| i["traits"]["email"] == email)
        .and_then(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
        .expect("có danh tính")
}

/// Spec platform-security, "Mã hóa khi truyền và khi lưu": mật khẩu băm Argon2id, khóa
/// webhook của nhà phát triển được mã hóa.
#[tokio::test]
async fn argon2id_passwords_and_encrypted_webhook_secrets() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("argon");
    register_and_login(&email, &password, UA_WINDOWS_CHROME).await;
    let id = subject_of(&email).await;
    let identity: Value = reqwest::Client::new()
        .get(format!(
            "http://127.0.0.1:4434/admin/identities/{id}?include_credential=password"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let hash = identity["credentials"]["password"]["config"]["hashed_password"]
        .as_str()
        .expect("có mã băm mật khẩu");
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    assert!(!hash.contains(&password));

    let st = local_state();
    let secret =
        store::register_webhook(&st.db, &st.data_key, "argon-app", "http://127.0.0.1:9/hook")
            .await
            .unwrap();
    let stored: String =
        sqlx::query_scalar("SELECT secret FROM app_webhooks WHERE client_id = 'argon-app'")
            .fetch_one(&st.db)
            .await
            .unwrap();
    assert!(stored.starts_with("enc:v1:"), "{stored}");
    assert!(!stored.contains(&secret));
}

/// Spec platform-security, "Nhật ký kiểm toán": ghi các thao tác nhạy cảm, không chứa bí
/// mật, không sửa được, chỉ xóa được bản ghi quá 1 năm.
#[tokio::test]
async fn audit_log_append_only_and_secret_free() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("audit");
    let c = register_and_login(&email, &password, UA_WINDOWS_CHROME).await;
    let subject = subject_of(&email).await;
    // Một lần đăng nhập sai và một lần đổi hồ sơ.
    let other = browser();
    login(&other, &email, "sai-mat-khau-hoan-toan").await;
    let settings = get(&c, "/settings").await;
    let form = parse_group_form(&settings, "profile");
    submit(
        &c,
        &form,
        &[
            ("traits.email", &email),
            ("traits.name", "Kiểm toán"),
            ("method", "profile"),
        ],
    )
    .await;

    let st = local_state();
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM audit_log WHERE actor_id = $1 ORDER BY id")
            .bind(subject.to_string())
            .fetch_all(&st.db)
            .await
            .unwrap();
    assert!(actions.contains(&"auth.login".to_string()), "{actions:?}");
    assert!(
        actions.contains(&"account.profile_changed".to_string()),
        "{actions:?}"
    );
    let failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'auth.login_failed' AND details->>'identifier_hash' = $1",
    )
    .bind(feco_server::audit::hash_identifier(&email))
    .fetch_one(&st.db)
    .await
    .unwrap();
    assert_eq!(failed, 1);

    // Không có mật khẩu, token hay email dạng rõ của lần đăng nhập sai.
    let dump: String = sqlx::query_scalar("SELECT coalesce(string_agg(details::text || coalesce(target, ''), ' '), '') FROM audit_log")
        .fetch_one(&st.db)
        .await
        .unwrap();
    assert!(!dump.contains(&password));
    assert!(!dump.contains("sai-mat-khau-hoan-toan"));
    assert!(!dump.to_lowercase().contains("csrf"));

    // Scenario "Thử sửa nhật ký": cơ sở dữ liệu từ chối.
    let upd = sqlx::query("UPDATE audit_log SET action = 'x' WHERE actor_id = $1")
        .bind(subject.to_string())
        .execute(&st.db)
        .await;
    assert!(upd.unwrap_err().to_string().contains("append-only"));
    let del = sqlx::query("DELETE FROM audit_log WHERE actor_id = $1")
        .bind(subject.to_string())
        .execute(&st.db)
        .await;
    assert!(del.unwrap_err().to_string().contains("append-only"));
    let trunc = sqlx::query("TRUNCATE audit_log").execute(&st.db).await;
    assert!(trunc.is_err());

    // Lưu 1 năm: bản ghi quá 1 năm được tác vụ lưu trữ xóa, bản ghi mới thì không.
    sqlx::query(
        "INSERT INTO audit_log (at, actor_type, action) VALUES (now() - interval '366 days', 'system', 'test.old')",
    )
    .execute(&st.db)
    .await
    .unwrap();
    assert!(
        feco_server::audit::purge_older_than_one_year(&st.db)
            .await
            .unwrap()
            >= 1
    );
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE actor_id = $1")
        .bind(subject.to_string())
        .fetch_one(&st.db)
        .await
        .unwrap();
    assert!(remaining >= 2);
}

async fn mail_count(to: &str, subject: &str) -> usize {
    let list: Value = reqwest::Client::new()
        .get(format!("{MAILPIT}/api/v1/messages"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    list["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            m["To"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| t["Address"] == to)
                && m["Subject"].as_str().is_some_and(|s| s.contains(subject))
        })
        .count()
}

/// Spec platform-security, "Phát hiện đăng nhập bất thường".
#[tokio::test]
async fn new_device_alert_and_not_me() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("device");
    let laptop = register_and_login(&email, &password, UA_WINDOWS_CHROME).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(
        mail_count(&email, "thiết bị mới").await,
        0,
        "lần đăng nhập đầu tiên không gửi cảnh báo"
    );
    // Đăng nhập lại cùng thiết bị: không cảnh báo.
    assert!(login(&browser(), &email, &password).await.contains(&email));

    // Thiết bị mới: có email kèm thông tin thiết bị và nút "Không phải tôi".
    let phone = browser_with(UA_LINUX_FIREFOX, "vi-VN");
    assert!(login(&phone, &email, &password).await.contains(&email));
    let mail = wait_mail(&email, "thiết bị mới").await;
    assert!(mail.contains("Firefox · Linux"), "{mail}");
    assert_eq!(mail_count(&email, "thiết bị mới").await, 1);
    let link = Regex::new(r"http://127\.0\.0\.1:8080/security/not-me/[0-9a-f]{64}")
        .unwrap()
        .find(&mail)
        .expect("có liên kết Không phải tôi")
        .as_str()
        .to_owned();

    // Chủ tài khoản bấm "Không phải tôi" từ trình duyệt bất kỳ.
    let owner = browser();
    let page = get(&owner, &link).await;
    let form = parse_form_where(&page, "form[action^=\"/security/not-me/\"]");
    let done = submit(&owner, &form, &[]).await;
    assert!(done.contains("Đã đăng xuất mọi thiết bị"), "{done}");
    assert!(get(&laptop, "/").await.contains("Chào mừng tới F-Eco"));
    assert!(get(&phone, "/").await.contains("Chào mừng tới F-Eco"));
    // Liên kết chỉ dùng được một lần.
    assert!(get(&owner, &link).await.contains("Liên kết không hợp lệ"));
}

/// Spec platform-security, "Giới hạn tần suất": vượt hạn mức nhận 429 kèm Retry-After.
#[tokio::test]
async fn v1_me_rate_limit_per_user() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("rate");
    let user = app_browser();
    let form = parse_form(&get(&user, "/registration").await);
    submit(
        &user,
        &form,
        &[
            ("traits.email", &email),
            ("password", &password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(login(&user, &email, &password).await.contains(&email));
    let app = create_app(
        "RateApp",
        serde_json::json!({"profile": {"required": true}}),
    )
    .await;
    let a = authorize(&user, &app, "openid profile").await;
    let a = answer_consent(&user, a, "accept", &[]).await;
    let token = exchange(&app, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();

    let http = reqwest::Client::new();
    let mut limited = None;
    for i in 1..=130 {
        let res = http
            .get(format!("{BASE}/v1/me"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        if res.status() == 429 {
            let retry: u64 = res.headers()["retry-after"]
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            let body: Value = res.json().await.unwrap();
            limited = Some((i, retry, body));
            break;
        }
        assert_eq!(res.status(), 200, "lần {i}");
    }
    let (i, retry, body) = limited.expect("phải bị giới hạn trước 130 lần");
    assert_eq!(i, 121, "hạn mức mặc định 120 lần/phút cho mỗi người dùng");
    assert!((1..=60).contains(&retry));
    assert_eq!(body["error"], "rate_limited");
}

async fn issue_launch_code(token: &str, client_id: &str) -> (u16, Value) {
    let res = reqwest::Client::new()
        .post(format!("{BASE}/v1/launch-codes"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "client_id": client_id }))
        .send()
        .await
        .unwrap();
    (res.status().as_u16(), res.json().await.unwrap())
}

/// App con đổi mã khởi chạy: trả (mã HTTP, phản hồi, PKCE verifier).
async fn exchange_launch_code(code: &str, client_id: &str, scope: &str) -> (u16, Value, String) {
    let (verifier, challenge) = pkce();
    let res = reqwest::Client::new()
        .post(format!("{BASE}/v1/launch-codes/exchange"))
        .json(&serde_json::json!({
            "code": code,
            "client_id": client_id,
            "redirect_uri": APP_CALLBACK,
            "code_challenge": challenge,
            "scope": scope,
        }))
        .send()
        .await
        .unwrap();
    (res.status().as_u16(), res.json().await.unwrap(), verifier)
}

async fn audit_actions(target: &str) -> Vec<String> {
    let st = local_state();
    sqlx::query_scalar("SELECT action FROM audit_log WHERE target = $1 ORDER BY id")
        .bind(target)
        .fetch_all(&st.db)
        .await
        .unwrap()
}

/// Spec launcher, "Đăng nhập trong launcher" và "Mở app con và truyền phiên đăng nhập":
/// launcher đăng nhập không cần màn hình đồng ý, xin mã khởi chạy cho app; app đổi mã lấy token
/// đúng một lần; mã dùng lại, dùng sai app, hết hạn hoặc do app khác xin đều bị từ chối.
#[tokio::test]
async fn launcher_login_and_single_use_launch_codes() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let launcher = LAUNCHER.get().unwrap().clone();
    let (email, password) = unique("launch");
    let user = app_browser();
    let form = parse_form(&get(&user, "/registration").await);
    submit(
        &user,
        &form,
        &[
            ("traits.email", &email),
            ("traits.name", "Minh Trần"),
            ("password", &password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(login(&user, &email, &password).await.contains(&email));

    // Launcher (bên thứ nhất): về thẳng callback, không có màn hình đồng ý.
    let a = authorize(&user, &launcher, "openid offline_access profile").await;
    assert!(a.consent_page.is_none(), "launcher không được hỏi đồng ý");
    let tokens = exchange(&launcher, &a).await;
    let launcher_token = tokens["access_token"].as_str().expect("token").to_owned();
    assert!(tokens["refresh_token"].is_string(), "{tokens}");
    let (status, me_body) = me(&launcher_token, None).await;
    assert_eq!((status, me_body["name"].as_str()), (200, Some("Minh Trần")));

    let scopes = serde_json::json!({
        "profile": {"required": true, "purpose_vi": "Hiển thị tên", "purpose_en": "Show your name"}
    });
    let soundlab = create_app("SoundLab", scopes.clone()).await;
    let other = create_app("OtherApp", scopes).await;

    // Chưa từng đồng ý cho SoundLab: mã khởi chạy không tự cấp quyền mới.
    let (status, body) = issue_launch_code(&launcher_token, &soundlab).await;
    assert_eq!(status, 200, "{body}");
    let (status, body, _) =
        exchange_launch_code(body["code"].as_str().unwrap(), &soundlab, "openid profile").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (403, Some("consent_required"))
    );

    // Người dùng đồng ý một lần qua trình duyệt.
    let a = authorize(&user, &soundlab, "openid profile").await;
    let a = answer_consent(&user, a, "accept", &[]).await;
    let interactive = exchange(&soundlab, &a).await;
    let pairwise_sub = id_token_sub(&interactive);

    // Mở SoundLab từ launcher: đổi mã lấy code, rồi đổi code lấy token bằng PKCE verifier.
    let (status, issued) = issue_launch_code(&launcher_token, &soundlab).await;
    assert_eq!((status, issued["expires_in"].as_u64()), (200, Some(60)));
    let code = issued["code"].as_str().unwrap().to_owned();
    let (status, body, verifier) = exchange_launch_code(&code, &soundlab, "openid profile").await;
    assert_eq!(status, 200, "{body}");
    let callback = url::Url::parse(&format!(
        "{APP_CALLBACK}?code={}",
        body["authorization_code"].as_str().unwrap()
    ))
    .unwrap();
    let tokens = exchange(
        &soundlab,
        &Authorize {
            verifier,
            consent_page: None,
            callback: Some(callback),
        },
    )
    .await;
    assert_eq!(tokens["scope"], "openid profile", "{tokens}");
    assert_eq!(
        id_token_sub(&tokens),
        pairwise_sub,
        "vẫn là định danh riêng của app"
    );
    let (status, me_body) = me(tokens["access_token"].as_str().unwrap(), None).await;
    assert_eq!((status, me_body["name"].as_str()), (200, Some("Minh Trần")));

    // Scenario "Mã bị dùng lại": máy chủ từ chối và ghi sự kiện bảo mật.
    let (status, body, _) = exchange_launch_code(&code, &soundlab, "openid profile").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (400, Some("invalid_grant"))
    );
    let actions = audit_actions(&soundlab).await;
    assert!(
        actions.contains(&"launch_code.reuse".to_owned()),
        "{actions:?}"
    );
    assert!(
        actions.contains(&"launch_code.exchange".to_owned()),
        "{actions:?}"
    );

    // Mã của SoundLab bị app khác dùng: từ chối, và mã bị đốt luôn.
    let (_, issued) = issue_launch_code(&launcher_token, &soundlab).await;
    let code = issued["code"].as_str().unwrap().to_owned();
    let (status, _, _) = exchange_launch_code(&code, &other, "openid profile").await;
    assert_eq!(status, 400);
    let (status, _, _) = exchange_launch_code(&code, &soundlab, "openid profile").await;
    assert_eq!(status, 400, "mã đã bị đốt");
    assert!(audit_actions(&soundlab)
        .await
        .contains(&"launch_code.client_mismatch".to_owned()));

    // Hết 60 giây: từ chối.
    let (_, issued) = issue_launch_code(&launcher_token, &soundlab).await;
    let code = issued["code"].as_str().unwrap().to_owned();
    let st = local_state();
    sqlx::query(
        "UPDATE launch_codes SET expires_at = now() - interval '1 second'
         WHERE client_id = $1 AND used_at IS NULL",
    )
    .bind(&soundlab)
    .execute(&st.db)
    .await
    .unwrap();
    let (status, _, _) = exchange_launch_code(&code, &soundlab, "openid profile").await;
    assert_eq!(status, 400);

    // Chỉ token của launcher được xin mã; app con không xin được mã cho app khác.
    let app_token = tokens["access_token"].as_str().unwrap();
    let (status, body) = issue_launch_code(app_token, &other).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (403, Some("launcher_only"))
    );
    let (status, body) = issue_launch_code(&launcher_token, "khong-ton-tai").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (404, Some("unknown_client"))
    );
}

// ---------- Nhóm 7: danh mục F-Store (spec app-catalog) ----------

async fn register_in(user: &reqwest::Client, email: &str, password: &str, name: &str) {
    let form = parse_form(&get(user, "/registration").await);
    submit(
        user,
        &form,
        &[
            ("traits.email", email),
            ("traits.name", name),
            ("password", password),
            ("method", "password"),
        ],
    )
    .await;
    assert!(login(user, email, password).await.contains(email));
}

async fn catalog_ids(query: &str) -> Vec<String> {
    let body: Value = reqwest::Client::new()
        .get(format!("{BASE}/v1/catalog?{query}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    body["apps"]
        .as_array()
        .expect("apps")
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect()
}

fn catalog_app(id: &str, client_id: &str, name: &str) -> feco_server::catalog::AppInput {
    feco_server::catalog::AppInput {
        id: id.into(),
        client_id: client_id.into(),
        name: name.into(),
        developer: "Công ty Ví Dụ".into(),
        license: "MIT".into(),
        category: "Giáo dục".into(),
        summary: "Ghi chép và ôn tập".into(),
        description: "Mô tả dài".into(),
        ..Default::default()
    }
}

fn version(v: &str, platform: &str) -> feco_server::catalog::VersionInput {
    feco_server::catalog::VersionInput {
        version: v.into(),
        platform: platform.into(),
        size_bytes: 12 * 1_048_576,
        tuf_target: format!("app-{v}-{platform}.zip"),
        changelog: format!("Thay đổi của bản {v}"),
    }
}

/// Spec app-catalog: chỉ hiện app đã duyệt, tìm không dấu và lọc, trang chi tiết có scope trước
/// nút cài, nhãn "Của F-Eco", đánh giá chỉ cho người đã cài, nhà phát triển trả lời, báo cáo vi
/// phạm có email kết quả, nhận xét vi phạm bị ẩn, app bị gỡ biến mất trong 5 phút.
#[tokio::test]
async fn catalog_search_detail_reviews_reports_and_removal() {
    use feco_server::catalog;
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let st = local_state();
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string();

    // Nhà phát triển và người dùng.
    let (dev_email, dev_password) = unique("dev");
    let developer = app_browser();
    register_in(&developer, &dev_email, &dev_password, "Nhà phát triển").await;
    let dev_subject = subject_of(&dev_email).await;
    let (email, password) = unique("store");
    let user = app_browser();
    register_in(&user, &email, &password, "Hà Linh").await;

    let client_id = create_app(
        "SoundLab",
        serde_json::json!({
            "profile": {"required": true, "purpose_vi": "Hiển thị trên bản nhạc bạn chia sẻ", "purpose_en": "Shown on tracks you share"},
            "email": {"required": false, "purpose_vi": "Gửi bản tin", "purpose_en": "Newsletter"}
        }),
    )
    .await;
    let notebook = format!("so-tay-{stamp}");
    let paid = format!("paid-{stamp}");
    let builder = format!("f-builder-{stamp}");
    let draft = format!("draft-{stamp}");
    let mut a = catalog_app(&notebook, &client_id, "Sổ tay Học tập");
    a.owner_subject = Some(dev_subject);
    a.developer = format!("Nhà PT {stamp}");
    catalog::upsert_app(&st.db, &st.catalog, &a).await.unwrap();
    catalog::release_version(
        &st.db,
        &st.catalog,
        &notebook,
        &version("1.1.0", "windows-x64"),
    )
    .await
    .unwrap();
    catalog::release_version(
        &st.db,
        &st.catalog,
        &notebook,
        &version("1.2.0", "windows-x64"),
    )
    .await
    .unwrap();
    let mut b = catalog_app(&paid, &client_id, &format!("Studio {stamp}"));
    b.price_vnd = 49_000;
    b.developer_verified = true;
    catalog::upsert_app(&st.db, &st.catalog, &b).await.unwrap();
    catalog::release_version(&st.db, &st.catalog, &paid, &version("2.0.0", "macos-arm64"))
        .await
        .unwrap();
    let mut c = catalog_app(&builder, &client_id, &format!("F-Builder {stamp}"));
    c.first_party = true;
    catalog::upsert_app(&st.db, &st.catalog, &c).await.unwrap();
    catalog::release_version(
        &st.db,
        &st.catalog,
        &builder,
        &version("0.1.0", "windows-x64"),
    )
    .await
    .unwrap();
    // App chưa có phiên bản nào qua kiểm duyệt: không hiện.
    catalog::upsert_app(
        &st.db,
        &st.catalog,
        &catalog_app(&draft, &client_id, &format!("Nháp {stamp}")),
    )
    .await
    .unwrap();

    // --- "Tìm không dấu": "so tay hoc tap" tìm ra "Sổ tay Học tập".
    assert!(catalog_ids("q=so%20tay%20hoc%20tap")
        .await
        .contains(&notebook));
    assert!(
        catalog_ids("q=S%E1%BB%94%20TAY").await.contains(&notebook),
        "SỔ TAY"
    );
    assert!(
        catalog_ids(&format!("q=nha%20pt%20{stamp}"))
            .await
            .contains(&notebook),
        "theo nhà phát triển"
    );
    // Bộ lọc.
    let paid_ids = catalog_ids(&format!("q={stamp}&price=paid")).await;
    assert_eq!(paid_ids, vec![paid.clone()]);
    assert_eq!(
        catalog_ids(&format!("q={stamp}&os=macos")).await,
        vec![paid.clone()]
    );
    assert_eq!(
        catalog_ids(&format!("q={stamp}&verified=1")).await,
        vec![paid.clone()]
    );
    let free = catalog_ids(&format!("q={stamp}&price=free&os=windows")).await;
    assert!(free.contains(&notebook) && free.contains(&builder) && !free.contains(&paid));
    assert!(
        !catalog_ids(&format!("q={stamp}")).await.contains(&draft),
        "chưa duyệt"
    );
    let res = reqwest::get(format!("{BASE}/store/{draft}")).await.unwrap();
    assert_eq!(res.status(), 404);

    // --- Trang chi tiết: scope và mục đích trước nút cài; license; chính sách; lịch sử.
    let page = get(&user, &format!("/store/{notebook}")).await;
    let scope_line = "Tên hiển thị và ảnh đại diện (bắt buộc): Hiển thị trên bản nhạc bạn chia sẻ";
    let scope_at = page
        .find(scope_line)
        .unwrap_or_else(|| panic!("thiếu scope:\n{page}"));
    assert!(scope_at < page.find("Cài bằng F-Launcher").expect("nút cài"));
    assert!(page.contains("Địa chỉ email (không bắt buộc): Gửi bản tin"));
    assert!(page.contains("https://app.example/privacy") && page.contains("MIT"));
    assert!(page.contains("1.2.0") && page.contains("Thay đổi của bản 1.1.0"));
    assert!(page.contains("quyền như mọi ứng dụng khác"));
    let detail: Value = reqwest::get(format!("{BASE}/v1/catalog/{notebook}"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["latest_version"], "1.2.0");
    assert_eq!(detail["scopes"][0]["required"], true);
    // "Nhãn app của nền tảng".
    let page = get(&user, &format!("/store/{builder}")).await;
    assert!(page.contains("Của F-Eco") && page.contains(scope_line));
    assert!(page.contains("https://app.example/privacy"));

    // --- "Chưa cài app": không đánh giá được.
    let page = get(&user, &format!("/store/{notebook}")).await;
    let review_form = parse_form_where(&page, "#review-form");
    let page = submit(
        &user,
        &review_form,
        &[("rating", "2"), ("body", "Hay treo máy")],
    )
    .await;
    assert!(
        page.contains("Bạn cần cài app trước khi đánh giá"),
        "{page}"
    );

    // Launcher báo đã cài (chỉ token của launcher được báo).
    let launcher = LAUNCHER.get().unwrap().clone();
    let a = authorize(&user, &launcher, "openid offline_access profile").await;
    let launcher_token = exchange(&launcher, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let install = |token: String, app: String| async move {
        reqwest::Client::new()
            .post(format!("{BASE}/v1/catalog/installs"))
            .bearer_auth(token)
            .json(&serde_json::json!({ "app_id": app }))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    };
    assert_eq!(install(launcher_token.clone(), notebook.clone()).await, 204);
    assert_eq!(
        install(launcher_token.clone(), format!("khong-co-{stamp}")).await,
        404
    );

    let page = submit(
        &user,
        &review_form,
        &[("rating", "2"), ("body", "Hay treo máy")],
    )
    .await;
    assert!(
        page.contains("Đã lưu nhận xét") && page.contains("★★☆☆☆"),
        "{page}"
    );
    assert!(page.contains("Hà Linh") && page.contains("Hay treo máy"));
    let review_id = catalog::reviews(&st.db, &notebook).await.unwrap()[0].id;

    // Người không phải nhà phát triển không trả lời được (và không thấy biểu mẫu trả lời).
    assert!(!page.contains("/reply\""));
    let csrf_form = parse_form_where(&page, "#review-form");
    let mut fake = csrf_form.fields.clone();
    fake.push(("body".into(), "Mạo danh".into()));
    let body = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fake.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .finish();
    let res = user
        .post(format!("{BASE}/store/{notebook}/reviews/{review_id}/reply"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);

    // --- "Nhà phát triển trả lời": hiện ngay dưới nhận xét, nhãn "Nhà phát triển".
    let page = get(&developer, &format!("/store/{notebook}")).await;
    let reply_form = parse_form_where(
        &page,
        &format!("form[action=\"/store/{notebook}/reviews/{review_id}/reply\"]"),
    );
    let page = submit(
        &developer,
        &reply_form,
        &[("body", "Đã sửa ở bản 1.2.1, cảm ơn bạn")],
    )
    .await;
    let review_at = page.find("Hay treo máy").expect("nhận xét");
    let reply_at = page.find("Đã sửa ở bản 1.2.1").expect("trả lời");
    assert!(review_at < reply_at);
    assert!(page[review_at..reply_at].contains("Nhà phát triển"));

    // --- "Báo cáo vi phạm bản quyền": vào hàng chờ, người báo cáo nhận email khi xử lý.
    let page = get(&user, &format!("/store/{paid}")).await;
    let report_form = parse_form_where(&page, "#report-form");
    let page = submit(
        &user,
        &report_form,
        &[
            ("reason", "copyright"),
            ("details", "Sao chép giao diện của app khác"),
        ],
    )
    .await;
    assert!(page.contains("Đã gửi báo cáo"), "{page}");
    let queue = catalog::open_reports(&st.db).await.unwrap();
    let report = queue
        .iter()
        .find(|r| r.app_id == paid)
        .expect("báo cáo trong hàng chờ");
    assert_eq!(report.reason, "copyright");
    assert!(feco_server::store_web::resolve_and_notify(
        &st,
        report.id,
        true,
        "Đã gỡ phiên bản vi phạm"
    )
    .await
    .unwrap());
    let mail = wait_mail(&email, "Kết quả xử lý báo cáo").await;
    assert!(
        mail.contains(&paid) && mail.contains("Đã gỡ phiên bản vi phạm"),
        "{mail}"
    );

    // Nhận xét vi phạm bị ẩn sau khi quản trị viên xác nhận báo cáo.
    let page = get(&developer, &format!("/store/{notebook}")).await;
    let report_review = parse_form_where(
        &page,
        &format!("#review-{review_id} form[action=\"/store/{notebook}/report\"]"),
    );
    submit(
        &developer,
        &report_review,
        &[("reason", "inappropriate"), ("details", "Spam")],
    )
    .await;
    let report = catalog::open_reports(&st.db)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.review_id == Some(review_id))
        .expect("báo cáo nhận xét");
    feco_server::store_web::resolve_and_notify(&st, report.id, true, "Ẩn nhận xét")
        .await
        .unwrap();
    let page = get(&user, &format!("/store/{notebook}")).await;
    assert!(!page.contains("Hay treo máy"), "nhận xét đã bị ẩn");

    // --- "App bị gỡ": biến khỏi tìm kiếm và trang danh mục trong tối đa 5 phút (đo thời gian).
    // Tìm một lần để kết quả nằm trong bộ nhớ đệm của máy chủ, rồi gỡ từ một tiến trình khác
    // (như khi chạy nhiều bản feco-server).
    let q = "q=so%20tay&category=Gi%C3%A1o%20d%E1%BB%A5c&os=windows".to_owned();
    assert!(catalog_ids(&q).await.contains(&notebook));
    let started = std::time::Instant::now();
    assert!(
        catalog::remove_app(&st.db, &st.catalog, &notebook, "FakeCleaner")
            .await
            .unwrap()
    );
    let res = reqwest::get(format!("{BASE}/store/{notebook}"))
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "trang chi tiết không còn ngay");
    while catalog_ids(&q).await.contains(&notebook) {
        assert!(
            started.elapsed() < Duration::from_secs(5 * 60),
            "quá 5 phút"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let elapsed = started.elapsed();
    eprintln!("app bị gỡ biến khỏi kết quả tìm kiếm sau {elapsed:?}");
    assert!(elapsed <= catalog::CACHE_TTL + Duration::from_secs(5));
    let list = get(&user, "/store?q=so+tay").await;
    assert!(!list.contains(&format!("/store/{notebook}")));
}
