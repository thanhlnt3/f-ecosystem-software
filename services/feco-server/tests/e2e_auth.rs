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
/// Bản ghi của máy chủ DNS giả mà máy chủ kiểm thử dùng để xác minh tên miền.
static DNS: OnceLock<feco_server::dns::testing::Records> = OnceLock::new();
/// Kho TUF mà máy chủ kiểm thử ký target khi phát hành.
static TUF_REPO: OnceLock<std::path::PathBuf> = OnceLock::new();
/// Cấu hình của máy chủ kiểm thử, để `local_state` dùng cùng cấu hình.
static SERVER_VARS: OnceLock<HashMap<String, String>> = OnceLock::new();
/// Người nhận cảnh báo kiểm duyệt.
const ADMIN_EMAIL: &str = "kiem-duyet@f-eco.test";
/// Cổng ZaloPay giả của máy chủ kiểm thử (khóa chỉ dùng cho cổng giả này).
static ZALOPAY: OnceLock<feco_server::zalopay::testing::FakeZaloPay> = OnceLock::new();
const ZP_APP_ID: i64 = 2553;
const ZP_KEY1: &str = "e2e-fake-key1";
const ZP_KEY2: &str = "e2e-fake-key2";
/// Cổng VNPAY và MoMo giả (khóa chỉ dùng cho cổng giả).
static VNPAY: OnceLock<feco_server::vnpay::testing::FakeVnpay> = OnceLock::new();
static MOMO: OnceLock<feco_server::momo::testing::FakeMomo> = OnceLock::new();
const VNP_TMN: &str = "FECOTEST";
const VNP_SECRET: &str = "e2e-fake-vnpay-secret";
const MOMO_PARTNER: &str = "MOMOFECOTEST";
const MOMO_ACCESS: &str = "e2e-fake-access";
const MOMO_SECRET: &str = "e2e-fake-momo-secret";
/// Toàn bộ nhật ký của máy chủ kiểm thử.
static LOGS: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());

struct LogTee;

