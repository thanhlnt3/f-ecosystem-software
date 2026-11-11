//! Kiểm tra tự động một gói (spec app-review, "Kiểm tra tự động"; design D6).
//!
//! - Quét mã độc bằng nhiều engine ([`Engine`]): engine chữ ký của F-Eco (mẫu thử EICAR và cơ sở
//!   dữ liệu SHA-256 trong bảng `malware_signatures`) và ClamAV qua giao thức `clamd` (INSTREAM).
//!   Production cấu hình thêm dịch vụ quét thương mại (xem QUESTIONS.md).
//! - So manifest với gói: tệp chạy phải có, kiến trúc phải khớp ([`executable_archs`]).
//! - Phát hiện tệp chạy đã bị nén hoặc làm rối bằng các packer thường gặp ([`packer`]).
//!
//! Đây là phân tích tĩnh: không chạy tệp nào. Khâu chạy thử trong máy ảo dùng một lần thuộc bảng
//! quản trị (nhóm 14) và cần hạ tầng (task 1.4).

use std::{net::SocketAddr, path::Path, time::Duration};

use sha2::Digest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Tệp lớn nhất trong gói được đọc để quét.
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Tổng dung lượng giải nén tối đa của một gói (chống "zip bomb").
const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const CLAMD_TIMEOUT: Duration = Duration::from_secs(120);

/// Mẫu thử chống virus EICAR, ghép lúc chạy để chính mã nguồn và tệp build không chứa nguyên
/// chuỗi (tránh bị phần mềm diệt virus trên máy dev cách ly).
pub fn eicar() -> Vec<u8> {
    [
        "X5O!P%@AP[4\\PZX54(P^)7CC)7}$",
        "EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*",
    ]
    .concat()
    .into_bytes()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug, Clone)]
pub enum Engine {
    /// Engine chữ ký của F-Eco: (SHA-256, tên mã độc).
    Signatures(Vec<(String, String)>),
    /// ClamAV (`clamd`) qua TCP.
    Clamd(SocketAddr),
}

impl Engine {
    pub fn name(&self) -> &'static str {
        match self {
            Engine::Signatures(_) => "feco-signatures",
            Engine::Clamd(_) => "clamav",
        }
    }

    /// `Ok(Some(tên))` nếu phát hiện mã độc.
    pub async fn scan(&self, bytes: &[u8]) -> Result<Option<String>, String> {
        match self {
            Engine::Signatures(db) => {
                let pattern = eicar();
                if bytes
                    .windows(pattern.len())
                    .any(|w| w == pattern.as_slice())
                {
                    return Ok(Some("EICAR-Test-File".into()));
                }
                let sha = sha256_hex(bytes);
                Ok(db.iter().find(|(h, _)| *h == sha).map(|(_, n)| n.clone()))
            }
            Engine::Clamd(addr) => tokio::time::timeout(CLAMD_TIMEOUT, clamd_scan(*addr, bytes))
                .await
                .map_err(|_| "clamd không trả lời kịp".to_owned())?,
        }
    }
}

/// Giao thức INSTREAM của clamd: `zINSTREAM\0`, các khối có tiền tố độ dài 4 byte, khối rỗng để
/// kết thúc; trả lời `stream: OK` hoặc `stream: <tên> FOUND`.
async fn clamd_scan(addr: SocketAddr, bytes: &[u8]) -> Result<Option<String>, String> {
    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .map_err(|e| format!("không kết nối được clamd {addr}: {e}"))?;
    let io = |e: std::io::Error| e.to_string();
    stream.write_all(b"zINSTREAM\0").await.map_err(io)?;
    for chunk in bytes.chunks(64 * 1024) {
        stream
            .write_all(&(chunk.len() as u32).to_be_bytes())
            .await
            .map_err(io)?;
        stream.write_all(chunk).await.map_err(io)?;
    }
    stream.write_all(&0u32.to_be_bytes()).await.map_err(io)?;
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).await.map_err(io)?;
    let reply = String::from_utf8_lossy(&reply);
    let reply = reply.trim_end_matches(['\0', '\n']).trim();
    let body = reply
        .strip_prefix("stream:")
        .map(str::trim)
        .unwrap_or(reply);
    if body == "OK" {
        Ok(None)
    } else if let Some(name) = body.strip_suffix("FOUND") {
        Ok(Some(name.trim().to_owned()))
    } else {
        Err(format!("clamd trả lời lạ: {reply}"))
    }
}

