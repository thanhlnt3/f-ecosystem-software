//! Cài bản F-Launcher đã tải về khi khởi động (thay đổi launcher-self-update, design S3).
//!
//! [`crate::Launcher::stage_self_update`] tải gói `feco-launcher` qua kho phân phối (TUF: chữ ký,
//! hash, độ dài), giải nén vào `self-update/<phiên bản>/` và ghi `self-update/pending.json`.
//! [`apply_pending`] chạy đầu `main`, trước khi mở cửa sổ: đổi tên tệp đang chạy thành `.old`
//! (Windows cho đổi tên nhưng không cho ghi đè tệp đang chạy), chép bản mới vào chỗ cũ; lỗi thì
//! đặt lại bản cũ. Tệp `.old` được xóa ở lần chạy sau.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::compare_versions;
use crate::paths::{safe_component, safe_relative};

/// Tên tệp ghi bản đang chờ cài trong thư mục `self-update`.
pub const PENDING_FILE: &str = "pending.json";

/// Bản F-Launcher đã tải về, chờ cài khi khởi động lại.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub version: String,
    /// Tệp chạy, tương đối với `self-update/<phiên bản>/`.
    pub entry: String,
}

/// Đọc bản đang chờ cài (`None`: không có, hoặc tệp hỏng).
pub fn read_pending(dir: &Path) -> Option<Pending> {
    let text = std::fs::read_to_string(dir.join(PENDING_FILE)).ok()?;
    let p: Pending = serde_json::from_str(&text).ok()?;
    (safe_component(&p.version) && safe_relative(&p.entry)).then_some(p)
}

pub fn write_pending(dir: &Path, pending: &Pending) -> std::io::Result<()> {
    std::fs::write(
        dir.join(PENDING_FILE),
        serde_json::to_vec(pending).expect("JSON"),
    )
}

/// Đường dẫn bản cũ được giữ tạm trong lúc cài: `feco-launcher.exe` → `feco-launcher.exe.old`.
pub fn old_path(exe: &Path) -> PathBuf {
    let mut name = exe.as_os_str().to_owned();
    name.push(".old");
    PathBuf::from(name)
}

/// Cài bản đang chờ (nếu mới hơn `current_version`) vào `current_exe`. Trả phiên bản đã cài;
/// `Ok(None)` khi không có gì để cài. Lỗi chép thì bản cũ được đặt lại và trả lỗi.
pub fn apply_pending(
    dir: &Path,
    current_exe: &Path,
    current_version: &str,
) -> std::io::Result<Option<String>> {
    apply_pending_with(dir, current_exe, current_version, |from, to| {
        std::fs::copy(from, to).map(|_| ())
    })
}

/// Như [`apply_pending`], với hàm chép cho trước (test giả lập chép lỗi).
fn apply_pending_with(
    dir: &Path,
    current_exe: &Path,
    current_version: &str,
    copy: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<Option<String>> {
    // Bản cũ của lần cập nhật trước (tiến trình cũ đã thoát).
    let old = old_path(current_exe);
    if old.exists() {
        let _ = std::fs::remove_file(&old);
    }
    let Some(pending) = read_pending(dir) else {
        let _ = std::fs::remove_file(dir.join(PENDING_FILE));
        return Ok(None);
    };
    let new_exe = dir.join(&pending.version).join(&pending.entry);
    if compare_versions(&pending.version, current_version) != Ordering::Greater
        || !new_exe.is_file()
    {
        // Đã cài (hoặc bản chờ cũ hơn, hoặc thư mục bị xóa): bỏ.
        std::fs::remove_file(dir.join(PENDING_FILE))?;
        return Ok(None);
    }
    std::fs::rename(current_exe, &old)?;
    if let Err(e) = copy(&new_exe, current_exe) {
        // Đặt lại bản cũ; giữ pending để thử lại lần sau.
        let _ = std::fs::remove_file(current_exe);
        std::fs::rename(&old, current_exe)?;
        return Err(e);
    }
    std::fs::remove_file(dir.join(PENDING_FILE))?;
    Ok(Some(pending.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("self-update");
        std::fs::create_dir_all(dir.join("99.0.0")).unwrap();
        std::fs::write(dir.join("99.0.0").join("feco-launcher.exe"), b"ban moi").unwrap();
        let exe = tmp.path().join("bin").join("feco-launcher.exe");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"ban cu").unwrap();
        (tmp, dir, exe)
    }

    /// Scenario "Có bản launcher mới": lúc khởi động, bản chờ thay tệp đang chạy; lần sau xóa `.old`.
    #[test]
    fn pending_update_replaces_the_executable() {
        let (_tmp, dir, exe) = setup();
        assert_eq!(
            apply_pending(&dir, &exe, "1.17.0").unwrap(),
            None,
            "chưa có gì chờ"
        );
        write_pending(
            &dir,
            &Pending {
                version: "99.0.0".into(),
                entry: "feco-launcher.exe".into(),
            },
        )
        .unwrap();
        assert_eq!(
            apply_pending(&dir, &exe, "1.17.0").unwrap().as_deref(),
            Some("99.0.0")
        );
        assert_eq!(std::fs::read(&exe).unwrap(), b"ban moi");
        assert_eq!(std::fs::read(old_path(&exe)).unwrap(), b"ban cu");
        assert!(read_pending(&dir).is_none());
        // Lần chạy sau (bản mới): xóa bản cũ, không cài lại.
        assert_eq!(apply_pending(&dir, &exe, "99.0.0").unwrap(), None);
        assert!(!old_path(&exe).exists());
    }

    /// Bản chờ không mới hơn bản đang chạy, hoặc `pending.json` không an toàn: bỏ, không cài.
    #[test]
    fn stale_or_unsafe_pending_is_ignored() {
        let (_tmp, dir, exe) = setup();
        write_pending(
            &dir,
            &Pending {
                version: "99.0.0".into(),
                entry: "feco-launcher.exe".into(),
            },
        )
        .unwrap();
        assert_eq!(apply_pending(&dir, &exe, "99.0.0").unwrap(), None);
        assert!(!dir.join(PENDING_FILE).exists());
        assert_eq!(std::fs::read(&exe).unwrap(), b"ban cu");
        std::fs::write(
            dir.join(PENDING_FILE),
            r#"{"version":"99.0.0","entry":"../../evil.exe"}"#,
        )
        .unwrap();
        assert!(read_pending(&dir).is_none());
        assert_eq!(apply_pending(&dir, &exe, "1.17.0").unwrap(), None);
        assert_eq!(std::fs::read(&exe).unwrap(), b"ban cu");
    }

    /// Scenario "Cài không thành công": chép lỗi thì bản cũ được đặt lại, bản chờ còn để thử lại.
    #[test]
    fn failed_copy_restores_the_old_executable() {
        let (_tmp, dir, exe) = setup();
        let pending = Pending {
            version: "99.0.0".into(),
            entry: "feco-launcher.exe".into(),
        };
        write_pending(&dir, &pending).unwrap();
        let err = apply_pending_with(&dir, &exe, "1.17.0", |_, to| {
            // Chép được một phần rồi lỗi (đĩa đầy).
            std::fs::write(to, b"do dang")?;
            Err(std::io::Error::other("đĩa đầy"))
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "đĩa đầy");
        assert_eq!(
            std::fs::read(&exe).unwrap(),
            b"ban cu",
            "bản cũ được đặt lại"
        );
        assert!(!old_path(&exe).exists());
        assert_eq!(read_pending(&dir), Some(pending), "bản chờ còn để thử lại");
    }
}
