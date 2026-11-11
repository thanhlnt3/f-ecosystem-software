//! App con mẫu cho môi trường dev: đăng nhập bằng tài khoản F-Eco qua Ory Hydra
//! (OAuth 2.0 Authorization Code + PKCE, client công khai), rồi gọi `GET /v1/me`.
//!
//! ```text
//! cargo run -p feco-demo-app                    # tạo app mới rồi đăng nhập
//! cargo run -p feco-demo-app -- --client-id ID  # dùng lại app đã tạo
//! ```
//!
//! Chỉ dùng cho dev: app tự đăng ký qua API quản trị của Hydra. App thật được đăng ký qua
//! cổng nhà phát triển (nhóm task 8).
//!
//! Khi được F-Launcher mở (có biến `FECO_LAUNCH_CODE`), app không mở trình duyệt: nó đăng nhập
//! bằng F-SDK (`fsdk::session_from_launch`), F-SDK đổi mã khởi chạy lấy token bằng PKCE.

use std::{process::ExitCode, sync::Arc, time::Duration};

use axum::{extract::Query, response::Html, routing::get, Router};
use base64::Engine;
use serde_json::{json, Value};
use sha2::Digest;
use tokio::sync::{oneshot, Mutex};

const CALLBACK: &str = "http://127.0.0.1:9999/cb";

struct Opts {
    hydra_public: String,
    hydra_admin: String,
    api: String,
    client_id: Option<String>,
    scope: String,
}

