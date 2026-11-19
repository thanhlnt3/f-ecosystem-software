//! `feco-codescan`: quét mã nguồn trước khi duyệt app và trước khi tích hợp app bên ngoài
//! (spec source-scan; design S1).
//!
//! - **Bí mật lộ** ([`rules`]): khóa riêng PEM, khóa AWS, token GitHub/GitLab/Slack/Stripe, tệp
//!   `.env` có giá trị, gán `password`/`secret`/`api_key` bằng chuỗi dài. Mức `critical`; trong
//!   thư mục kiểm thử (`test`, `tests`, `fixtures`, `examples`...) hạ xuống `medium`. Chuỗi mẫu
//!   rõ ràng (`EXAMPLE`, `xxxx`, `changeme`, `<...>`) không tính. Trích đoạn chỉ giữ 4 ký tự đầu.
//! - **Mã đáng ngờ** ([`rules`]): đào tiền ảo, đọc kho mật khẩu/cookie trình duyệt rồi gửi mạng,
//!   tắt Defender, móc bàn phím (`critical`); tải rồi chạy ngay, khối base64 lớn được giải mã rồi
//!   chạy, webhook Discord (`high`).
//! - **Thư viện có lỗ hổng** ([`lockfiles`], [`osv`]): đọc lockfile thành (hệ sinh thái, tên,
//!   phiên bản) rồi tra OSV.dev theo lô. Chỉ ba giá trị này được gửi ra ngoài.
//! - **Giấy phép** ([`license`]): giấy phép trong `LICENSE*` và manifest gói so với giấy phép
//!   khai trong `f-app.toml`.
//!
//! [`scan_dir`] chạy đồng bộ (không dùng mạng); [`check_vulnerabilities`] tra OSV; [`scan`] làm cả
//! hai cho một thư mục hoặc tệp zip rồi áp [`AllowList`].

pub mod allow;
pub mod archive;
pub mod license;
pub mod lockfiles;
pub mod markdown;
pub mod osv;
pub mod rules;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub use allow::AllowList;
pub use archive::{download_github_zip, FetchError};
pub use osv::{check_vulnerabilities, OsvClient};

/// Tổng dung lượng nội dung được quét (200 MB).
pub const MAX_BYTES: u64 = 200 * 1024 * 1024;
/// Số tệp tối đa được quét, và số mục tối đa trong tệp zip.
pub const MAX_FILES: usize = 50_000;
/// Tệp lớn hơn 2 MB không được quét nội dung.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Lockfile được đọc tới 64 MB (lockfile của dự án lớn thường vượt 2 MB).
pub const MAX_LOCKFILE_BYTES: u64 = 64 * 1024 * 1024;
/// Thư mục không quét: mã của bên thứ ba hoặc sản phẩm dựng.
pub const SKIP_DIRS: [&str; 8] = [
    ".git",
    "node_modules",
    "target",
    "vendor",
    "dist",
    "build",
    ".venv",
    "__pycache__",
];

/// Mức nghiêm trọng, từ thấp tới cao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub const ALL: [Severity; 4] = [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Info,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Option<Severity> {
        Severity::ALL.into_iter().find(|v| v.as_str() == s.trim())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Secret,
    Vulnerability,
    Suspicious,
    License,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Secret => "secret",
            Category::Vulnerability => "vulnerability",
            Category::Suspicious => "suspicious",
            Category::License => "license",
        }
    }
}

/// Một phát hiện. `excerpt` (nếu có) đã được che, chỉ giữ 4 ký tự đầu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub rule: String,
    pub category: Category,
    pub severity: Severity,
    /// Đường dẫn tương đối, ngăn cách bằng `/`.
    pub path: String,
    /// Dòng (từ 1); 0 khi không gắn với dòng nào.
    pub line: u32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

/// Phát hiện đã được xem xét và cho phép (mục trong `scan-allow.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suppressed {
    pub finding: Finding,
    pub reason: String,
}

/// Một thư viện đọc từ lockfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// Hệ sinh thái OSV: `crates.io`, `npm`, `PyPI`, `Go`.
    pub ecosystem: String,
    pub name: String,
    pub version: String,
    /// Lockfile chứa thư viện.
    pub path: String,
    pub line: u32,
}

/// Giấy phép nhận dạng được trong mã nguồn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedLicense {
    pub path: String,
    /// Mã SPDX (hoặc biểu thức SPDX khi đọc từ manifest gói).
    pub spdx: String,
}

/// Kết quả tra OSV.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "reason", rename_all = "lowercase")]
pub enum OsvStatus {
    Checked,
    #[default]
    Skipped,
    Failed(String),
}

