//! Thư mục của launcher, nằm trong thư mục riêng của người dùng (không cần quyền quản trị;
//! spec launcher, "App con chạy với quyền người dùng thường").

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `%LOCALAPPDATA%\F-Eco\Launcher` trên Windows, `~/Library/Application Support/F-Eco/Launcher`
    /// trên macOS.
    pub fn default_root() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("F-Eco").join("Launcher"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn apps(&self) -> PathBuf {
        self.root.join("apps")
    }

    /// Thư mục cài đặt của một app; mỗi phiên bản là một thư mục con.
    pub fn app(&self, app: &str) -> PathBuf {
        self.apps().join(app)
    }

    pub fn version(&self, app: &str, version: &str) -> PathBuf {
        self.app(app).join(version)
    }

    /// Dữ liệu cục bộ của app (giữ lại khi gỡ, trừ khi người dùng chọn xóa).
    pub fn app_data(&self, app: &str) -> PathBuf {
        self.root.join("appdata").join(app)
    }

    pub fn downloads(&self) -> PathBuf {
        self.root.join("downloads")
    }

    /// Metadata TUF đã tin, để chặn rollback giữa các lần đồng bộ.
    pub fn tuf_datastore(&self) -> PathBuf {
        self.root.join("tuf")
    }

    pub fn state_file(&self) -> PathBuf {
        self.root.join("state.json")
    }

    pub fn events_file(&self) -> PathBuf {
        self.root.join("security-events.jsonl")
    }
}

/// Tên app hoặc phiên bản dùng làm tên thư mục: chỉ chữ, số, `.`, `-`, `_`.
pub fn safe_component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && !s.starts_with('.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Đường dẫn tương đối trong gói (file chạy): không tuyệt đối, không có `..`.
pub fn safe_relative(s: &str) -> bool {
    let p = Path::new(s);
    !s.is_empty()
        && p.is_relative()
        && p.components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(safe_component("soundlab"));
        assert!(safe_component("1.2.0-beta_1"));
        for bad in ["", "..", ".hidden", "a/b", "a\\b", "x y"] {
            assert!(!safe_component(bad), "{bad}");
        }
        assert!(safe_relative("bin/soundlab.exe"));
        for bad in ["", "../x", "/etc/passwd", "a/../../b"] {
            assert!(!safe_relative(bad), "{bad}");
        }
    }
}