impl std::io::Write for LogTee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        LOGS.lock().unwrap().extend_from_slice(buf);
        let line = String::from_utf8_lossy(buf);
        if line.contains(" WARN ") || line.contains(" ERROR ") {
            eprint!("{line}");
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Dựng kho TUF tạm; trả (thư mục kho, danh sách file khóa targets, snapshot, timestamp).
async fn init_tuf(work: &std::path::Path) -> (std::path::PathBuf, String) {
    use feco_tuf::{keys, Expiry, InitKeys, Repo};
    let mut paths = Vec::new();
    let mut public = Vec::new();
    for name in [
        "root-1",
        "root-2",
        "root-3",
        "targets",
        "snapshot",
        "timestamp",
    ] {
        let path = work.join("keys").join(format!("{name}.pk8"));
        keys::generate(&path).await.unwrap();
        public.push(
            keys::public_key(keys::source(&path).as_ref())
                .await
                .unwrap(),
        );
        paths.push(path);
    }
    let repo = Repo::open(work.join("tuf"));
    let signers: Vec<_> = [0, 1, 3, 4, 5]
        .iter()
        .map(|i| keys::source(&paths[*i]))
        .collect();
    repo.init(
        &InitKeys {
            root: public[..3].to_vec(),
            root_threshold: 2,
            targets: vec![public[3].clone()],
            snapshot: vec![public[4].clone()],
            timestamp: vec![public[5].clone()],
        },
        &signers,
        Expiry::default(),
        jiff::Timestamp::now(),
    )
    .await
    .unwrap();
    let keys = paths[3..]
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(",");
    (repo.dir().to_path_buf(), keys)
}

/// Kiểm thử E2E chỉ chạy khi đặt FECO_E2E=1. Lần đầu gọi sẽ dựng feco-server ở cổng
/// 8080 trên một luồng riêng, dùng chung cho mọi test trong file.
fn e2e_ready() -> bool {
    if std::env::var("FECO_E2E").is_err() {
        eprintln!("bỏ qua: đặt FECO_E2E=1 và chạy Kratos + Mailpit để kiểm thử đầu-cuối");
        return false;
    }
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        // Nhật ký của feco-server (tới mức debug) được giữ lại để quét dữ liệu thẻ (tasks.md 13.6);
        // chỉ dòng cảnh báo và lỗi được in ra.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "warn,feco_server=debug".into()),
            )
            .with_ansi(false)
            .with_writer(|| LogTee)
            .try_init();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let launcher = create_launcher().await;
                LAUNCHER.set(launcher.clone()).unwrap();
                let records = feco_server::dns::testing::Records::default();
                let dns = feco_server::dns::testing::serve("127.0.0.1:0", records.clone())
                    .await
                    .unwrap();
                let _ = DNS.set(records);
                let work = std::env::temp_dir().join(format!("feco-e2e-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&work);
                let (tuf_repo, tuf_keys) = init_tuf(&work).await;
                let _ = TUF_REPO.set(tuf_repo.clone());
                let clamd = feco_server::scan::testing::serve_clamd("127.0.0.1:0")
                    .await
                    .unwrap();
                let zalopay =
                    feco_server::zalopay::testing::FakeZaloPay::serve(ZP_APP_ID, ZP_KEY1, ZP_KEY2)
                        .await
                        .unwrap();
                let zalopay_endpoint = zalopay.endpoint();
                let _ = ZALOPAY.set(zalopay);
                let vnpay = feco_server::vnpay::testing::FakeVnpay::serve(VNP_TMN, VNP_SECRET)
                    .await
                    .unwrap();
                let (vnpay_url, vnpay_api) = (vnpay.payment_url(), vnpay.api_url());
                let _ = VNPAY.set(vnpay);
                let momo = feco_server::momo::testing::FakeMomo::serve(
                    MOMO_PARTNER,
                    MOMO_ACCESS,
                    MOMO_SECRET,
                )
                .await
                .unwrap();
                let momo_endpoint = momo.endpoint();
                let _ = MOMO.set(momo);
                let vars: HashMap<String, String> = [
                    ("FECO_ENV", "dev".to_owned()),
                    ("FECO_LAUNCHER_CLIENT_ID", launcher.clone()),
                    ("FECO_DNS_SERVERS", dns.to_string()),
                    (
                        "FECO_PACKAGE_DIR",
                        work.join("packages").to_string_lossy().into_owned(),
                    ),
                    ("FECO_TUF_REPO", tuf_repo.to_string_lossy().into_owned()),
                    ("FECO_TUF_SIGN_KEYS", tuf_keys),
                    ("FECO_CLAMD_ADDR", clamd.to_string()),
                    ("FECO_ADMIN_EMAILS", ADMIN_EMAIL.to_owned()),
                    ("FECO_ZALOPAY_APP_ID", ZP_APP_ID.to_string()),
                    ("FECO_ZALOPAY_KEY1", ZP_KEY1.to_owned()),
                    ("FECO_ZALOPAY_KEY2", ZP_KEY2.to_owned()),
                    ("FECO_ZALOPAY_ENDPOINT", zalopay_endpoint),
                    ("FECO_VNPAY_TMN_CODE", VNP_TMN.to_owned()),
                    ("FECO_VNPAY_HASH_SECRET", VNP_SECRET.to_owned()),
                    ("FECO_VNPAY_PAYMENT_URL", vnpay_url),
                    ("FECO_VNPAY_API_URL", vnpay_api),
                    ("FECO_MOMO_PARTNER_CODE", MOMO_PARTNER.to_owned()),
                    ("FECO_MOMO_ACCESS_KEY", MOMO_ACCESS.to_owned()),
                    ("FECO_MOMO_SECRET_KEY", MOMO_SECRET.to_owned()),
                    ("FECO_MOMO_ENDPOINT", momo_endpoint),
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
    let vars: HashMap<String, String> = SERVER_VARS.get().cloned().unwrap_or_else(|| {
        [("FECO_ENV", "dev")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    });
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

// ---------- Nhóm 8: cổng nhà phát triển (spec developer-portal) ----------

fn csrf_of(page: &str) -> String {
    Regex::new(r#"name="feco_csrf" value="([^"]+)""#)
        .unwrap()
        .captures(page)
        .unwrap_or_else(|| panic!("trang không có token CSRF:\n{page}"))[1]
        .to_owned()
}

/// Gửi một biểu mẫu; trả mã HTTP và nội dung trang cuối cùng (sau chuyển hướng).
async fn post(
    c: &reqwest::Client,
    path: &str,
    csrf: &str,
    fields: &[(&str, &str)],
) -> (u16, String) {
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
    (res.status().as_u16(), res.text().await.unwrap())
}

async fn verify_email(c: &reqwest::Client, email: &str) {
    let mail = wait_mail(email, "Xác minh email").await;
    let link = Regex::new(r"https?://\S+/self-service/verification\?\S+")
        .unwrap()
        .find(&mail)
        .expect("email có liên kết xác minh")
        .as_str()
        .to_owned();
    let page = get(c, &link).await;
    assert!(page.contains("Bạn đã xác minh email thành công"), "{page}");
}

async fn enable_totp(c: &reqwest::Client) {
    let settings = get(c, "/settings").await;
    let secret = text_of(&settings, "code#totp_secret_key");
    let form = parse_group_form(&settings, "totp");
    let page = submit(
        c,
        &form,
        &[("totp_code", &totp_now(&secret)), ("method", "totp")],
    )
    .await;
    assert!(page.contains("Đã lưu thay đổi."), "{page}");
}

/// Người dùng đủ điều kiện làm nhà phát triển: email đã xác minh, đã bật TOTP.
async fn developer_user(prefix: &str, name: &str) -> (reqwest::Client, String) {
    let (email, password) = unique(prefix);
    let c = app_browser();
    register_in(&c, &email, &password, name).await;
    verify_email(&c, &email).await;
    enable_totp(&c).await;
    (c, email)
}

fn app_manifest(app_id: &str, version: &str) -> String {
    format!(
        r#"id = "{app_id}"
name = "SoundLab"
version = "{version}"
license = "MIT"
privacy_policy = "https://soundlab.example/privacy"
summary = "Thu âm và phối nhạc"
category = "Âm nhạc"

[[platforms]]
os = "windows"
arch = "x64"
entry = "SoundLab.exe"

[scopes.profile]
required = true
purpose = "Hiển thị trên bản nhạc bạn chia sẻ"
purpose_en = "Shown on tracks you share"

[scopes.email]
purpose = "Gửi bản tin"

[permissions]
network = true
microphone = true
"#
    )
}

/// Thư mục `dist/` chứa tệp chạy Windows đã ký hoặc chưa ký.
fn dist(root: &std::path::Path, signed: bool) -> std::path::PathBuf {
    let dir = root.join(if signed {
        "dist-signed"
    } else {
        "dist-unsigned"
    });
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SoundLab.exe"),
        feco_server::codesign::fixtures::pe(signed),
    )
    .unwrap();
    dir
}

async fn f_dev_publish(
    manifest: &std::path::Path,
    package: &std::path::Path,
    token: &str,
) -> Result<String, String> {
    let args: Vec<String> = [
        "publish",
        "--manifest",
        &manifest.to_string_lossy(),
        "--package",
        &package.to_string_lossy(),
        "--server",
        BASE,
        "--token",
        token,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    f_dev::run(&args).await
}

async fn rollout_version(app_id: &str, device: &str) -> String {
    let body: Value = reqwest::Client::new()
        .get(format!(
            "{BASE}/v1/catalog/{app_id}/update?platform=windows-x64&device={device}"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    body["version"].as_str().unwrap_or_default().to_owned()
}

/// Spec developer-portal: đăng ký (email đã xác minh, 2FA, thỏa thuận), xác minh tên miền bằng
/// DNS TXT, client ID và địa chỉ chuyển hướng, nhóm và vai trò, manifest, nộp gói bằng `f-dev`
/// (đã ký và chưa ký), phát hành dần 10%, số liệu tổng hợp, xóa app.
#[tokio::test]
async fn developer_portal_end_to_end() {
    use feco_server::developer;
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let st = local_state();
    let stamp = (SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000_000_000)
        .to_string();
    let app_id = format!("soundlab-{stamp}");
    let domain = format!("acme-{stamp}.vn");
    let work = tempfile::tempdir().unwrap();

    // --- 8.1 Đăng ký: cần email đã xác minh và xác thực hai lớp.
    let (email, password) = unique("owner");
    let owner = app_browser();
    register_in(&owner, &email, &password, "Chủ Acme").await;
    let page = get(&owner, "/developer").await;
    assert!(
        page.contains("Bạn cần xác minh email trước khi đăng ký nhà phát triển"),
        "{page}"
    );
    verify_email(&owner, &email).await;
    let page = get(&owner, "/developer").await;
    assert!(page.contains("Bạn cần bật xác thực hai lớp"), "{page}");
    enable_totp(&owner).await;
    let page = get(&owner, "/developer").await;
    let form = parse_form_where(&page, "#register-form");
    let page = submit(
        &owner,
        &form,
        &[
            ("kind", "organization"),
            ("display_name", &format!("Acme {stamp}")),
            ("legal_name", "Công ty TNHH Acme"),
            ("business_id", "0101234567"),
            ("domain", &domain),
        ],
    )
    .await;
    assert!(
        page.contains("Đã đăng ký nhà phát triển") && page.contains("Chủ sở hữu"),
        "{page}"
    );
    let csrf = csrf_of(&page);
    let new_app: [(&str, &str); 4] = [
        ("app_id", &app_id),
        ("name", "SoundLab"),
        ("kind", "desktop"),
        ("redirects", APP_CALLBACK),
    ];

    // Scenario "Chưa chấp nhận thỏa thuận".
    let (status, body) = post(&owner, "/developer/apps", &csrf, &new_app).await;
    assert_eq!(status, 403, "{body}");
    assert!(
        body.contains("Bạn cần đọc và chấp nhận Thỏa thuận nhà phát triển trước"),
        "{body}"
    );
    let (_, page) = post(
        &owner,
        "/developer/agreement",
        &csrf,
        &[("accept_agreement", "on")],
    )
    .await;
    assert!(
        page.contains("Đã ghi nhận việc chấp nhận thỏa thuận"),
        "{page}"
    );
    let (version, accepted): (Option<String>, Option<time::OffsetDateTime>) = sqlx::query_as(
        "SELECT agreement_version, agreement_accepted_at FROM developer_accounts WHERE domain = $1",
    )
    .bind(&domain)
    .fetch_one(&st.db)
    .await
    .unwrap();
    assert_eq!(version.as_deref(), Some(developer::AGREEMENT_VERSION));
    assert!(accepted.is_some(), "ghi thời điểm chấp nhận");

    // Scenario "Xác minh tên miền".
    let txt = text_of(&page, "#txt-record");
    assert!(txt.starts_with("feco-domain-verification="), "{txt}");
    let (status, body) = post(&owner, "/developer/domain", &csrf, &[]).await;
    assert_eq!(status, 409, "{body}");
    assert!(body.contains("Chưa thấy bản ghi TXT"), "{body}");
    DNS.get()
        .unwrap()
        .lock()
        .unwrap()
        .insert(domain.clone(), vec!["v=spf1 -all".into(), txt.clone()]);
    let (_, page) = post(&owner, "/developer/domain", &csrf, &[]).await;
    assert!(
        page.contains("Tên miền đã được xác minh") && page.contains("id=\"verified\""),
        "{page}"
    );

    // --- 8.2 Client ID và địa chỉ chuyển hướng.
    let (_, page) = post(&owner, "/developer/apps", &csrf, &new_app).await;
    let client_id = text_of(&page, "#client-id");
    assert!(
        !page.contains("id=\"secret\""),
        "app desktop không có secret"
    );
    let http = reqwest::Client::new();
    let client: Value = http
        .get(format!("{HYDRA_ADMIN}/admin/clients/{client_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(client["token_endpoint_auth_method"], "none");
    assert_eq!(client["redirect_uris"][0], APP_CALLBACK);
    let (status, body) = post(
        &owner,
        &format!("/developer/apps/{app_id}/secret"),
        &csrf,
        &[],
    )
    .await;
    assert_eq!(status, 400, "{body}");

    // Scenario "Địa chỉ chuyển hướng lạ": Hydra không cấp mã.
    let (_, challenge) = pkce();
    let mut url = url::Url::parse(&format!("{HYDRA_PUBLIC}/oauth2/auth")).unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid")
        .append_pair("redirect_uri", "http://127.0.0.1:9999/chua-dang-ky")
        .append_pair("state", "trang-thai-du-dai")
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    let res = owner.get(url).send().await.unwrap();
    let (final_url, redirected_to) = (
        res.url().to_string(),
        res.headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned(),
    );
    let body = res.text().await.unwrap();
    assert!(!final_url.contains("code=") && !redirected_to.contains("code="));
    assert!(
        final_url.contains("error") || body.contains("redirect"),
        "Hydra phải từ chối: {final_url}\n{body}"
    );

    // App có máy chủ riêng: client secret hiện một lần và xoay vòng được.
    let server_app = format!("api-{stamp}");
    let (_, page) = post(
        &owner,
        "/developer/apps",
        &csrf,
        &[
            ("app_id", &server_app),
            ("name", "SoundLab API"),
            ("kind", "server"),
            ("redirects", "https://soundlab.example/cb"),
        ],
    )
    .await;
    let server_client = text_of(&page, "#client-id");
    let secret_1 = text_of(&page, "#secret code");
    let (_, page) = post(
        &owner,
        &format!("/developer/apps/{server_app}/secret"),
        &csrf,
        &[],
    )
    .await;
    let secret_2 = text_of(&page, "#secret code");
    assert_ne!(secret_1, secret_2);
    let token_error = |secret: String| {
        let http = http.clone();
        let server_client = server_client.clone();
        async move {
            let v: Value = http
                .post(format!("{HYDRA_PUBLIC}/oauth2/token"))
                .basic_auth(&server_client, Some(&secret))
                .header("content-type", "application/x-www-form-urlencoded")
                .body("grant_type=refresh_token&refresh_token=khong-co")
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            v["error"].as_str().unwrap_or_default().to_owned()
        }
    };
    assert_eq!(
        token_error(secret_1).await,
        "invalid_client",
        "secret cũ hết hiệu lực"
    );
    assert_ne!(
        token_error(secret_2).await,
        "invalid_client",
        "secret mới được chấp nhận"
    );

    // Nhóm và vai trò: mời một người Phát triển và một người Quản trị.
    let (dev_c, dev_email) = developer_user("dev", "Dev Acme").await;
    let (adm_c, adm_email) = developer_user("adm", "Admin Acme").await;
    for (invitee, role) in [(&dev_email, "developer"), (&adm_email, "admin")] {
        let (_, page) = post(
            &owner,
            "/developer/members",
            &csrf,
            &[("email", invitee), ("role", role)],
        )
        .await;
        assert!(page.contains("Đã gửi lời mời"), "{page}");
    }
    assert!(wait_mail(&dev_email, "Lời mời vào nhóm nhà phát triển")
        .await
        .contains("Phát triển"));
    for c in [&dev_c, &adm_c] {
        let page = get(c, "/developer").await;
        let form = parse_form_where(&page, "form.invitation");
        let page = submit(c, &form, &[]).await;
        assert!(page.contains("Bạn đã tham gia nhóm"), "{page}");
    }
    let dev_page = get(&dev_c, "/developer").await;
    assert_eq!(text_of(&dev_page, "#role"), "Phát triển");
    let csrf_dev = csrf_of(&dev_page);
    let csrf_adm = csrf_of(&get(&adm_c, "/developer").await);

    // --- 8.3 Manifest: scenario "Thiếu mục đích của scope" (qua cổng web).
    let manifest_path = format!("/developer/apps/{app_id}/manifest");
    let bad = app_manifest(&app_id, "1.2.0").replace("purpose = \"Gửi bản tin\"\n", "");
    let (status, body) = post(&dev_c, &manifest_path, &csrf_dev, &[("manifest", &bad)]).await;
    assert_eq!(status, 422, "{body}");
    assert!(body.contains("scopes.email.purpose là bắt buộc"), "{body}");

    // --- 8.4 Nộp bằng CLI f-dev: gói chưa ký bị từ chối, gói đã ký vào "Chờ duyệt".
    let (_, page) = post(&dev_c, "/developer/tokens", &csrf_dev, &[("name", "ci")]).await;
    let token = text_of(&page, "#secret code");
    assert!(token.starts_with("fdev_"), "{token}");
    let manifest_file = work.path().join("f-app.toml");
    std::fs::write(&manifest_file, app_manifest(&app_id, "1.2.0")).unwrap();
    let err = f_dev_publish(&manifest_file, &dist(work.path(), false), &token)
        .await
        .expect_err("gói chưa ký");
    assert!(
        err.contains("Gói Windows phải được ký Authenticode"),
        "{err}"
    );
    let err = f_dev_publish(&manifest_file, &dist(work.path(), true), "fdev_sai")
        .await
        .expect_err("token sai");
    assert!(err.contains("401"), "{err}");
    let out = f_dev_publish(&manifest_file, &dist(work.path(), true), &token)
        .await
        .expect("gói đã ký");
    assert!(out.contains("Chờ duyệt"), "{out}");
    let version_id = out
        .rsplit("mã phiên bản: ")
        .next()
        .unwrap()
        .trim()
        .to_owned();
    assert!(version_id.parse::<uuid::Uuid>().is_ok(), "{out}");

    // Chưa duyệt thì chưa phát hành được; bản đầu tiên qua kiểm tra tự động rồi duyệt thủ công.
    let release_path = format!("/developer/versions/{version_id}/release");
    let full: [(&str, &str); 2] = [("channel", "stable"), ("percent", "100")];
    let (status, _) = post(&adm_c, &release_path, &csrf_adm, &full).await;
    assert_eq!(status, 409, "chưa qua kiểm duyệt");
    feco_server::review::process_due(&st).await.unwrap();
    let id = version_id.parse().unwrap();
    feco_server::review::approve(&st, id, "e2e", "Đạt")
        .await
        .unwrap();

    // Scenario "Phát triển viên phát hành".
    let (status, body) = post(&dev_c, &release_path, &csrf_dev, &full).await;
    assert_eq!(status, 403, "{body}");
    assert!(
        body.contains("Cần vai trò Quản trị hoặc Chủ sở hữu"),
        "{body}"
    );
    let (_, page) = post(&adm_c, &release_path, &csrf_adm, &full).await;
    assert!(page.contains("Đang phát hành"), "{page}");

    // Bản phát hành lên F-Store với dấu "Đã xác minh" và scope kèm mục đích từ manifest,
    // và được ký thành target TUF.
    let store = get(&owner, &format!("/store/{app_id}")).await;
    assert!(store.contains("Đã xác minh"), "{store}");
    assert!(
        store.contains(
            "Tên hiển thị và ảnh đại diện (bắt buộc): Hiển thị trên bản nhạc bạn chia sẻ"
        ),
        "{store}"
    );
    assert!(store.contains("https://soundlab.example/privacy"));
    let repo = feco_tuf::Repo::open(TUF_REPO.get().unwrap());
    let tuf =
        feco_tuf::Client::for_local_repo(&repo, &repo.root_path(1), work.path().join("tuf-client"))
            .await
            .unwrap();
    let synced = tuf.sync().await.unwrap();
    let target = &synced.targets[&format!("{app_id}-1.2.0-windows-x64.zip")];
    let meta = target.meta.as_ref().expect("metadata của target");
    assert_eq!(meta.entry.as_deref(), Some("SoundLab.exe"));
    assert_eq!(meta.client_id.as_deref(), Some(client_id.as_str()));

    // Chỉ Chủ sở hữu xóa được app.
    let delete_path = format!("/developer/apps/{app_id}/delete");
    let (status, body) = post(&adm_c, &delete_path, &csrf_adm, &[]).await;
    assert_eq!(status, 403, "{body}");
    assert!(body.contains("Chỉ Chủ sở hữu mới xóa được app"), "{body}");

    // Scenario "Phiên bản không tăng".
    let older = app_manifest(&app_id, "1.1.9");
    let (status, body) = post(&dev_c, &manifest_path, &csrf_dev, &[("manifest", &older)]).await;
    assert_eq!(status, 422, "{body}");
    assert!(
        body.contains("phải lớn hơn bản đã phát hành 1.2.0"),
        "{body}"
    );

    // --- 8.5 Scenario "Phát hành 10%".
    std::fs::write(&manifest_file, app_manifest(&app_id, "1.3.0")).unwrap();
    let out = f_dev_publish(&manifest_file, &dist(work.path(), true), &token)
        .await
        .unwrap();
    let next_id = out
        .rsplit("mã phiên bản: ")
        .next()
        .unwrap()
        .trim()
        .to_owned();
    // Không đổi scope hay quyền: duyệt tự động.
    feco_server::review::process_due(&st).await.unwrap();
    let next_release = format!("/developer/versions/{next_id}/release");
    let (_, page) = post(
        &adm_c,
        &next_release,
        &csrf_adm,
        &[("channel", "stable"), ("percent", "10")],
    )
    .await;
    assert!(page.contains("stable 10%"), "{page}");
    let mut updated = Vec::new();
    for i in 0..200 {
        let device = format!("thiet-bi-{stamp}-{i}");
        match rollout_version(&app_id, &device).await.as_str() {
            "1.3.0" => updated.push(device),
            "1.2.0" => {}
            other => panic!("phiên bản lạ: {other:?}"),
        }
    }
    assert!(
        (8..=36).contains(&updated.len()),
        "{} / 200 thiết bị nhận 1.3.0",
        updated.len()
    );
    // Hỏi lại vẫn cùng kết quả; dừng phát hành dần thì không thiết bị nào nhận bản mới nữa.
    assert_eq!(rollout_version(&app_id, &updated[0]).await, "1.3.0");
    let (_, page) = post(
        &adm_c,
        &format!("/developer/versions/{next_id}/pause"),
        &csrf_adm,
        &[],
    )
    .await;
    assert!(page.contains("Đã dừng phát hành"), "{page}");
    for device in updated.iter().take(5) {
        assert_eq!(rollout_version(&app_id, device).await, "1.2.0");
    }
    post(&adm_c, &next_release, &csrf_adm, &full).await;
    for i in 0..20 {
        assert_eq!(
            rollout_version(&app_id, &format!("thiet-bi-{stamp}-{i}")).await,
            "1.3.0"
        );
    }

    // Nộp qua cổng web: manifest, tải gói (multipart), gửi duyệt.
    let web_manifest = app_manifest(&app_id, "1.4.0");
    let (_, page) = post(
        &dev_c,
        &manifest_path,
        &csrf_dev,
        &[("manifest", &web_manifest)],
    )
    .await;
    assert!(page.contains("Manifest hợp lệ"), "{page}");
    let upload_form = parse_form_where(&page, "form[enctype=\"multipart/form-data\"]");
    let web_upload = |signed: bool| {
        let (c, action, csrf) = (dev_c.clone(), upload_form.action.clone(), csrf_dev.clone());
        async move {
            let mut bytes = Vec::new();
            {
                use std::io::Write;
                let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
                zip.start_file("SoundLab.exe", zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(&feco_server::codesign::fixtures::pe(signed))
                    .unwrap();
                zip.finish().unwrap();
            }
            let form = reqwest::multipart::Form::new()
                .text("feco_csrf", csrf)
                .text("platform", "windows-x64")
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(bytes).file_name("windows-x64.zip"),
                );
            let res = c
                .post(format!("{BASE}{action}"))
                .multipart(form)
                .send()
                .await
                .unwrap();
            (res.status().as_u16(), res.text().await.unwrap())
        }
    };
    let (status, body) = web_upload(false).await;
    assert_eq!(status, 422, "{body}");
    assert!(
        body.contains("Gói Windows phải được ký Authenticode"),
        "{body}"
    );
    let (_, page) = web_upload(true).await;
    assert!(page.contains("Đã nhận gói"), "{page}");
    let web_id = upload_form.action.split('/').nth(3).unwrap().to_owned();
    let (_, page) = post(
        &dev_c,
        &format!("/developer/versions/{web_id}/submit"),
        &csrf_dev,
        &[],
    )
    .await;
    assert!(
        page.contains("Đã gửi duyệt") && page.contains("Chờ duyệt"),
        "{page}"
    );

    // Nhà phát triển cá nhân: không cần tên pháp lý hay tên miền; không mời được thành viên.
    let (solo, _) = developer_user("solo", "Lập Trình Viên").await;
    let page = get(&solo, "/developer").await;
    let form = parse_form_where(&page, "#register-form");
    let page = submit(
        &solo,
        &form,
        &[
            ("kind", "individual"),
            ("display_name", &format!("Solo {stamp}")),
            ("accept_agreement", "on"),
        ],
    )
    .await;
    assert!(
        page.contains("Đã đăng ký nhà phát triển") && page.contains("Chủ sở hữu"),
        "{page}"
    );
    assert!(
        !page.contains("id=\"agreement-form\"") && !page.contains("id=\"txt-record\""),
        "{page}"
    );
    let (status, body) = post(
        &solo,
        "/developer/members",
        &csrf_of(&page),
        &[("email", "ai-do@example.com"), ("role", "developer")],
    )
    .await;
    assert_eq!(status, 400, "{body}");

    // Số liệu tổng hợp: một người dùng cài, mở app và app báo một lần lỗi.
    let (fan_email, fan_password) = unique("fan");
    let fan = app_browser();
    register_in(&fan, &fan_email, &fan_password, "Người Hâm Mộ").await;
    let launcher = LAUNCHER.get().unwrap().clone();
    let a = authorize(&fan, &launcher, "openid offline_access profile").await;
    let launcher_token = exchange(&launcher, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let res = http
        .post(format!("{BASE}/v1/catalog/installs"))
        .bearer_auth(&launcher_token)
        .json(&serde_json::json!({ "app_id": app_id, "version": "1.3.0" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    let a = authorize(&fan, &client_id, "openid profile").await;
    let consent = a.consent_page.clone().expect("màn hình đồng ý");
    assert!(
        consent.contains("Hiển thị trên bản nhạc bạn chia sẻ"),
        "{consent}"
    );
    let a = answer_consent(&fan, a, "accept", &[]).await;
    let app_token = exchange(&client_id, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(issue_launch_code(&launcher_token, &client_id).await.0, 200);
    let res = http
        .post(format!("{BASE}/v1/apps/crashes"))
        .bearer_auth(&app_token)
        .json(&serde_json::json!({ "version": "1.3.0" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    // Scenario "Xem số liệu": số tổng hợp, không có email hay tên người dùng.
    let page = get(&owner, &format!("/developer/apps/{app_id}")).await;
    assert_eq!(text_of(&page, "#m-installs"), "1");
    assert_eq!(text_of(&page, "#m-active7"), "1");
    assert_eq!(text_of(&page, "#m-active30"), "1");
    assert_eq!(text_of(&page, "#m-crashes"), "1");
    assert_eq!(text_of(&page, "#m-versions"), "1.3.0: 1");
    assert!(
        !page.contains(&fan_email) && !page.contains("Người Hâm Mộ"),
        "{page}"
    );

    // Chủ sở hữu xóa app: biến khỏi F-Store, OAuth client bị xóa.
    let (_, page) = post(&owner, &delete_path, &csrf, &[]).await;
    assert!(
        !page.contains(&format!("/developer/apps/{app_id}\"")),
        "{page}"
    );
    let res = reqwest::get(format!("{BASE}/store/{app_id}"))
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let res = http
        .get(format!("{HYDRA_ADMIN}/admin/clients/{client_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ---------- Nhóm 9: kiểm duyệt (spec app-review) ----------

/// Nhà phát triển cá nhân đã chấp nhận thỏa thuận, có một app desktop và token cho `f-dev`.
/// Trả (trình duyệt, email, token).
async fn developer_with_app(
    prefix: &str,
    display: &str,
    app_id: &str,
) -> (reqwest::Client, String, String) {
    let (c, email) = developer_user(prefix, display).await;
    let page = get(&c, "/developer").await;
    let form = parse_form_where(&page, "#register-form");
    let page = submit(
        &c,
        &form,
        &[
            ("kind", "individual"),
            ("display_name", display),
            ("accept_agreement", "on"),
        ],
    )
    .await;
    let csrf = csrf_of(&page);
    let (_, page) = post(
        &c,
        "/developer/apps",
        &csrf,
        &[
            ("app_id", app_id),
            ("name", "SoundLab"),
            ("kind", "desktop"),
            ("redirects", APP_CALLBACK),
        ],
    )
    .await;
    assert!(page.contains("client-id"), "{page}");
    let (_, page) = post(&c, "/developer/tokens", &csrf, &[("name", "ci")]).await;
    (c, email, text_of(&page, "#secret code"))
}

/// Thư mục gói với các tệp cho trước.
fn dist_with(
    root: &std::path::Path,
    name: &str,
    files: Vec<(&str, Vec<u8>)>,
) -> std::path::PathBuf {
    let dir = root.join(name);
    for (file, bytes) in files {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    dir
}

/// Nộp một phiên bản bằng `f-dev`; trả mã phiên bản.
async fn submit_version(
    work: &std::path::Path,
    token: &str,
    manifest: &str,
    dist: &std::path::Path,
) -> uuid::Uuid {
    let file = work.join(format!("f-app-{}.toml", uuid::Uuid::new_v4()));
    std::fs::write(&file, manifest).unwrap();
    let out = f_dev_publish(&file, dist, token)
        .await
        .expect("nộp phiên bản");
    assert!(out.contains("Chờ duyệt"), "{out}");
    out.rsplit("mã phiên bản: ")
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

async fn version_state(st: &AppState, id: uuid::Uuid) -> (String, Option<String>, String) {
    let (status, stage, note): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT status, review_stage, review_note FROM app_versions WHERE id = $1")
            .bind(id)
            .fetch_one(&st.db)
            .await
            .unwrap();
    (status, stage, note.unwrap_or_default())
}

/// Spec app-review: kiểm tra tự động (EICAR qua hai engine, kiến trúc, packer), duyệt thủ công
/// hay tự động, từ chối theo chính sách kèm email và kháng nghị một lần, quét lại định kỳ, gỡ
/// khẩn cấp lan tới danh mục, launcher và nhà phát triển.
#[tokio::test]
async fn app_review_end_to_end() {
    use feco_server::{codesign::fixtures, review, scan};
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let st = local_state();
    let stamp = (SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000_000_000)
        .to_string();
    let work = tempfile::tempdir().unwrap();
    let signed_exe = || ("SoundLab.exe", fixtures::pe(true));

    // --- Scenario "Phát hiện mã độc" (mẫu thử EICAR).
    let bad_app = format!("fakecleaner-{stamp}");
    let (bad_c, bad_email, bad_token) =
        developer_with_app("malware", &format!("Bad {stamp}"), &bad_app).await;
    // Gói zip dựng sẵn (nén deflate): tệp EICAR không bao giờ nằm trần trên đĩa, nên phần mềm
    // diệt virus của máy chạy test không cách ly nó trước khi máy chủ kịp quét.
    let infected = work.path().join("infected");
    std::fs::create_dir_all(&infected).unwrap();
    {
        use std::io::Write;
        let file = std::fs::File::create(infected.join("windows-x64.zip")).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in [signed_exe(), ("data/eicar.com", scan::eicar())] {
            zip.start_file(name, options).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    let id = submit_version(
        work.path(),
        &bad_token,
        &app_manifest(&bad_app, "1.0.0"),
        &infected,
    )
    .await;
    review::process_due(&st).await.unwrap();
    let (status, _, note) = version_state(&st, id).await;
    assert_eq!(status, "rejected");
    assert!(
        note.starts_with("Bị từ chối: mã độc") && note.contains("data/eicar.com"),
        "{note}"
    );
    let page = get(&bad_c, &format!("/developer/apps/{bad_app}")).await;
    assert!(page.contains("Bị từ chối: mã độc"), "{page}");
    let home = get(&bad_c, "/developer").await;
    assert!(home.contains("đang bị tạm khóa chờ điều tra"), "{home}");
    let alert = wait_mail(ADMIN_EMAIL, &format!("Phát hiện mã độc trong {bad_app}")).await;
    assert!(
        alert.contains("tạm khóa") && alert.contains("EICAR"),
        "{alert}"
    );
    assert!(wait_mail(&bad_email, "bị từ chối").await.contains("Mã độc"));
    let err = f_dev_publish(&work.path().join("x.toml"), &infected, &bad_token)
        .await
        .unwrap_err();
    assert!(err.contains("x.toml"), "{err}");
    std::fs::write(
        work.path().join("again.toml"),
        app_manifest(&bad_app, "1.0.1"),
    )
    .unwrap();
    let err = f_dev_publish(&work.path().join("again.toml"), &infected, &bad_token)
        .await
        .unwrap_err();
    assert!(err.contains("đang bị tạm khóa chờ điều tra"), "{err}");

    // Nhà phát triển thứ hai: SoundLab.
    let app = format!("soundlab-r{stamp}");
    let display = format!("Good {stamp}");
    let (c, email, token) = developer_with_app("review", &display, &app).await;

    // --- Scenario "Kiến trúc không khớp".
    let arm = dist_with(
        work.path(),
        "arm",
        vec![("SoundLab.exe", fixtures::pe_for(0xaa64, true, &[]))],
    );
    let id = submit_version(work.path(), &token, &app_manifest(&app, "0.9.0"), &arm).await;
    review::process_due(&st).await.unwrap();
    let (status, _, note) = version_state(&st, id).await;
    assert_eq!(status, "rejected");
    assert!(
        note.contains(
            "Kiến trúc không khớp: f-app.toml khai báo windows-x64 nhưng SoundLab.exe là arm64"
        ),
        "{note}"
    );
    // Gói bị làm rối bằng packer.
    let packed = dist_with(
        work.path(),
        "packed",
        vec![(
            "SoundLab.exe",
            fixtures::pe_for(0x8664, true, &["UPX0", "UPX1"]),
        )],
    );
    let id = submit_version(work.path(), &token, &app_manifest(&app, "0.9.1"), &packed).await;
    review::process_due(&st).await.unwrap();
    let (status, _, note) = version_state(&st, id).await;
    assert_eq!(status, "rejected");
    assert!(note.contains("làm rối bằng UPX"), "{note}");

    // --- Bản đầu tiên qua kiểm tra tự động: vào hàng chờ duyệt thủ công.
    let marker = format!("build {stamp}");
    let clean = dist_with(
        work.path(),
        "clean",
        vec![
            signed_exe(),
            ("data/build.txt", marker.clone().into_bytes()),
        ],
    );
    let v120 = submit_version(work.path(), &token, &app_manifest(&app, "1.2.0"), &clean).await;
    review::process_due(&st).await.unwrap();
    assert_eq!(version_state(&st, v120).await.1.as_deref(), Some("manual"));
    let queue = review::manual_queue(&st.db).await.unwrap();
    let item = queue
        .iter()
        .find(|q| q.version_id == v120)
        .expect("trong hàng chờ");
    assert_eq!(item.reason, "Phiên bản đầu tiên của app");
    let page = get(&c, &format!("/developer/apps/{app}")).await;
    assert!(page.contains("Chờ duyệt thủ công"), "{page}");
    review::approve(&st, v120, "e2e", "Đạt").await.unwrap();
    let csrf = csrf_of(&page);
    let (_, page) = post(
        &c,
        &format!("/developer/versions/{v120}/release"),
        &csrf,
        &[("channel", "stable"), ("percent", "100")],
    )
    .await;
    assert!(page.contains("Đang phát hành"), "{page}");

    // --- Scenario "Bản cập nhật không đổi quyền": 1.2.1 được duyệt tự động.
    let v121 = submit_version(work.path(), &token, &app_manifest(&app, "1.2.1"), &clean).await;
    review::process_due(&st).await.unwrap();
    let (status, stage, note) = version_state(&st, v121).await;
    assert_eq!(
        (status.as_str(), stage.as_deref()),
        ("approved", Some("done"))
    );
    assert!(note.starts_with("Duyệt tự động"), "{note}");

    // --- Scenario "Thêm scope": 1.3.0 thêm `phone` vào hàng chờ duyệt thủ công.
    let with_phone = app_manifest(&app, "1.3.0").replace(
        "[permissions]",
        "[scopes.phone]\npurpose = \"Gửi mã xác nhận\"\n\n[permissions]",
    );
    let v130 = submit_version(work.path(), &token, &with_phone, &clean).await;
    review::process_due(&st).await.unwrap();
    assert_eq!(version_state(&st, v130).await.1.as_deref(), Some("manual"));
    let queue = review::manual_queue(&st.db).await.unwrap();
    assert_eq!(
        queue
            .iter()
            .find(|q| q.version_id == v130)
            .map(|q| q.reason.as_str()),
        Some("Thêm scope dữ liệu: phone")
    );

    // --- Scenario "Bị từ chối": email nêu điều khoản và cách sửa, kèm nút Kháng nghị.
    review::reject(
        &st,
        v130,
        "e2e",
        "undeclared_data",
        "App dùng email để gửi tiếp thị trong khi khai báo \"chỉ để đăng nhập\".",
    )
    .await
    .unwrap();
    let mail = wait_mail(&email, &format!("{app} 1.3.0 bị từ chối")).await;
    assert!(
        mail.contains("Điều 2 chính sách nội dung: Thu thập hoặc dùng dữ liệu trái khai báo"),
        "{mail}"
    );
    assert!(
        mail.contains("Cách khắc phục:") && mail.contains("Kháng nghị"),
        "{mail}"
    );
    assert!(mail.contains(&format!("/developer/apps/{app}")), "{mail}");
    // Kháng nghị một lần qua cổng.
    let page = get(&c, &format!("/developer/apps/{app}")).await;
    let form = parse_form_where(&page, ".appeal-form");
    let page = submit(
        &c,
        &form,
        &[(
            "message",
            "Email chỉ dùng để đăng nhập; chúng tôi đã gỡ bản tin.",
        )],
    )
    .await;
    assert!(page.contains("Đã gửi kháng nghị"), "{page}");
    let (status, body) = post(
        &c,
        &format!("/developer/versions/{v130}/appeal"),
        &csrf_of(&page),
        &[("message", "Lần hai")],
    )
    .await;
    assert_eq!(status, 409, "{body}");
    assert!(
        body.contains("Mỗi lần từ chối chỉ được kháng nghị một lần"),
        "{body}"
    );
    assert!(review::open_appeals(&st.db)
        .await
        .unwrap()
        .iter()
        .any(|a| a.version_id == v130));
    review::resolve_appeal(&st, v130, "e2e", true, "Đã xác minh bản tin đã gỡ")
        .await
        .unwrap();
    assert_eq!(version_state(&st, v130).await.0, "approved");
    assert!(
        wait_mail(&email, &format!("Kết quả kháng nghị {app} 1.3.0"))
            .await
            .contains("được chấp nhận")
    );

    // --- Scenario "Giả mạo thương hiệu" (bản thêm quyền camera nên vào duyệt thủ công).
    let fake_brand = with_phone
        .replace("version = \"1.3.0\"", "version = \"1.4.0\"")
        .replace("name = \"SoundLab\"", "name = \"MegaOffice Pro\"")
        .replace("microphone = true", "microphone = true\ncamera = true");
    let v140 = submit_version(work.path(), &token, &fake_brand, &clean).await;
    review::process_due(&st).await.unwrap();
    assert_eq!(version_state(&st, v140).await.1.as_deref(), Some("manual"));
    review::reject(
        &st,
        v140,
        "e2e",
        "brand_impersonation",
        "Tên và biểu tượng giống bộ ứng dụng văn phòng MegaOffice.",
    )
    .await
    .unwrap();
    let (status, _, note) = version_state(&st, v140).await;
    assert_eq!(status, "rejected");
    assert!(note.contains("Giả mạo thương hiệu"), "{note}");
    let policy = get(&c, "/policy/content").await;
    assert!(
        policy.contains("Giả mạo thương hiệu") && policy.contains("Đào tiền mã hóa ẩn"),
        "{policy}"
    );

    // --- Scenario "Phát hiện muộn": cơ sở dữ liệu mã độc mới nhận ra một tệp trong bản 1.2.0
    // đã phát hành 20 ngày; quản trị viên nhận cảnh báo kèm số người đang cài.
    feco_server::catalog::record_install(&st.db, uuid::Uuid::new_v4(), &app, Some("1.2.0"))
        .await
        .unwrap();
    sqlx::query("INSERT INTO malware_signatures (sha256, name) VALUES ($1, $2)")
        .bind(scan::sha256_hex(marker.as_bytes()))
        .bind(format!("Trojan.Late.{stamp}"))
        .execute(&st.db)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE app_versions SET released_at = now() - interval '20 days',
             last_scanned_at = now() - interval '20 days' WHERE id = $1",
    )
    .bind(v120)
    .execute(&st.db)
    .await
    .unwrap();
    assert!(review::enqueue_rescans(&st.db).await.unwrap() >= 1);
    review::process_due(&st).await.unwrap();
    let alert = wait_mail(
        ADMIN_EMAIL,
        &format!("Quét lại phát hiện mã độc trong {app} 1.2.0"),
    )
    .await;
    assert!(alert.contains("1 người dùng đang cài"), "{alert}");
    assert!(alert.contains("Thu hồi khẩn cấp"), "{alert}");

    // --- Scenario "Thu hồi toàn bộ app" (đo thời gian lan tới danh mục).
    let before = catalog_ids(&format!("q=good%20{stamp}")).await;
    assert!(before.contains(&app), "{before:?}");
    let client_id: String =
        sqlx::query_scalar("SELECT client_id FROM developer_apps WHERE app_id = $1")
            .bind(&app)
            .fetch_one(&st.db)
            .await
            .unwrap();
    let (hook_url, _hooks) = webhook_receiver().await;
    store::register_webhook(&st.db, &st.data_key, &client_id, &hook_url)
        .await
        .unwrap();
    let started = std::time::Instant::now();
    let n = review::revoke(&st, &app, None, "e2e", "Lừa đảo")
        .await
        .unwrap();
    assert!(n >= 2, "thu hồi mọi phiên bản: {n}");
    // Spec app-sdk, "Sự kiện cho app": app nhận webhook app.version.revoked.
    let payload: Value = sqlx::query_scalar(
        "SELECT payload FROM webhook_outbox WHERE client_id = $1 AND event_type = 'app.version.revoked'",
    )
    .bind(&client_id)
    .fetch_one(&st.db)
    .await
    .unwrap();
    assert_eq!(payload["app_id"], app.as_str());
    assert_eq!(payload["reason"], "Lừa đảo");
    assert!(
        payload["versions"].as_array().unwrap().len() >= 2,
        "{payload}"
    );
    let res = reqwest::get(format!("{BASE}/v1/catalog/{app}"))
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    assert!(!catalog_ids(&format!("q={stamp}")).await.contains(&app));
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(5 * 60), "{elapsed:?}");
    eprintln!("app bị thu hồi biến khỏi danh mục sau {elapsed:?}");
    // Launcher: metadata TUF mới có bản ghi thu hồi cho mọi phiên bản đã ký của app.
    let repo = feco_tuf::Repo::open(TUF_REPO.get().unwrap());
    let tuf =
        feco_tuf::Client::for_local_repo(&repo, &repo.root_path(1), work.path().join("tuf-client"))
            .await
            .unwrap();
    let synced = tuf.sync().await.unwrap();
    // Chỉ 1.2.0 đã được phát hành (đã ký thành target).
    {
        let version = "1.2.0";
        let target = format!("{app}-{version}-windows-x64.zip");
        let revoked = synced
            .revocation(&target)
            .unwrap_or_else(|| panic!("thiếu bản ghi thu hồi {target}"));
        assert_eq!(revoked.reason, "Lừa đảo");
        assert!(!synced.targets.contains_key(&target));
    }
    let mail = wait_mail(&email, &format!("{app}: thu hồi khẩn cấp")).await;
    assert!(
        mail.contains("toàn bộ app") && mail.contains("Lừa đảo"),
        "{mail}"
    );
    let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM audit_log WHERE target = $1")
        .bind(&app)
        .fetch_all(&st.db)
        .await
        .unwrap();
    for a in [
        "review.approve",
        "review.reject",
        "review.appeal",
        "review.revoke",
    ] {
        assert!(actions.contains(&a.to_owned()), "{a}: {actions:?}");
    }
}

// ---------- Nhóm 10: F-SDK (spec app-sdk) ----------

/// App con xin cả scope `purchases` (kiểm tra quyền sử dụng).
async fn create_sdk_app(name: &str) -> String {
    let body = serde_json::json!({
        "client_name": name,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": "openid offline_access profile email purchases",
        "redirect_uris": [APP_CALLBACK],
        "token_endpoint_auth_method": "none",
        "subject_type": "pairwise",
        "policy_uri": "https://app.example/privacy",
        "metadata": {"developer": "Kho Việt", "scopes": {
            "profile": {"required": true, "purpose_vi": "Hiển thị tên", "purpose_en": "Show your name"},
            "purchases": {"required": true, "purpose_vi": "Kiểm tra bản quyền", "purpose_en": "Check your licence"}
        }},
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

/// "Trình duyệt hệ thống" của người dùng cho F-SDK: mở trang đăng nhập (đã có phiên), đồng ý nếu
/// được hỏi, rồi theo chuyển hướng về địa chỉ loopback mà F-SDK đang nghe.
fn open_in(user: reqwest::Client) -> impl FnOnce(&url::Url) -> Result<(), String> {
    move |url| {
        let url = url.clone();
        tokio::spawn(async move {
            let res = user.get(url).send().await.unwrap();
            let mut a = finish(res, String::new()).await;
            if a.consent_page.is_some() {
                a = answer_consent(&user, a, "accept", &[]).await;
            }
            let callback = a.callback.expect("phải về callback của app");
            reqwest::get(callback).await.unwrap();
        });
        Ok(())
    }
}

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Thư viện C ABI của F-SDK cho binding Python; tự dựng nếu chưa có.
fn fsdk_c_library() -> std::path::PathBuf {
    let name = if cfg!(windows) {
        "fsdk_c.dll"
    } else if cfg!(target_os = "macos") {
        "libfsdk_c.dylib"
    } else {
        "libfsdk_c.so"
    };
    built("fsdk-c", name)
}

/// Tệp trong `target/debug` do gói `package` dựng ra; tự dựng nếu chưa có.
fn built(package: &str, file: &str) -> std::path::PathBuf {
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| repo_root().join("target"));
    let path = target.join("debug").join(file);
    if !path.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let ok = std::process::Command::new(cargo)
            .args(["build", "-p", package])
            .current_dir(repo_root())
            .status()
            .unwrap()
            .success();
        assert!(ok && path.exists(), "không dựng được {}", path.display());
    }
    path
}

/// App mẫu của một binding: trình chạy (thử lần lượt), tham số, biến môi trường thêm.
struct Example {
    programs: Vec<String>,
    args: Vec<String>,
    env: Vec<(&'static str, String)>,
}

/// Chạy app mẫu của một binding như F-Launcher mở app (biến môi trường kèm mã khởi chạy);
/// trả JSON app in ra, hoặc `None` nếu máy không có trình chạy (trừ khi FECO_E2E_SDKS=1).
fn run_example(programs: &[&str], args: &[String], env: &[(&str, String)]) -> Option<Value> {
    let required = std::env::var("FECO_E2E_SDKS").is_ok();
    for program in programs {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args).current_dir(repo_root());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = match cmd.output() {
            Ok(out) => out,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => panic!("{program}: {e}"),
        };
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{program} {args:?}: {}\n{stdout}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        let start = stdout
            .lines()
            .position(|l| l.starts_with('{'))
            .unwrap_or_else(|| panic!("không có JSON: {stdout}"));
        let json: String = stdout
            .lines()
            .skip(start)
            .take_while(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        return Some(serde_json::from_str(&json).unwrap());
    }
    assert!(!required, "thiếu trình chạy {programs:?}");
    eprintln!("bỏ qua app mẫu {args:?}: máy không có {programs:?}");
    None
}

/// Spec app-sdk: F-SDK Rust đăng nhập qua launcher và qua trình duyệt (PKCE), tự làm mới token,
/// `/v1/me` theo scope, `/v1/entitlements` chỉ của chính app, webhook có chữ ký kiểm được bằng
/// F-SDK, mã lỗi thống nhất; app mẫu Python, TypeScript (và C trong CI) đăng nhập được.
#[tokio::test]
async fn sdk_end_to_end() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let st = local_state();
    let launcher = LAUNCHER.get().unwrap().clone();
    let (email, password) = unique("sdk");
    let user = app_browser();
    register_in(&user, &email, &password, "Lan Phạm").await;
    let a = authorize(&user, &launcher, "openid offline_access profile").await;
    let launcher_token = exchange(&launcher, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();

    let kho = create_sdk_app("QuanLyKho").await;
    let other = create_sdk_app("AppKhac").await;
    let scopes = "openid offline_access profile purchases";
    let config = |client: &str, scopes: &str| {
        fsdk::Config::new(client, &format!("{HYDRA_PUBLIC}/"), &format!("{BASE}/"))
            .unwrap()
            .redirect_uri(APP_CALLBACK)
            .scopes(scopes)
    };

    // --- Scenario "Chạy ngoài launcher": F-SDK mở trình duyệt, PKCE, người dùng đồng ý một lần.
    let browser_session = fsdk::login_with(&config(&kho, scopes), open_in(user.clone()))
        .await
        .expect("đăng nhập qua trình duyệt");
    let me_browser = browser_session.me().await.unwrap();
    assert_eq!(me_browser.name.as_deref(), Some("Lan Phạm"));
    assert!(browser_session.granted_scopes().await.contains("purchases"));
    assert!(browser_session.id_token().await.is_some());

    // --- Scenario "Đăng nhập bằng F-SDK": launcher mở app với mã khởi chạy trong biến môi trường.
    let (status, issued) = issue_launch_code(&launcher_token, &kho).await;
    assert_eq!(status, 200, "{issued}");
    for (k, v) in [
        (
            "FECO_LAUNCH_CODE",
            issued["code"].as_str().unwrap().to_owned(),
        ),
        ("FECO_CLIENT_ID", kho.clone()),
        ("FECO_ISSUER_URL", format!("{HYDRA_PUBLIC}/")),
        ("FECO_SERVER_URL", format!("{BASE}/")),
        ("FSDK_REDIRECT_URI", APP_CALLBACK.to_owned()),
        ("FSDK_SCOPES", scopes.to_owned()),
    ] {
        std::env::set_var(k, v);
    }
    let session = fsdk::session_from_launcher().await;
    for k in [
        "FECO_LAUNCH_CODE",
        "FECO_CLIENT_ID",
        "FECO_ISSUER_URL",
        "FECO_SERVER_URL",
    ] {
        std::env::remove_var(k);
    }
    let session = session.expect("phiên từ launcher");
    let me = session.me().await.unwrap();
    // Scenario "Token chỉ có profile": không có email.
    assert_eq!(me.sub, me_browser.sub, "cùng định danh riêng của app");
    assert_eq!(me.name.as_deref(), Some("Lan Phạm"));
    assert!(
        me.email.is_none() && me.raw.get("email").is_none(),
        "{}",
        me.raw
    );
    assert_ne!(me.sub, subject_of(&email).await.to_string(), "sub pairwise");

    // Tự làm mới token: token sắp hết hạn thì F-SDK đổi refresh token lấy token mới.
    let old_access = session.access_token().await.unwrap();
    let old_refresh = session.refresh_token().await;
    session.expire_now().await;
    let new_access = session.access_token().await.unwrap();
    assert_ne!(old_access, new_access);
    assert_ne!(
        session.refresh_token().await,
        old_refresh,
        "refresh token xoay vòng"
    );
    assert_eq!(session.me().await.unwrap().sub, me.sub);

    // --- Scenario "App được xây theo yêu cầu" + webhook entitlement.changed.
    let (hook_url, hooks) = webhook_receiver().await;
    let secret = store::register_webhook(&st.db, &st.data_key, &kho, &hook_url)
        .await
        .unwrap();
    assert!(session.entitlements().await.unwrap().is_empty());
    let subject = subject_of(&email).await;
    use feco_server::entitlements;
    assert!(
        entitlements::grant(&st, &kho, subject, "owner", "custom-build:DH-0001", None)
            .await
            .unwrap()
    );
    entitlements::grant(&st, &other, subject, "license", "purchase:DH-0002", None)
        .await
        .unwrap();
    let list = session.entitlements().await.unwrap();
    assert_eq!(list.len(), 1, "chỉ thấy quyền với chính app: {list:?}");
    assert_eq!(list[0].kind, "owner");
    assert_eq!(list[0].source.as_deref(), Some("custom-build:DH-0001"));
    feco_server::webhooks::deliver_due(&st.db, &st.http, &st.data_key)
        .await
        .unwrap();
    let got = hooks.lock().unwrap().clone();
    let (_, signature, body) = got
        .iter()
        .find(|(event, ..)| event == "entitlement.changed")
        .unwrap_or_else(|| panic!("thiếu entitlement.changed: {got:?}"));
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    assert!(fsdk::verify_webhook(&secret, signature, body, now));
    assert!(!fsdk::verify_webhook(&secret, signature, b"{}", now));
    assert!(!fsdk::verify_webhook(&secret, signature, body, now + 3600));
    let payload: Value = serde_json::from_slice(body).unwrap();
    assert_eq!(
        (
            payload["sub"].as_str(),
            payload["kind"].as_str(),
            payload["action"].as_str()
        ),
        (Some(me.sub.as_str()), Some("owner"), Some("granted"))
    );

    // Token không có scope purchases: 403 insufficient_scope.
    let (_, issued) = issue_launch_code(&launcher_token, &kho).await;
    let narrow = fsdk::session_from_launch(
        &config(&kho, "openid profile"),
        &fsdk::Launch {
            code: issued["code"].as_str().unwrap().to_owned(),
        },
    )
    .await
    .unwrap();
    match narrow.entitlements().await {
        Err(fsdk::Error::Api(e)) => {
            assert_eq!((e.status, e.error.as_str()), (403, "insufficient_scope"))
        }
        other => panic!("{other:?}"),
    }
    session.report_crash("1.0.0").await.unwrap();

    // --- App mẫu của các binding đăng nhập được (sandbox: biến môi trường như launcher truyền).
    let library = fsdk_c_library();
    let mut examples = vec![
        Example {
            programs: vec!["python3".into(), "python".into()],
            args: vec!["sdk/python/examples/login.py".into()],
            env: vec![("FSDK_LIBRARY", library.to_string_lossy().into_owned())],
        },
        Example {
            programs: vec!["node".into()],
            args: vec!["sdk/typescript/examples/login.ts".into()],
            env: vec![],
        },
    ];
    // Demo App (tools/feco-demo-app) dùng F-SDK khi F-Launcher mở nó.
    let demo_app = if cfg!(windows) {
        "feco-demo-app.exe"
    } else {
        "feco-demo-app"
    };
    examples.push(Example {
        programs: vec![built("feco-demo-app", demo_app)
            .to_string_lossy()
            .into_owned()],
        args: vec![],
        env: vec![],
    });
    if let Ok(c_example) = std::env::var("FSDK_C_EXAMPLE") {
        let lib_dir = library.parent().unwrap().to_string_lossy().into_owned();
        examples.push(Example {
            programs: vec![c_example],
            args: vec![],
            env: vec![("LD_LIBRARY_PATH", lib_dir)],
        });
    }
    for example in examples {
        let (_, issued) = issue_launch_code(&launcher_token, &kho).await;
        let mut env = vec![
            (
                "FECO_LAUNCH_CODE",
                issued["code"].as_str().unwrap().to_owned(),
            ),
            ("FECO_CLIENT_ID", kho.clone()),
            ("FECO_ISSUER_URL", format!("{HYDRA_PUBLIC}/")),
            ("FECO_SERVER_URL", format!("{BASE}/")),
            ("FSDK_REDIRECT_URI", APP_CALLBACK.to_owned()),
            ("FSDK_SCOPES", scopes.to_owned()),
        ];
        env.extend(example.env.iter().cloned());
        let programs: Vec<&str> = example.programs.iter().map(String::as_str).collect();
        let Some(out) = run_example(&programs, &example.args, &env) else {
            continue;
        };
        let me_out = if out.get("me").is_some() {
            &out["me"]
        } else {
            &out
        };
        assert_eq!(me_out["sub"], me.sub.as_str(), "{out}");
        assert_eq!(me_out["name"], "Lan Phạm", "{out}");
        assert!(me_out.get("email").is_none(), "{out}");
        if let Some(list) = out.get("entitlements") {
            assert_eq!(list[0]["kind"], "owner", "{out}");
        }
        eprintln!(
            "app mẫu {:?} đăng nhập được",
            example.args.first().unwrap_or(&example.programs[0])
        );
    }

    // --- Scenario "Token hết hạn" (token đã bị thu hồi): mọi endpoint trả 401 invalid_token.
    let dead = session.access_token().await.unwrap();
    session.logout().await.unwrap();
    let http = reqwest::Client::new();
    for (method, path, body) in [
        ("GET", "/v1/me", Value::Null),
        ("GET", "/v1/entitlements", Value::Null),
        (
            "POST",
            "/v1/apps/crashes",
            serde_json::json!({"version": "1.0.0"}),
        ),
        (
            "POST",
            "/v1/launch-codes",
            serde_json::json!({"client_id": kho}),
        ),
        (
            "POST",
            "/v1/catalog/installs",
            serde_json::json!({"app_id": "x"}),
        ),
    ] {
        let mut req = http
            .request(method.parse().unwrap(), format!("{BASE}{path}"))
            .bearer_auth(&dead);
        if !body.is_null() {
            req = req.json(&body);
        }
        let res = req.send().await.unwrap();
        let status = res.status().as_u16();
        let body: Value = res.json().await.unwrap();
        assert_eq!(status, 401, "{path}: {body}");
        assert_eq!(body["error"], "invalid_token", "{path}: {body}");
        for field in ["message", "message_vi", "request_id"] {
            assert!(
                body[field].as_str().is_some_and(|s| !s.is_empty()),
                "{path} thiếu {field}: {body}"
            );
        }
        assert!(feco_server::errors::documented("invalid_token"));
    }
    match session.me().await {
        Err(fsdk::Error::Api(e)) => {
            assert_eq!((e.status, e.error.as_str()), (401, "invalid_token"));
            assert!(!e.request_id.is_empty());
        }
        other => panic!("{other:?}"),
    }

    // Tài liệu liệt kê mọi mã lỗi kèm cách xử lý, và chính sách phiên bản API.
    let docs = reqwest::get(format!("{BASE}/developer/docs"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let rows = Html::parse_document(&docs)
        .select(&Selector::parse("#error-codes td.code").unwrap())
        .count();
    assert_eq!(rows, feco_server::errors::CODES.len());
    assert!(docs.contains("12 tháng") || docs.contains("12 months"));
}

/// tasks.md 10.4 (spec app-sdk, "Chuẩn mở cho xác thực"): app Python dùng thư viện OIDC phổ biến
/// (Authlib, không dùng F-SDK) khám phá cấu hình, đăng nhập bằng PKCE và kiểm chữ ký ID token qua
/// JWKS; ID token bị sửa thì bị từ chối. Cần `pip install authlib requests` (CI đặt
/// `FECO_E2E_SDKS=1` nên thiếu là lỗi).
#[tokio::test]
async fn standard_oidc_library_integration() {
    if !e2e_ready() {
        return;
    }
    let python = ["python3", "python"].into_iter().find(|p| {
        std::process::Command::new(p)
            .args(["-c", "import authlib, requests"])
            .output()
            .is_ok_and(|o| o.status.success())
    });
    let Some(python) = python else {
        assert!(
            std::env::var("FECO_E2E_SDKS").is_err(),
            "CI phải có Authlib để chạy kiểm thử này"
        );
        eprintln!("bỏ qua: chưa cài authlib, requests (pip install authlib requests)");
        return;
    };
    let _serial = SERIAL.lock().await;
    let (email, password) = unique("oidc");
    let user = app_browser();
    register_in(&user, &email, &password, "Hải Đỗ").await;
    let client_id = create_sdk_app("OidcChuan").await;
    let mut child = std::process::Command::new(python)
        .args([
            "examples/standard-oidc/authlib_login.py",
            &format!("{HYDRA_PUBLIC}/"),
            &client_id,
            APP_CALLBACK,
        ])
        .current_dir(repo_root())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::{BufRead, Write};
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let auth_url = line.trim().to_owned();
    assert!(auth_url.starts_with(HYDRA_PUBLIC), "{auth_url}");
    let res = user.get(&auth_url).send().await.unwrap();
    let mut a = finish(res, String::new()).await;
    if a.consent_page.is_some() {
        a = answer_consent(&user, a, "accept", &[]).await;
    }
    let callback = a.callback.expect("về callback");
    writeln!(child.stdin.take().unwrap(), "{callback}").unwrap();
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut stdout, &mut rest).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{rest}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out: Value = serde_json::from_str(rest.trim()).unwrap();
    assert_eq!(out["aud"], client_id.as_str(), "{out}");
    assert_eq!(out["iss"], format!("{HYDRA_PUBLIC}/"), "{out}");
    assert_eq!(out["tampered_rejected"], true, "{out}");
    let (status, me_body) = me(out["access_token"].as_str().unwrap(), None).await;
    assert_eq!(status, 200);
    assert_eq!(
        me_body["sub"], out["sub"],
        "sub trong ID token trùng /v1/me"
    );
    assert_ne!(me_body["sub"], subject_of(&email).await.to_string());
}

// ---------- Đơn xây app theo yêu cầu (tasks.md 12.1–12.5) ----------

/// Gửi yêu cầu xây app qua biểu mẫu multipart; trả (mã HTTP, trang cuối, địa chỉ cuối).
async fn send_order_request(
    c: &reqwest::Client,
    title: &str,
    accept_tos: bool,
    files: Vec<(&str, reqwest::Body)>,
) -> (u16, String, String) {
    let csrf = csrf_of(&get(c, "/orders").await);
    let mut form = reqwest::multipart::Form::new()
        .text("feco_csrf", csrf)
        .text("title", title.to_owned())
        .text(
            "description",
            "Quản lý nhập, xuất, tồn kho; in mã vạch; báo cáo theo tháng.",
        )
        .text("platform", "windows")
        .text("wanted_by", "2027-03-31")
        .text("budget_vnd", "30.000.000");
    if accept_tos {
        form = form.text("accept_tos", "on");
    }
    for (name, body) in files {
        form = form.part(
            "files",
            reqwest::multipart::Part::stream(body).file_name(name.to_owned()),
        );
    }
    let res = c
        .post(format!("{BASE}/orders"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    let url = res.url().to_string();
    (status, res.text().await.unwrap(), url)
}

fn order_id_of(url: &str) -> uuid::Uuid {
    Regex::new(r"/orders/([0-9a-f-]{36})")
        .unwrap()
        .captures(url)
        .unwrap_or_else(|| panic!("không phải trang đơn: {url}"))[1]
        .parse()
        .unwrap()
}

fn three_milestones(valid_days: Option<i64>) -> feco_server::orders::QuoteInput {
    let m = |title: &str, amount: i64, days: i32| feco_server::orders::MilestoneInput {
        title: title.into(),
        deliverable: format!("Sản phẩm của {title}"),
        amount_vnd: amount,
        days,
        free_revisions: None,
    };
    feco_server::orders::QuoteInput {
        scope: "Nhập, xuất, tồn kho, in mã vạch, báo cáo.".into(),
        exclusions: "Đồng bộ phần mềm kế toán.".into(),
        ownership: "Khách hàng sở hữu toàn bộ mã nguồn sau khi thanh toán đủ.".into(),
        valid_days,
        milestones: vec![
            m("Đặt cọc", 9_000_000, 7),
            m("Bản chạy thử", 12_000_000, 30),
            m("Bản hoàn chỉnh", 9_000_000, 45),
        ],
    }
}

async fn pending_charge(
    db: &feco_server::store::Db,
    order: uuid::Uuid,
) -> (uuid::Uuid, i64, String) {
    sqlx::query_as(
        "SELECT id, amount_vnd, reason FROM payment_requests
         WHERE order_id = $1 AND kind = 'charge' AND status = 'pending' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(order)
    .fetch_one(db)
    .await
    .expect("có yêu cầu thanh toán đang chờ")
}

fn delivery(app: &str, version: &str) -> feco_server::orders::Delivery {
    feco_server::orders::Delivery {
        app_id: app.into(),
        app_version: version.into(),
        notes: format!("Bản {version}"),
        check_report: "Nhập kho: đạt\nXuất kho: đạt".into(),
    }
}

/// Spec custom-build-orders: gửi yêu cầu (tệp, giới hạn 200 MB, Điều khoản dịch vụ), máy trạng
/// thái và lịch sử, báo giá theo mốc (đặt cọc, hết hạn, thương lượng), đặt cọc, bàn giao dạng app
/// riêng tư, lượt sửa, nhắc sau 7 ngày và tự nghiệm thu sau 14 ngày (thời gian mô phỏng), tin nhắn,
/// tranh chấp, bàn giao cuối chỉ khi trả đủ, hủy đơn.
#[tokio::test]
async fn custom_build_order_end_to_end() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    use feco_server::orders::{self, Resolution, Status};
    use time::{Duration as Days, OffsetDateTime};
    let state = local_state();
    let db = state.db.clone();

    // Khách hàng chưa xác minh email: không gửi được yêu cầu.
    let (email, password) = unique("khach-xay-app");
    let user = app_browser();
    register_in(&user, &email, &password, "Chị Hoa").await;
    let page = get(&user, "/orders").await;
    assert!(
        page.contains("Hãy xác minh email trước khi gửi yêu cầu."),
        "{page}"
    );
    verify_email(&user, &email).await;
    let customer = subject_of(&email).await;

    // Chưa chấp nhận Điều khoản dịch vụ: bị từ chối.
    let (status, page, _) = send_order_request(&user, "Thiếu điều khoản", false, vec![]).await;
    assert_eq!(status, 400, "{page}");
    assert!(
        page.contains("Bạn cần chấp nhận Điều khoản dịch vụ xây app"),
        "{page}"
    );

    // Scenario "Tệp quá lớn": tổng vượt 200 MB thì bị từ chối, không tạo đơn.
    let big = vec![0u8; 201 * 1024 * 1024];
    let (status, page, _) = send_order_request(
        &user,
        "Tệp quá lớn",
        true,
        vec![("video.mp4", reqwest::Body::from(big))],
    )
    .await;
    assert_eq!(status, 413, "{page}");
    assert!(page.contains("vượt giới hạn 200 MB"), "{page}");
    assert!(
        orders::list_for(&db, customer).await.unwrap().is_empty(),
        "không tạo đơn khi tệp quá lớn"
    );

    // Scenario "Gửi yêu cầu": đơn "Đã gửi", email xác nhận kèm mã đơn, đội F-Eco có việc báo giá.
    let spec = b"%PDF-1.4 spec kho".to_vec();
    let (status, page, url) = send_order_request(
        &user,
        "App quản lý kho cho cửa hàng",
        true,
        vec![("spec-kho.pdf", reqwest::Body::from(spec.clone()))],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    let id = order_id_of(&url);
    assert!(page.contains(r#"data-status="submitted""#), "{page}");
    assert!(page.contains("spec-kho.pdf"), "{page}");
    let order = orders::get(&db, id).await.unwrap();
    assert_eq!(order.tos_version.as_deref(), Some(orders::TOS_VERSION));
    let mail = wait_mail(&email, &format!("Đã nhận yêu cầu {}", order.code)).await;
    assert!(
        mail.contains(&order.code) && mail.contains("3 ngày làm việc"),
        "{mail}"
    );
    wait_mail(ADMIN_EMAIL, &format!("Đơn {} cần báo giá", order.code)).await;
    // Tệp đính kèm tải lại được, và chỉ chủ đơn thấy đơn.
    let file_id = orders::files(&db, id).await.unwrap()[0].id;
    let bytes = user
        .get(format!("{BASE}/orders/{id}/files/{file_id}"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(bytes.as_ref(), spec.as_slice());
    let (other_email, other_password) = unique("nguoi-khac");
    let other = app_browser();
    register_in(&other, &other_email, &other_password, "Người khác").await;
    let res = other
        .get(format!("{BASE}/orders/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 404);

    // Scenario "Chuyển trạng thái không hợp lệ".
    let mut conn = db.acquire().await.unwrap();
    let err = orders::transition(&mut conn, id, Status::Completed, orders::SYSTEM_ACTOR, "")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid_transition"), "{err}");
    drop(conn);
    assert_eq!(
        orders::get(&db, id).await.unwrap().status,
        Status::Submitted
    );

    // Báo giá đặt cọc quá 30% bị từ chối.
    let mut greedy = three_milestones(None);
    greedy.milestones[0].amount_vnd = 10_000_000;
    greedy.milestones[2].amount_vnd = 8_000_000;
    assert!(
        orders::create_quote(&state, id, "Lan", &greedy, OffsetDateTime::now_utc())
            .await
            .is_err()
    );

    // Scenario "Báo giá hết hạn": nút Chấp nhận bị vô hiệu, có nút "Yêu cầu báo giá lại".
    let past = OffsetDateTime::now_utc() - Days::days(8);
    orders::create_quote(&state, id, "Lan", &three_milestones(None), past)
        .await
        .unwrap();
    wait_mail(&email, &format!("Báo giá cho đơn {}", order.code)).await;
    let page = get(&user, &format!("/orders/{id}")).await;
    assert!(
        page.contains("Khách hàng sở hữu toàn bộ mã nguồn"),
        "quyền sở hữu hiện trước khi chấp nhận"
    );
    assert!(
        page.contains(r#"<button type="submit" disabled>Chấp nhận báo giá</button>"#),
        "{page}"
    );
    assert!(page.contains("Yêu cầu báo giá lại"), "{page}");
    let (status, page) = post(
        &user,
        &format!("/orders/{id}/accept-quote"),
        &csrf_of(&page),
        &[],
    )
    .await;
    assert_eq!(status, 409, "{page}");
    assert!(page.contains("Báo giá đã hết hạn"), "{page}");

    // Scenario "Thương lượng": đơn vẫn "Đã báo giá", đội F-Eco nhận đề nghị.
    let page = get(&user, &format!("/orders/{id}")).await;
    let (_, page) = post(
        &user,
        &format!("/orders/{id}/adjust"),
        &csrf_of(&page),
        &[("note", "Xin giãn mốc 2 thêm 10 ngày")],
    )
    .await;
    assert!(page.contains(r#"data-status="quoted""#), "{page}");
    assert!(page.contains("Xin giãn mốc 2 thêm 10 ngày"), "{page}");
    wait_mail(
        ADMIN_EMAIL,
        &format!("Đơn {} đề nghị điều chỉnh", order.code),
    )
    .await;

    // Scenario "Khách chấp nhận báo giá": báo giá mới 9 + 12 + 9 triệu → yêu cầu thanh toán 9 triệu.
    orders::create_quote(
        &state,
        id,
        "Lan",
        &three_milestones(None),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let page = get(&user, &format!("/orders/{id}")).await;
    assert!(
        page.contains("Báo giá số 2") && page.contains("30.000.000 ₫"),
        "{page}"
    );
    // Chưa trả đủ: bàn giao cuối bị khóa.
    let res = user
        .get(format!("{BASE}/orders/{id}/handover"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 403);
    let (_, page) = post(
        &user,
        &format!("/orders/{id}/accept-quote"),
        &csrf_of(&page),
        &[],
    )
    .await;
    assert!(
        page.contains("Yêu cầu thanh toán tiền đặt cọc đã được tạo"),
        "{page}"
    );
    let (deposit, amount, _) = pending_charge(&db, id).await;
    assert_eq!(amount, 9_000_000);
    assert_eq!(
        orders::get(&db, id).await.unwrap().status,
        Status::Quoted,
        "chỉ bắt đầu khi đặt cọc thành công"
    );

    // Scenario "Đặt cọc thành công": "Đang thực hiện", email kèm lịch các mốc tính từ hôm nay.
    let start = OffsetDateTime::now_utc();
    orders::payment_settled(&state, deposit, true, start)
        .await
        .unwrap();
    let o = orders::get(&db, id).await.unwrap();
    assert_eq!(o.status, Status::InProgress);
    let mail = wait_mail(&email, &format!("Đơn {} bắt đầu thực hiện", order.code)).await;
    assert!(mail.contains("Mốc 3: Bản hoàn chỉnh"), "{mail}");
    let q = orders::current_quote(&db, id).await.unwrap().unwrap();
    assert_eq!(
        q.milestones[1].due_at.unwrap().date(),
        (start + Days::days(30)).date()
    );

    // App riêng tư: không hiện công khai, chỉ khách hàng của đơn thấy qua launcher.
    let app = format!("kho-{}", &id.simple().to_string()[..8]);
    let client_id = uuid::Uuid::new_v4().to_string();
    feco_server::catalog::upsert_app(
        &db,
        &state.catalog,
        &catalog_app(&app, &client_id, "Kho Cửa Hàng"),
    )
    .await
    .unwrap();
    // Đặt riêng tư trước khi phát hành, để app không hiện công khai lúc nào.
    feco_server::catalog::set_private(&db, &state.catalog, &app, Some(customer))
        .await
        .unwrap();
    for v in ["0.1.0", "0.2.0", "0.2.1", "0.2.2", "1.0.0"] {
        feco_server::catalog::release_version(
            &db,
            &state.catalog,
            &app,
            &version(v, "windows-x64"),
        )
        .await
        .unwrap();
    }
    assert!(
        !catalog_ids("q=Kho").await.contains(&app),
        "app riêng tư không hiện công khai"
    );
    let launcher = LAUNCHER.get().unwrap().clone();
    let a = authorize(&user, &launcher, "openid offline_access profile").await;
    let token = exchange(&launcher, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let private: Value = reqwest::Client::new()
        .get(format!("{BASE}/v1/catalog/private"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        private["apps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == app.as_str()),
        "{private}"
    );
    assert!(
        feco_server::catalog::private_for(&db, subject_of(&other_email).await)
            .await
            .unwrap()
            .is_empty()
    );

    // Mốc 1: bàn giao (bản build qua launcher, ghi chú, kết quả kiểm tra) rồi khách nghiệm thu
    // → yêu cầu thanh toán mốc 2.
    assert!(
        orders::deliver(&state, id, 1, "Lan", &delivery(&app, "9.9.9"), start)
            .await
            .is_err(),
        "bản chưa phát hành"
    );
    orders::deliver(&state, id, 1, "Lan", &delivery(&app, "0.1.0"), start)
        .await
        .unwrap();
    assert!(feco_server::entitlements::active(&db, &client_id, customer)
        .await
        .unwrap()
        .iter()
        .any(|e| e.kind == "license"));
    let page = get(&user, &format!("/orders/{id}")).await;
    assert!(
        page.contains(r#"data-status="delivered""#) && page.contains(&format!("{app} 0.1.0")),
        "{page}"
    );
    assert!(page.contains("Nhập kho: đạt"), "{page}");
    let (_, page) = post(
        &user,
        &format!("/orders/{id}/accept-milestone"),
        &csrf_of(&page),
        &[("milestone", "1")],
    )
    .await;
    assert!(page.contains(r#"data-status="in_progress""#), "{page}");
    let (m2, amount, _) = pending_charge(&db, id).await;
    assert_eq!(
        amount, 12_000_000,
        "Scenario \"Nghiệm thu mốc\": yêu cầu thanh toán mốc kế tiếp"
    );
    orders::payment_settled(&state, m2, true, start)
        .await
        .unwrap();

    // Scenario "Yêu cầu sửa trong giới hạn": mốc 2 còn 2 lượt → quay lại thực hiện, còn 1 lượt.
    orders::deliver(&state, id, 2, "Lan", &delivery(&app, "0.2.0"), start)
        .await
        .unwrap();
    let page = get(&user, &format!("/orders/{id}")).await;
    let (_, page) = post(
        &user,
        &format!("/orders/{id}/revise"),
        &csrf_of(&page),
        &[("milestone", "2"), ("note", "Xuất kho âm khi bấm hai lần")],
    )
    .await;
    assert!(page.contains(r#"data-status="in_progress""#), "{page}");
    let left = |q: orders::Quote| q.milestones[1].revisions_left();
    assert_eq!(
        left(orders::current_quote(&db, id).await.unwrap().unwrap()),
        1
    );
    wait_mail(
        ADMIN_EMAIL,
        &format!("Đơn {} yêu cầu sửa mốc 2", order.code),
    )
    .await;
    orders::deliver(&state, id, 2, "Lan", &delivery(&app, "0.2.1"), start)
        .await
        .unwrap();
    assert_eq!(
        orders::request_revision(&state, customer, id, 2, "Thiếu nút in")
            .await
            .unwrap(),
        0
    );
    // Scenario "Hết lượt sửa miễn phí": cần báo giá bổ sung.
    orders::deliver(&state, id, 2, "Lan", &delivery(&app, "0.2.2"), start)
        .await
        .unwrap();
    let page = get(&user, &format!("/orders/{id}")).await;
    let (status, page) = post(
        &user,
        &format!("/orders/{id}/revise"),
        &csrf_of(&page),
        &[("milestone", "2"), ("note", "Đổi màu nút")],
    )
    .await;
    assert_eq!(status, 409, "{page}");
    assert!(page.contains("cần báo giá bổ sung"), "{page}");

    // Nghiệm thu theo thời gian (mô phỏng): nhắc sau 7 ngày, tự nghiệm thu sau 14 ngày.
    // (Đơn còn sót từ lần chạy trước có thể cũng được xử lý, nên chỉ xét đơn này.)
    orders::process_due(&state, start + Days::days(6))
        .await
        .unwrap();
    assert_eq!(
        mail_count(
            &email,
            &format!("Nhắc nghiệm thu mốc 2 của đơn {}", order.code)
        )
        .await,
        0
    );
    orders::process_due(&state, start + Days::days(7) + Days::hours(1))
        .await
        .unwrap();
    assert_eq!(
        orders::get(&db, id).await.unwrap().status,
        Status::Delivered,
        "chưa tự nghiệm thu trước 14 ngày"
    );
    wait_mail(
        &email,
        &format!("Nhắc nghiệm thu mốc 2 của đơn {}", order.code),
    )
    .await;
    orders::process_due(&state, start + Days::days(8))
        .await
        .unwrap();
    assert_eq!(
        mail_count(
            &email,
            &format!("Nhắc nghiệm thu mốc 2 của đơn {}", order.code)
        )
        .await,
        1,
        "chỉ nhắc một lần"
    );
    assert!(
        orders::process_due(&state, start + Days::days(14) + Days::hours(1))
            .await
            .unwrap()
            >= 1
    );
    let history = orders::history(&db, id).await.unwrap();
    assert!(
        history
            .iter()
            .any(|e| e.actor == orders::SYSTEM_ACTOR && e.note == "Nghiệm thu mốc 2"),
        "{history:?}"
    );
    let (m3, amount, _) = pending_charge(&db, id).await;
    assert_eq!(amount, 9_000_000);

    // Scenario "Gửi tin nhắn": đội F-Eco hỏi, khách nhận email và trả lời ngay trong đơn (có tệp).
    orders::post_message(
        &state,
        id,
        Some("Lan"),
        "Chị nhập kho theo lô hay theo từng món ạ?",
    )
    .await
    .unwrap();
    wait_mail(&email, &format!("Tin nhắn mới trong đơn {}", order.code)).await;
    let page = get(&user, &format!("/orders/{id}")).await;
    assert!(
        page.contains("Chị nhập kho theo lô hay theo từng món ạ?"),
        "{page}"
    );
    let form = reqwest::multipart::Form::new()
        .text("feco_csrf", csrf_of(&page))
        .text("body", "Theo lô, mẫu phiếu đính kèm.")
        .part(
            "files",
            reqwest::multipart::Part::bytes(b"lo,ma,sl".to_vec()).file_name("phieu-nhap.csv"),
        );
    let page = user
        .post(format!("{BASE}/orders/{id}/messages"))
        .multipart(form)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        page.contains("Theo lô, mẫu phiếu đính kèm.") && page.contains("phieu-nhap.csv"),
        "{page}"
    );
    wait_mail(
        ADMIN_EMAIL,
        &format!("Tin nhắn mới trong đơn {}", order.code),
    )
    .await;

    // Scenario "Mở tranh chấp" cho mốc 3: quản trị viên có việc kèm hạn 2 ngày làm việc; giải
    // quyết bằng hoàn một phần.
    orders::payment_settled(&state, m3, true, start)
        .await
        .unwrap();
    let delivered = OffsetDateTime::now_utc();
    orders::deliver(&state, id, 3, "Lan", &delivery(&app, "1.0.0"), delivered)
        .await
        .unwrap();
    let page = get(&user, &format!("/orders/{id}")).await;
    let (_, page) = post(
        &user,
        &format!("/orders/{id}/dispute"),
        &csrf_of(&page),
        &[
            ("milestone", "3"),
            ("note", "Báo cáo tháng không đúng spec mục 4"),
        ],
    )
    .await;
    assert!(page.contains(r#"data-status="disputed""#), "{page}");
    let respond_by = orders::business_days_after(OffsetDateTime::now_utc(), 2);
    let alert = wait_mail(ADMIN_EMAIL, &format!("Tranh chấp đơn {} mốc 3", order.code)).await;
    assert!(
        alert.contains(
            &respond_by
                .format(&time::macros::format_description!("[day]/[month]/[year]"))
                .unwrap()
        ),
        "{alert}"
    );
    let refund = orders::resolve_dispute(&state, id, "admin", Resolution::PartialRefund(2_000_000))
        .await
        .unwrap();
    assert!(refund.is_some());
    assert_eq!(
        orders::get(&db, id).await.unwrap().status,
        Status::Delivered
    );
    wait_mail(&email, &format!("Kết quả tranh chấp đơn {}", order.code)).await;
    // Quá 14 ngày sau bàn giao: không mở tranh chấp được nữa.
    assert!(
        orders::open_dispute(&state, customer, id, 3, "muộn", delivered + Days::days(15))
            .await
            .is_err()
    );

    // Nghiệm thu mốc cuối → Hoàn tất; đã trả đủ → tải được bàn giao cuối.
    let page = get(&user, &format!("/orders/{id}")).await;
    let (_, page) = post(
        &user,
        &format!("/orders/{id}/accept-milestone"),
        &csrf_of(&page),
        &[("milestone", "3")],
    )
    .await;
    assert!(page.contains(r#"data-status="completed""#), "{page}");
    assert!(page.contains(r#"id="handover""#), "{page}");
    let zip = orders::handover_path(&state, &orders::get(&db, id).await.unwrap());
    std::fs::create_dir_all(zip.parent().unwrap()).unwrap();
    std::fs::write(&zip, b"PK\x05\x06 ban giao").unwrap();
    let res = user
        .get(format!("{BASE}/orders/{id}/handover"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(res.headers()["content-type"], "application/zip");
    assert_eq!(res.bytes().await.unwrap().as_ref(), b"PK\x05\x06 ban giao");
    // Lịch sử đầy đủ: ai, khi nào, từ đâu sang đâu.
    let page = get(&user, &format!("/orders/{id}")).await;
    for step in [
        "Nháp",
        "Đã gửi",
        "Đã báo giá",
        "Đang thực hiện",
        "Đã bàn giao",
        "Đang tranh chấp",
        "Hoàn tất",
        "Đội F-Eco (Lan)",
        "Hệ thống",
    ] {
        assert!(page.contains(step), "lịch sử thiếu {step}");
    }

    // Scenario "Hủy khi đang ở mốc 2": hoàn mốc 2, giữ tiền đặt cọc.
    let (_, page, url) = send_order_request(&user, "App bán hàng", true, vec![]).await;
    assert!(page.contains(r#"data-status="submitted""#), "{page}");
    let second = order_id_of(&url);
    orders::create_quote(
        &state,
        second,
        "Lan",
        &three_milestones(None),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    orders::accept_quote(&state, customer, second, OffsetDateTime::now_utc())
        .await
        .unwrap();
    let (dep, _, _) = pending_charge(&db, second).await;
    orders::payment_settled(&state, dep, true, OffsetDateTime::now_utc())
        .await
        .unwrap();
    orders::deliver(
        &state,
        second,
        1,
        "Lan",
        &delivery(&app, "0.1.0"),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    orders::accept_milestone(
        &state,
        second,
        1,
        &orders::customer_actor(customer),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let (m2, _, _) = pending_charge(&db, second).await;
    orders::payment_settled(&state, m2, true, OffsetDateTime::now_utc())
        .await
        .unwrap();
    let page = get(&user, &format!("/orders/{second}")).await;
    let (_, page) = post(
        &user,
        &format!("/orders/{second}/cancel"),
        &csrf_of(&page),
        &[("note", "Đổi kế hoạch")],
    )
    .await;
    assert!(page.contains(r#"data-status="cancelled""#), "{page}");
    let refunds: Vec<(i64, String)> = sqlx::query_as(
        "SELECT amount_vnd, reason FROM payment_requests WHERE order_id = $1 AND kind = 'refund'",
    )
    .bind(second)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(
        refunds,
        vec![(12_000_000, "Hủy đơn: hoàn mốc 2".to_owned())],
        "giữ tiền đặt cọc"
    );

    // Hủy trước khi đặt cọc: không mất phí, không có yêu cầu thanh toán nào còn chờ.
    let (_, _, url) = send_order_request(&user, "App chấm công", true, vec![]).await;
    let third = order_id_of(&url);
    orders::create_quote(
        &state,
        third,
        "Lan",
        &three_milestones(None),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    orders::accept_quote(&state, customer, third, OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert!(orders::cancel(&state, customer, third, "")
        .await
        .unwrap()
        .is_empty());
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM payment_requests WHERE order_id = $1 AND status = 'pending'",
    )
    .bind(third)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(pending, 0);
    assert!(
        orders::cancel(&state, customer, third, "").await.is_err(),
        "đã hủy thì là điểm cuối"
    );

    // Điều khoản dịch vụ xây app (bản nháp, phiên bản đang áp dụng).
    let terms = get(&reqwest::Client::new(), "/policy/custom-build").await;
    assert!(
        terms.contains(orders::TOS_VERSION) && terms.contains("Quyền sở hữu mã nguồn"),
        "{terms}"
    );
}

// ---------- Thanh toán và hóa đơn (tasks.md 13.1–13.6) ----------

/// Chờ email có tiêu đề chứa `subject`; trả nội dung chữ và tên các tệp đính kèm.
async fn wait_mail_with_files(to: &str, subject: &str) -> (String, Vec<String>) {
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
            let files = msg["Attachments"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| a["FileName"].as_str().map(str::to_owned))
                .collect();
            return (msg["Text"].as_str().unwrap_or_default().to_owned(), files);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("không thấy email tới {to} với tiêu đề chứa {subject:?}");
}

/// Khách chọn phương thức trên trang thanh toán; trả mã giao dịch (`app_trans_id`) mà F-Eco tạo
/// trên cổng (trình duyệt được chuyển tới trang của cổng giả).
async fn start_payment(c: &reqwest::Client, request: uuid::Uuid, method: &str) -> String {
    start_payment_url(c, request, method).await.0
}

/// Như `start_payment`, kèm địa chỉ cổng mà trình duyệt được chuyển tới.
async fn start_payment_url(
    c: &reqwest::Client,
    request: uuid::Uuid,
    method: &str,
) -> (String, String) {
    let page = get(c, &format!("/payments/{request}/pay")).await;
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("feco_csrf", &csrf_of(&page))
        .append_pair("method", method)
        .finish();
    let res = c
        .post(format!("{BASE}/payments/{request}/pay"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();
    let url = res.url().clone();
    if let Some((_, r)) = url.query_pairs().find(|(k, _)| k == "vnp_TxnRef") {
        return (r.into_owned(), url.to_string());
    }
    assert!(
        url.path().starts_with("/pay/"),
        "phải chuyển sang trang của cổng: {url} {}",
        res.text().await.unwrap()
    );
    (
        url.path().trim_start_matches("/pay/").to_owned(),
        url.to_string(),
    )
}

async fn zp_callback(body: &Value) -> Value {
    reqwest::Client::new()
        .post(format!("{BASE}/v1/payments/zalopay/callback"))
        .json(body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// Toàn bộ dữ liệu trong cơ sở dữ liệu, mỗi dòng một bản ghi JSON.
async fn dump_database(db: &feco_server::store::Db) -> String {
    let tables: Vec<String> = sqlx::query_scalar("SELECT table_name::text FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE'")
        .fetch_all(db)
        .await
        .unwrap();
    let mut out = String::new();
    for t in tables {
        let rows: Option<String> = sqlx::query_scalar(&format!(
            "SELECT string_agg(row_to_json(x)::text, E'\\n') FROM \"{t}\" x"
        ))
        .fetch_one(db)
        .await
        .unwrap();
        out.push_str(&rows.unwrap_or_default());
        out.push('\n');
    }
    out
}

/// Spec payments: chọn phương thức, vòng đời giao dịch (webhook có chữ ký, trang chuyển hướng giả,
/// liên kết hết hạn sau 15 phút), không thu hai lần, hoàn tiền (một phần, vượt số đã thu, hai người
/// duyệt), hóa đơn cho khách doanh nghiệp, lịch sử và tải lại hóa đơn, đối soát giao dịch thiếu
/// webhook, và quét nhật ký cùng cơ sở dữ liệu sau một giao dịch thẻ thử.
#[tokio::test]
async fn payments_end_to_end() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    use feco_server::{
        orders::{self, Status},
        payments::{self, Requester, Settled, TxStatus},
    };
    use time::{Duration as Span, OffsetDateTime};
    let state = local_state();
    let db = state.db.clone();
    let fake = ZALOPAY.get().unwrap().clone();

    let (email, password) = unique("khach-thanh-toan");
    let user = app_browser();
    register_in(&user, &email, &password, "Anh Nam").await;
    verify_email(&user, &email).await;
    let customer = subject_of(&email).await;

    // Thông tin xuất hóa đơn: doanh nghiệp cần mã số thuế hợp lệ.
    let page = get(&user, "/payments").await;
    assert!(page.contains("Bạn chưa có giao dịch nào."), "{page}");
    let billing = |tax: &'static str| {
        vec![
            ("kind", "company"),
            ("name", "Công ty TNHH Kho Vui"),
            ("tax_code", tax),
            ("address", "12 Láng Hạ, Hà Nội"),
            ("email", email.as_str()),
        ]
    };
    let (status, page) = post(
        &user,
        "/payments/billing",
        &csrf_of(&page),
        &billing("01012345"),
    )
    .await;
    assert_eq!(status, 400, "{page}");
    assert!(page.contains("mã số thuế hợp lệ"), "{page}");
    let (_, page) = post(
        &user,
        "/payments/billing",
        &csrf_of(&page),
        &billing("0101234567"),
    )
    .await;
    assert!(page.contains("Đã lưu thông tin xuất hóa đơn."), "{page}");

    // Đơn 1 (9 + 12 + 9 triệu), khách chấp nhận báo giá: có yêu cầu thanh toán 9 triệu.
    let (_, _, url) = send_order_request(&user, "App bán hàng có hóa đơn", true, vec![]).await;
    let order = order_id_of(&url);
    let now = OffsetDateTime::now_utc();
    orders::create_quote(&state, order, "Lan", &three_milestones(None), now)
        .await
        .unwrap();
    orders::accept_quote(&state, customer, order, now)
        .await
        .unwrap();
    let (deposit, _, _) = pending_charge(&db, order).await;

    // Scenario "Chọn phương thức": số tiền, nội dung mốc, bốn phương thức.
    let page = get(&user, &format!("/orders/{order}")).await;
    assert!(
        page.contains(&format!("/payments/{deposit}/pay")),
        "trang đơn có nút Thanh toán"
    );
    let page = get(&user, &format!("/payments/{deposit}/pay")).await;
    assert!(
        page.contains("9.000.000 ₫") && page.contains("Mốc 1: Đặt cọc"),
        "{page}"
    );
    for m in [
        "QR chuyển khoản (VietQR)",
        "Thẻ ATM nội địa",
        "Ví điện tử ZaloPay",
        "Thẻ quốc tế (Visa, Mastercard, JCB)",
    ] {
        assert!(page.contains(m), "thiếu phương thức {m}");
    }
    // Người khác không mở được trang thanh toán của khách này.
    let (other_email, other_password) = unique("khac-thanh-toan");
    let other = app_browser();
    register_in(&other, &other_email, &other_password, "Người khác").await;
    assert_eq!(
        other
            .get(format!("{BASE}/payments/{deposit}/pay"))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        404
    );

    // Scenario "Liên kết hết hạn": quá 15 phút chưa trả → Hết hạn, tạo được liên kết mới.
    let first = start_payment(&user, deposit, "zalopay:domestic_card").await;
    assert_eq!(
        start_payment(&user, deposit, "zalopay:domestic_card").await,
        first,
        "liên kết còn hạn được dùng lại (idempotent)"
    );
    let created = fake.order(&first).unwrap();
    assert_eq!(created.amount, 9_000_000);
    assert!(
        created.embed_data.contains("domestic_card")
            && created.embed_data.contains("/payments/return"),
        "{}",
        created.embed_data
    );
    assert!(
        !created.description.contains(&email),
        "không gửi email khách cho cổng"
    );
    payments::process_due(&state, OffsetDateTime::now_utc() + Span::minutes(16))
        .await
        .unwrap();
    assert_eq!(
        payments::by_ref(&state, &first).await.unwrap().status,
        TxStatus::Expired
    );
    let second = start_payment(&user, deposit, "zalopay:domestic_card").await;
    assert_ne!(second, first, "liên kết mới cho cùng mốc");
    assert_eq!(payments::by_ref(&state, &second).await.unwrap().attempt, 2);

    // Scenario "Trang chuyển hướng bị giả": tham số "thành công" không có tác dụng.
    let fake_return = get(
        &user,
        &format!("/payments/return?apptransid={second}&status=1&checksum=gia"),
    )
    .await;
    assert!(
        fake_return.contains(r#"data-status="pending""#),
        "{fake_return}"
    );
    assert_eq!(
        payments::by_ref(&state, &second).await.unwrap().status,
        TxStatus::Pending
    );
    assert_eq!(
        orders::get(&db, order).await.unwrap().status,
        Status::Quoted
    );

    // Khách đổi sang VietQR (lần thử 3) và mở thêm thẻ quốc tế (lần thử 4) ở tab khác.
    let qr = start_payment(&user, deposit, "zalopay:vietqr").await;
    let card = start_payment(&user, deposit, "zalopay:international_card").await;

    // Scenario "Khách đóng trình duyệt sau khi trả tiền" + "Webhook đến hai lần": trả bằng thẻ
    // trên trang của cổng, không quay về; cổng gửi webhook đã ký ba lần.
    const PAN: &str = "4111111111111111";
    const CVV: &str = "987";
    let mut callback = fake.pay(&card, 36, Some(PAN));
    // Phòng trường hợp cổng gửi kèm dữ liệu thẻ: F-Eco phải lọc trước khi lưu.
    let mut data: Value = serde_json::from_str(callback["data"].as_str().unwrap()).unwrap();
    data["card_number"] = Value::from(PAN);
    data["cvv"] = Value::from(CVV);
    let data = data.to_string();
    callback = serde_json::json!({"data": data, "mac": feco_server::zalopay::hmac_hex(ZP_KEY2, &data), "type": 1});
    let mut forged = callback.clone();
    forged["mac"] = Value::from(feco_server::zalopay::hmac_hex("khoa-gia", &data));
    assert_eq!(
        zp_callback(&forged).await["return_code"],
        -1,
        "chữ ký sai bị từ chối"
    );
    assert_eq!(
        payments::by_ref(&state, &card).await.unwrap().status,
        TxStatus::Pending
    );
    for _ in 0..3 {
        assert_eq!(zp_callback(&callback).await["return_code"], 1);
    }
    let paid_card = payments::by_ref(&state, &card).await.unwrap();
    assert_eq!(paid_card.status, TxStatus::Succeeded);
    let o = orders::get(&db, order).await.unwrap();
    assert_eq!(
        o.status,
        Status::InProgress,
        "đơn được cập nhật như bình thường"
    );
    wait_mail(&email, &format!("Đã nhận thanh toán đơn {}", o.code)).await;
    assert_eq!(
        mail_count(&email, &format!("Đã nhận thanh toán đơn {}", o.code)).await,
        1,
        "chỉ ghi nhận một lần"
    );
    let succeeded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM payment_transactions WHERE request_id = $1 AND status = 'succeeded'",
    )
    .bind(deposit)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(succeeded, 1);

    // Scenario "Khách doanh nghiệp": hóa đơn mang tên công ty và mã số thuế, gửi qua email.
    let (mail, files) =
        wait_mail_with_files(&email, &format!("Hóa đơn thanh toán đơn {}", o.code)).await;
    assert!(
        mail.contains("Công ty TNHH Kho Vui") && mail.contains("0101234567"),
        "{mail}"
    );
    assert!(
        files
            .iter()
            .any(|f| f.starts_with("FECO-NHAP-") && f.ends_with(".html")),
        "{files:?}"
    );

    // Trang chuyển về sau khi đã trả: Thành công.
    let back = get(&user, &format!("/payments/return?apptransid={card}")).await;
    assert!(back.contains(r#"data-status="succeeded""#), "{back}");

    // Scenario "Khách trả hai lần": trả tiếp bằng QR → giao dịch thứ hai tự hoàn, quản trị viên được báo.
    let second_pay = fake.pay(&qr, 38, None);
    assert_eq!(zp_callback(&second_pay).await["return_code"], 1);
    let twice = payments::by_ref(&state, &qr).await.unwrap();
    assert_eq!(twice.status, TxStatus::Succeeded);
    assert_eq!(
        twice.refunded_vnd, 9_000_000,
        "đã hoàn toàn bộ giao dịch thứ hai"
    );
    assert_eq!(
        fake.refunded(twice.gateway_trans_id.as_deref().unwrap()),
        9_000_000,
        "cổng nhận lệnh hoàn"
    );
    assert_eq!(
        fake.refunded(paid_card.gateway_trans_id.as_deref().unwrap()),
        0
    );
    wait_mail(ADMIN_EMAIL, &format!("Giao dịch {qr} thu thừa")).await;
    wait_mail(&email, &format!("Hoàn tiền đơn {}", o.code)).await;
    wait_mail_with_files(&email, &format!("Hóa đơn điều chỉnh đơn {}", o.code)).await;
    // Giao dịch còn treo (lần thử 2) cổng cũng báo đã trả qua truy vấn chủ động: cũng tự hoàn.
    let _ = fake.pay(&second, 39, None); // mất webhook
    assert!(matches!(
        payments::confirm(&state, &second, "query", OffsetDateTime::now_utc())
            .await
            .unwrap(),
        TxStatus::Succeeded
    ));
    assert_eq!(
        payments::by_ref(&state, &second)
            .await
            .unwrap()
            .refunded_vnd,
        9_000_000
    );

    // Scenario "Hoàn một phần" và "Hoàn vượt số đã thu" (giao dịch 9 triệu bằng thẻ).
    let partial = payments::request_refund(
        &state,
        paid_card.id,
        4_000_000,
        "Tranh chấp mốc 1",
        Requester::Admin("lan".into()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        payments::refund(&state, partial).await.unwrap().status,
        "succeeded"
    );
    assert_eq!(
        fake.refunded(paid_card.gateway_trans_id.as_deref().unwrap()),
        4_000_000
    );
    let err = payments::request_refund(
        &state,
        paid_card.id,
        6_000_000,
        "Hoàn thêm",
        Requester::Admin("lan".into()),
        None,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("refund_exceeds_payment"), "{err}");
    let page = get(&user, "/payments").await;
    assert!(
        page.contains("Đã hoàn một phần") && page.contains("Đã hoàn tiền"),
        "{page}"
    );

    // Scenario "Tải lại hóa đơn": từ Lịch sử thanh toán; người khác không tải được.
    let invoice = Regex::new(r#"href="(/payments/invoices/[0-9a-f-]{36})""#)
        .unwrap()
        .captures(&page)
        .unwrap()[1]
        .to_owned();
    let doc = get(&user, &invoice).await;
    assert!(
        doc.contains("Công ty TNHH Kho Vui") && doc.contains("0101234567"),
        "{doc}"
    );
    assert_eq!(
        other
            .get(format!("{BASE}{invoice}"))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        404
    );

    // Đơn 2 (18 + 30 + 12 triệu): Scenario "Giao dịch thiếu webhook" qua đối soát hằng ngày.
    let (_, _, url) = send_order_request(&user, "App chấm công", true, vec![]).await;
    let order2 = order_id_of(&url);
    let mut q = three_milestones(None);
    for (m, amount) in q
        .milestones
        .iter_mut()
        .zip([18_000_000, 30_000_000, 12_000_000])
    {
        m.amount_vnd = amount;
    }
    orders::create_quote(&state, order2, "Lan", &q, OffsetDateTime::now_utc())
        .await
        .unwrap();
    orders::accept_quote(&state, customer, order2, OffsetDateTime::now_utc())
        .await
        .unwrap();
    let (deposit2, _, _) = pending_charge(&db, order2).await;
    let missing = start_payment(&user, deposit2, "zalopay:zalopay_wallet").await;
    let _ = fake.pay(&missing, 38, None); // cổng đã thu, webhook không tới
    let today = OffsetDateTime::now_utc()
        .to_offset(time::macros::offset!(+7))
        .date();
    let mismatches = payments::reconcile(&state, today, OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert!(
        mismatches
            .iter()
            .any(|m| m.gateway_ref == missing && m.kind == "missing_webhook"),
        "{mismatches:?}"
    );
    assert_eq!(
        payments::by_ref(&state, &missing).await.unwrap().status,
        TxStatus::Succeeded
    );
    assert_eq!(
        orders::get(&db, order2).await.unwrap().status,
        Status::InProgress,
        "đơn được cập nhật"
    );
    wait_mail(ADMIN_EMAIL, &format!("Đối soát ngày {today}")).await;
    // Chạy lại: không còn chênh lệch thiếu webhook.
    let again = payments::reconcile(&state, today, OffsetDateTime::now_utc())
        .await
        .unwrap();
    assert!(!again.iter().any(|m| m.gateway_ref == missing), "{again:?}");

    // Scenario "Hoàn tiền lớn": 15 triệu chờ quản trị viên thứ hai phê duyệt.
    let tx2 = payments::by_ref(&state, &missing).await.unwrap();
    let big = payments::request_refund(
        &state,
        tx2.id,
        15_000_000,
        "Khách hủy phần lớn phạm vi",
        Requester::Admin("lan".into()),
        None,
    )
    .await
    .unwrap();
    let r = payments::refund(&state, big).await.unwrap();
    assert_eq!(
        (r.status.as_str(), r.label(true)),
        ("pending_approval", "Chờ phê duyệt")
    );
    assert_eq!(
        fake.refunded(tx2.gateway_trans_id.as_deref().unwrap()),
        0,
        "chưa gửi lệnh tới cổng"
    );
    let err = payments::approve_refund(&state, big, "lan")
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("second_approver_required"),
        "{err}"
    );
    assert_eq!(
        payments::approve_refund(&state, big, "minh")
            .await
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        fake.refunded(tx2.gateway_trans_id.as_deref().unwrap()),
        15_000_000
    );
    // Vượt số đã thu: 15 + 13 > 18 triệu.
    assert!(payments::request_refund(
        &state,
        tx2.id,
        13_000_000,
        "Hoàn thêm",
        Requester::Admin("lan".into()),
        None
    )
    .await
    .is_err());
    // Callback trùng của một giao dịch đã ghi nhận vẫn là "Duplicate".
    assert_eq!(
        payments::settle(
            &state,
            &missing,
            tx2.gateway_trans_id.as_deref().unwrap(),
            18_000_000,
            None,
            "callback",
            OffsetDateTime::now_utc()
        )
        .await
        .unwrap(),
        Settled::Duplicate
    );

    // tasks.md 13.6: sau giao dịch thẻ thử, không có số thẻ đầy đủ hay CVV trong nhật ký và CSDL.
    assert!(
        fake.inner.lock().unwrap().cards.iter().any(|c| c == PAN),
        "khách đã nhập thẻ trên trang của cổng"
    );
    let dump = dump_database(&db).await;
    let logs = String::from_utf8_lossy(&LOGS.lock().unwrap()).into_owned();
    assert!(
        logs.contains(&card),
        "nhật ký có ghi về giao dịch này (đã bắt được nhật ký)"
    );
    for (name, text) in [("cơ sở dữ liệu", &dump), ("nhật ký", &logs)] {
        assert!(
            !text.contains(PAN) && !text.contains("4111 1111 1111 1111"),
            "số thẻ trong {name}"
        );
        let lower = text.to_ascii_lowercase();
        assert!(
            !lower.contains("\"cvv\"") && !lower.contains("\"card_number\""),
            "trường thẻ trong {name}"
        );
    }
    assert!(
        dump.contains(&card),
        "bản sao CSDL có dữ liệu giao dịch (phép quét có nghĩa)"
    );
}

/// Tạo một đơn đã chấp nhận báo giá (9 + 12 + 9 triệu); trả (đơn, yêu cầu thanh toán đặt cọc).
async fn accepted_order(
    state: &AppState,
    user: &reqwest::Client,
    customer: uuid::Uuid,
    title: &str,
) -> (uuid::Uuid, uuid::Uuid) {
    let (_, _, url) = send_order_request(user, title, true, vec![]).await;
    let order = order_id_of(&url);
    let now = time::OffsetDateTime::now_utc();
    feco_server::orders::create_quote(state, order, "Lan", &three_milestones(None), now)
        .await
        .unwrap();
    feco_server::orders::accept_quote(state, customer, order, now)
        .await
        .unwrap();
    let (deposit, _, _) = pending_charge(&state.db, order).await;
    (order, deposit)
}

/// Task 13.2: cổng VNPAY (QR, thẻ ATM, thẻ quốc tế) và MoMo (ví): ký URL thanh toán, IPN có chữ
/// ký (lặp lại, giả mạo), truy vấn chủ động (hết hạn, thiếu IPN), hoàn tiền một phần.
#[tokio::test]
async fn vnpay_and_momo_end_to_end() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    use feco_server::{
        orders::{self, Status},
        payments::{self, Requester, TxStatus},
    };
    use time::{Duration as Span, OffsetDateTime};
    let state = local_state();
    let db = state.db.clone();
    let vnpay = VNPAY.get().unwrap().clone();
    let momo = MOMO.get().unwrap().clone();

    let (email, password) = unique("khach-vnpay-momo");
    let user = app_browser();
    register_in(&user, &email, &password, "Chị Mai").await;
    verify_email(&user, &email).await;
    let customer = subject_of(&email).await;

    // Trang thanh toán hiện phương thức của cả ba cổng.
    let (order, deposit) = accepted_order(&state, &user, customer, "App giữ chỗ nhà hàng").await;
    let page = get(&user, &format!("/payments/{deposit}/pay")).await;
    for m in [
        "QR chuyển khoản (VietQR) (VNPAY)",
        "Thẻ ATM nội địa (VNPAY)",
        "Thẻ quốc tế (Visa, Mastercard, JCB) (VNPAY)",
        "Ví điện tử MoMo (MoMo)",
        "Ví điện tử ZaloPay (ZaloPay)",
    ] {
        assert!(page.contains(m), "thiếu {m}");
    }

    // VNPAY: URL thanh toán được ký đúng (cổng giả kiểm chữ ký như VNPAY).
    let (txn, url) = start_payment_url(&user, deposit, "vnpay:vietqr").await;
    assert!(
        url.contains("vnp_BankCode=VNPAYQR") && url.contains("vnp_Amount=900000000"),
        "{url}"
    );
    assert!(!txn.contains('_'), "mã giao dịch VNPAY chỉ có chữ và số");
    assert_eq!(vnpay.open(&url).unwrap(), txn);
    // IPN có chữ ký: ghi nhận một lần; lặp lại → 02; giả mạo → 97.
    let ipn = vnpay.pay(&txn, "QRCODE");
    let call = |params: std::collections::BTreeMap<String, String>| async move {
        let q = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params.iter())
            .finish();
        reqwest::Client::new()
            .get(format!("{BASE}/v1/payments/vnpay/ipn?{q}"))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    let mut forged = ipn.clone();
    forged.insert("vnp_Amount".into(), "100".into());
    assert_eq!(call(forged).await["RspCode"], "97");
    assert_eq!(call(ipn.clone()).await["RspCode"], "00");
    assert_eq!(call(ipn.clone()).await["RspCode"], "02", "IPN lặp lại");
    let paid = payments::by_ref(&state, &txn).await.unwrap();
    assert_eq!(
        (paid.status, paid.provider.as_str()),
        (TxStatus::Succeeded, "vnpay")
    );
    assert_eq!(
        orders::get(&db, order).await.unwrap().status,
        Status::InProgress
    );
    // Trang chuyển về của VNPAY (vnp_TxnRef) cũng tra đúng giao dịch.
    let back = get(
        &user,
        &format!("/payments/return?vnp_TxnRef={txn}&vnp_ResponseCode=00"),
    )
    .await;
    assert!(back.contains(r#"data-status="succeeded""#), "{back}");
    // Hoàn một phần qua API hoàn tiền của VNPAY (chữ ký pipe, HMAC-SHA512).
    let r = payments::request_refund(
        &state,
        paid.id,
        4_000_000,
        "Tranh chấp",
        Requester::Admin("lan".into()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        payments::refund(&state, r).await.unwrap().status,
        "succeeded"
    );
    assert_eq!(vnpay.refunded(&txn), 4_000_000);

    // VNPAY querydr: liên kết hết hạn → Hết hạn; trả mà thiếu IPN → truy vấn ghi nhận.
    let (order2, deposit2) = accepted_order(&state, &user, customer, "App đặt lịch spa").await;
    let (atm, url) = start_payment_url(&user, deposit2, "vnpay:domestic_card").await;
    vnpay.open(&url).unwrap();
    payments::process_due(&state, OffsetDateTime::now_utc() + Span::minutes(16))
        .await
        .unwrap();
    assert_eq!(
        payments::by_ref(&state, &atm).await.unwrap().status,
        TxStatus::Expired
    );
    let (card, url) = start_payment_url(&user, deposit2, "vnpay:international_card").await;
    assert_ne!(card, atm);
    vnpay.open(&url).unwrap();
    let _ = vnpay.pay(&card, "ATM"); // IPN không tới
    assert_eq!(
        payments::confirm(&state, &card, "query", OffsetDateTime::now_utc())
            .await
            .unwrap(),
        TxStatus::Succeeded
    );
    assert_eq!(
        orders::get(&db, order2).await.unwrap().status,
        Status::InProgress
    );

    // MoMo: ví MoMo, IPN có chữ ký trả 204, giả mạo bị từ chối; hoàn một phần.
    let (order3, deposit3) = accepted_order(&state, &user, customer, "App bán vé").await;
    let wallet = start_payment(&user, deposit3, "momo:momo_wallet").await;
    let created = momo.order(&wallet).unwrap();
    assert_eq!(created.amount, 9_000_000);
    assert!(
        created.ipn_url.ends_with("/v1/payments/momo/ipn"),
        "{}",
        created.ipn_url
    );
    let ipn = momo.pay(&wallet, 777_001);
    let post_ipn = |b: Value| async move {
        reqwest::Client::new()
            .post(format!("{BASE}/v1/payments/momo/ipn"))
            .json(&b)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    };
    let mut bad = ipn.clone();
    bad["amount"] = Value::from(1000);
    assert_eq!(post_ipn(bad).await, 400);
    assert_eq!(post_ipn(ipn.clone()).await, 204);
    assert_eq!(
        post_ipn(ipn).await,
        204,
        "IPN lặp lại vẫn được nhận, không ghi hai lần"
    );
    let tx = payments::by_ref(&state, &wallet).await.unwrap();
    assert_eq!(
        (tx.status, tx.provider.as_str()),
        (TxStatus::Succeeded, "momo")
    );
    assert_eq!(
        orders::get(&db, order3).await.unwrap().status,
        Status::InProgress
    );
    let r = payments::request_refund(
        &state,
        tx.id,
        2_000_000,
        "Hoàn một phần",
        Requester::Admin("lan".into()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        payments::refund(&state, r).await.unwrap().status,
        "succeeded"
    );
    assert_eq!(momo.refunded(777_001), 2_000_000);

    // MoMo thiếu IPN: truy vấn chủ động ghi nhận.
    let (order4, deposit4) = accepted_order(&state, &user, customer, "App học từ vựng").await;
    let wallet = start_payment(&user, deposit4, "momo:momo_wallet").await;
    let _ = momo.pay(&wallet, 777_002);
    assert_eq!(
        payments::confirm(&state, &wallet, "query", OffsetDateTime::now_utc())
            .await
            .unwrap(),
        TxStatus::Succeeded
    );
    assert_eq!(
        orders::get(&db, order4).await.unwrap().status,
        Status::InProgress
    );
}

/// Task 13.4: hóa đơn điện tử qua MISA meInvoice (máy chủ giả cùng giao thức): phát hành sau khi
/// thanh toán thành công cho khách doanh nghiệp, gửi email kèm PDF và mã tra cứu, hóa đơn điều
/// chỉnh giảm khi hoàn tiền tham chiếu hóa đơn gốc, tải lại PDF.
#[tokio::test]
async fn misa_einvoice_end_to_end() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    use feco_server::payments::{self, BillingProfile, Requester};
    let misa = feco_server::misa::testing::FakeMisa::serve("e2e-misa", "e2e-misa-secret")
        .await
        .unwrap();
    let mut vars = SERVER_VARS.get().unwrap().clone();
    for (k, v) in [
        ("FECO_MISA_CLIENT_ID", "e2e-misa"),
        ("FECO_MISA_CLIENT_SECRET", "e2e-misa-secret"),
        ("FECO_MISA_TAX_CODE", "0109999999"),
        ("FECO_MISA_USERNAME", "ketoan@f-eco.test"),
        ("FECO_MISA_PASSWORD", "e2e-fake-misa-password"),
        ("FECO_MISA_INV_SERIES", "1C26TFE"),
        ("FECO_MISA_VAT_RATE", "10%"),
    ] {
        vars.insert(k.into(), v.into());
    }
    vars.insert("FECO_MISA_ENDPOINT".into(), misa.endpoint());
    let state = AppState::from_config(&Config::from_vars(&vars).unwrap()).unwrap();
    let now = time::OffsetDateTime::now_utc();

    let (email, password) = unique("khach-hoa-don");
    let user = app_browser();
    register_in(&user, &email, &password, "Anh Long").await;
    verify_email(&user, &email).await;
    let customer = subject_of(&email).await;
    payments::save_billing_profile(
        &state,
        customer,
        &BillingProfile {
            kind: "company".into(),
            name: "Công ty CP Phần mềm Sao Mai".into(),
            tax_code: Some("0312345678".into()),
            address: "12 Lê Lợi, Quận 1, TP Hồ Chí Minh".into(),
            email: email.clone(),
        },
    )
    .await
    .unwrap();
    let (order, deposit) = accepted_order(&state, &user, customer, "App kế toán kho").await;
    let code = feco_server::orders::get(&state.db, order)
        .await
        .unwrap()
        .code;
    let gateway_ref = start_payment(&user, deposit, "zalopay:vietqr").await;
    let body = ZALOPAY.get().unwrap().pay(&gateway_ref, 38, None);
    assert_eq!(
        payments::handle_callback(&state, &body, now).await["return_code"],
        1
    );

    // Hóa đơn gốc phát hành qua MISA với thông tin doanh nghiệp; giá đã gồm thuế 10%.
    let inv = misa
        .published
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("đã phát hành ở MISA");
    assert_eq!(inv["BuyerLegalName"], "Công ty CP Phần mềm Sao Mai");
    assert_eq!(inv["BuyerTaxCode"], "0312345678");
    assert_eq!(inv["InvSeries"], "1C26TFE");
    assert_eq!(
        (
            inv["TotalAmount"].as_i64(),
            inv["TotalAmountWithoutVAT"].as_i64(),
            inv["TotalVATAmount"].as_i64()
        ),
        (Some(9_000_000), Some(8_181_818), Some(818_182))
    );
    let original_no = misa.numbers.lock().unwrap().last().cloned().unwrap();
    let (mail, files) =
        wait_mail_with_files(&email, &format!("Hóa đơn thanh toán đơn {code}")).await;
    assert!(
        mail.contains("Mã tra cứu hóa đơn điện tử: TRA") && mail.contains("0312345678"),
        "{mail}"
    );
    assert!(
        files
            .iter()
            .any(|f| f.starts_with("1C26TFE-") && f.ends_with(".pdf")),
        "{files:?}"
    );
    let invoices = payments::invoices_of(&state, customer).await.unwrap();
    let original = invoices.iter().find(|i| i.kind == "original").unwrap();
    assert_eq!(original.provider, "misa");
    let pdf = payments::invoice_document(&state, customer, original.id)
        .await
        .unwrap();
    assert_eq!(pdf.content_type, "application/pdf");
    assert!(pdf.bytes.starts_with(b"%PDF"));

    // Hoàn 4 triệu → hóa đơn điều chỉnh giảm qua MISA, tham chiếu hóa đơn gốc.
    let tx = payments::by_ref(&state, &gateway_ref).await.unwrap();
    payments::request_refund(
        &state,
        tx.id,
        4_000_000,
        "Tranh chấp mốc 1",
        Requester::Admin("lan".into()),
        None,
    )
    .await
    .unwrap();
    let adj = misa.published.lock().unwrap().last().cloned().unwrap();
    assert_eq!(adj["ReferenceType"], 2);
    assert_eq!(adj["OrgInvNo"], original_no.as_str());
    assert_eq!(adj["OrgInvSeries"], "C26TFE");
    assert_eq!(adj["TotalAmount"], 4_000_000);
    let (mail, _) = wait_mail_with_files(&email, &format!("Hóa đơn điều chỉnh đơn {code}")).await;
    assert!(mail.contains("điều chỉnh giảm"), "{mail}");
}

// ---------- Bảng quản trị và thông báo (tasks.md 14.1–14.3) ----------

/// Khóa bảo mật phần mềm cho kiểm thử: tạo và ký phản hồi WebAuthn (ES256, attestation "none")
/// như một khóa phần cứng, để đăng ký passkey và đăng nhập bước hai với Kratos thật.
struct SoftKey {
    key: aws_lc_rs::signature::EcdsaKeyPair,
    id: Vec<u8>,
    counter: u32,
}

mod cbor {
    fn head(out: &mut Vec<u8>, major: u8, v: u64) {
        let m = major << 5;
        match v {
            0..=23 => out.push(m | v as u8),
            24..=0xff => out.extend_from_slice(&[m | 24, v as u8]),
            0x100..=0xffff => {
                out.push(m | 25);
                out.extend_from_slice(&(v as u16).to_be_bytes());
            }
            _ => {
                out.push(m | 26);
                out.extend_from_slice(&(v as u32).to_be_bytes());
            }
        }
    }
    pub fn int(out: &mut Vec<u8>, v: i64) {
        if v >= 0 {
            head(out, 0, v as u64)
        } else {
            head(out, 1, (-1 - v) as u64)
        }
    }
    pub fn bytes(out: &mut Vec<u8>, b: &[u8]) {
        head(out, 2, b.len() as u64);
        out.extend_from_slice(b);
    }
    pub fn text(out: &mut Vec<u8>, s: &str) {
        head(out, 3, s.len() as u64);
        out.extend_from_slice(s.as_bytes());
    }
    pub fn map(out: &mut Vec<u8>, n: usize) {
        head(out, 5, n as u64);
    }
}

fn b64url(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

fn sha256(b: &[u8]) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(b).to_vec()
}

/// Tùy chọn WebAuthn Kratos đặt trong `onclick` của nút passkey.
fn webauthn_options(page: &str, button: &str) -> Value {
    let doc = Html::parse_document(page);
    let onclick = doc
        .select(&Selector::parse(&format!("button[name=\"{button}\"]")).unwrap())
        .next()
        .and_then(|b| b.value().attr("onclick").map(str::to_owned))
        .unwrap_or_else(|| panic!("trang không có nút {button}:\n{page}"));
    let start = onclick.find('(').unwrap() + 1;
    let end = onclick.rfind(')').unwrap();
    serde_json::from_str(&onclick[start..end])
        .unwrap_or_else(|e| panic!("tùy chọn WebAuthn không đọc được ({e}): {onclick}"))
}

impl SoftKey {
    fn new() -> Self {
        let key = aws_lc_rs::signature::EcdsaKeyPair::generate(
            &aws_lc_rs::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
        )
        .unwrap();
        let mut id = vec![0u8; 32];
        getrandom_fill(&mut id);
        SoftKey {
            key,
            id,
            counter: 0,
        }
    }

    fn client_data(kind: &str, challenge: &str) -> Vec<u8> {
        serde_json::json!({"type": kind, "challenge": challenge, "origin": BASE, "crossOrigin": false}).to_string().into_bytes()
    }

    /// Phản hồi `navigator.credentials.create()`.
    fn register(&self, options: &Value) -> String {
        use aws_lc_rs::signature::KeyPair;
        let pk = &options["publicKey"];
        let rp_id = pk["rp"]["id"].as_str().unwrap();
        let point = self.key.public_key().as_ref();
        let mut cose = Vec::new();
        cbor::map(&mut cose, 5);
        cbor::int(&mut cose, 1);
        cbor::int(&mut cose, 2);
        cbor::int(&mut cose, 3);
        cbor::int(&mut cose, -7);
        cbor::int(&mut cose, -1);
        cbor::int(&mut cose, 1);
        cbor::int(&mut cose, -2);
        cbor::bytes(&mut cose, &point[1..33]);
        cbor::int(&mut cose, -3);
        cbor::bytes(&mut cose, &point[33..65]);
        let mut auth = sha256(rp_id.as_bytes());
        auth.push(0x45); // UP, UV, AT
        auth.extend_from_slice(&0u32.to_be_bytes());
        auth.extend_from_slice(&[0u8; 16]);
        auth.extend_from_slice(&(self.id.len() as u16).to_be_bytes());
        auth.extend_from_slice(&self.id);
        auth.extend_from_slice(&cose);
        let mut att = Vec::new();
        cbor::map(&mut att, 3);
        cbor::text(&mut att, "fmt");
        cbor::text(&mut att, "none");
        cbor::text(&mut att, "attStmt");
        cbor::map(&mut att, 0);
        cbor::text(&mut att, "authData");
        cbor::bytes(&mut att, &auth);
        let client = Self::client_data("webauthn.create", pk["challenge"].as_str().unwrap());
        serde_json::json!({
            "id": b64url(&self.id), "rawId": b64url(&self.id), "type": "public-key",
            "response": {"attestationObject": b64url(&att), "clientDataJSON": b64url(&client)},
            "clientExtensionResults": {}
        })
        .to_string()
    }

    /// Phản hồi `navigator.credentials.get()`.
    fn assert(&mut self, options: &Value) -> String {
        let pk = &options["publicKey"];
        let rp_id = pk["rpId"].as_str().unwrap();
        self.counter += 1;
        let mut auth = sha256(rp_id.as_bytes());
        auth.push(0x05); // UP, UV
        auth.extend_from_slice(&self.counter.to_be_bytes());
        let client = Self::client_data("webauthn.get", pk["challenge"].as_str().unwrap());
        let mut signed = auth.clone();
        signed.extend_from_slice(&sha256(&client));
        let sig = self
            .key
            .sign(&aws_lc_rs::rand::SystemRandom::new(), &signed)
            .unwrap();
        serde_json::json!({
            "id": b64url(&self.id), "rawId": b64url(&self.id), "type": "public-key",
            "response": {"authenticatorData": b64url(&auth), "clientDataJSON": b64url(&client), "signature": b64url(sig.as_ref())},
            "clientExtensionResults": {}
        })
        .to_string()
    }
}

fn getrandom_fill(b: &mut [u8]) {
    use aws_lc_rs::rand::SecureRandom;
    aws_lc_rs::rand::SystemRandom::new().fill(b).unwrap();
}

/// Gửi biểu mẫu của một nhóm Kratos, thay các trường ẩn trùng tên bằng giá trị mới.
async fn submit_group(
    c: &reqwest::Client,
    page: &str,
    group: &str,
    extra: &[(&str, &str)],
) -> String {
    let mut form = parse_group_form(page, group);
    form.fields
        .retain(|(k, _)| !extra.iter().any(|(e, _)| e == k));
    submit(c, &form, extra).await
}

/// Một quản trị viên có passkey, đã đăng nhập bằng mật khẩu và passkey.
async fn admin_with_passkey(
    state: &AppState,
    prefix: &str,
    roles: &[feco_server::admin::Role],
) -> (reqwest::Client, String) {
    let (email, password) = unique(prefix);
    let c = app_browser();
    register_in(&c, &email, &password, "Quản trị viên").await;
    verify_email(&c, &email).await;
    feco_server::admin::set_member(state, "khoi-tao", &email, roles)
        .await
        .unwrap();
    // Chưa có passkey: không vào được bảng quản trị.
    let res = c.get(format!("{BASE}/admin")).send().await.unwrap();
    assert_eq!(res.status().as_u16(), 403);
    assert!(
        res.text().await.unwrap().contains("passkey"),
        "phải yêu cầu passkey"
    );
    // Thêm passkey (khóa bảo mật) trong Cài đặt.
    let key = SoftKey::new();
    let settings = get(&c, "/settings").await;
    let options = webauthn_options(&settings, "webauthn_register_trigger");
    let page = submit_group(
        &c,
        &settings,
        "webauthn",
        &[
            ("webauthn_register", &key.register(&options)),
            ("webauthn_register_displayname", "Khóa thử"),
        ],
    )
    .await;
    assert!(page.contains("Đã lưu thay đổi."), "{page}");
    // Mật khẩu vẫn chưa đủ: phiên phải có bước passkey.
    let mut key = key;
    let other = app_browser();
    let page = login(&other, &email, &password).await;
    let options = webauthn_options(&page, "webauthn_login_trigger");
    let home = submit_group(
        &other,
        &page,
        "webauthn",
        &[("webauthn_login", &key.assert(&options))],
    )
    .await;
    assert!(home.contains(&email), "{home}");
    (other, email)
}

/// Spec admin-console và notifications.
#[tokio::test]
async fn admin_console_end_to_end() {
    if !e2e_ready() {
        return;
    }
    let _serial = SERIAL.lock().await;
    use feco_server::{
        admin::Role,
        notifications::{self, Category, Note},
        orders, payments,
    };
    use time::{Duration as Span, OffsetDateTime};
    let state = local_state();
    let db = state.db.clone();

    // Khách hàng có hai đơn đang thực hiện (đặt cọc đã thu).
    let (cust_email, cust_password) = unique("khach-quan-tri");
    let customer_browser = app_browser();
    register_in(&customer_browser, &cust_email, &cust_password, "Chị Thu").await;
    verify_email(&customer_browser, &cust_email).await;
    let customer = subject_of(&cust_email).await;
    let (late, late_deposit) =
        accepted_order(&state, &customer_browser, customer, "App tồn kho sắp trễ").await;
    let (calm, calm_deposit) =
        accepted_order(&state, &customer_browser, customer, "App lịch hẹn").await;
    for d in [late_deposit, calm_deposit] {
        payments::record_manual(&state, d, OffsetDateTime::now_utc())
            .await
            .unwrap();
    }
    // Mốc 2 của đơn "sắp trễ" còn 1 ngày tới hạn, chưa bàn giao.
    sqlx::query("UPDATE order_milestones SET due_at = now() + interval '1 day' WHERE order_id = $1 AND position = 2").bind(late).execute(&db).await.unwrap();
    let late_code = orders::get(&db, late).await.unwrap().code;
    let calm_code = orders::get(&db, calm).await.unwrap().code;

    // Scenario "Hỗ trợ cố hoàn tiền": chức năng bị ẩn, gọi thẳng bị 403.
    let (support, support_email) = admin_with_passkey(&state, "ho-tro", &[Role::Support]).await;
    let home = get(&support, "/admin").await;
    assert!(
        home.contains("Hỗ trợ người dùng") && !home.contains("/admin/refunds"),
        "{home}"
    );
    assert_eq!(
        support
            .get(format!("{BASE}/admin/refunds"))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        403
    );
    let token = csrf_of(&get(&support, "/orders").await);
    let (status, _) = post(
        &support,
        "/admin/refunds",
        &token,
        &[
            ("transaction", &uuid::Uuid::new_v4().to_string()),
            ("amount", "1000"),
            ("reason", "thử"),
        ],
    )
    .await;
    assert_eq!(status, 403, "API hoàn tiền từ chối vai trò Hỗ trợ");

    // Scenario "Xem hồ sơ": phải nhập lý do, lần xem vào nhật ký kiểm toán; email bị che.
    let page = get(&support, &format!("/admin/users?q={cust_email}")).await;
    let results: Vec<String> = Html::parse_document(&page)
        .select(&Selector::parse(".user-result code").unwrap())
        .map(|c| c.text().collect())
        .collect();
    assert_eq!(
        results,
        vec![feco_server::admin::mask_email(&cust_email)],
        "{page}"
    );
    let by_order = get(&support, &format!("/admin/users?q={late_code}")).await;
    assert!(
        by_order.contains(&format!("/admin/users/{customer}/view")),
        "tìm được theo mã đơn"
    );
    let (status, _) = post(
        &support,
        &format!("/admin/users/{customer}/view"),
        &csrf_of(&page),
        &[("reason", "")],
    )
    .await;
    assert_eq!(status, 400);
    let (status, profile) = post(
        &support,
        &format!("/admin/users/{customer}/view"),
        &csrf_of(&page),
        &[("reason", "Phiếu HT-1024")],
    )
    .await;
    assert_eq!(status, 200, "{profile}");
    assert!(
        profile.contains(&late_code) && !profile.contains(&cust_email),
        "{profile}"
    );
    let logged: Option<Value> = sqlx::query_scalar("SELECT details FROM audit_log WHERE action = 'admin.user_view' AND target = $1 AND actor_id = $2 ORDER BY at DESC LIMIT 1")
        .bind(customer.to_string())
        .bind(&support_email)
        .fetch_optional(&db)
        .await
        .unwrap();
    assert_eq!(logged.unwrap()["reason"], "Phiếu HT-1024");
    // Đăng xuất mọi phiên của người dùng theo yêu cầu.
    let (_, profile) = post(
        &support,
        &format!("/admin/users/{customer}/logout"),
        &csrf_of(&profile),
        &[("reason", "Phiếu HT-1024")],
    )
    .await;
    assert!(profile.contains("Đã đăng xuất mọi phiên"), "{profile}");
    assert!(state
        .kratos
        .identity_sessions(&customer.to_string())
        .await
        .unwrap()
        .iter()
        .all(|s| s["active"] != true));
    login(&customer_browser, &cust_email, &cust_password).await;

    // Quản lý dự án, tài chính, người duyệt, quản trị hệ thống.
    let (pm, _) = admin_with_passkey(
        &state,
        "quan-ly",
        &[
            Role::ProjectManager,
            Role::Finance,
            Role::Reviewer,
            Role::Sysadmin,
        ],
    )
    .await;

    // Scenario "Đơn sắp trễ": cảnh báo, đứng đầu danh sách.
    let list = get(&pm, "/admin/orders").await;
    let rows: Vec<(String, bool)> = Html::parse_document(&list)
        .select(&Selector::parse("tr.admin-order").unwrap())
        .map(|r| {
            (
                r.value().attr("data-code").unwrap().to_owned(),
                r.value().attr("class").unwrap().contains("warning"),
            )
        })
        .collect();
    // (Đơn còn lại từ lần chạy trước có thể cũng đang cảnh báo.)
    assert!(rows.contains(&(late_code.clone(), true)), "{rows:?}");
    let first_calm = rows.iter().position(|r| !r.1).unwrap_or(rows.len());
    assert!(
        rows[first_calm..].iter().all(|r| !r.1),
        "đơn cảnh báo đứng đầu: {rows:?}"
    );
    assert!(rows.contains(&(calm_code.clone(), false)), "{rows:?}");

    // Scenario "Tin nhắn mới trong đơn": email chỉ báo có tin nhắn, không chép nội dung.
    let detail = get(&pm, &format!("/admin/orders/{late}")).await;
    let (status, _) = post(
        &pm,
        &format!("/admin/orders/{late}/message"),
        &csrf_of(&detail),
        &[("body", "Mật mã kho là KHO-7781, chị đổi giúp nhé")],
    )
    .await;
    assert_eq!(status, 200);
    let mail = wait_mail(&cust_email, &format!("Tin nhắn mới trong đơn {late_code}")).await;
    assert!(
        mail.contains(&format!("Bạn có tin nhắn mới trong đơn #{late_code}"))
            && !mail.contains("KHO-7781"),
        "{mail}"
    );
    assert!(mail.contains(&format!("/orders/{late}#thread")), "{mail}");

    // Tài chính hoàn một phần qua bảng quản trị.
    let tx: uuid::Uuid =
        sqlx::query_scalar("SELECT t.id FROM payment_transactions t WHERE t.request_id = $1")
            .bind(late_deposit)
            .fetch_one(&db)
            .await
            .unwrap();
    let refunds = get(&pm, "/admin/refunds").await;
    let (status, page) = post(
        &pm,
        "/admin/refunds",
        &csrf_of(&refunds),
        &[
            ("transaction", &tx.to_string()),
            ("amount", "1.000.000"),
            ("reason", "Hỗ trợ thiện chí"),
        ],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(
        payments::transaction(&state, tx)
            .await
            .unwrap()
            .refunded_vnd,
        1_000_000
    );

    // Scenario "Xem khác biệt quyền": SoundLab 1.3.0 thêm scope phone so với 1.2.0.
    let account = uuid::Uuid::new_v4();
    let app = format!("soundlab-{}", &account.simple().to_string()[..8]);
    sqlx::query("INSERT INTO developer_accounts (id, kind, display_name, domain_token) VALUES ($1, 'individual', 'Nhóm SoundLab', 'x')").bind(account).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO developer_apps (app_id, account_id, client_id, name, kind) VALUES ($1, $2, $3, 'SoundLab', 'desktop')")
        .bind(&app)
        .bind(account)
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&db)
        .await
        .unwrap();
    let manifest = |v: &str, extra: &str| {
        let text = format!(
            "id = \"{app}\"\nname = \"SoundLab\"\nversion = \"{v}\"\nlicense = \"MIT\"\nprivacy_policy = \"https://soundlab.example/privacy\"\nsummary = \"Thu âm\"\ncategory = \"Âm nhạc\"\n\n[[platforms]]\nos = \"windows\"\narch = \"x64\"\nentry = \"SoundLab.exe\"\n\n[scopes.profile]\npurpose = \"Hiển thị trên bản nhạc bạn chia sẻ\"\n{extra}"
        );
        (
            serde_json::to_value(feco_manifest::parse(&text).unwrap()).unwrap(),
            text,
        )
    };
    for (v, extra, status, stage, at) in [
        (
            "1.2.0",
            "",
            "approved",
            "done",
            OffsetDateTime::now_utc() - Span::days(30),
        ),
        (
            "1.3.0",
            "\n[scopes.phone]\npurpose = \"gửi mã xác nhận\"\n",
            "pending_review",
            "manual",
            OffsetDateTime::now_utc(),
        ),
    ] {
        let (json, toml) = manifest(v, extra);
        sqlx::query(
            "INSERT INTO app_versions (id, app_id, version, manifest, manifest_toml, status, created_by, created_at, submitted_at, review_stage, manual_reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, $9, 'Thêm scope')",
        )
        .bind(uuid::Uuid::new_v4())
        .bind(&app)
        .bind(v)
        .bind(json)
        .bind(toml)
        .bind(status)
        .bind(account)
        .bind(at)
        .bind(stage)
        .execute(&db)
        .await
        .unwrap();
    }
    let queue = get(&pm, "/admin/reviews").await;
    assert!(queue.contains(&format!("{app} 1.3.0")), "{queue}");
    assert!(
        queue.contains("Thêm scope: phone (mục đích: gửi mã xác nhận)"),
        "{queue}"
    );
    let version_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM app_versions WHERE app_id = $1 AND version = '1.3.0'")
            .bind(&app)
            .fetch_one(&db)
            .await
            .unwrap();
    let (status, page) = post(
        &pm,
        &format!("/admin/reviews/{version_id}/reject"),
        &csrf_of(&queue),
        &[("clause", "misleading"), ("note", "")],
    )
    .await;
    assert_eq!(status, 400, "quyết định phải kèm lý do: {page}");

    // Quản trị hệ thống thêm vai trò; người không phải quản trị bị 403.
    let members = get(&pm, "/admin/members").await;
    let (status, page) = post(
        &pm,
        "/admin/members",
        &csrf_of(&members),
        &[("email", &cust_email), ("role", "support")],
    )
    .await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(
        feco_server::admin::admin_roles(&state, customer)
            .await
            .unwrap()
            .unwrap()
            .1,
        vec![Role::Support]
    );
    feco_server::admin::set_member(&state, "khoi-tao", &cust_email, &[])
        .await
        .unwrap();
    assert_eq!(
        customer_browser
            .get(format!("{BASE}/admin"))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        403
    );

    // Scenario "Báo giá mới": email và thông báo trong launcher mở đúng trang báo giá.
    let (_, _, url) = send_order_request(&customer_browser, "App đặt món", true, vec![]).await;
    let fresh = order_id_of(&url);
    orders::create_quote(
        &state,
        fresh,
        "Lan",
        &three_milestones(None),
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let fresh_code = orders::get(&db, fresh).await.unwrap().code;
    wait_mail(&cust_email, &format!("Báo giá cho đơn {fresh_code}")).await;
    let launcher = LAUNCHER.get().unwrap().clone();
    let a = authorize(
        &customer_browser,
        &launcher,
        "openid offline_access profile",
    )
    .await;
    let token = exchange(&launcher, &a).await["access_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let center: Value = reqwest::Client::new()
        .get(format!("{BASE}/v1/notifications"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let quote = center["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["title"] == format!("Báo giá cho đơn {fresh_code}"))
        .expect("có thông báo báo giá")
        .clone();
    assert!(
        quote["link"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/orders/{fresh}#quote")),
        "{quote}"
    );

    // Scenario "Tắt thông báo cập nhật": không còn email cập nhật app, vẫn nhận email bảo mật.
    let page = get(&customer_browser, "/notifications").await;
    assert!(
        page.contains("Báo giá cho đơn"),
        "trung tâm thông báo trên web"
    );
    let (_, page) = post(
        &customer_browser,
        "/notifications/settings",
        &csrf_of(&page),
        &[],
    )
    .await;
    assert!(page.contains("Đã lưu tùy chọn thông báo."), "{page}");
    notifications::notify(
        &state,
        customer,
        None,
        Category::AppUpdates,
        &Note::vi(
            "SoundLab có bản cập nhật 1.3.0",
            "Bản mới.",
            "/store/soundlab",
        ),
    )
    .await;
    notifications::notify(
        &state,
        customer,
        None,
        Category::Security,
        &Note::vi(
            "Cảnh báo bảo mật thử nghiệm",
            "Có đăng nhập mới.",
            "/sessions",
        ),
    )
    .await;
    wait_mail(&cust_email, "Cảnh báo bảo mật thử nghiệm").await;
    assert_eq!(
        mail_count(&cust_email, "SoundLab có bản cập nhật").await,
        0,
        "đã tắt thông báo cập nhật"
    );

    // Scenario "Hủy đăng ký từ email": bản tin mặc định tắt; bật rồi hủy từ email, không cần đăng nhập.
    assert!(
        !notifications::enabled(&state, customer, Category::Newsletter).await,
        "bản tin mặc định tắt"
    );
    let page = get(&customer_browser, "/notifications").await;
    post(
        &customer_browser,
        "/notifications/settings",
        &csrf_of(&page),
        &[("on", "newsletter")],
    )
    .await;
    notifications::notify(
        &state,
        customer,
        None,
        Category::Newsletter,
        &Note::vi("Bản tin F-Eco tháng 11", "Tin mới.", "/store"),
    )
    .await;
    let mail = wait_mail(&cust_email, "Bản tin F-Eco tháng 11").await;
    let link = Regex::new(r"https?://\S+/notifications/unsubscribe/\S+")
        .unwrap()
        .find(&mail)
        .expect("email có liên kết hủy đăng ký")
        .as_str()
        .to_owned();
    let stranger = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let page = get(&stranger, &link).await;
    let form = parse_form_where(&page, "form#unsubscribe-form");
    let done = submit(&stranger, &form, &[]).await;
    assert!(done.contains("Đã hủy đăng ký"), "{done}");
    assert!(!notifications::enabled(&state, customer, Category::Newsletter).await);
    let tampered = link.replace("newsletter", "security");
    assert_eq!(
        stranger
            .get(&tampered)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        404,
        "không hủy được thông báo bắt buộc"
    );
}
