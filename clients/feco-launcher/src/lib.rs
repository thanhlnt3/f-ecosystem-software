//! F-Launcher (spec `launcher`, design D4).
//!
//! Thư viện chứa toàn bộ phần logic để kiểm thử được mà không cần giao diện; `main.rs` chỉ là
//! giao diện egui gọi vào đây.
//!
//! - [`auth`]: đăng nhập qua trình duyệt hệ thống bằng OAuth 2.0 + PKCE với địa chỉ loopback;
//!   refresh token nằm trong kho bảo mật của hệ điều hành (`keyring-core` cùng kho gốc của Windows và macOS).
//! - [`install::Launcher`]: đồng bộ danh mục qua TUF (`feco-tuf`/`tough`), cài, cập nhật nguyên
//!   tử, quay lại bản trước, gỡ app, chặn mở phiên bản bị thu hồi; "App của tôi" vẫn dùng được
//!   khi ngoại tuyến.
//! - [`launch`] và [`broker`]: mở app con với mã khởi chạy dùng một lần; API cục bộ chỉ cấp mã
//!   cho đúng app mà launcher đã khởi chạy. App có cờ `terminal` mở trong cửa sổ terminal riêng.
//! - [`library`]: tìm, lọc, sắp xếp app; biểu tượng chữ cái đầu. [`prefs`]: cài đặt giao diện
//!   (`settings.json`).

pub mod auth;
pub mod broker;
pub mod catalog;
pub mod config;
pub mod events;
pub mod i18n;
mod i18n_ja;
pub mod install;
pub mod launch;
pub mod library;
pub mod paths;
pub mod prefs;
pub mod state;
pub mod style;

pub use catalog::{AppInfo, CatalogEntry};
pub use config::Config;
pub use install::{AppStatus, LaunchPlan, Launcher, LauncherError, MyApp};
pub use paths::Paths;

/// Dòng công bố mức cô lập trên màn hình cài đặt (spec launcher, "Công bố mức cô lập").
pub const ISOLATION_NOTICE: &str = "App này chạy với quyền như mọi ứng dụng khác trên máy bạn";

pub(crate) fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên của hệ điều hành");
    hex::encode(buf)
}
