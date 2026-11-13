//! `feco-sbom`: SBOM (CycloneDX 1.5 JSON) và `THIRD_PARTY_NOTICES.md` cho một sản phẩm phát hành
//! (task 15.5), lấy từ cây phụ thuộc thật mà Cargo phân giải cho nền tảng đích.
//!
//! ```bash
//! feco-sbom <gói> <target triple> <thư mục ra> [đường dẫn Cargo.toml]
//! feco-sbom feco-launcher x86_64-pc-windows-msvc dist/sbom
//! feco-sbom f-builder x86_64-pc-windows-msvc dist/sbom clients/f-builder/Cargo.toml
//! ```
//!
//! Chỉ tính phụ thuộc thường (bỏ dev và build-dependencies, vì không đi kèm sản phẩm). Toàn văn
//! LICENSE, COPYING, NOTICE của từng crate (nếu có trong mã nguồn crate) được chép vào notices.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use serde_json::{json, Value};

fn metadata(manifest: Option<&str>, target: &str) -> Result<Value, String> {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.args([
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--filter-platform",
        target,
    ]);
    if let Some(m) = manifest {
        cmd.args(["--manifest-path", m]);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("không chạy được cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

/// `(tên, phiên bản)` của mọi phụ thuộc thường (bắc cầu) của gói, đúng như Cargo phân giải khi
/// dựng riêng gói đó cho nền tảng đích (`cargo tree`, không gộp tính năng của gói khác trong
/// workspace).
fn tree(
    manifest: Option<&str>,
    name: &str,
    target: &str,
) -> Result<BTreeSet<(String, String)>, String> {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.args([
        "tree",
        "-p",
        name,
        "-e",
        "normal",
        "--prefix",
        "none",
        "--no-dedupe",
        "--target",
        target,
        "--locked",
        "--format",
        "{p}",
    ]);
    if let Some(m) = manifest {
        cmd.args(["--manifest-path", m]);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("không chạy được cargo tree: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((
                it.next()?.to_owned(),
                it.next()?.trim_start_matches('v').to_owned(),
            ))
        })
        .collect())
}

/// Gói và mọi phụ thuộc thường của nó (theo [`tree`]); gói gốc đứng đầu.
fn closure(
    meta: &Value,
    manifest: Option<&str>,
    root_name: &str,
    target: &str,
) -> Result<Vec<Value>, String> {
    let packages: Vec<&Value> = meta["packages"].as_array().into_iter().flatten().collect();
    let root = packages
        .iter()
        .find(|p| p["name"] == root_name)
        .ok_or(format!("không có gói {root_name} trong workspace"))?;
    let deps = tree(manifest, root_name, target)?;
    let mut out = vec![(*root).clone()];
    for (name, version) in &deps {
        let p = packages
            .iter()
            .find(|p| p["name"] == name.as_str() && p["version"] == version.as_str())
            .ok_or(format!("cargo metadata thiếu {name} {version}"))?;
        out.push((*p).clone());
    }
    Ok(out)
}

fn license_files(manifest_path: &str) -> Vec<(String, String)> {
    let dir = Path::new(manifest_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_ascii_uppercase();
            p.is_file()
                && (name.starts_with("LICENSE")
                    || name.starts_with("LICENCE")
                    || name.starts_with("COPYING")
                    || name.starts_with("NOTICE"))
        })
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            Some((
                p.file_name()?.to_string_lossy().into_owned(),
                std::fs::read_to_string(&p).ok()?,
            ))
        })
        .collect()
}

fn purl(p: &Value) -> String {
    format!(
        "pkg:cargo/{}@{}",
        p["name"].as_str().unwrap_or_default(),
        p["version"].as_str().unwrap_or_default()
    )
}

