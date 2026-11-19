//! Giải nén zip an toàn và tải mã nguồn GitHub tại một commit.
//!
//! Giải nén: mọi tên mục được kiểm **trước** khi ghi tệp nào (`enclosed_name`, không có `:`), nên
//! zip có mục `../../evil.sh` bị từ chối mà không ghi gì; symlink bị bỏ qua (không tạo ra);
//! tổng dung lượng giải nén đếm theo byte thật, tối đa 1 GB (chặn bom zip).

use std::{
    io::{Read, Write},
    path::Path,
};

use crate::ScanError;

/// Tổng dung lượng giải nén tối đa.
pub const MAX_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Extracted {
    pub files: usize,
    pub bytes: u64,
    pub symlinks_skipped: usize,
}

/// Giải nén `zip` vào `dest` (thư mục rỗng).
pub fn extract(zip: &Path, dest: &Path, max_files: usize) -> Result<Extracted, ScanError> {
    extract_limited(zip, dest, max_files, MAX_EXTRACTED_BYTES)
}

/// Kiểm một tệp zip mà không giải nén: mở được, không quá `max_files` mục, mọi tên mục an toàn.
/// Trả số mục.
pub fn validate(zip: &Path, max_files: usize) -> Result<usize, ScanError> {
    let file =
        std::fs::File::open(zip).map_err(|e| ScanError::Io(format!("{}: {e}", zip.display())))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| ScanError::NotZip(e.to_string()))?;
    check_names(&mut archive, max_files)?;
    Ok(archive.len())
}

fn check_names<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    max_files: usize,
) -> Result<(), ScanError> {
    if archive.len() > max_files {
        return Err(ScanError::TooManyFiles(archive.len()));
    }
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| ScanError::NotZip(e.to_string()))?;
        let raw = String::from_utf8_lossy(entry.name_raw()).into_owned();
        if entry.enclosed_name().is_none()
            || raw.contains(':')
            || raw.starts_with('/')
            || raw.starts_with('\\')
        {
            return Err(ScanError::UnsafePath(raw));
        }
    }
    Ok(())
}

pub fn extract_limited(
    zip: &Path,
    dest: &Path,
    max_files: usize,
    max_bytes: u64,
) -> Result<Extracted, ScanError> {
    let file =
        std::fs::File::open(zip).map_err(|e| ScanError::Io(format!("{}: {e}", zip.display())))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| ScanError::NotZip(e.to_string()))?;
    // Lượt 1: kiểm mọi tên mục trước khi ghi.
    check_names(&mut archive, max_files)?;
    // Lượt 2: giải nén.
    let mut out = Extracted::default();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ScanError::NotZip(e.to_string()))?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(ScanError::UnsafePath(
                entry.name().unwrap_or_default().into_owned(),
            ));
        };
        if entry.is_symlink() {
            out.symlinks_skipped += 1;
            continue;
        }
        let path = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| ScanError::Io(e.to_string()))?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ScanError::Io(e.to_string()))?;
        }
        let mut file = std::fs::File::create(&path).map_err(|e| ScanError::Io(e.to_string()))?;
        let remaining = max_bytes - out.bytes;
        let copied = std::io::copy(&mut (&mut entry).take(remaining + 1), &mut file)
            .map_err(|e| ScanError::NotZip(e.to_string()))?;
        out.bytes += copied;
        if out.bytes > max_bytes {
            return Err(ScanError::TooLarge);
        }
        out.files += 1;
    }
    Ok(out)
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FetchError {
    /// Lỗi mạng (không kết nối được, hết giờ, máy chủ 5xx): thử lại sau.
    #[error("lỗi mạng khi tải mã nguồn: {0}")]
    Network(String),
    /// Máy chủ trả mã 4xx (repo riêng tư, commit không tồn tại).
    #[error("máy chủ mã nguồn trả HTTP {0}")]
    Status(u16),
    #[error("mã nguồn lớn hơn {0} byte")]
    TooLarge(u64),
    #[error("{0}")]
    Io(String),
}

/// Tên chủ repo hoặc repo GitHub hợp lệ.
pub fn valid_github_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && s != "."
        && s != ".."
}

