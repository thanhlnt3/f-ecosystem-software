//! Đọc lockfile thành danh sách (hệ sinh thái OSV, tên, phiên bản): `Cargo.lock`,
//! `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `requirements*.txt` (chỉ dòng ghim `==`),
//! `poetry.lock`, `uv.lock`, `go.sum`. Thư viện lấy từ git hay đường dẫn cục bộ bị bỏ qua (OSV
//! không tra được).

use std::{collections::HashMap, sync::OnceLock};

use regex::Regex;
use serde_json::Value;

use crate::Dependency;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    CargoLock,
    PackageLock,
    PnpmLock,
    YarnLock,
    Requirements,
    PoetryLock,
    UvLock,
    GoSum,
}

/// Loại lockfile theo tên tệp.
pub fn kind(path: &str) -> Option<Kind> {
    let file = path.rsplit('/').next().unwrap_or(path);
    Some(match file {
        "Cargo.lock" => Kind::CargoLock,
        "package-lock.json" | "npm-shrinkwrap.json" => Kind::PackageLock,
        "pnpm-lock.yaml" => Kind::PnpmLock,
        "yarn.lock" => Kind::YarnLock,
        "poetry.lock" => Kind::PoetryLock,
        "uv.lock" => Kind::UvLock,
        "go.sum" => Kind::GoSum,
        f if f.starts_with("requirements") && f.ends_with(".txt") => Kind::Requirements,
        _ => return None,
    })
}

pub fn parse(kind: Kind, path: &str, text: &str) -> Vec<Dependency> {
    let mut out = Vec::new();
    let mut push = |ecosystem: &str, name: &str, version: &str, line: usize| {
        let (name, version) = (name.trim(), version.trim());
        if !name.is_empty() && version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            out.push(Dependency {
                ecosystem: ecosystem.into(),
                name: name.into(),
                version: version.into(),
                path: path.into(),
                line: line as u32,
            });
        }
    };
    match kind {
        Kind::CargoLock => {
            for p in toml_packages(text) {
                if p.source
                    .as_deref()
                    .is_some_and(|s| s.starts_with("registry+") || s.starts_with("sparse+"))
                {
                    push("crates.io", &p.name, &p.version, p.line);
                }
            }
        }
        Kind::PoetryLock => {
            for p in toml_packages(text) {
                if !matches!(
                    p.source_type.as_deref(),
                    Some("git" | "directory" | "file" | "url")
                ) {
                    push("PyPI", &p.name, &p.version, p.line);
                }
            }
        }
        Kind::UvLock => {
            for p in toml_packages(text) {
                if p.source.as_deref().is_some_and(|s| s.contains("registry")) {
                    push("PyPI", &p.name, &p.version, p.line);
                }
            }
        }
        Kind::PackageLock => package_lock(text, &mut push),
        Kind::PnpmLock => pnpm(text, &mut push),
        Kind::YarnLock => yarn(text, &mut push),
        Kind::Requirements => {
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| {
                Regex::new(r"^\s*([A-Za-z0-9][A-Za-z0-9._-]*)\s*(?:\[[^\]]*\])?\s*==\s*([0-9][A-Za-z0-9.!+_-]*)")
                    .unwrap()
            });
            for (i, line) in text.lines().enumerate() {
                if let Some(c) = re.captures(line) {
                    push("PyPI", &c[1].to_ascii_lowercase(), &c[2], i + 1);
                }
            }
        }
        Kind::GoSum => {
            let mut seen = std::collections::HashSet::new();
            for (i, line) in text.lines().enumerate() {
                let mut parts = line.split_whitespace();
                let (Some(module), Some(version)) = (parts.next(), parts.next()) else {
                    continue;
                };
                let version = version.trim_end_matches("/go.mod");
                let version = version.strip_prefix('v').unwrap_or(version);
                if seen.insert((module.to_owned(), version.to_owned())) {
                    push("Go", module, version, i + 1);
                }
            }
        }
    }
    out
}

struct TomlPackage {
    name: String,
    version: String,
    source: Option<String>,
    source_type: Option<String>,
    line: usize,
}

