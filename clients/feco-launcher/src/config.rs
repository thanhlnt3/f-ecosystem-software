//! Cấu hình launcher. Bản phát hành sẽ đóng gói sẵn các giá trị này (kể cả `1.root.json`);
//! bản dev đọc từ biến môi trường.
//!
//! | Biến | Mặc định | Ý nghĩa |
//! |---|---|---|
//! | `FECO_LAUNCHER_CLIENT_ID` | (bắt buộc) | Client ID của launcher trong Hydra |
//! | `FECO_LAUNCHER_SERVER` | `http://127.0.0.1:8080/` | feco-server |
//! | `FECO_LAUNCHER_HYDRA` | `http://127.0.0.1:4444/` | Hydra (OAuth 2.0) |
//! | `FECO_LAUNCHER_TUF` | (bắt buộc) | Gốc kho TUF (có `metadata/` và `targets/`): HTTPS, hoặc thư mục cục bộ khi dev |
//! | `FECO_LAUNCHER_TRUSTED_ROOT` | `<TUF>/metadata/1.root.json` nếu là thư mục | root.json tin cậy đi kèm bộ cài |
//! | `FECO_LAUNCHER_DATA_DIR` | thư mục dữ liệu của người dùng | Nơi cài app và lưu trạng thái |
//! | `FECO_LAUNCHER_PLATFORM` | nền tảng của máy | Ví dụ `windows-x64` |

use crate::paths::Paths;
use std::path::PathBuf;
use url::Url;

#[derive(Clone, Debug)]
pub struct Config {
    pub client_id: String,
    pub server: Url,
    pub hydra: Url,
    pub tuf_metadata: Url,
    pub tuf_targets: Url,
    pub trusted_root: PathBuf,
    pub data_dir: PathBuf,
    pub platform: String,
}

/// `windows-x64`, `macos-arm64`, `linux-x64`...
pub fn current_platform() -> String {
    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os}-{arch}")
}

fn with_slash(mut url: Url) -> Url {
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url
}