/// Commit đủ 40 ký tự hex.
pub fn valid_commit(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Tải `{base}/{owner}/{repo}/zip/{commit}` (codeload.github.com) vào `dest`, tối đa `max_bytes`.
/// Chỉ gửi đi tên repo và commit.
pub async fn download_github_zip(
    http: &reqwest::Client,
    base: &str,
    owner: &str,
    repo: &str,
    commit: &str,
    dest: &Path,
    max_bytes: u64,
) -> Result<u64, FetchError> {
    if !valid_github_name(owner) || !valid_github_name(repo) || !valid_commit(commit) {
        return Err(FetchError::Io(format!(
            "repo hoặc commit không hợp lệ: {owner}/{repo}@{commit}"
        )));
    }
    let url = format!("{}/{owner}/{repo}/zip/{commit}", base.trim_end_matches('/'));
    let mut res = http
        .get(&url)
        .send()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))?;
    let status = res.status();
    if status.is_server_error() || status.as_u16() == 429 {
        return Err(FetchError::Network(format!("HTTP {}", status.as_u16())));
    }
    if !status.is_success() {
        return Err(FetchError::Status(status.as_u16()));
    }
    if res.content_length().is_some_and(|n| n > max_bytes) {
        return Err(FetchError::TooLarge(max_bytes));
    }
    let mut file = std::fs::File::create(dest).map_err(|e| FetchError::Io(e.to_string()))?;
    let mut total = 0u64;
    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))?
    {
        total += chunk.len() as u64;
        if total > max_bytes {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(FetchError::TooLarge(max_bytes));
        }
        file.write_all(&chunk)
            .map_err(|e| FetchError::Io(e.to_string()))?;
    }
    file.flush().map_err(|e| FetchError::Io(e.to_string()))?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_zip(path: &Path, entries: &[(&str, &[u8])], symlink: Option<(&str, &str)>) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        if let Some((name, target)) = symlink {
            zip.add_symlink(name, target, options).unwrap();
        }
        zip.finish().unwrap();
    }

    /// Scenario "Zip độc".
    #[test]
    fn evil_zip_is_refused_without_writing() {
        let work = tempfile::tempdir().unwrap();
        let zip = work.path().join("evil.zip");
        make_zip(
            &zip,
            &[("ok.txt", b"ok"), ("../../evil.sh", b"rm -rf /")],
            None,
        );
        let dest = work.path().join("a").join("b").join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let err = extract(&zip, &dest, 100).unwrap_err();
        assert_eq!(err, ScanError::UnsafePath("../../evil.sh".into()));
        // Không ghi gì, kể cả mục hợp lệ đứng trước.
        assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 0);
        assert!(!work.path().join("a").join("evil.sh").exists());
        assert!(!work.path().join("evil.sh").exists());
        // scan_zip cũng từ chối.
        let err = crate::scan_zip(&zip, &crate::ScanOptions::default()).unwrap_err();
        assert!(matches!(err, ScanError::UnsafePath(_)));

        for bad in ["/etc/passwd", "C:/Windows/x.dll", "a/../../b", "x:stream"] {
            make_zip(&zip, &[(bad, b"x")], None);
            assert!(
                matches!(extract(&zip, &dest, 100), Err(ScanError::UnsafePath(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn symlinks_skipped_and_bombs_refused() {
        let work = tempfile::tempdir().unwrap();
        let zip = work.path().join("s.zip");
        make_zip(&zip, &[("src/a.txt", b"a")], Some(("link", "/etc/passwd")));
        let dest = work.path().join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let got = extract(&zip, &dest, 100).unwrap();
        assert_eq!((got.files, got.symlinks_skipped), (1, 1));
        assert!(!dest.join("link").exists());

        // Bom zip: 2 MB số 0 nén còn vài KB, giới hạn 1 MB.
        let bomb = work.path().join("bomb.zip");
        {
            let file = std::fs::File::create(&bomb).unwrap();
            let mut z = zip::ZipWriter::new(file);
            let o = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            z.start_file("zeros.bin", o).unwrap();
            z.write_all(&vec![0u8; 2 * 1024 * 1024]).unwrap();
            z.finish().unwrap();
        }
        assert!(std::fs::metadata(&bomb).unwrap().len() < 100 * 1024);
        let dest2 = work.path().join("out2");
        std::fs::create_dir_all(&dest2).unwrap();
        assert_eq!(
            extract_limited(&bomb, &dest2, 100, 1024 * 1024).unwrap_err(),
            ScanError::TooLarge
        );
        // Quá nhiều mục.
        let many = work.path().join("many.zip");
        let names: Vec<String> = (0..5).map(|i| format!("f{i}")).collect();
        let entries: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b"x"[..])).collect();
        make_zip(&many, &entries, None);
        assert_eq!(
            extract(&many, &dest2, 3).unwrap_err(),
            ScanError::TooManyFiles(5)
        );
    }

    #[test]
    fn github_names() {
        assert!(valid_github_name("katD2707"));
        assert!(valid_github_name("agent-mentor-maximum-experience"));
        assert!(!valid_github_name("a/b"));
        assert!(!valid_github_name(".."));
        assert!(valid_commit("0a3b8b708cf435f730a77453b8203fbff280d198"));
        assert!(!valid_commit("main"));
    }
}
