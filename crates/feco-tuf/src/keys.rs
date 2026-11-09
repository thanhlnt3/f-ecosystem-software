//! Khóa ký. Bản dev lưu khóa Ed25519 dạng PKCS#8 DER trong file; production thay
//! [`LocalKeySource`] bằng nguồn khóa KMS/HSM (cùng trait `KeySource` của `tough`), riêng khóa
//! root được giữ ngoại tuyến.

use crate::error::{io_err, Error, Result};
use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::Ed25519KeyPair;
use std::io::Write;
use std::path::{Path, PathBuf};
use tough::key_source::{KeySource, LocalKeySource};
use tough::schema::key::Key;

/// Tạo khóa Ed25519 mới và ghi vào `path` (không ghi đè file có sẵn; trên Unix quyền 0600).
/// Trả về key ID dạng hex.
pub async fn generate(path: &Path) -> Result<String> {
    let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map_err(|_| Error::Key {
        source_name: path.display().to_string(),
        message: "không tạo được khóa Ed25519".into(),
    })?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(io_err(parent))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(io_err(path))?;
    file.write_all(doc.as_ref()).map_err(io_err(path))?;
    file.sync_all().map_err(io_err(path))?;
    key_id(source(path).as_ref()).await
}

/// Nguồn khóa từ file (bản dev).
pub fn source(path: impl Into<PathBuf>) -> Box<dyn KeySource> {
    Box::new(LocalKeySource { path: path.into() })
}

/// Khóa công khai theo định dạng TUF của một nguồn khóa.
pub async fn public_key(source: &dyn KeySource) -> Result<Key> {
    let signer = source.as_sign().await.map_err(|e| Error::Key {
        source_name: format!("{source:?}"),
        message: e.to_string(),
    })?;
    Ok(signer.tuf_key())
}

/// Key ID (hex) của một nguồn khóa, như ghi trong root.json.
pub async fn key_id(source: &dyn KeySource) -> Result<String> {
    Ok(hex::encode(public_key(source).await?.key_id()?))
}