/// Đọc các khối `[[package]]` của `Cargo.lock`, `poetry.lock`, `uv.lock` theo dòng (để biết số
/// dòng của từng thư viện).
fn toml_packages(text: &str) -> Vec<TomlPackage> {
    static KV: OnceLock<Regex> = OnceLock::new();
    let kv = KV.get_or_init(|| Regex::new(r#"^(name|version|source|type)\s*=\s*(.+)$"#).unwrap());
    let mut out: Vec<TomlPackage> = Vec::new();
    // Đang ở phần đầu khối [[package]] (trước bảng con), hoặc ở [package.source] của poetry.
    let (mut head, mut in_source) = (false, false);
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line == "[[package]]" {
            out.push(TomlPackage {
                name: String::new(),
                version: String::new(),
                source: None,
                source_type: None,
                line: i + 1,
            });
            (head, in_source) = (true, false);
            continue;
        }
        if line.starts_with('[') {
            head = false;
            in_source = line == "[package.source]";
            continue;
        }
        let Some(c) = kv.captures(line) else { continue };
        let Some(p) = out.last_mut() else { continue };
        let value = c[2].trim();
        let unquoted = value.trim_matches('"').to_owned();
        match (&c[1], head, in_source) {
            ("name", true, _) => p.name = unquoted,
            ("version", true, _) => p.version = unquoted,
            ("source", true, _) => p.source = Some(unquoted),
            ("type", _, true) => p.source_type = Some(unquoted),
            _ => {}
        }
    }
    out
}

fn package_lock(text: &str, push: &mut impl FnMut(&str, &str, &str, usize)) {
    let Ok(json) = serde_json::from_str::<Value>(text) else {
        return;
    };
    // Số dòng của từng khóa "node_modules/..." (lockfile v2, v3).
    static KEY: OnceLock<Regex> = OnceLock::new();
    let key_re = KEY
        .get_or_init(|| Regex::new(r#"^\s*"((?:[^"]*/)?node_modules/[^"]+)"\s*:\s*\{"#).unwrap());
    let lines: HashMap<String, usize> = text
        .lines()
        .enumerate()
        .filter_map(|(i, l)| key_re.captures(l).map(|c| (c[1].to_owned(), i + 1)))
        .collect();
    let local = |v: &Value| {
        v["link"].as_bool() == Some(true)
            || v["resolved"]
                .as_str()
                .is_some_and(|r| r.starts_with("file:") || r.starts_with("git"))
    };
    if let Some(packages) = json["packages"].as_object() {
        for (key, v) in packages {
            let Some(idx) = key.rfind("node_modules/") else {
                continue; // "" là chính dự án
            };
            if local(v) {
                continue;
            }
            let name = v["name"]
                .as_str()
                .unwrap_or(&key[idx + "node_modules/".len()..]);
            if let Some(version) = v["version"].as_str() {
                push("npm", name, version, lines.get(key).copied().unwrap_or(0));
            }
        }
        return;
    }
    // Lockfile v1: cây `dependencies` lồng nhau.
    fn walk(deps: &Value, push: &mut impl FnMut(&str, &str, &str, usize)) {
        let Some(map) = deps.as_object() else { return };
        for (name, v) in map {
            let resolved = v["resolved"].as_str().unwrap_or_default();
            if let Some(version) = v["version"].as_str() {
                if !resolved.starts_with("file:") && !resolved.starts_with("git") {
                    push("npm", name, version, 0);
                }
            }
            walk(&v["dependencies"], push);
        }
    }
    walk(&json["dependencies"], push);
}

/// Tách `name@version` (tên có thể bắt đầu bằng `@scope/`).
fn split_at_version(spec: &str) -> Option<(&str, &str)> {
    let at = spec.get(1..)?.rfind('@')? + 1;
    Some((&spec[..at], &spec[at + 1..]))
}

/// `pnpm-lock.yaml` (v5 tới v9): khóa trong phần `packages:`, dạng `/name/1.2.3`,
/// `/name@1.2.3(peer@x)` hoặc `name@1.2.3`.
fn pnpm(text: &str, push: &mut impl FnMut(&str, &str, &str, usize)) {
    static V5: OnceLock<Regex> = OnceLock::new();
    let v5 = V5.get_or_init(|| Regex::new(r"^((?:@[^/]+/)?[^/@]+)/([0-9][^/]*)$").unwrap());
    let mut in_packages = false;
    for (i, line) in text.lines().enumerate() {
        if !line.starts_with(' ') && !line.is_empty() {
            in_packages = line.trim_end() == "packages:";
            continue;
        }
        if !in_packages || !line.starts_with("  ") || line.starts_with("   ") {
            continue;
        }
        let Some(key) = line.trim().strip_suffix(':') else {
            continue;
        };
        let key = key.trim_matches(|c| c == '\'' || c == '"');
        let key = key.split('(').next().unwrap_or(key);
        let key = key.strip_prefix('/').unwrap_or(key);
        // v5: name/version (tên có scope: @scope/name/version); v6 trở đi: name@version.
        let (name, version) = match v5.captures(key) {
            Some(c) => (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str()),
            None => match split_at_version(key) {
                Some(x) => x,
                None => continue,
            },
        };
        let version = version.split('_').next().unwrap_or(version);
        push("npm", name, version, i + 1);
    }
}

/// `yarn.lock` v1 (`version "1.2.3"`) và Berry (`version: 1.2.3`).
fn yarn(text: &str, push: &mut impl FnMut(&str, &str, &str, usize)) {
    let mut current: Option<(String, usize)> = None;
    let mut seen = std::collections::HashSet::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            current = None;
            let Some(header) = line.trim_end().strip_suffix(':') else {
                continue;
            };
            let first = header.split(", ").next().unwrap_or(header);
            let first = first.trim_matches('"');
            if first == "__metadata" {
                continue;
            }
            let Some((name, range)) = split_at_version(first) else {
                continue;
            };
            let local = [
                "workspace:",
                "file:",
                "link:",
                "portal:",
                "patch:",
                "git",
                "http",
            ]
            .iter()
            .any(|p| range.starts_with(p) || range.contains(&format!("@{p}")));
            if !local {
                current = Some((name.to_owned(), i + 1));
            }
            continue;
        }
        let trimmed = line.trim();
        let version = trimmed
            .strip_prefix("version ")
            .or_else(|| trimmed.strip_prefix("version: "));
        if let (Some(version), Some((name, at))) = (version, &current) {
            let version = version.trim().trim_matches('"');
            if seen.insert((name.clone(), version.to_owned())) {
                push("npm", name, version, *at);
            }
            current = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deps(path: &str, text: &str) -> Vec<(String, String, String, u32)> {
        parse(kind(path).unwrap(), path, text)
            .into_iter()
            .map(|d| (d.ecosystem, d.name, d.version, d.line))
            .collect()
    }

    fn t(e: &str, n: &str, v: &str, l: u32) -> (String, String, String, u32) {
        (e.into(), n.into(), v.into(), l)
    }

    #[test]
    fn cargo_lock() {
        let text = "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\n \"serde\",\n]\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"abc\"\n\n[[package]]\nname = \"gitdep\"\nversion = \"0.2.0\"\nsource = \"git+https://github.com/x/y#abc\"\n";
        assert_eq!(
            deps("Cargo.lock", text),
            [t("crates.io", "serde", "1.0.200", 10)]
        );
    }

    #[test]
    fn package_lock_v3_and_v1() {
        let v3 = r#"{
  "name": "web",
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "web", "version": "1.0.0" },
    "node_modules/lodash": { "version": "4.17.20" },
    "node_modules/@scope/pkg": { "version": "2.0.0", "dev": true },
    "node_modules/a/node_modules/lodash": { "version": "3.10.1" },
    "packages/ui": { "version": "0.0.1" },
    "node_modules/ui": { "resolved": "packages/ui", "link": true }
  }
}"#;
        let mut got = deps("web/package-lock.json", v3);
        got.sort();
        assert_eq!(
            got,
            [
                t("npm", "@scope/pkg", "2.0.0", 7),
                t("npm", "lodash", "3.10.1", 8),
                t("npm", "lodash", "4.17.20", 6),
            ]
        );
        let v1 = r#"{"lockfileVersion": 1, "dependencies": {"left-pad": {"version": "1.3.0", "dependencies": {"x": {"version": "0.1.0"}}}}}"#;
        let mut got = deps("package-lock.json", v1);
        got.sort();
        assert_eq!(
            got,
            [t("npm", "left-pad", "1.3.0", 0), t("npm", "x", "0.1.0", 0)]
        );
    }

    #[test]
    fn pnpm_versions() {
        let v9 = "lockfileVersion: '9.0'\n\nimporters:\n\n  .:\n    dependencies:\n      next:\n        specifier: 16.0.0\n        version: 16.0.0(react@19.0.0)\n\npackages:\n\n  '@babel/core@7.26.0':\n    resolution: {integrity: sha512-x}\n\n  next@16.0.0:\n    resolution: {integrity: sha512-y}\n\nsnapshots:\n\n  next@16.0.0(react@19.0.0):\n    dependencies: {}\n";
        assert_eq!(
            deps("pnpm-lock.yaml", v9),
            [
                t("npm", "@babel/core", "7.26.0", 13),
                t("npm", "next", "16.0.0", 16)
            ]
        );
        let v6 = "lockfileVersion: '6.0'\npackages:\n  /react@18.2.0:\n    resolution: {}\n  /@types/node@20.1.0(x@1.0.0):\n    dev: true\n";
        assert_eq!(
            deps("pnpm-lock.yaml", v6),
            [
                t("npm", "react", "18.2.0", 3),
                t("npm", "@types/node", "20.1.0", 5)
            ]
        );
        let v5 = "lockfileVersion: 5.4\npackages:\n  /react/17.0.2:\n    resolution: {}\n  /@babel/core/7.1.0_x@1.0.0:\n    dev: true\n";
        assert_eq!(
            deps("pnpm-lock.yaml", v5),
            [
                t("npm", "react", "17.0.2", 3),
                t("npm", "@babel/core", "7.1.0", 5)
            ]
        );
    }

    #[test]
    fn yarn_v1_and_berry() {
        let v1 = "# yarn lockfile v1\n\n\"@babel/core@^7.0.0\", \"@babel/core@^7.1.0\":\n  version \"7.26.0\"\n  resolved \"https://x\"\n\nlodash@^4.17.0:\n  version \"4.17.21\"\n";
        assert_eq!(
            deps("yarn.lock", v1),
            [
                t("npm", "@babel/core", "7.26.0", 3),
                t("npm", "lodash", "4.17.21", 7)
            ]
        );
        let berry = "__metadata:\n  version: 6\n\n\"lodash@npm:^4.17.0\":\n  version: 4.17.21\n\n\"app@workspace:.\":\n  version: 0.0.0-use.local\n";
        assert_eq!(deps("yarn.lock", berry), [t("npm", "lodash", "4.17.21", 4)]);
    }

    #[test]
    fn python_and_go() {
        let req = "# ghi chú\nfastapi==0.115.0\nUvicorn[standard] == 0.30.1 ; python_version > '3.8'\nrequests>=2.0\n-r base.txt\n";
        assert_eq!(
            deps("requirements-dev.txt", req),
            [
                t("PyPI", "fastapi", "0.115.0", 2),
                t("PyPI", "uvicorn", "0.30.1", 3)
            ]
        );
        let poetry = "[[package]]\nname = \"jinja2\"\nversion = \"3.1.2\"\n\n[package.dependencies]\nmarkupsafe = \">=2.0\"\n\n[[package]]\nname = \"mylib\"\nversion = \"0.1.0\"\n\n[package.source]\ntype = \"git\"\nurl = \"https://x\"\n";
        assert_eq!(
            deps("poetry.lock", poetry),
            [t("PyPI", "jinja2", "3.1.2", 1)]
        );
        let uv = "version = 1\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\nsource = { editable = \".\" }\n\n[[package]]\nname = \"httpx\"\nversion = \"0.27.0\"\nsource = { registry = \"https://pypi.org/simple\" }\n";
        assert_eq!(deps("uv.lock", uv), [t("PyPI", "httpx", "0.27.0", 8)]);
        let gosum = "github.com/gin-gonic/gin v1.9.0 h1:abc=\ngithub.com/gin-gonic/gin v1.9.0/go.mod h1:def=\n";
        assert_eq!(
            deps("go.sum", gosum),
            [t("Go", "github.com/gin-gonic/gin", "1.9.0", 1)]
        );
    }

    #[test]
    fn lockfile_names() {
        assert_eq!(kind("a/requirements.txt"), Some(Kind::Requirements));
        assert_eq!(kind("requirements-prod.txt"), Some(Kind::Requirements));
        assert_eq!(kind("src/Cargo.toml"), None);
    }
}