impl OsvStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OsvStatus::Checked => "checked",
            OsvStatus::Skipped => "skipped",
            OsvStatus::Failed(_) => "failed",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub files_scanned: u64,
    pub bytes_scanned: u64,
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub suppressed: Vec<Suppressed>,
    pub dependencies: Vec<Dependency>,
    pub licenses: Vec<DetectedLicense>,
    pub osv: OsvStatus,
    /// Dừng sớm vì vượt giới hạn số tệp hoặc dung lượng.
    pub truncated: bool,
    /// Tệp bỏ qua khi quét nội dung vì lớn hơn 2 MB.
    #[serde(default)]
    pub skipped_large: u64,
}

impl Report {
    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Số phát hiện theo mức: `{"critical": 0, "high": 1, "medium": 0, "info": 0}`.
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        Severity::ALL
            .into_iter()
            .map(|s| (s.as_str(), self.count(s)))
            .collect()
    }

    /// Có phát hiện (chưa được cho phép) từ mức `threshold` trở lên.
    pub fn exceeds(&self, threshold: Severity) -> bool {
        self.findings.iter().any(|f| f.severity >= threshold)
    }

    /// Sắp xếp: mức cao trước, rồi theo tệp và dòng.
    pub fn sort(&mut self) {
        self.findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.line.cmp(&b.line))
                .then_with(|| a.rule.cmp(&b.rule))
        });
    }
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Tra lỗ hổng thư viện; `None` thì `osv` là `skipped`.
    pub osv: Option<OsvClient>,
    pub max_bytes: u64,
    pub max_files: usize,
    pub allow: AllowList,
    /// Giấy phép khai trong `f-app.toml` (để phát hiện `license.mismatch`).
    pub manifest_license: Option<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            osv: None,
            max_bytes: MAX_BYTES,
            max_files: MAX_FILES,
            allow: AllowList::default(),
            manifest_license: None,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScanError {
    #[error("không đọc được {0}")]
    Io(String),
    #[error("không phải tệp zip hợp lệ: {0}")]
    NotZip(String),
    #[error("tệp zip có đường dẫn không an toàn: {0}")]
    UnsafePath(String),
    #[error("tệp zip có quá nhiều mục ({0}, tối đa {MAX_FILES})")]
    TooManyFiles(usize),
    #[error("tệp zip giải nén ra quá lớn (tối đa {} MB)", archive::MAX_EXTRACTED_BYTES / 1024 / 1024)]
    TooLarge,
}

/// Che bí mật: giữ 4 ký tự đầu.
pub fn mask(secret: &str) -> String {
    let head: String = secret.chars().take(4).collect();
    format!("{head}****")
}

/// Đường dẫn tương đối dạng `a/b/c`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0)
}

/// Quét một thư mục (đồng bộ, không dùng mạng): bí mật, mã đáng ngờ, giấy phép, đọc lockfile.
/// Phát hiện khớp `opts.allow` được chuyển sang `suppressed`.
pub fn scan_dir(root: &Path, opts: &ScanOptions) -> Report {
    let mut report = Report::default();
    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !(e.file_type().is_dir()
                    && SKIP_DIRS.contains(&e.file_name().to_string_lossy().as_ref()))
        });
    for entry in walker {
        let Ok(entry) = entry else { continue };
        // Symlink không được theo (có thể trỏ ra ngoài thư mục quét).
        if !entry.file_type().is_file() {
            continue;
        }
        if report.files_scanned as usize >= opts.max_files {
            report.truncated = true;
            break;
        }
        let rel = relative(root, entry.path());
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        report.files_scanned += 1;
        let lockfile = lockfiles::kind(&rel);
        let limit = if lockfile.is_some() {
            MAX_LOCKFILE_BYTES
        } else {
            MAX_FILE_BYTES
        };
        if size > limit {
            report.skipped_large += 1;
            continue;
        }
        if report.bytes_scanned + size > opts.max_bytes {
            report.truncated = true;
            break;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        report.bytes_scanned += bytes.len() as u64;
        if looks_binary(&bytes) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        if let Some(kind) = lockfile {
            report
                .dependencies
                .extend(lockfiles::parse(kind, &rel, &text));
            continue;
        }
        if entry.depth() == 1 {
            report.licenses.extend(license::detect(&rel, &text));
        }
        rules::scan_text(&rel, &text, &mut report.findings);
    }
    if let Some(declared) = &opts.manifest_license {
        if let Some(f) = license::mismatch(declared, &report.licenses) {
            report.findings.push(f);
        }
    }
    report.dependencies.sort_by(|a, b| {
        (&a.ecosystem, &a.name, &a.version, &a.path).cmp(&(
            &b.ecosystem,
            &b.name,
            &b.version,
            &b.path,
        ))
    });
    report.dependencies.dedup_by(|a, b| {
        a.ecosystem == b.ecosystem && a.name == b.name && a.version == b.version && a.path == b.path
    });
    opts.allow.apply(&mut report);
    report.sort();
    report
}

