//! Kiểm thử đầu-cuối với Ory Kratos và Mailpit thật (tasks.md 2.1, 2.2).
//!
//! Chỉ chạy khi đặt `FECO_E2E=1`, với Kratos ở 127.0.0.1:4433/4434 và Mailpit ở
//! 127.0.0.1:1025/8025 (docker compose trong `infra/`, hoặc `scripts/dev-local.sh`).
//! feco-server được chạy ngay trong test ở 127.0.0.1:8080, vì Kratos chuyển hướng
//! trình duyệt về địa chỉ này.
//!
//! Mỗi bước ứng với một scenario trong specs/account-identity/spec.md.

use std::{collections::HashMap, time::Duration};

use feco_config::Config;
use feco_server::{router, AppState};
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value;

const BASE: &str = "http://127.0.0.1:8080";
const MAILPIT: &str = "http://127.0.0.1:8025";

fn browser() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

async fn start_server() {
    let vars: HashMap<String, String> = [("FECO_ENV", "dev")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let config = Config::from_vars(&vars).unwrap();
    let state = AppState::from_config(&config).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("cổng 8080 phải trống cho kiểm thử E2E");
    tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });
}

/// Biểu mẫu đầu tiên trên trang: địa chỉ gửi và các trường ẩn.
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
    if std::env::var("FECO_E2E").is_err() {
        eprintln!("bỏ qua: đặt FECO_E2E=1 và chạy Kratos + Mailpit để kiểm thử đầu-cuối");
        return;
    }
    start_server().await;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let email = format!("e2e-{stamp}@example.com");
    // Khác hẳn email, vì Kratos từ chối mật khẩu quá giống định danh.
    let password = format!("Tra-sua-tran-chau-{:x}-Q", stamp.wrapping_mul(7919));

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