fn run(args: &[String]) -> Result<String, String> {
    let [name, target, out_dir, rest @ ..] = args else {
        return Err("cách dùng: feco-sbom <gói> <target triple> <thư mục ra> [Cargo.toml]".into());
    };
    let meta = metadata(rest.first().map(String::as_str), target)?;
    let pkgs = closure(&meta, rest.first().map(String::as_str), name, target)?;
    let root = &pkgs[0];
    let version = root["version"].as_str().unwrap_or_default();
    let components: Vec<Value> = pkgs[1..]
        .iter()
        .map(|p| {
            let mut c = json!({
                "type": "library",
                "bom-ref": purl(p),
                "name": p["name"],
                "version": p["version"],
                "purl": purl(p),
            });
            if let Some(l) = p["license"].as_str() {
                c["licenses"] = json!([{ "expression": l }]);
            }
            if let Some(r) = p["repository"].as_str() {
                c["externalReferences"] = json!([{ "type": "vcs", "url": r }]);
            }
            c
        })
        .collect();
    let bom = json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {"type": "application", "bom-ref": purl(root), "name": name, "version": version, "purl": purl(root)},
            "properties": [{"name": "target", "value": target}],
        },
        "components": components,
        "dependencies": [{"ref": purl(root), "dependsOn": pkgs[1..].iter().map(purl).collect::<Vec<_>>()}],
    });
    let out = Path::new(out_dir);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let bom_path = out.join(format!("{name}-{version}-{target}.cdx.json"));
    std::fs::write(&bom_path, serde_json::to_string_pretty(&bom).unwrap())
        .map_err(|e| e.to_string())?;

    let mut notices = format!(
        "# Thông báo bản quyền bên thứ ba: {name} {version} ({target})\n\n{name} dùng {} thư viện mã nguồn mở dưới đây. Danh sách sinh tự động từ cây phụ thuộc Cargo; toàn văn giấy phép của từng thư viện ở cuối mỗi mục.\n\n",
        pkgs.len() - 1
    );
    notices.push_str("| Thư viện | Phiên bản | Giấy phép |\n|---|---|---|\n");
    for p in &pkgs[1..] {
        notices.push_str(&format!(
            "| {} | {} | {} |\n",
            p["name"].as_str().unwrap_or_default(),
            p["version"].as_str().unwrap_or_default(),
            p["license"].as_str().unwrap_or("(xem tệp giấy phép)")
        ));
    }
    for p in &pkgs[1..] {
        notices.push_str(&format!(
            "\n## {} {}\n\n",
            p["name"].as_str().unwrap_or_default(),
            p["version"].as_str().unwrap_or_default()
        ));
        if let Some(r) = p["repository"].as_str() {
            notices.push_str(&format!("Nguồn: {r}\n\n"));
        }
        notices.push_str(&format!(
            "Giấy phép: {}\n",
            p["license"]
                .as_str()
                .unwrap_or("(không khai trong Cargo.toml)")
        ));
        for (file, text) in license_files(p["manifest_path"].as_str().unwrap_or_default()) {
            notices.push_str(&format!(
                "\n<details><summary>{file}</summary>\n\n```text\n{}\n```\n\n</details>\n",
                text.trim_end()
            ));
        }
    }
    let notices_path = out.join(format!("{name}-{version}-THIRD_PARTY_NOTICES.md"));
    std::fs::write(&notices_path, notices).map_err(|e| e.to_string())?;
    Ok(format!(
        "{} thư viện → {} và {}",
        pkgs.len() - 1,
        bom_path.display(),
        notices_path.display()
    ))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task 15.5: danh sách khớp cây phụ thuộc: mỗi crate trong SBOM đều có thật trong
    /// `cargo metadata`, và gói chỉ dùng trong test (dev-dependency) không lọt vào.
    #[test]
    fn closure_matches_dependency_tree() {
        let target = if cfg!(windows) {
            "x86_64-pc-windows-msvc"
        } else {
            "x86_64-unknown-linux-gnu"
        };
        let meta = metadata(None, target).unwrap();
        let pkgs = closure(&meta, None, "fsdk", target).unwrap();
        let names: BTreeSet<&str> = pkgs.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(pkgs[0]["name"], "fsdk");
        assert!(
            names.contains("reqwest") && names.contains("tokio"),
            "{names:?}"
        );
        assert!(
            !names.contains("tempfile") && !names.contains("feco-server"),
            "không có dev-dependency: {names:?}"
        );
        assert_eq!(pkgs.len() - 1, tree(None, "fsdk", target).unwrap().len());
    }
}
