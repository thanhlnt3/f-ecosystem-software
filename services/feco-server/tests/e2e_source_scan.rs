//! Kiểm thử đầu-cuối quét mã nguồn khi duyệt app (thay đổi chat-and-source-scan, tasks.md 3.2;
//! spec source-scan "Quét mã nguồn khi duyệt app", app-review "Kiểm tra tự động gồm mã nguồn").
//!
//! Chỉ chạy khi đặt `FECO_E2E=1`, với Kratos, Hydra, Mailpit và Postgres của môi trường dev (như
//! `e2e_auth.rs`). feco-server chạy ngay trong test ở 127.0.0.1:8080; OSV.dev và codeload.github.com
//! được thay bằng máy chủ giả trên loopback.

use std::{
    collections::HashMap,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::{Duration, SystemTime},
};

use feco_config::Config;
use feco_server::{codesign::fixtures, review, router, scan, store, AppState};
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::{json, Value};

const BASE: &str = "http://127.0.0.1:8080";
const MAILPIT: &str = "http://127.0.0.1:8025";
const APP_CALLBACK: &str = "http://127.0.0.1:9999/cb";
const ADMIN_EMAIL: &str = "kiem-duyet-ma-nguon@f-eco.test";
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";
/// Thư viện mà OSV giả báo lỗ hổng mức HIGH.
const VULN_PKG: (&str, &str) = ("left-pad-e2e", "1.0.0");
const COMMIT: &str = "0a3b8b708cf435f730a77453b8203fbff280d198";

static SERVER_VARS: OnceLock<HashMap<String, String>> = OnceLock::new();
/// Repo `e2e-flaky/*` trả 503 cho tới khi cờ này bật (giả lập mạng lỗi rồi phục hồi).
static FLAKY_UP: AtomicBool = AtomicBool::new(false);

