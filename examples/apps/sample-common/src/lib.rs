//! Phần dùng chung của các app mẫu F-Eco (`f-notes`, `f-timer`).
//!
//! - [`install_fonts`]: phông chữ hệ thống có dấu tiếng Việt (không đóng gói phông chữ).
//! - [`data_dir`]: thư mục dữ liệu mà F-Launcher cấp cho app (`FECO_APP_DATA_DIR`).
//! - [`Account`]: đăng nhập bằng F-SDK chạy nền. Mở từ F-Launcher thì dùng mã khởi chạy, không
//!   cần trình duyệt (lần đầu, nếu người dùng chưa đồng ý quyền, F-SDK mở trình duyệt để hỏi);
//!   mở trực tiếp thì app chạy ở chế độ khách.

use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Dùng phông chữ có sẵn của hệ điều hành (có dấu tiếng Việt).
pub fn install_fonts(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let mut candidates: Vec<PathBuf> = ["segoeui.ttf", "arial.ttf", "tahoma.ttf"]
        .iter()
        .map(|n| PathBuf::from(&windir).join("Fonts").join(n))
        .collect();
    for p in [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ] {
        candidates.push(p.into());
    }
    // Không có phông chữ hệ thống thì giữ phông mặc định của egui (thiếu một số dấu).
    let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::empty();
    fonts
        .font_data
        .insert("ui".into(), Arc::new(egui::FontData::from_owned(bytes)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(family, vec!["ui".into()]);
    }
    ctx.set_fonts(fonts);
}

/// Thư mục dữ liệu của app: `FECO_APP_DATA_DIR` khi được F-Launcher mở, không thì
/// `./<app>-data` (chạy thử khi phát triển).
pub fn data_dir(app: &str) -> PathBuf {
    std::env::var_os("FECO_APP_DATA_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{app}-data")))
}

/// Trạng thái đăng nhập hiển thị trên giao diện.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountState {
    /// Không được F-Launcher mở: dùng app không cần tài khoản.
    Guest,
    SigningIn,
    SignedIn {
        name: String,
    },
    Failed(String),
}

impl AccountState {
    pub fn describe(&self) -> String {
        match self {
            AccountState::Guest => "Chế độ khách (mở từ F-Launcher để dùng tài khoản F-Eco)".into(),
            AccountState::SigningIn => "Đang đăng nhập bằng F-Eco...".into(),
            AccountState::SignedIn { name } => format!("Xin chào {name}"),
            AccountState::Failed(e) => format!("Không đăng nhập được: {e}"),
        }
    }
}

/// Đăng nhập bằng F-SDK trên một luồng nền; giao diện đọc [`Account::state`] mỗi khung hình.
#[derive(Clone)]
pub struct Account {
    state: Arc<Mutex<AccountState>>,
}

impl Account {
    /// Bắt đầu đăng nhập nếu app được F-Launcher mở (có `FECO_LAUNCH_CODE`).
    pub fn start(ctx: &egui::Context) -> Self {
        let launched = fsdk::Launch::from_env().is_some();
        let account = Self {
            state: Arc::new(Mutex::new(if launched {
                AccountState::SigningIn
            } else {
                AccountState::Guest
            })),
        };
        if launched {
            let state = account.state.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())
                    .and_then(|rt| {
                        rt.block_on(async {
                            // Mã khởi chạy của launcher; lần đầu người dùng chưa đồng ý quyền
                            // thì F-SDK mở trình duyệt để hỏi (fsdk::session).
                            let config = fsdk::Config::from_env().map_err(|e| e.to_string())?;
                            let session =
                                fsdk::session(&config).await.map_err(|e| e.to_string())?;
                            let me = session.me().await.map_err(|e| e.to_string())?;
                            Ok::<_, String>(me.name.unwrap_or_else(|| "bạn".into()))
                        })
                    });
                *state.lock().expect("khóa") = match result {
                    Ok(name) => AccountState::SignedIn { name },
                    Err(e) => AccountState::Failed(e),
                };
                ctx.request_repaint();
            });
        }
        account
    }

    pub fn state(&self) -> AccountState {
        self.state.lock().expect("khóa").clone()
    }
}

/// Ghi file nguyên tử (file tạm rồi đổi tên), để mất điện giữa chừng không làm hỏng dữ liệu.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_when_not_launched() {
        assert!(AccountState::Guest.describe().contains("khách"));
        assert_eq!(
            AccountState::SignedIn { name: "Lan".into() }.describe(),
            "Xin chào Lan"
        );
    }

    #[test]
    fn atomic_write_replaces_file() {
        let dir = std::env::temp_dir().join(format!("feco-sample-{}", std::process::id()));
        let path = dir.join("a.json");
        write_atomic(&path, b"1").unwrap();
        write_atomic(&path, b"2").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"2");
        assert!(!path.with_extension("tmp").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
