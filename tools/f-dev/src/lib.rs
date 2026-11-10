//! `f-dev`: công cụ dòng lệnh cho nhà phát triển F-Eco (spec developer-portal, "Nộp gói phiên
//! bản").
//!
//! ```text
//! f-dev validate --manifest f-app.toml
//! f-dev publish  --manifest f-app.toml --package dist/ [--server URL] [--token TOKEN]
//! ```
//!
//! `--package` là một file `.zip` (khi manifest chỉ có một nền tảng), hoặc một thư mục chứa
//! `<os>-<arch>.zip` hay thư mục con `<os>-<arch>/` cho từng nền tảng. Nếu manifest chỉ có một
//! nền tảng và thư mục không có các mục trên, cả thư mục được nén thành gói.
//!
//! Token lấy từ `--token` hoặc biến môi trường `FDEV_TOKEN`; máy chủ từ `--server` hoặc
//! `FDEV_SERVER` (mặc định `http://127.0.0.1:8080/`).

use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use feco_manifest::Manifest;
use serde_json::Value;
use sha2::Digest;

const USAGE: &str = "\
f-dev: công cụ cho nhà phát triển F-Eco

  f-dev validate --manifest f-app.toml
  f-dev publish  --manifest f-app.toml --package dist/ [--server URL] [--token TOKEN]

Biến môi trường: FDEV_TOKEN (token tạo ở Cổng nhà phát triển), FDEV_SERVER.
";

fn flags(args: &[String]) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let name = arg
            .strip_prefix("--")
            .ok_or_else(|| format!("tham số lạ: {arg}\n\n{USAGE}"))?;
        let value = iter
            .next()
            .ok_or_else(|| format!("--{name} cần một giá trị"))?;
        out.insert(name.to_owned(), value.clone());
    }
    Ok(out)
}

fn read_manifest(flags: &HashMap<String, String>) -> Result<(String, Manifest), String> {
    let path = flags
        .get("manifest")
        .ok_or_else(|| format!("thiếu --manifest\n\n{USAGE}"))?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("không đọc được {path}: {e}"))?;
    match feco_manifest::parse(&text) {
        Ok(m) => Ok((text, m)),
        Err(errors) => Err(format!(
            "{path} không hợp lệ:\n  - {}",
            errors.join("\n  - ")
        )),
    }
}

/// Chạy một lệnh `f-dev`; trả nội dung cần in ra.
pub async fn run(args: &[String]) -> Result<String, String> {
    let Some(command) = args.first() else {
        return Err(USAGE.to_owned());
    };
    match command.as_str() {
        "validate" => {
            let (_, m) = read_manifest(&flags(&args[1..])?)?;
            let platforms: Vec<String> = m.platforms.iter().map(|p| p.key()).collect();
            Ok(format!(
                "{} {} hợp lệ ({})",
                m.id,
                m.version,
                platforms.join(", ")
            ))
        }
        "publish" => publish(&flags(&args[1..])?).await,
        "help" | "--help" | "-h" => Ok(USAGE.to_owned()),
        other => Err(format!("lệnh không hợp lệ: {other}\n\n{USAGE}")),
    }
}

/// Nén một thư mục thành file zip (đường dẫn trong zip dùng `/`).
fn zip_dir(dir: &Path, out: &Path) -> Result<(), String> {
    fn walk(
        zip: &mut zip::ZipWriter<std::fs::File>,
        root: &Path,
        dir: &Path,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .flatten()
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(zip, root, &path)?;
                continue;
            }
            let name = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let options = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            let mut file =
                std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            std::io::copy(&mut file, zip).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    let file = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    walk(&mut zip, dir, dir)?;
    zip.finish()
        .map_err(|e| e.to_string())?
        .flush()
        .map_err(|e| e.to_string())
}