/// Kiến trúc của tệp chạy PE hoặc Mach-O (`x64`, `arm64`, `x86`); `None` nếu không phải tệp chạy.
pub fn executable_archs(bytes: &[u8]) -> Option<Vec<&'static str>> {
    let u16le = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let u32le = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let u32be = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let macho_cpu = |cpu: u32| match cpu {
        0x0100_0007 => "x64",
        0x0100_000c => "arm64",
        7 => "x86",
        _ => "khác",
    };
    if bytes.get(..2) == Some(b"MZ") {
        let nt = u32le(0x3c)? as usize;
        if bytes.get(nt..nt + 4)? != b"PE\0\0" {
            return None;
        }
        return Some(vec![match u16le(nt + 4)? {
            0x8664 => "x64",
            0xaa64 => "arm64",
            0x014c => "x86",
            _ => "khác",
        }]);
    }
    match u32le(0)? {
        0xfeed_facf | 0xfeed_face => return Some(vec![macho_cpu(u32le(4)?)]),
        _ => {}
    }
    if u32be(0)? == 0xcafe_babe {
        let count = u32be(4)? as usize;
        if count == 0 || count > 16 {
            return None;
        }
        return (0..count)
            .map(|i| u32be(8 + i * 20).map(macho_cpu))
            .collect();
    }
    None
}

/// Tên packer nếu tệp chạy đã bị nén hoặc làm rối (theo tên section PE hoặc dấu hiệu UPX).
pub fn packer(bytes: &[u8]) -> Option<&'static str> {
    const SECTIONS: [(&[u8], &str); 12] = [
        (b"UPX0", "UPX"),
        (b"UPX1", "UPX"),
        (b".aspack", "ASPack"),
        (b".adata", "ASPack"),
        (b"MPRESS1", "MPRESS"),
        (b"MPRESS2", "MPRESS"),
        (b".petite", "Petite"),
        (b"PEC2", "PECompact"),
        (b".nsp0", "NsPack"),
        (b".themida", "Themida"),
        (b".vmp0", "VMProtect"),
        (b".vmp1", "VMProtect"),
    ];
    executable_archs(bytes)?;
    if bytes.get(..2) == Some(b"MZ") {
        let u16le = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
        let nt = u32::from_le_bytes(bytes.get(0x3c..0x40)?.try_into().ok()?) as usize;
        let count = u16le(nt + 6)? as usize;
        let optional_size = u16le(nt + 20)? as usize;
        let table = nt + 24 + optional_size;
        for i in 0..count.min(96) {
            let name = bytes.get(table + i * 40..table + i * 40 + 8)?;
            let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(8)];
            if let Some((_, p)) = SECTIONS.iter().find(|(s, _)| name.eq_ignore_ascii_case(s)) {
                return Some(p);
            }
        }
    }
    let head = &bytes[..bytes.len().min(4096)];
    head.windows(4).any(|w| w == b"UPX!").then_some("UPX")
}

/// Kết quả kiểm tra một gói.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Report {
    pub platform: String,
    pub files: usize,
    /// (engine, tên mã độc, tệp).
    pub malware: Option<(String, String, String)>,
    /// Lỗi kỹ thuật khiến gói bị từ chối, mỗi lỗi chỉ rõ chỗ sai.
    pub problems: Vec<String>,
    /// Engine không chạy được (gói chưa thể kết luận là sạch).
    pub engine_errors: Vec<String>,
}

