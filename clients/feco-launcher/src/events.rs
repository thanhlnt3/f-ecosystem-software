//! Nhật ký sự kiện bảo mật trên máy (gói bị sửa, metadata giả, app xin mã của app khác...).
//! Mỗi dòng là một JSON. Nhóm 14 sẽ gửi các sự kiện này về máy chủ.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub at: Timestamp,
    pub kind: String,
    pub details: Value,
}

#[derive(Clone, Debug)]
pub struct EventLog {
    path: PathBuf,
}

impl EventLog {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn record(&self, kind: &str, details: Value) {
        let event = SecurityEvent {
            at: Timestamp::now(),
            kind: kind.to_owned(),
            details,
        };
        let result = (|| -> std::io::Result<()> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            let mut line = serde_json::to_vec(&event)?;
            line.push(b'\n');
            file.write_all(&line)
        })();
        if let Err(e) = result {
            eprintln!("không ghi được sự kiện bảo mật {kind}: {e}");
        }
    }

    pub fn read_all(&self) -> Vec<SecurityEvent> {
        std::fs::read_to_string(&self.path)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
}