/// Tìm (hoặc tạo) file zip cho từng nền tảng trong manifest.
fn packages(
    manifest: &Manifest,
    package: &Path,
    work: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    let single = manifest.platforms.len() == 1;
    let mut out = Vec::new();
    for platform in &manifest.platforms {
        let key = platform.key();
        let zipped = work.join(format!("{key}.zip"));
        let path = if package.is_file() {
            if !single {
                return Err(
                    "manifest có nhiều nền tảng: --package phải là thư mục chứa <os>-<arch>.zip"
                        .into(),
                );
            }
            package.to_path_buf()
        } else if package.join(format!("{key}.zip")).is_file() {
            package.join(format!("{key}.zip"))
        } else if package.join(&key).is_dir() {
            zip_dir(&package.join(&key), &zipped)?;
            zipped
        } else if single && package.is_dir() {
            zip_dir(package, &zipped)?;
            zipped
        } else {
            return Err(format!(
                "không tìm thấy gói cho {key} trong {} (cần {key}.zip hoặc thư mục {key}/)",
                package.display()
            ));
        };
        out.push((key, path));
    }
    Ok(out)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Đọc phản hồi JSON; lỗi thì trả thông báo tiếng Việt của máy chủ.
async fn expect(res: reqwest::Response, what: &str) -> Result<Value, String> {
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(body);
    }
    let message = body["message_vi"].as_str().unwrap_or(text.trim());
    Err(format!("{what} bị từ chối ({status}):\n{message}"))
}

async fn publish(flags: &HashMap<String, String>) -> Result<String, String> {
    let (toml, manifest) = read_manifest(flags)?;
    let package = PathBuf::from(
        flags
            .get("package")
            .ok_or_else(|| format!("thiếu --package\n\n{USAGE}"))?,
    );
    let token = flags
        .get("token")
        .cloned()
        .or_else(|| std::env::var("FDEV_TOKEN").ok())
        .ok_or("thiếu token: đặt FDEV_TOKEN hoặc dùng --token (tạo token ở Cổng nhà phát triển)")?;
    let server = flags
        .get("server")
        .cloned()
        .or_else(|| std::env::var("FDEV_SERVER").ok())
        .unwrap_or_else(|| "http://127.0.0.1:8080/".into());
    let server = format!("{}/", server.trim_end_matches('/'));
    if !(server.starts_with("https://")
        || server.starts_with("http://127.0.0.1")
        || server.starts_with("http://localhost"))
    {
        return Err(format!("máy chủ phải dùng HTTPS: {server}"));
    }

    let work = std::env::temp_dir().join(format!("f-dev-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = upload(&server, &token, &toml, &manifest, &package, &work).await;
    let _ = std::fs::remove_dir_all(&work);
    result
}

async fn upload(
    server: &str,
    token: &str,
    toml: &str,
    manifest: &Manifest,
    package: &Path,
    work: &Path,
) -> Result<String, String> {
    let packages = packages(manifest, package, work)?;
    let mut out = String::new();
    let http = reqwest::Client::new();
    let version = expect(
        http.post(format!("{server}v1/dev/apps/{}/versions", manifest.id))
            .bearer_auth(token)
            .header("content-type", "text/plain; charset=utf-8")
            .body(toml.to_owned())
            .send()
            .await
            .map_err(|e| format!("không kết nối được {server}: {e}"))?,
        "manifest",
    )
    .await?;
    let id = version["id"]
        .as_str()
        .ok_or("máy chủ không trả mã phiên bản")?;
    for (platform, path) in &packages {
        let sha = sha256_file(path)?;
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let size = file.metadata().await.map_err(|e| e.to_string())?.len();
        out.push_str(&format!(
            "gói {platform}: {size} byte, sha256 {sha}
"
        ));
        expect(
            http.put(format!("{server}v1/dev/versions/{id}/packages/{platform}"))
                .bearer_auth(token)
                .header("x-feco-sha256", &sha)
                .header("content-length", size)
                .body(reqwest::Body::from(file))
                .send()
                .await
                .map_err(|e| e.to_string())?,
            &format!("gói {platform}"),
        )
        .await?;
    }
    let submitted = expect(
        http.post(format!("{server}v1/dev/versions/{id}/submit"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?,
        "gửi duyệt",
    )
    .await?;
    let status = match submitted["status"].as_str() {
        Some("pending_review") => "Chờ duyệt",
        Some(other) => other,
        None => "?",
    };
    out.push_str(&format!(
        "{} {}: {status}
mã phiên bản: {id}",
        manifest.id, manifest.version
    ));
    Ok(out)
}