/// Đọc lần lượt từng tệp trong gói (không giữ cả gói trong bộ nhớ).
async fn entry_at(path: &Path, index: usize) -> Result<Option<(String, Vec<u8>, bool)>, String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("gói không phải zip: {e}"))?;
        if index >= zip.len() {
            return Ok(None);
        }
        let item = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = item.name().map(|n| n.into_owned()).unwrap_or_default();
        if item.is_dir() {
            return Ok(Some((name, Vec::new(), true)));
        }
        let too_big = item.size() > MAX_FILE_BYTES;
        let mut bytes = Vec::new();
        item.take(MAX_FILE_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok(Some((name, bytes, too_big)))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Kiểm tra một gói. `entry`/`arch` là tệp chạy và kiến trúc manifest khai báo; `full` thì kiểm
/// cả manifest và packer (lần nộp), không thì chỉ quét mã độc (quét lại định kỳ).
pub async fn check_package(
    path: &Path,
    platform: &str,
    entry: &str,
    arch: &str,
    engines: &[Engine],
    full: bool,
) -> Report {
    let mut report = Report {
        platform: platform.to_owned(),
        ..Default::default()
    };
    let mut total = 0u64;
    let mut found_entry = false;
    let mut index = 0;
    loop {
        let (name, bytes, too_big) = match entry_at(path, index).await {
            Ok(Some(x)) => x,
            Ok(None) => break,
            Err(e) => {
                report
                    .problems
                    .push(format!("Không đọc được gói {platform}: {e}"));
                return report;
            }
        };
        index += 1;
        if too_big {
            report
                .problems
                .push(format!("{name} lớn hơn 1 GB, không quét được"));
            continue;
        }
        total += bytes.len() as u64;
        if total > MAX_TOTAL_BYTES {
            report
                .problems
                .push(format!("Gói {platform} giải nén ra quá 8 GB"));
            break;
        }
        if bytes.is_empty() {
            continue;
        }
        report.files += 1;
        if full {
            let archs = executable_archs(&bytes);
            if name == entry {
                found_entry = true;
                match &archs {
                    Some(a) if a.contains(&arch) => {}
                    Some(a) => report.problems.push(format!(
                        "Kiến trúc không khớp: f-app.toml khai báo {platform} nhưng {name} là {}",
                        a.join(", ")
                    )),
                    None => report
                        .problems
                        .push(format!("{name} không phải tệp chạy của {platform}")),
                }
            }
            if let Some(p) = packer(&bytes) {
                report.problems.push(format!(
                    "{name} đã bị nén hoặc làm rối bằng {p}; F-Eco không nhận tệp chạy bị làm rối"
                ));
            }
        }
        for engine in engines {
            match engine.scan(&bytes).await {
                Ok(Some(malware)) => {
                    report.malware = Some((engine.name().to_owned(), malware, name.clone()));
                    return report;
                }
                Ok(None) => {}
                Err(e) => report.engine_errors.push(format!("{}: {e}", engine.name())),
            }
        }
    }
    if full && !found_entry {
        report
            .problems
            .push(format!("Gói {platform} không có tệp chạy {entry}"));
    }
    report
}

/// Máy chủ clamd giả cho test: báo `Eicar-Test-Signature FOUND` nếu luồng chứa mẫu EICAR.
pub mod testing {
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub async fn serve_clamd(addr: &str) -> std::io::Result<SocketAddr> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let local = listener.local_addr()?;
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut command = [0u8; 10];
                    if socket.read_exact(&mut command).await.is_err() || &command != b"zINSTREAM\0"
                    {
                        let _ = socket.write_all(b"UNKNOWN COMMAND\0").await;
                        return;
                    }
                    let mut data = Vec::new();
                    loop {
                        let mut len = [0u8; 4];
                        if socket.read_exact(&mut len).await.is_err() {
                            return;
                        }
                        let len = u32::from_be_bytes(len) as usize;
                        if len == 0 {
                            break;
                        }
                        let start = data.len();
                        data.resize(start + len, 0);
                        if socket.read_exact(&mut data[start..]).await.is_err() {
                            return;
                        }
                    }
                    let pattern = super::eicar();
                    let infected = data.windows(pattern.len()).any(|w| w == pattern.as_slice());
                    let reply: &[u8] = if infected {
                        b"stream: Eicar-Test-Signature FOUND\0"
                    } else {
                        b"stream: OK\0"
                    };
                    let _ = socket.write_all(reply).await;
                });
            }
        });
        Ok(local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codesign::fixtures;
    use std::io::Write;

    fn zip_with(files: &[(&str, Vec<u8>)]) -> tempfile::NamedTempFile {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut zip = zip::ZipWriter::new(tmp.reopen().unwrap());
        for (name, data) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        tmp
    }

    #[test]
    fn architectures_and_packers() {
        assert_eq!(executable_archs(&fixtures::pe(true)), Some(vec!["x64"]));
        assert_eq!(
            executable_archs(&fixtures::pe_for(0xaa64, true, &[])),
            Some(vec!["arm64"])
        );
        assert_eq!(
            executable_archs(&fixtures::macho(Some(4096))),
            Some(vec!["arm64"])
        );
        assert_eq!(executable_archs(b"#!/bin/sh"), None);
        assert_eq!(packer(&fixtures::pe(true)), None);
        assert_eq!(
            packer(&fixtures::pe_for(0x8664, true, &["UPX0", "UPX1"])),
            Some("UPX")
        );
        assert_eq!(
            packer(&fixtures::pe_for(0x8664, true, &[".text", ".vmp0"])),
            Some("VMProtect")
        );
        assert_eq!(
            packer(&fixtures::pe_for(0x8664, true, &[".text", ".rdata"])),
            None
        );
    }

    #[tokio::test]
    async fn engines_detect_eicar_and_known_hashes() {
        let clamd = testing::serve_clamd("127.0.0.1:0").await.unwrap();
        let bad = b"day la ma doc gia".to_vec();
        let engines = [
            Engine::Signatures(vec![(sha256_hex(&bad), "Trojan.Test".into())]),
            Engine::Clamd(clamd),
        ];
        for engine in &engines {
            assert_eq!(
                engine.scan(b"sach").await.unwrap(),
                None,
                "{}",
                engine.name()
            );
            assert!(
                engine.scan(&eicar()).await.unwrap().is_some(),
                "{}",
                engine.name()
            );
        }
        assert_eq!(
            engines[0].scan(&bad).await.unwrap().as_deref(),
            Some("Trojan.Test")
        );
        // clamd không chạy: lỗi engine, không coi là sạch.
        let dead = Engine::Clamd("127.0.0.1:9".parse().unwrap());
        assert!(dead.scan(b"x").await.is_err());
    }

    #[tokio::test]
    async fn package_reports() {
        let clamd = testing::serve_clamd("127.0.0.1:0").await.unwrap();
        let engines = [Engine::Signatures(Vec::new()), Engine::Clamd(clamd)];
        let check = |files: Vec<(&'static str, Vec<u8>)>| {
            let engines = engines.clone();
            async move {
                let zip = zip_with(&files);
                check_package(
                    zip.path(),
                    "windows-x64",
                    "SoundLab.exe",
                    "x64",
                    &engines,
                    true,
                )
                .await
            }
        };
        let clean = check(vec![
            ("SoundLab.exe", fixtures::pe(true)),
            ("README.txt", b"hi".to_vec()),
        ])
        .await;
        assert!(
            clean.malware.is_none() && clean.problems.is_empty(),
            "{clean:?}"
        );
        assert_eq!(clean.files, 2);

        let infected = check(vec![
            ("SoundLab.exe", fixtures::pe(true)),
            ("data/eicar.com", eicar()),
        ])
        .await;
        let (engine, name, file) = infected.malware.expect("phát hiện EICAR");
        assert_eq!(
            (engine.as_str(), file.as_str()),
            ("feco-signatures", "data/eicar.com")
        );
        assert!(name.contains("EICAR"));

        let arm = check(vec![("SoundLab.exe", fixtures::pe_for(0xaa64, true, &[]))]).await;
        assert_eq!(
            arm.problems,
            ["Kiến trúc không khớp: f-app.toml khai báo windows-x64 nhưng SoundLab.exe là arm64"]
        );
        let packed = check(vec![(
            "SoundLab.exe",
            fixtures::pe_for(0x8664, true, &["UPX0"]),
        )])
        .await;
        assert!(
            packed.problems[0].contains("làm rối bằng UPX"),
            "{packed:?}"
        );
        let missing = check(vec![("Other.exe", fixtures::pe(true))]).await;
        assert!(missing
            .problems
            .iter()
            .any(|p| p.contains("không có tệp chạy SoundLab.exe")));
    }
}
