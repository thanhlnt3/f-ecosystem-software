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
use feco_server::{account, kratos::KratosClient, router, AppState};
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
                let vars: HashMap<String, String> = [("FECO_ENV", "dev")]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                let config = Config::from_vars(&vars).unwrap();
                let state = AppState::from_config(&config).unwrap();
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
    account::purge_due(&k, after_29_days).await.unwrap();
    assert!(
        k.identity_exists(&email).await.unwrap(),
        "chưa đủ 30 ngày thì chưa xóa"
    );
    let after_31_days = SystemTime::now() + Duration::from_secs(31 * 24 * 3600);
    assert!(account::purge_due(&k, after_31_days).await.unwrap() >= 1);
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
