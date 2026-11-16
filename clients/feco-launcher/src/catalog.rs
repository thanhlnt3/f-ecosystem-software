//! Danh mục trong launcher, dựng từ metadata TUF đã xác minh. Khi có dịch vụ danh mục (nhóm 7),
//! phần mô tả, ảnh, đánh giá lấy từ đó; nguồn tin cậy cho gói cài đặt vẫn là TUF.

use feco_tuf::repo::{valid_web_url, WEB_PLATFORM};
use feco_tuf::SyncState;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogEntry {
    pub app: String,
    pub title: String,
    pub version: String,
    pub platform: String,
    pub target: String,
    pub length: u64,
    pub sha256: String,
    pub entry: String,
    pub client_id: Option<String>,
    /// App riêng tư: chỉ hiện khi máy chủ liệt kê cho người dùng đang đăng nhập.
    #[serde(default)]
    pub private: bool,
    /// Thông tin hiển thị (mô tả, nhóm, nhà phát triển...), từ metadata TUF đã ký.
    #[serde(default)]
    pub info: AppInfo,
    /// Mọi nền tảng có gói đã ký của cùng app và cùng phiên bản (trang chi tiết: "Chạy trên").
    #[serde(default)]
    pub platforms: Vec<String>,
}

/// Thông tin hiển thị của một app (spec launcher, "Thông tin hiển thị của app"). Chỉ lấy từ
/// metadata TUF đã được nền tảng ký; metadata cũ không có thì để trống.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppInfo {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub developer: Option<String>,
    /// Địa chỉ mã nguồn (chỉ nhận HTTPS khi hiển thị liên kết).
    #[serde(default)]
    pub source: Option<String>,
    /// App chạy trong cửa sổ terminal.
    #[serde(default)]
    pub terminal: bool,
    /// Giới thiệu chung (trang chi tiết).
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    /// Website (chỉ nhận HTTPS).
    #[serde(default)]
    pub website: Option<String>,
    /// Các thay đổi trong phiên bản này.
    #[serde(default)]
    pub release_notes: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    /// Thời điểm phát hành (RFC 3339).
    #[serde(default)]
    pub released_at: Option<String>,
    /// Logo PNG mã hóa base64, từ metadata TUF đã ký.
    #[serde(default)]
    pub icon: Option<String>,
    /// Địa chỉ của app web (nền tảng `web`), từ metadata TUF đã ký và đã kiểm
    /// [`feco_tuf::repo::valid_web_url`].
    #[serde(default)]
    pub web_url: Option<String>,
    /// Target nền tảng `web` (kể cả khi `web_url` không hợp lệ: khi đó không mở được).
    #[serde(default)]
    pub web: bool,
}

impl CatalogEntry {
    /// App web: mở bằng trình duyệt, không có tệp chạy.
    pub fn is_web(&self) -> bool {
        self.platform == WEB_PLATFORM
    }
}

impl AppInfo {
    fn from_meta(meta: &feco_tuf::TargetMeta) -> Self {
        let clean = |v: &Option<String>| {
            v.as_ref()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        Self {
            summary: clean(&meta.summary),
            category: clean(&meta.category),
            developer: clean(&meta.developer),
            source: clean(&meta.source).filter(|s| s.starts_with("https://")),
            terminal: meta.terminal,
            description: clean(&meta.description),
            features: meta
                .features
                .iter()
                .map(|f| f.trim().to_owned())
                .filter(|f| !f.is_empty())
                .collect(),
            website: clean(&meta.website).filter(|s| s.starts_with("https://")),
            release_notes: clean(&meta.release_notes),
            license: clean(&meta.license),
            released_at: clean(&meta.released_at),
            icon: clean(&meta.icon),
            web_url: clean(&meta.web_url).filter(|u| valid_web_url(u)),
            web: meta.platform == WEB_PLATFORM,
        }
    }
}

/// Mọi phiên bản đang phát hành cho nền tảng này và mọi app web, xếp theo app rồi phiên bản mới
/// nhất trước. App có gói cho nền tảng này thì bỏ bản web của nó.
pub fn build(state: &SyncState, platform: &str) -> Vec<CatalogEntry> {
    let mut out: Vec<CatalogEntry> = state
        .targets
        .values()
        .filter_map(|t| {
            let meta = t.meta.as_ref()?;
            if meta.platform != platform && meta.platform != WEB_PLATFORM {
                return None;
            }
            Some(CatalogEntry {
                app: meta.app.clone(),
                title: meta.title.clone().unwrap_or_else(|| meta.app.clone()),
                version: meta.version.clone(),
                platform: meta.platform.clone(),
                target: t.name.clone(),
                length: t.length,
                sha256: t.sha256.clone(),
                entry: meta.entry.clone()?,
                client_id: meta.client_id.clone(),
                private: meta.private,
                info: AppInfo::from_meta(meta),
                platforms: Vec::new(),
            })
        })
        .collect();
    let native: std::collections::HashSet<String> = out
        .iter()
        .filter(|e| !e.is_web())
        .map(|e| e.app.clone())
        .collect();
    out.retain(|e| !e.is_web() || !native.contains(&e.app));
    // Nền tảng của cùng app và phiên bản, kể cả nền tảng khác máy này.
    for entry in &mut out {
        let mut platforms: Vec<String> = state
            .targets
            .values()
            .filter_map(|t| t.meta.as_ref())
            .filter(|m| m.app == entry.app && m.version == entry.version)
            .map(|m| m.platform.clone())
            .collect();
        platforms.sort();
        platforms.dedup();
        entry.platforms = platforms;
    }
    out.sort_by(|a, b| {
        a.app
            .cmp(&b.app)
            .then_with(|| compare_versions(&b.version, &a.version))
    });
    out
}

/// Bản mới nhất của từng app.
pub fn latest(entries: &[CatalogEntry]) -> Vec<&CatalogEntry> {
    let mut out: Vec<&CatalogEntry> = Vec::new();
    for e in entries {
        match out.iter_mut().find(|x| x.app == e.app) {
            Some(x) if compare_versions(&e.version, &x.version) == Ordering::Greater => *x = e,
            Some(_) => {}
            None => out.push(e),
        }
    }
    out
}

/// So sánh theo SemVer; phiên bản không theo SemVer thì so như chuỗi.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_order() {
        assert_eq!(compare_versions("1.10.0", "1.9.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.2.0-beta", "1.2.0"), Ordering::Less);
        assert_eq!(compare_versions("2024.1", "2023.9"), Ordering::Greater);
    }
}
