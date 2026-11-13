//! Trạng thái cục bộ của launcher (`state.json`): app đã cài, bản trước để quay lại, danh mục và
//! danh sách thu hồi của lần đồng bộ gần nhất. Ghi kiểu nguyên tử (file tạm rồi đổi tên).

use crate::catalog::{AppInfo, CatalogEntry};
use feco_tuf::Revocation;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledVersion {
    pub app: String,
    pub version: String,
    pub title: String,
    /// Tên target TUF của gói đã cài.
    pub target: String,
    pub sha256: String,
    /// File chạy, tương đối với thư mục phiên bản.
    pub entry: String,
    pub client_id: Option<String>,
    pub installed_at: Timestamp,
    /// Thông tin hiển thị lúc cài (state.json của bản 1.0 không có: để trống).
    #[serde(default)]
    pub info: AppInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledApp {
    pub current: InstalledVersion,
    /// Bản ngay trước đó, vẫn nằm trên đĩa để quay lại không cần tải.
    pub previous: Option<InstalledVersion>,
    /// Lần cuối người dùng mở app từ launcher (sắp xếp "Mở gần đây").
    #[serde(default)]
    pub last_opened: Option<Timestamp>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LocalState {
    #[serde(default)]
    pub apps: BTreeMap<String, InstalledApp>,
    /// Tên người dùng của lần đăng nhập gần nhất (hiện khi ngoại tuyến).
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub last_sync: Option<Timestamp>,
    #[serde(default)]
    pub catalog: Vec<CatalogEntry>,
    #[serde(default)]
    pub revocations: Vec<Revocation>,
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default)]
    pub last_update_check: Option<Timestamp>,
    /// Mã ngẫu nhiên của bản cài launcher này, dùng để máy chủ chia nhóm phát hành dần.
    /// Không gắn với tài khoản người dùng.
    #[serde(default)]
    pub device_id: Option<String>,
    /// Phiên bản máy chủ cho phép thiết bị này nhận, theo từng app (phát hành dần).
    #[serde(default)]
    pub rollout: BTreeMap<String, String>,
    /// App riêng tư mà máy chủ cho người dùng đang đăng nhập thấy (`GET /v1/catalog/private`).
    #[serde(default)]
    pub private_apps: std::collections::BTreeSet<String>,
}

impl LocalState {
    /// Đọc trạng thái; chưa có file thì trả trạng thái rỗng. File hỏng được đổi tên thành
    /// `state.json.corrupt` để không mất dấu vết, rồi bắt đầu lại từ trạng thái rỗng.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(state) => Ok(state),
                Err(e) => {
                    eprintln!("state.json hỏng ({e}), bắt đầu lại");
                    std::fs::rename(path, path.with_extension("json.corrupt"))?;
                    Ok(Self::default())
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)
    }
}
