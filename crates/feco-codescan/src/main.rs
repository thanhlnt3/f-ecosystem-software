//! Lệnh `feco-codescan` (design S1):
//!
//! ```text
//! feco-codescan <thư-mục|tệp.zip|github:owner/repo@commit> [--allow scan-allow.toml]
//!     [--json out.json] [--markdown out.md] [--no-osv] [--fail-on critical|high] [--license SPDX]
//! ```
//!
//! Mã thoát: 0 khi không có phát hiện từ ngưỡng `--fail-on` (mặc định `critical`) trở lên, 2 khi
//! có, 1 khi lỗi. Đây là nơi duy nhất gửi dữ liệu ra ngoài: tải zip của GitHub (chỉ gửi tên repo
//! và commit) và tra OSV.dev (chỉ gửi hệ sinh thái, tên và phiên bản thư viện).
//!
//! Biến môi trường: `FECO_OSV_URL` (mặc định <https://api.osv.dev>), `FECO_OSV=off`,
//! `FECO_CODESCAN_GITHUB_URL` (mặc định <https://codeload.github.com>, đổi được để kiểm thử).

use std::{path::PathBuf, process::ExitCode};

use feco_codescan::{markdown, AllowList, OsvClient, ScanOptions, Severity};

const USAGE: &str = "\
feco-codescan: quét mã nguồn (bí mật lộ, thư viện có lỗ hổng, mã đáng ngờ, giấy phép)

  feco-codescan <thư-mục|tệp.zip|github:owner/repo@commit> [tùy chọn]

  --allow FILE       danh sách phát hiện đã xem xét (scan-allow.toml)
  --json FILE        ghi báo cáo JSON
  --markdown FILE    ghi báo cáo Markdown
  --no-osv           không tra lỗ hổng thư viện trên OSV.dev
  --fail-on MỨC      critical (mặc định) hoặc high: thoát mã 2 khi có phát hiện từ mức này
  --license SPDX     giấy phép khai trong f-app.toml (phát hiện license.mismatch)

Mã thoát: 0 đạt, 2 có phát hiện vượt ngưỡng, 1 lỗi.
";

/// Tải zip tối đa 1 GB.
const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;

struct Args {
    target: String,
    allow: Option<PathBuf>,
    json: Option<PathBuf>,
    markdown: Option<PathBuf>,
    osv: bool,
    fail_on: Severity,
    license: Option<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        target: String::new(),
        allow: None,
        json: None,
        markdown: None,
        osv: true,
        fail_on: Severity::Critical,
        license: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} cần một giá trị"))
        };
        match a.as_str() {
            "--allow" => out.allow = Some(value("--allow")?.into()),
            "--json" => out.json = Some(value("--json")?.into()),
            "--markdown" => out.markdown = Some(value("--markdown")?.into()),
            "--no-osv" => out.osv = false,
            "--license" => out.license = Some(value("--license")?),
            "--fail-on" => {
                let v = value("--fail-on")?;
                out.fail_on = match v.as_str() {
                    "critical" => Severity::Critical,
                    "high" => Severity::High,
                    _ => {
                        return Err(format!(
                            "--fail-on phải là critical hoặc high, không phải {v:?}"
                        ))
                    }
                }
            }
            "-h" | "--help" => return Err(USAGE.to_owned()),
            s if s.starts_with("--") => return Err(format!("tùy chọn lạ: {s}\n\n{USAGE}")),
            s if out.target.is_empty() => out.target = s.to_owned(),
            s => return Err(format!("thừa tham số: {s}\n\n{USAGE}")),
        }
    }
    if out.target.is_empty() {
        return Err(USAGE.to_owned());
    }
    Ok(out)
}