/// Chỉ cho HTTPS; HTTP chỉ khi trỏ vào máy này (dev); `file://` cho kho cục bộ khi dev.
fn check_transport(name: &str, url: &Url) -> Result<(), String> {
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    match url.scheme() {
        "https" => Ok(()),
        "http" if local => Ok(()),
        "file" if name == "FECO_LAUNCHER_TUF" => Ok(()),
        _ => Err(format!("{name} phải dùng HTTPS: {url}")),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_vars(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
    }

    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let url = |name: &str, default: &str| -> Result<Url, String> {
            let raw = get(name).unwrap_or_else(|| default.to_owned());
            let url = Url::parse(&raw).map_err(|e| format!("{name} không hợp lệ: {e}"))?;
            check_transport(name, &url)?;
            Ok(with_slash(url))
        };
        let client_id = get("FECO_LAUNCHER_CLIENT_ID")
            .ok_or("thiếu FECO_LAUNCHER_CLIENT_ID (client ID của launcher trong Hydra)")?;
        let tuf_raw = get("FECO_LAUNCHER_TUF")
            .ok_or("thiếu FECO_LAUNCHER_TUF (địa chỉ hoặc thư mục kho TUF)")?;
        let (tuf, local_dir) = match Url::parse(&tuf_raw) {
            Ok(u) if u.scheme().len() > 1 => (with_slash(u), None),
            // Không phải URL (kể cả "C:\..."): coi là thư mục cục bộ.
            _ => {
                let dir = std::path::absolute(&tuf_raw)
                    .map_err(|e| format!("FECO_LAUNCHER_TUF không hợp lệ: {e}"))?;
                let u = Url::from_directory_path(&dir)
                    .map_err(|()| format!("FECO_LAUNCHER_TUF không hợp lệ: {}", dir.display()))?;
                (u, Some(dir))
            }
        };
        check_transport("FECO_LAUNCHER_TUF", &tuf)?;
        let trusted_root = match (get("FECO_LAUNCHER_TRUSTED_ROOT"), &local_dir) {
            (Some(p), _) => PathBuf::from(p),
            (None, Some(dir)) => dir.join("metadata").join("1.root.json"),
            (None, None) => {
                return Err("thiếu FECO_LAUNCHER_TRUSTED_ROOT (root.json đi kèm bộ cài)".into())
            }
        };
        let data_dir = match get("FECO_LAUNCHER_DATA_DIR") {
            Some(p) => PathBuf::from(p),
            None => {
                Paths::default_root().ok_or("không xác định được thư mục dữ liệu người dùng")?
            }
        };
        Ok(Self {
            client_id,
            server: url("FECO_LAUNCHER_SERVER", "http://127.0.0.1:8080/")?,
            hydra: url("FECO_LAUNCHER_HYDRA", "http://127.0.0.1:4444/")?,
            tuf_metadata: tuf.join("metadata/").map_err(|e| e.to_string())?,
            tuf_targets: tuf.join("targets/").map_err(|e| e.to_string())?,
            trusted_root,
            data_dir,
            platform: get("FECO_LAUNCHER_PLATFORM").unwrap_or_else(current_platform),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(vars: &[(&str, &str)]) -> Result<Config, String> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_vars(|k| map.get(k).cloned())
    }

    #[test]
    fn https_required_except_local_dev() {
        let ok = config(&[
            ("FECO_LAUNCHER_CLIENT_ID", "launcher"),
            ("FECO_LAUNCHER_TUF", "https://dl.f-eco.example/tuf"),
            ("FECO_LAUNCHER_TRUSTED_ROOT", "root.json"),
            ("FECO_LAUNCHER_DATA_DIR", "data"),
            ("FECO_LAUNCHER_SERVER", "https://id.f-eco.example"),
            ("FECO_LAUNCHER_HYDRA", "https://auth.f-eco.example"),
        ])
        .unwrap();
        assert_eq!(
            ok.tuf_metadata.as_str(),
            "https://dl.f-eco.example/tuf/metadata/"
        );
        assert_eq!(
            ok.tuf_targets.as_str(),
            "https://dl.f-eco.example/tuf/targets/"
        );
        assert_eq!(ok.server.as_str(), "https://id.f-eco.example/");

        let err = config(&[
            ("FECO_LAUNCHER_CLIENT_ID", "launcher"),
            ("FECO_LAUNCHER_TUF", "http://dl.f-eco.example/tuf"),
            ("FECO_LAUNCHER_TRUSTED_ROOT", "root.json"),
        ])
        .unwrap_err();
        assert!(err.contains("HTTPS"), "{err}");
        let err = config(&[
            ("FECO_LAUNCHER_CLIENT_ID", "launcher"),
            ("FECO_LAUNCHER_TUF", "https://dl.f-eco.example/tuf"),
            ("FECO_LAUNCHER_TRUSTED_ROOT", "root.json"),
            ("FECO_LAUNCHER_SERVER", "http://id.f-eco.example"),
        ])
        .unwrap_err();
        assert!(err.contains("FECO_LAUNCHER_SERVER"), "{err}");
    }

    #[test]
    fn local_directory_for_dev() {
        let dir = std::env::temp_dir().join("feco-tuf-repo");
        let c = config(&[
            ("FECO_LAUNCHER_CLIENT_ID", "launcher"),
            ("FECO_LAUNCHER_TUF", dir.to_str().unwrap()),
            ("FECO_LAUNCHER_DATA_DIR", "data"),
        ])
        .unwrap();
        assert_eq!(c.tuf_metadata.scheme(), "file");
        assert!(
            c.trusted_root.ends_with("metadata/1.root.json")
                || c.trusted_root.ends_with("metadata\\1.root.json")
        );
        assert!(!current_platform().is_empty());
    }
}