fn parse_args() -> Result<Opts, String> {
    let mut o = Opts {
        hydra_public: "http://127.0.0.1:4444".into(),
        hydra_admin: "http://127.0.0.1:4445".into(),
        api: "http://127.0.0.1:8080".into(),
        client_id: None,
        scope: "openid profile email".into(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().ok_or(format!("thiếu giá trị cho {a}"));
        match a.as_str() {
            "--hydra" => o.hydra_public = val()?,
            "--hydra-admin" => o.hydra_admin = val()?,
            "--api" => o.api = val()?,
            "--client-id" => o.client_id = Some(val()?),
            "--scope" => o.scope = val()?,
            "-h" | "--help" => {
                return Err(
                    "dùng: feco-demo-app [--client-id ID] [--scope \"openid profile email\"] \
                            [--hydra URL] [--hydra-admin URL] [--api URL]"
                        .into(),
                )
            }
            other => return Err(format!("tham số lạ: {other}")),
        }
    }
    Ok(o)
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Đăng ký app mẫu trong Hydra, khai báo scope và mục đích như cổng nhà phát triển sẽ làm.
async fn register(http: &reqwest::Client, admin: &str) -> Result<String, String> {
    let body = json!({
        "client_name": "Demo App",
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": "openid offline_access profile email locale phone",
        "redirect_uris": [CALLBACK],
        "token_endpoint_auth_method": "none",
        "subject_type": "pairwise",
        "policy_uri": "https://demo.example/privacy",
        "metadata": {
            "developer": "F-Eco (app mẫu)",
            "scopes": {
                "profile": {"required": true, "purpose_vi": "Hiển thị tên của bạn trong app", "purpose_en": "Show your name in the app"},
                "email": {"required": false, "purpose_vi": "Gửi thông báo về tài khoản", "purpose_en": "Send account notifications"},
                "locale": {"required": false, "purpose_vi": "Hiển thị đúng ngôn ngữ", "purpose_en": "Use your language"},
                "phone": {"required": false, "purpose_vi": "Gửi mã xác nhận", "purpose_en": "Send confirmation codes"}
            }
        }
    });
    let res: Value = http
        .post(format!("{admin}/admin/clients"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("không gọi được Hydra admin ({admin}): {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    res["client_id"]
        .as_str()
        .map(str::to_owned)
        .ok_or(format!("Hydra không trả client_id: {res}"))
}

/// Được F-Launcher mở: đăng nhập bằng F-SDK với mã khởi chạy dùng một lần, không cần trình duyệt.
async fn run_from_launcher(opts: &Opts, launch_code: &str) -> Result<(), String> {
    let env = |k: &str, default: String| std::env::var(k).unwrap_or(default);
    let client_id =
        std::env::var("FECO_CLIENT_ID").map_err(|_| "thiếu FECO_CLIENT_ID".to_owned())?;
    let issuer = env("FECO_ISSUER_URL", format!("{}/", opts.hydra_public));
    let server = env("FECO_SERVER_URL", format!("{}/", opts.api));
    println!("Demo App được F-Launcher mở (client_id = {client_id}), đăng nhập bằng F-SDK.");
    let config = fsdk::Config::new(&client_id, &issuer, &server)
        .map_err(|e| e.to_string())?
        .redirect_uri(CALLBACK)
        .scopes(&env("FSDK_SCOPES", opts.scope.clone()));
    let launch = fsdk::Launch {
        code: launch_code.to_owned(),
    };
    let session = match fsdk::session_from_launch(&config, &launch).await {
        Err(fsdk::Error::ConsentRequired) => {
            return Err("bạn chưa đồng ý cho app này; chạy `cargo run -p feco-demo-app -- --client-id <ID>` một lần để đồng ý qua trình duyệt".into())
        }
        other => other.map_err(|e| e.to_string())?,
    };
    let me = session.me().await.map_err(|e| e.to_string())?;
    println!("Scope được cấp: {}", session.granted_scopes().await);
    println!(
        "Đã đăng nhập không cần trình duyệt. GET /v1/me:
{}",
        serde_json::to_string_pretty(&me.raw).unwrap_or_default()
    );
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match run(opts).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Lỗi: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(opts: Opts) -> Result<(), String> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    if let Ok(code) = std::env::var("FECO_LAUNCH_CODE") {
        let result = run_from_launcher(&opts, &code).await;
        if let Err(e) = &result {
            eprintln!("Lỗi: {e}");
        }
        println!("\nNhấn Enter để đóng.");
        let _ = std::io::stdin().read_line(&mut String::new());
        return result;
    }
    let client_id = match opts.client_id {
        Some(id) => id,
        None => {
            let id = register(&http, &opts.hydra_admin).await?;
            println!("Đã tạo app mẫu, client_id = {id}");
            println!("(lần sau dùng lại: cargo run -p feco-demo-app -- --client-id {id})\n");
            id
        }
    };

    // PKCE (RFC 7636) và state chống CSRF.
    let verifier = random_hex(32);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(verifier.as_bytes()));
    let state = random_hex(16);
    let mut auth = url::Url::parse(&format!("{}/oauth2/auth", opts.hydra_public))
        .map_err(|e| e.to_string())?;
    auth.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("response_type", "code")
        .append_pair("scope", &opts.scope)
        .append_pair("redirect_uri", CALLBACK)
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");

    // Máy chủ nhỏ nhận callback ở 127.0.0.1:9999.
    let (tx, rx) = oneshot::channel::<(Option<String>, Option<String>, Option<String>)>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let app = Router::new().route(
        "/cb",
        get(move |Query(q): Query<std::collections::HashMap<String, String>>| {
            let tx = tx.clone();
            async move {
                if let Some(tx) = tx.lock().await.take() {
                    let _ = tx.send((q.get("code").cloned(), q.get("state").cloned(), q.get("error").cloned()));
                }
                Html("<!doctype html><meta charset=utf-8><p>Demo App đã nhận kết quả. Hãy quay lại cửa sổ terminal. / Demo App received the result. Return to the terminal.</p>")
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9999")
        .await
        .map_err(|e| format!("cổng 9999 đang bận: {e}"))?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    println!("Mở địa chỉ sau trong trình duyệt để đăng nhập bằng tài khoản F-Eco:\n\n{auth}\n");
    println!("Đang chờ (tối đa 10 phút)...");
    let (code, got_state, error) = tokio::time::timeout(Duration::from_secs(600), rx)
        .await
        .map_err(|_| "hết thời gian chờ".to_owned())?
        .map_err(|_| "callback bị đóng".to_owned())?;
    if let Some(e) = error {
        return Err(format!("người dùng không cho phép hoặc có lỗi: {e}"));
    }
    if got_state.as_deref() != Some(state.as_str()) {
        return Err("state không khớp, bỏ qua kết quả".into());
    }
    let code = code.ok_or("callback không có code")?;

    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", &code)
        .append_pair("redirect_uri", CALLBACK)
        .append_pair("client_id", &client_id)
        .append_pair("code_verifier", &verifier)
        .finish();
    let tokens: Value = http
        .post(format!("{}/oauth2/token", opts.hydra_public))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let Some(access) = tokens["access_token"].as_str() else {
        return Err(format!("không đổi được code lấy token: {tokens}"));
    };
    println!("\nĐã nhận token. Scope được cấp: {}", tokens["scope"]);

    let me: Value = http
        .get(format!("{}/v1/me", opts.api))
        .bearer_auth(access)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "\nGET /v1/me:\n{}",
        serde_json::to_string_pretty(&me).unwrap_or_default()
    );
    Ok(())
}