/// `package-lock.json` có thư viện bị OSV giả báo lỗ hổng mức HIGH (ở dòng 6).
fn vulnerable_lock() -> String {
    let (name, version) = VULN_PKG;
    [
        "{".to_owned(),
        r#"  "name": "app","#.to_owned(),
        r#"  "lockfileVersion": 3,"#.to_owned(),
        r#"  "packages": {"#.to_owned(),
        r#"    "": { "name": "app", "version": "1.0.0" },"#.to_owned(),
        format!(r#"    "node_modules/{name}": {{ "version": "{version}" }},"#),
        r#"    "node_modules/react": { "version": "19.0.0" }"#.to_owned(),
        "  }".to_owned(),
        "}".to_owned(),
    ]
    .join("\n")
}

fn zip_bytes(files: &[(&str, String)]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, text) in files {
            z.start_file(*name, o).unwrap();
            z.write_all(text.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

/// OSV.dev giả.
async fn fake_osv() -> String {
    use axum::{extract::Path, routing::get, routing::post, Json, Router};
    let app = Router::new()
        .route(
            "/v1/querybatch",
            post(|Json(body): Json<Value>| async move {
                let results: Vec<Value> = body["queries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|q| {
                        if q["package"]["name"] == VULN_PKG.0 && q["version"] == VULN_PKG.1 {
                            json!({"vulns": [{"id": "GHSA-e2e0-high-0001"}]})
                        } else {
                            json!({})
                        }
                    })
                    .collect();
                Json(json!({ "results": results }))
            }),
        )
        .route(
            "/v1/vulns/{id}",
            get(|Path(id): Path<String>| async move {
                Json(json!({"id": id, "summary": "Prototype pollution (giả)", "database_specific": {"severity": "HIGH"}}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// codeload.github.com giả: zip bọc trong `<repo>-<commit>/`; `e2e-flaky/*` lỗi 503 khi chưa bật
/// [`FLAKY_UP`]; chủ repo khác trả 404.
async fn fake_codeload() -> String {
    use axum::{extract::Path, http::StatusCode, routing::get, Router};
    let app = Router::new().route(
        "/{owner}/{repo}/zip/{commit}",
        get(
            |Path((owner, repo, commit)): Path<(String, String, String)>| async move {
                let ok = match owner.as_str() {
                    "e2e-ok" => true,
                    "e2e-flaky" => FLAKY_UP.load(Ordering::SeqCst),
                    _ => return Err(StatusCode::NOT_FOUND),
                };
                if !ok {
                    return Err(StatusCode::SERVICE_UNAVAILABLE);
                }
                let prefix = format!("{repo}-{commit}");
                Ok(zip_bytes(&[
                    (&format!("{prefix}/package-lock.json"), vulnerable_lock()),
                    (&format!("{prefix}/src/index.js"), "console.log('xin chào');\n".into()),
                    (&format!("{prefix}/LICENSE"), "MIT License\n\nPermission is hereby granted, free of charge, to any person".into()),
                ]))
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// CSDL riêng của bộ E2E này (`FECO_E2E_DATABASE_URL`, mặc định `feco_scan_e2e` trên Postgres
/// dev): migration của nhánh này không được chạy vào CSDL `feco` dùng chung với nhánh khác. Tạo
/// CSDL nếu chưa có, qua kết nối quản trị tới CSDL `postgres`.
async fn e2e_database() -> String {
    use sqlx::Connection;
    let url = std::env::var("FECO_E2E_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://feco:feco-dev-only@127.0.0.1:5432/feco_scan_e2e".to_owned()
    });
    let parsed = url::Url::parse(&url).expect("FECO_E2E_DATABASE_URL hợp lệ");
    let name = parsed.path().trim_start_matches('/').to_owned();
    assert!(
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "tên CSDL E2E không hợp lệ: {name:?}"
    );
    let mut admin_url = parsed.clone();
    admin_url.set_path("/postgres");
    let mut conn = sqlx::PgConnection::connect(admin_url.as_str())
        .await
        .expect("kết nối Postgres dev (CSDL postgres)");
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(&name)
        .fetch_optional(&mut conn)
        .await
        .unwrap();
    if exists.is_none() {
        sqlx::query(&format!("CREATE DATABASE \"{name}\""))
            .execute(&mut conn)
            .await
            .unwrap_or_else(|e| {
                panic!("không tạo được CSDL E2E {name} (vai trò cần CREATEDB): {e}")
            });
    }
    url
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
            .try_init();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let work =
                    std::env::temp_dir().join(format!("feco-e2e-src-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&work);
                let clamd = scan::testing::serve_clamd("127.0.0.1:0").await.unwrap();
                let vars: HashMap<String, String> = [
                    ("FECO_ENV", "dev".to_owned()),
                    (
                        "FECO_PACKAGE_DIR",
                        work.join("packages").to_string_lossy().into_owned(),
                    ),
                    ("FECO_CLAMD_ADDR", clamd.to_string()),
                    ("FECO_ADMIN_EMAILS", ADMIN_EMAIL.to_owned()),
                    ("FECO_ADMIN_PASSKEY_OPTIONAL", "true".to_owned()),
                    ("FECO_OSV_URL", fake_osv().await),
                    ("FECO_SOURCE_FETCH_URL", fake_codeload().await),
                    ("FECO_DATABASE_URL", e2e_database().await),
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

fn local_state() -> AppState {
    let vars = SERVER_VARS.get().cloned().unwrap();
    AppState::from_config(&Config::from_vars(&vars).unwrap()).unwrap()
}

// ---------- Trình duyệt và biểu mẫu (như e2e_auth.rs) ----------

fn unique(prefix: &str) -> (String, String) {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (
        format!("{prefix}-{stamp}@example.com"),
        format!("Tra-sua-tran-chau-{:x}-Q", stamp.wrapping_mul(7919)),
    )
}

/// Trình duyệt dừng khi được chuyển về địa chỉ callback của app.
fn app_browser() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(UA)
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

struct Form {
    action: String,
    fields: Vec<(String, String)>,
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

fn csrf_of(page: &str) -> String {
    Regex::new(r#"name="feco_csrf" value="([^"]+)""#)
        .unwrap()
        .captures(page)
        .unwrap_or_else(|| panic!("trang không có token CSRF:\n{page}"))[1]
        .to_owned()
}

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

async fn login(c: &reqwest::Client, email: &str, password: &str) -> String {
    let form = parse_form_where(&get(c, "/login").await, "form");
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

async fn register_in(user: &reqwest::Client, email: &str, password: &str, name: &str) {
    let form = parse_form_where(&get(user, "/registration").await, "form");
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

/// Chờ email tới `to` có tiêu đề chứa `subject`; trả nội dung chữ.
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

fn totp_now(secret_b32: &str) -> String {
    let secret = totp_rs::Secret::Encoded(secret_b32.to_owned())
        .to_bytes()
        .unwrap();
    totp_rs::TOTP::new_unchecked(totp_rs::Algorithm::SHA1, 6, 1, 30, secret)
        .generate_current()
        .unwrap()
}

async fn enable_totp(c: &reqwest::Client) {
    let settings = get(c, "/settings").await;
    let secret = text_of(&settings, "code#totp_secret_key");
    let form = parse_form_where(&settings, "form[data-group=\"totp\"]");
    let page = submit(
        c,
        &form,
        &[("totp_code", &totp_now(&secret)), ("method", "totp")],
    )
    .await;
    assert!(page.contains("Đã lưu thay đổi."), "{page}");
}

async fn subject_of(email: &str) -> uuid::Uuid {
    feco_server::kratos::KratosClient::new(
        url::Url::parse("http://127.0.0.1:4433/").unwrap(),
        url::Url::parse("http://127.0.0.1:4434/").unwrap(),
    )
    .list_identities()
    .await
    .unwrap()
    .into_iter()
    .find(|i| i["traits"]["email"] == email)
    .and_then(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
    .expect("có danh tính")
}

/// Nhà phát triển đủ điều kiện (vai trò Lập trình viên, email đã xác minh, TOTP), có một app và
/// token CLI.
async fn developer_with_app(
    prefix: &str,
    display: &str,
    app_id: &str,
) -> (reqwest::Client, String, String) {
    let (email, password) = unique(prefix);
    let c = app_browser();
    register_in(&c, &email, &password, display).await;
    feco_server::roles::set_role(
        &local_state().db,
        subject_of(&email).await,
        &email,
        feco_server::roles::PlatformRole::Developer,
        &[],
        &feco_server::roles::ChangedBy {
            subject: None,
            name: "e2e".into(),
        },
        "Kiểm thử E2E",
    )
    .await
    .unwrap();
    verify_email(&c, &email).await;
    enable_totp(&c).await;
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

fn app_manifest(app_id: &str, version: &str) -> String {
    format!(
        r#"id = "{app_id}"
name = "SoundLab"
version = "{version}"
license = "MIT"
privacy_policy = "https://soundlab.example/privacy"

[[platforms]]
os = "windows"
arch = "x64"
entry = "SoundLab.exe"

[scopes.profile]
purpose = "Hiển thị trên bản nhạc bạn chia sẻ"

[permissions]
network = true
"#
    )
}

fn with_git_source(manifest: &str, owner: &str) -> String {
    format!("{manifest}\n[source]\nrepository = \"https://github.com/{owner}/soundlab\"\ncommit = \"{COMMIT}\"\n")
}

fn write_files(root: &std::path::Path, files: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
    for (name, bytes) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    root.to_path_buf()
}

/// Nộp một phiên bản bằng `f-dev publish` (có thể kèm `--source`); trả mã phiên bản và đầu ra.
async fn publish(
    work: &std::path::Path,
    token: &str,
    manifest: &str,
    dist: &std::path::Path,
    source: Option<&std::path::Path>,
) -> (uuid::Uuid, String) {
    let file = work.join(format!("f-app-{}.toml", uuid::Uuid::new_v4()));
    std::fs::write(&file, manifest).unwrap();
    let mut args: Vec<String> = [
        "publish",
        "--manifest",
        &file.to_string_lossy(),
        "--package",
        &dist.to_string_lossy(),
        "--server",
        BASE,
        "--token",
        token,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Some(src) = source {
        args.push("--source".into());
        args.push(src.to_string_lossy().into_owned());
    }
    let out = f_dev::run(&args).await.expect("nộp phiên bản");
    assert!(out.contains("Chờ duyệt"), "{out}");
    let id = out
        .rsplit("mã phiên bản: ")
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    (id, out)
}

type State = (
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<Value>,
);

async fn version_state(st: &AppState, id: uuid::Uuid) -> State {
    sqlx::query_as(
        "SELECT status, review_stage, COALESCE(review_note, ''), reject_code, manual_reason, checks
         FROM app_versions WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&st.db)
    .await
    .unwrap()
}

async fn source_row(st: &AppState, id: uuid::Uuid) -> Option<(String, String, Option<Value>)> {
    sqlx::query_as("SELECT origin, status, report FROM source_submissions WHERE version_id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await
        .unwrap()
}

/// Tách bảng phát hiện trên một trang: (mức, luật, tệp:dòng, trích đoạn).
fn findings_on(page: &str, scope: &str) -> Vec<(String, String, String, String)> {
    let doc = Html::parse_document(page);
    let sel = Selector::parse(&format!("{scope} table.source-findings tbody tr")).unwrap();
    let td = Selector::parse("td").unwrap();
    doc.select(&sel)
        .map(|tr| {
            let cells: Vec<String> = tr
                .select(&td)
                .map(|c| c.text().collect::<String>().trim().to_owned())
                .collect();
            (
                cells[0].clone(),
                cells[1].clone(),
                cells[2].clone(),
                cells[4].clone(),
            )
        })
        .collect()
}

/// Scenario "Bí mật lộ bị từ chối", "Lỗ hổng cần người duyệt", "Không kèm mã nguồn", "Thứ tự kiểm
/// tra"; nộp mã nguồn qua web, API và `f-dev publish --source`; tải repo lỗi mạng thì xếp hàng lại.
#[tokio::test]
async fn source_scan_end_to_end() {
    if !e2e_ready() {
        return;
    }
    let st = local_state();
    let stamp = (SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000_000_000)
        .to_string();
    let work = tempfile::tempdir().unwrap();
    let clean = write_files(
        &work.path().join("dist-clean"),
        &[("SoundLab.exe", fixtures::pe(true))],
    );
    let app = format!("srcscan-{stamp}");
    let (c, email, token) = developer_with_app("srcscan", &format!("Src {stamp}"), &app).await;

    // --- Scenario "Không kèm mã nguồn": bản đầu vẫn vào duyệt tay vì là bản đầu.
    let (v100, _) = publish(
        work.path(),
        &token,
        &app_manifest(&app, "1.0.0"),
        &clean,
        None,
    )
    .await;
    review::process_due(&st).await.unwrap();
    let (status, stage, _, _, reason, checks) = version_state(&st, v100).await;
    assert_eq!(
        (status.as_str(), stage.as_deref()),
        ("pending_review", Some("manual"))
    );
    assert_eq!(reason.as_deref(), Some("Phiên bản đầu tiên của app"));
    let checks = checks.unwrap();
    assert_eq!(checks["source"]["note"], "Không có mã nguồn", "{checks}");
    assert!(
        checks["packages"].as_array().is_some_and(|p| p.len() == 1),
        "{checks}"
    );
    review::approve(&st, v100, "e2e", "Đạt").await.unwrap();
    let page = get(&c, &format!("/developer/apps/{app}")).await;
    assert!(page.contains("Không có mã nguồn"), "{page}");
    // Bản cập nhật không đổi quyền, không kèm mã nguồn: vẫn được duyệt tự động.
    let (v101, _) = publish(
        work.path(),
        &token,
        &app_manifest(&app, "1.0.1"),
        &clean,
        None,
    )
    .await;
    review::process_due(&st).await.unwrap();
    let (status, stage, note, _, _, checks) = version_state(&st, v101).await;
    assert_eq!(
        (status.as_str(), stage.as_deref()),
        ("approved", Some("done")),
        "{note}"
    );
    assert_eq!(checks.unwrap()["source"]["status"], "none");

    // --- Scenario "Lỗ hổng cần người duyệt": bản cập nhật vốn được tự duyệt, `[source]` trỏ tới
    // repo có thư viện lỗ hổng mức HIGH.
    let manifest = with_git_source(&app_manifest(&app, "1.0.2"), "e2e-ok");
    let (v102, _) = publish(work.path(), &token, &manifest, &clean, None).await;
    let (origin, status, _) = source_row(&st, v102).await.expect("có mã nguồn git");
    assert_eq!((origin.as_str(), status.as_str()), ("git", "pending"));
    review::process_due(&st).await.unwrap();
    let (status, stage, _, _, reason, checks) = version_state(&st, v102).await;
    assert_eq!(
        (status.as_str(), stage.as_deref()),
        ("pending_review", Some("manual"))
    );
    assert_eq!(
        reason.as_deref(),
        Some("Quét mã nguồn: 1 phát hiện cần xem")
    );
    let checks = checks.unwrap();
    assert_eq!(checks["source"]["counts"]["high"], 1, "{checks}");
    assert_eq!(checks["source"]["osv"], "checked");
    let (_, status, report) = source_row(&st, v102).await.unwrap();
    assert_eq!(status, "scanned");
    let report = report.unwrap();
    assert_eq!(report["findings"][0]["rule"], "vulnerability.osv");
    assert!(report["findings"][0]["message"]
        .as_str()
        .unwrap()
        .contains("GHSA-e2e0-high-0001"));
    let queue = review::manual_queue(&st.db).await.unwrap();
    assert!(queue.iter().any(|q| q.version_id == v102));
    // Trang phiên bản của nhà phát triển hiện bảng phát hiện.
    let page = get(&c, &format!("/developer/apps/{app}")).await;
    let rows = findings_on(&page, "div.version[data-version=\"1.0.2\"]");
    assert_eq!(rows.len(), 1, "{page}");
    assert_eq!(
        (rows[0].0.as_str(), rows[0].1.as_str(), rows[0].2.as_str()),
        ("high", "vulnerability.osv", "package-lock.json:6")
    );
    // Trang duyệt của người duyệt (vai trò Reviewer) cũng hiện bảng.
    let (rev_email, rev_password) = unique("reviewer-src");
    let reviewer = app_browser();
    register_in(&reviewer, &rev_email, &rev_password, "Người duyệt").await;
    verify_email(&reviewer, &rev_email).await;
    feco_server::admin::set_member(
        &st,
        "khoi-tao",
        &rev_email,
        &[feco_server::admin::Role::Reviewer],
    )
    .await
    .unwrap();
    let admin_page = get(&reviewer, "/admin/reviews").await;
    let rows = findings_on(
        &admin_page,
        &format!("div.queue-item[data-app=\"{app}\"][data-version=\"1.0.2\"]"),
    );
    assert_eq!(rows.len(), 1, "{admin_page}");
    assert_eq!(rows[0].1, "vulnerability.osv");
    assert!(admin_page.contains("e2e-ok") && admin_page.contains(&COMMIT[..10]));
    review::approve(&st, v102, "e2e", "Thư viện lỗi không dùng tới")
        .await
        .unwrap();

    // --- Scenario "Bí mật lộ bị từ chối": `f-dev publish --source` với token GitHub thật trong mã.
    let token_value = ["ghp_", "Zq3Lm9Xc2Vb7Nn1Kk5Hh8Gg4Ff6Dd0Ss2Aa9"].concat();
    let src = write_files(
        &work.path().join("src-secret"),
        &[
            (
                "src/config.ts",
                format!("export const api = 'https://api.example';\nexport const gh = \"{token_value}\";\n").into_bytes(),
            ),
            ("target/debug/build.log", b"bo qua".to_vec()),
        ],
    );
    let (v103, out) = publish(
        work.path(),
        &token,
        &app_manifest(&app, "1.0.3"),
        &clean,
        Some(&src),
    )
    .await;
    assert!(out.contains("mã nguồn: 1 tệp"), "{out}");
    let (origin, _, _) = source_row(&st, v103).await.expect("có mã nguồn tải lên");
    assert_eq!(origin, "upload");
    review::process_due(&st).await.unwrap();
    let (status, _, note, code, _, checks) = version_state(&st, v103).await;
    assert_eq!(status, "rejected");
    assert_eq!(code.as_deref(), Some("source_scan"));
    assert!(
        note.contains("secret.github_token: src/config.ts dòng 2"),
        "{note}"
    );
    assert!(!note.contains(&token_value[4..]), "{note}");
    assert_eq!(checks.unwrap()["source"]["counts"]["critical"], 1);
    let mail = wait_mail(&email, &format!("{app} 1.0.3 bị từ chối")).await;
    assert!(
        mail.contains("secret.github_token") && mail.contains("src/config.ts"),
        "{mail}"
    );
    assert!(mail.contains("Không qua kiểm tra kỹ thuật"), "{mail}");
    assert!(
        !mail.contains(&token_value[4..]),
        "email không được chép token"
    );
    let page = get(&c, &format!("/developer/apps/{app}")).await;
    let rows = findings_on(&page, "div.version[data-version=\"1.0.3\"]");
    assert_eq!(
        rows,
        [(
            "critical".to_owned(),
            "secret.github_token".to_owned(),
            "src/config.ts:2".to_owned(),
            "ghp_****".to_owned()
        )]
    );
    assert!(!page.contains(&token_value[4..]));
    let stored = source_row(&st, v103).await.unwrap().2.unwrap().to_string();
    assert!(
        !stored.contains(&token_value[4..]),
        "báo cáo lưu đã che bí mật"
    );

    // --- Tải repo lỗi mạng: chưa kết luận, xếp hàng lại sau; mạng phục hồi thì quét xong.
    let manifest = with_git_source(&app_manifest(&app, "1.0.4"), "e2e-flaky");
    let (v104, _) = publish(work.path(), &token, &manifest, &clean, None).await;
    review::process_due(&st).await.unwrap();
    let (status, stage, ..) = version_state(&st, v104).await;
    assert_eq!(
        (status.as_str(), stage.as_deref()),
        ("pending_review", Some("checks"))
    );
    let jobs: Vec<(String, bool)> = sqlx::query_as(
        "SELECT status, COALESCE(not_before > now(), false) FROM review_jobs WHERE version_id = $1 ORDER BY id",
    )
    .bind(v104)
    .fetch_all(&st.db)
    .await
    .unwrap();
    assert_eq!(
        jobs,
        [("failed".to_owned(), false), ("queued".to_owned(), true)],
        "job lỗi được xếp lại có hẹn giờ"
    );
    FLAKY_UP.store(true, Ordering::SeqCst);
    sqlx::query(
        "UPDATE review_jobs SET not_before = now() WHERE version_id = $1 AND status = 'queued'",
    )
    .bind(v104)
    .execute(&st.db)
    .await
    .unwrap();
    review::process_due(&st).await.unwrap();
    let (_, stage, _, _, reason, _) = version_state(&st, v104).await;
    assert_eq!(stage.as_deref(), Some("manual"));
    assert_eq!(
        reason.as_deref(),
        Some("Quét mã nguồn: 1 phát hiện cần xem")
    );

    // --- Nộp mã nguồn qua web và API (bản nháp 1.0.5).
    let v105: uuid::Uuid = {
        let res = reqwest::Client::new()
            .post(format!("{BASE}/v1/dev/apps/{app}/versions"))
            .bearer_auth(&token)
            .body(app_manifest(&app, "1.0.5"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), 201);
        let body: Value = res.json().await.unwrap();
        body["id"].as_str().unwrap().parse().unwrap()
    };
    let source_zip = zip_bytes(&[("src/main.rs", "fn main() {}\n".into())]);
    let put = |body: Vec<u8>, sha: &str| {
        reqwest::Client::new()
            .put(format!("{BASE}/v1/dev/versions/{v105}/source"))
            .bearer_auth(&token)
            .header("x-feco-sha256", sha)
            .body(body)
            .send()
    };
    let res = put(source_zip.clone(), &"0".repeat(64)).await.unwrap();
    assert_eq!(res.status().as_u16(), 422);
    assert_eq!(
        res.json::<Value>().await.unwrap()["error"],
        "invalid_source"
    );
    let res = put(
        b"khong phai zip".to_vec(),
        &scan::sha256_hex(b"khong phai zip"),
    )
    .await
    .unwrap();
    assert_eq!(res.status().as_u16(), 422);
    let evil = zip_bytes(&[("../../evil.sh", "rm -rf /".into())]);
    let res = put(evil.clone(), &scan::sha256_hex(&evil)).await.unwrap();
    assert_eq!(
        res.status().as_u16(),
        422,
        "zip có đường dẫn thoát ra ngoài bị từ chối"
    );
    let res = put(source_zip.clone(), &scan::sha256_hex(&source_zip))
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 204);
    assert_eq!(source_row(&st, v105).await.unwrap().0, "upload");
    // Web: repo và commit (sai rồi đúng), rồi tải tệp zip.
    let page = get(&c, &format!("/developer/apps/{app}")).await;
    let csrf = csrf_of(&page);
    let (status, body) = post(
        &c,
        &format!("/developer/versions/{v105}/source-git"),
        &csrf,
        &[("repository", "https://gitlab.com/a/b"), ("commit", COMMIT)],
    )
    .await;
    assert_eq!(status, 422, "{body}");
    assert!(body.contains("repo GitHub công khai"), "{body}");
    let (status, body) = post(
        &c,
        &format!("/developer/versions/{v105}/source-git"),
        &csrf,
        &[
            ("repository", "https://github.com/e2e-ok/soundlab"),
            ("commit", COMMIT),
        ],
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("Đã lưu mã nguồn"), "{body}");
    assert_eq!(source_row(&st, v105).await.unwrap().0, "git");
    let form = reqwest::multipart::Form::new()
        .text("feco_csrf", csrf.clone())
        .part(
            "file",
            reqwest::multipart::Part::bytes(source_zip.clone()).file_name("source.zip"),
        );
    let res = c
        .post(format!("{BASE}/developer/versions/{v105}/source"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert!(res.text().await.unwrap().contains("Đã lưu mã nguồn"));
    assert_eq!(source_row(&st, v105).await.unwrap().0, "upload");

    // --- Scenario "Thứ tự kiểm tra": gói có mã độc và mã nguồn có bí mật lộ → từ chối vì mã độc,
    // mã nguồn không được quét. (Tài khoản riêng: mã độc làm tài khoản bị tạm khóa.)
    let bad_app = format!("srcbad-{stamp}");
    let (_, _, bad_token) = developer_with_app("srcbad", &format!("Bad {stamp}"), &bad_app).await;
    let infected = work.path().join("infected");
    std::fs::create_dir_all(&infected).unwrap();
    {
        let file = std::fs::File::create(infected.join("windows-x64.zip")).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in [
            ("SoundLab.exe", fixtures::pe(true)),
            ("data/eicar.com", scan::eicar()),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    let (bad, _) = publish(
        work.path(),
        &bad_token,
        &app_manifest(&bad_app, "1.0.0"),
        &infected,
        Some(&src),
    )
    .await;
    review::process_due(&st).await.unwrap();
    let (status, _, note, code, _, checks) = version_state(&st, bad).await;
    assert_eq!(status, "rejected");
    assert_eq!(code.as_deref(), Some("malware"), "{note}");
    assert!(checks.unwrap().get("source").is_none());
    let (origin, status, report) = source_row(&st, bad).await.unwrap();
    assert_eq!((origin.as_str(), status.as_str()), ("upload", "pending"));
    assert!(report.is_none(), "mã nguồn không được quét");
}