/// Gốc thật của mã nguồn trong thư mục giải nén: zip của GitHub (codeload) bọc mọi thứ trong
/// một thư mục `<repo>-<commit 40 hex>/`. Zip khác giữ nguyên đường dẫn.
fn source_root(dir: &Path) -> PathBuf {
    let entries: Vec<_> = std::fs::read_dir(dir)
        .map(|r| r.flatten().collect())
        .unwrap_or_default();
    let github_wrapper = |name: &str| {
        name.rsplit_once('-')
            .is_some_and(|(repo, commit)| !repo.is_empty() && archive::valid_commit(commit))
    };
    match entries.as_slice() {
        [only]
            if only.file_type().is_ok_and(|t| t.is_dir())
                && github_wrapper(&only.file_name().to_string_lossy()) =>
        {
            only.path()
        }
        _ => dir.to_path_buf(),
    }
}

/// Giải nén an toàn vào thư mục tạm rồi [`scan_dir`]. Zip có đường dẫn thoát ra ngoài, quá
/// nhiều mục hay giải nén ra quá lớn bị từ chối; symlink bị bỏ qua.
pub fn scan_zip(zip: &Path, opts: &ScanOptions) -> Result<Report, ScanError> {
    let temp = tempfile::Builder::new()
        .prefix("feco-codescan-")
        .tempdir()
        .map_err(|e| ScanError::Io(e.to_string()))?;
    archive::extract(zip, temp.path(), opts.max_files)?;
    Ok(scan_dir(&source_root(temp.path()), opts))
}

/// Quét thư mục hoặc tệp zip, tra OSV nếu có `opts.osv`, rồi áp danh sách cho phép.
pub async fn scan(target: &Path, opts: &ScanOptions) -> Result<Report, ScanError> {
    let (target_owned, opts_owned) = (target.to_path_buf(), opts.clone());
    let mut report = tokio::task::spawn_blocking(move || {
        if target_owned.is_dir() {
            Ok(scan_dir(&target_owned, &opts_owned))
        } else {
            scan_zip(&target_owned, &opts_owned)
        }
    })
    .await
    .map_err(|e| ScanError::Io(e.to_string()))??;
    if let Some(osv) = &opts.osv {
        check_vulnerabilities(&mut report, osv).await;
        opts.allow.apply(&mut report);
        report.sort();
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn write(root: &Path, files: &[(&str, &str)]) {
        for (name, text) in files {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    #[test]
    fn mask_keeps_four_chars() {
        assert_eq!(mask("ghp_abcdefgh"), "ghp_****");
        assert_eq!(mask("ab"), "ab****");
    }

    #[test]
    fn skips_vendor_dirs_and_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let miner = "pool = \"stratum+tcp://pool.example:3333\"\n";
        write(
            dir.path(),
            &[
                ("node_modules/x/index.js", miner),
                (".git/config", miner),
                ("src/ok.rs", "fn main() {}\n"),
            ],
        );
        let big = "a".repeat(MAX_FILE_BYTES as usize + 10) + "\nstratum+tcp://x:1\n";
        std::fs::write(dir.path().join("src/big.txt"), big).unwrap();
        let r = scan_dir(dir.path(), &ScanOptions::default());
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        assert_eq!(r.skipped_large, 1);
        assert_eq!(r.files_scanned, 2);
    }

    /// Chỉ thư mục bọc của GitHub (`<repo>-<commit>`) bị bỏ khỏi đường dẫn.
    #[test]
    fn zip_paths_keep_user_folders() {
        use std::io::Write;
        let work = tempfile::tempdir().unwrap();
        let make = |name: &str, entry: &str| {
            let path = work.path().join(name);
            let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            z.start_file(entry, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"x = 'stratum+tcp://pool.example:1'").unwrap();
            z.finish().unwrap();
            path
        };
        let opts = ScanOptions::default();
        let own = make("own.zip", "src/miner.py");
        assert_eq!(
            scan_zip(&own, &opts).unwrap().findings[0].path,
            "src/miner.py"
        );
        let gh = make(
            "gh.zip",
            "app-0123456789abcdef0123456789abcdef01234567/src/miner.py",
        );
        assert_eq!(
            scan_zip(&gh, &opts).unwrap().findings[0].path,
            "src/miner.py"
        );
    }

    #[test]
    fn file_limit_truncates() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), "x").unwrap();
        }
        let opts = ScanOptions {
            max_files: 3,
            ..ScanOptions::default()
        };
        let r = scan_dir(dir.path(), &opts);
        assert!(r.truncated);
        assert_eq!(r.files_scanned, 3);
    }

    #[test]
    fn counts_and_threshold() {
        let mut r = Report::default();
        r.findings.push(Finding {
            rule: "x".into(),
            category: Category::Secret,
            severity: Severity::High,
            path: "a".into(),
            line: 1,
            message: String::new(),
            excerpt: None,
        });
        assert_eq!(r.counts()["high"], 1);
        assert_eq!(r.counts()["critical"], 0);
        assert!(r.exceeds(Severity::High));
        assert!(!r.exceeds(Severity::Critical));
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["osv"]["status"], "skipped");
        assert_eq!(json["findings"][0]["severity"], "high");
    }
}