/// `github:owner/repo@commit`.
fn github_target(target: &str) -> Option<Result<(String, String, String), String>> {
    let rest = target.strip_prefix("github:")?;
    let parsed = (|| {
        let (repo, commit) = rest.split_once('@')?;
        let (owner, name) = repo.split_once('/')?;
        Some((owner.to_owned(), name.to_owned(), commit.to_owned()))
    })();
    Some(match parsed {
        Some((o, r, c))
            if feco_codescan::archive::valid_github_name(&o)
                && feco_codescan::archive::valid_github_name(&r)
                && feco_codescan::archive::valid_commit(&c) =>
        {
            Ok((o, r, c))
        }
        _ => Err(format!(
            "{target:?}: dạng đúng là github:owner/repo@<commit 40 ký tự hex>"
        )),
    })
}

async fn run(args: Args) -> Result<bool, String> {
    let allow = match &args.allow {
        Some(p) => AllowList::load(p)?,
        None => AllowList::default(),
    };
    let osv = if args.osv {
        OsvClient::from_env()
    } else {
        None
    };
    let opts = ScanOptions {
        osv,
        allow: allow.clone(),
        manifest_license: args.license.clone(),
        ..ScanOptions::default()
    };
    // Giữ thư mục tạm tới khi quét xong.
    let mut _download: Option<tempfile::TempDir> = None;
    let path = match github_target(&args.target) {
        Some(parsed) => {
            let (owner, repo, commit) = parsed?;
            let base = std::env::var("FECO_CODESCAN_GITHUB_URL")
                .unwrap_or_else(|_| "https://codeload.github.com".into());
            let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
            let zip = dir.path().join("source.zip");
            let http = reqwest::Client::builder()
                .user_agent(concat!("feco-codescan/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|e| e.to_string())?;
            eprintln!("tải {owner}/{repo}@{commit} ...");
            let size = feco_codescan::download_github_zip(
                &http,
                &base,
                &owner,
                &repo,
                &commit,
                &zip,
                MAX_DOWNLOAD,
            )
            .await
            .map_err(|e| format!("không tải được {owner}/{repo}@{commit}: {e}"))?;
            eprintln!("đã tải {} KB", size / 1024);
            _download = Some(dir);
            zip
        }
        None => PathBuf::from(&args.target),
    };
    if !path.exists() {
        return Err(format!("không có {}", path.display()));
    }
    let report = feco_codescan::scan(&path, &opts)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(p) = &args.json {
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(p, json + "\n").map_err(|e| format!("{}: {e}", p.display()))?;
    }
    if let Some(p) = &args.markdown {
        std::fs::write(p, markdown::render(&report, &args.target))
            .map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let counts = report.counts();
    println!(
        "{}: {} tệp, {} thư viện, OSV {}; critical {}, high {}, medium {}, info {}; đã cho phép {}",
        args.target,
        report.files_scanned,
        report.dependencies.len(),
        report.osv.as_str(),
        counts["critical"],
        counts["high"],
        counts["medium"],
        counts["info"],
        report.suppressed.len()
    );
    if let feco_codescan::OsvStatus::Failed(reason) = &report.osv {
        println!("cảnh báo: không tra được OSV: {reason}");
    }
    for f in report
        .findings
        .iter()
        .filter(|f| f.severity >= Severity::High)
    {
        println!(
            "  {} {} {}:{} {}{}",
            f.severity.as_str(),
            f.rule,
            f.path,
            f.line,
            f.message,
            f.excerpt
                .as_deref()
                .map(|e| format!(" ({e})"))
                .unwrap_or_default()
        );
    }
    for label in allow.unused(&report) {
        println!("cảnh báo: mục cho phép {label} không khớp phát hiện nào");
    }
    Ok(report.exceeds(args.fail_on))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let fail_on = args.fail_on;
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    match runtime.block_on(run(args)) {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => {
            eprintln!(
                "có phát hiện từ mức {} trở lên chưa được cho phép",
                fail_on.as_str()
            );
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("lỗi: {e}");
            ExitCode::from(1)
        }
    }
}
