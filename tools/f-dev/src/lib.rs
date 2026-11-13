//! `f-dev`: công cụ dòng lệnh cho nhà phát triển F-Eco (spec developer-portal, "Nộp gói phiên
//! bản").
//!
//! ```text
//! f-dev validate --manifest f-app.toml
//! f-dev pack     --manifest f-app.toml --package build/ [--out dist/]
//! f-dev publish  --manifest f-app.toml --package dist/ [--server URL] [--token TOKEN]
//! ```
//!
//! `--package` là một file `.zip` (khi manifest chỉ có một nền tảng), hoặc một thư mục chứa
//! `<os>-<arch>.zip` hay thư mục con `<os>-<arch>/` cho từng nền tảng. Nếu manifest chỉ có một
//! nền tảng và thư mục không có các mục trên, cả thư mục được nén thành gói.
//!
//! Manifest có `source` (app dựng từ mã nguồn bên ngoài, ví dụ trên GitHub) thì mỗi gói phải có
//! file giấy phép gốc (`LICENSE*`, `COPYING*` hoặc `NOTICE*`); `pack` và `publish` từ chối gói
//! thiếu file này.
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
  f-dev pack     --manifest f-app.toml --package build/ [--out dist/]
                 đóng gói <os>-<arch>.zip và kiểm tra trước khi nộp
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
            let mut out = format!("{} {} hợp lệ ({})", m.id, m.version, platforms.join(", "));
            for warning in warnings(&m) {
                out.push_str(&format!("\ncảnh báo: {warning}"));
            }
            Ok(out)
        }
        "pack" => pack(&flags(&args[1..])?),
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

/// Điều nên sửa nhưng không chặn việc nộp.
pub fn warnings(m: &Manifest) -> Vec<String> {
    let mut out = Vec::new();
    // design E5: trên macOS app terminal được mở qua `open -a Terminal`, không truyền được biến
    // môi trường nên app không nhận được mã khởi chạy của launcher.
    if m.terminal && !m.scopes.is_empty() && m.platforms.iter().any(|p| p.os == "macos") {
        out.push(
            "app terminal trên macOS không nhận được mã khởi chạy từ launcher: app phải tự đăng nhập bằng trình duyệt (F-SDK `session`)"
                .to_owned(),
        );
    }
    out
}

/// Tên file giấy phép gốc: `LICENSE*`, `LICENCE*`, `COPYING*`, `NOTICE*` (không phân biệt hoa
/// thường), ở bất kỳ thư mục nào trong gói.
fn is_license_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_uppercase();
    ["LICENSE", "LICENCE", "COPYING", "NOTICE"]
        .iter()
        .any(|p| name.starts_with(p))
}

/// Spec developer-portal, "App mã nguồn mở dựng từ mã nguồn bên ngoài": gói của app có `source`
/// phải kèm giấy phép gốc.
fn check_license(manifest: &Manifest, key: &str, zip_path: &Path) -> Result<(), String> {
    let Some(source) = &manifest.source else {
        return Ok(());
    };
    let file = std::fs::File::open(zip_path).map_err(|e| format!("{}: {e}", zip_path.display()))?;
    let archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("{} không phải file zip: {e}", zip_path.display()))?;
    if archive
        .file_names()
        .flatten()
        .any(|name| is_license_file(&name))
    {
        return Ok(());
    }
    Err(format!(
        "gói {key} thiếu file giấy phép gốc (LICENSE*, COPYING* hoặc NOTICE*) của {source}: \
         chép file giấy phép của dự án vào gói rồi thử lại"
    ))
}

/// `f-dev pack`: tạo `<os>-<arch>.zip` cho từng nền tảng trong `--out` (mặc định `dist/`).
fn pack(flags: &HashMap<String, String>) -> Result<String, String> {
    let (_, manifest) = read_manifest(flags)?;
    let package = PathBuf::from(
        flags
            .get("package")
            .ok_or_else(|| format!("thiếu --package\n\n{USAGE}"))?,
    );
    let out = PathBuf::from(flags.get("out").map_or("dist", String::as_str));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let work = std::env::temp_dir().join(format!("f-dev-pack-{}", std::process::id()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = packages(&manifest, &package, &work).and_then(|found| {
        let mut lines = Vec::new();
        for (key, path) in found {
            let dest = out.join(format!("{key}.zip"));
            if path != dest {
                std::fs::copy(&path, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
            }
            lines.push(format!("đã đóng gói {}", dest.display()));
        }
        Ok(lines.join("\n"))
    });
    let _ = std::fs::remove_dir_all(&work);
    result
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
        check_license(manifest, &key, &path)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "id = \"kibi\"\nname = \"kibi\"\nversion = \"0.3.3\"\nlicense = \"MIT OR Apache-2.0\"\n\
        privacy_policy = \"https://github.com/ilai-deutel/kibi\"\nsource = \"https://github.com/ilai-deutel/kibi\"\n\
        terminal = true\n[[platforms]]\nos = \"windows\"\narch = \"x64\"\nentry = \"bin/kibi.exe\"\n";

    fn pack_dir(files: &[&str]) -> (tempfile::TempDir, Result<String, String>) {
        let dir = tempfile::tempdir().unwrap();
        let build = dir.path().join("build");
        for f in files {
            let p = build.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        let manifest = dir.path().join("f-app.toml");
        std::fs::write(&manifest, MANIFEST).unwrap();
        let args: Vec<String> = [
            "--manifest",
            manifest.to_str().unwrap(),
            "--package",
            build.to_str().unwrap(),
            "--out",
            dir.path().join("dist").to_str().unwrap(),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let result = pack(&flags(&args).unwrap());
        (dir, result)
    }

    /// Scenario "Gói thiếu giấy phép gốc".
    #[test]
    fn source_package_needs_upstream_license() {
        let (dir, result) = pack_dir(&["bin/kibi.exe"]);
        let err = result.unwrap_err();
        assert!(err.contains("thiếu file giấy phép gốc"), "{err}");
        assert!(!dir.path().join("dist/windows-x64.zip").exists());

        let (dir, result) = pack_dir(&[
            "bin/kibi.exe",
            "licenses/LICENSE-MIT",
            "licenses/LICENSE-APACHE",
        ]);
        assert!(result.unwrap().contains("windows-x64.zip"));
        let file = std::fs::File::open(dir.path().join("dist/windows-x64.zip")).unwrap();
        let zip = zip::ZipArchive::new(file).unwrap();
        let mut names: Vec<String> = zip.file_names().flatten().map(|n| n.into_owned()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "bin/kibi.exe",
                "licenses/LICENSE-APACHE",
                "licenses/LICENSE-MIT"
            ]
        );
    }

    #[test]
    fn license_file_names() {
        for ok in [
            "LICENSE",
            "a/b/license-mit.txt",
            "COPYING",
            "NOTICE.md",
            "Licence",
        ] {
            assert!(is_license_file(ok), "{ok}");
        }
        for bad in ["README.md", "bin/kibi.exe", "licenses/", "my-license.txt"] {
            assert!(!is_license_file(bad), "{bad}");
        }
    }

    #[test]
    fn terminal_app_with_scopes_on_macos_warns() {
        let base = MANIFEST
            .replace("os = \"windows\"", "os = \"macos\"")
            .replace("arch = \"x64\"", "arch = \"arm64\"");
        assert!(warnings(&feco_manifest::parse(&base).unwrap()).is_empty());
        let with_scope = format!("{base}[scopes.profile]\npurpose = \"Tên\"\n");
        let w = warnings(&feco_manifest::parse(&with_scope).unwrap());
        assert_eq!(w.len(), 1, "{w:?}");
    }
}
