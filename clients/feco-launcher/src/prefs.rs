//! Cài đặt giao diện của launcher (spec launcher, "Cài đặt giao diện của launcher", "Lưu cài
//! đặt giao diện trên máy"; design E1 của improve-launcher-experience).
//!
//! Lưu trong `<data>/settings.json`, tách khỏi `state.json`, chỉ trên máy người dùng. Đọc từng
//! trường riêng: trường hỏng hoặc lạ dùng giá trị mặc định, các trường khác vẫn giữ. Cỡ chữ bị
//! kẹp trong [`FONT_SCALE_MIN`]–[`FONT_SCALE_MAX`].

use crate::library::SortKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

pub const FONT_SCALE_MIN: f32 = 0.8;
pub const FONT_SCALE_MAX: f32 = 1.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];
    pub const fn label(self) -> &'static str {
        match self {
            Theme::System => "Theo hệ thống",
            Theme::Light => "Sáng",
            Theme::Dark => "Tối",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    #[default]
    Blue,
    Teal,
    Green,
    Purple,
    Pink,
    Orange,
}

impl Accent {
    pub const ALL: [Accent; 6] = [
        Accent::Blue,
        Accent::Teal,
        Accent::Green,
        Accent::Purple,
        Accent::Pink,
        Accent::Orange,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Accent::Blue => "Xanh dương",
            Accent::Teal => "Xanh ngọc",
            Accent::Green => "Xanh lá",
            Accent::Purple => "Tím",
            Accent::Pink => "Hồng",
            Accent::Orange => "Cam",
        }
    }
    /// Màu nhấn (RGB).
    pub const fn rgb(self) -> [u8; 3] {
        match self {
            Accent::Blue => [0x25, 0x63, 0xEB],
            Accent::Teal => [0x0D, 0x94, 0x88],
            Accent::Green => [0x16, 0xA3, 0x4A],
            Accent::Purple => [0x7C, 0x3A, 0xED],
            Accent::Pink => [0xDB, 0x27, 0x77],
            Accent::Orange => [0xEA, 0x58, 0x0C],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Grid,
    List,
}

impl Layout {
    pub const fn label(self) -> &'static str {
        match self {
            Layout::Grid => "Lưới",
            Layout::List => "Danh sách",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl TileSize {
    pub const ALL: [TileSize; 3] = [TileSize::Small, TileSize::Medium, TileSize::Large];
    pub const fn label(self) -> &'static str {
        match self {
            TileSize::Small => "Nhỏ",
            TileSize::Medium => "Vừa",
            TileSize::Large => "Lớn",
        }
    }
    /// Chiều rộng của một ô lưới (điểm ảnh logic, trước khi nhân cỡ chữ).
    pub const fn width(self) -> f32 {
        match self {
            TileSize::Small => 132.0,
            TileSize::Medium => 172.0,
            TileSize::Large => 220.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartPage {
    #[default]
    Library,
    Store,
}

impl StartPage {
    pub const fn label(self) -> &'static str {
        match self {
            StartPage::Library => "Thư viện",
            StartPage::Store => "Cửa hàng",
        }
    }
}

/// Cài đặt giao diện. Thêm trường mới thì thêm vào [`Prefs::from_value`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    pub theme: Theme,
    pub accent: Accent,
    /// Hệ số cỡ chữ (1.0 = 100%).
    pub font_scale: f32,
    /// Bố cục của Thư viện và Cửa hàng; nút trên thanh công cụ đổi giá trị này.
    pub layout: Layout,
    pub tile_size: TileSize,
    pub start_page: StartPage,
    pub show_summary: bool,
    /// Chế độ gọn: giảm khoảng cách, hàng danh sách thấp hơn.
    pub compact: bool,
    pub library_sort: SortKey,
    pub store_sort: SortKey,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            accent: Accent::Blue,
            font_scale: 1.0,
            layout: Layout::Grid,
            tile_size: TileSize::Medium,
            start_page: StartPage::Library,
            show_summary: true,
            compact: false,
            library_sort: SortKey::Name,
            store_sort: SortKey::Name,
        }
    }
}

fn field<T: DeserializeOwned>(map: &Map<String, Value>, key: &str, default: T) -> T {
    map.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(default)
}

pub fn clamp_font_scale(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(FONT_SCALE_MIN, FONT_SCALE_MAX)
    } else {
        1.0
    }
}

impl Prefs {
    /// Đọc từ JSON, từng trường một; trường hỏng dùng mặc định.
    pub fn from_value(value: &Value) -> Self {
        let d = Prefs::default();
        let Some(map) = value.as_object() else {
            return d;
        };
        let prefs = Prefs {
            theme: field(map, "theme", d.theme),
            accent: field(map, "accent", d.accent),
            font_scale: clamp_font_scale(field(map, "font_scale", d.font_scale)),
            layout: field(map, "layout", d.layout),
            tile_size: field(map, "tile_size", d.tile_size),
            start_page: field(map, "start_page", d.start_page),
            show_summary: field(map, "show_summary", d.show_summary),
            compact: field(map, "compact", d.compact),
            library_sort: field(map, "library_sort", d.library_sort),
            store_sort: field(map, "store_sort", d.store_sort),
        };
        prefs.sanitized()
    }

    /// Kiểu sắp xếp chỉ hợp với một trang (ví dụ "Dung lượng" chỉ ở Cửa hàng) về mặc định.
    pub fn sanitized(mut self) -> Self {
        self.font_scale = clamp_font_scale(self.font_scale);
        if !SortKey::for_library().contains(&self.library_sort) {
            self.library_sort = SortKey::Name;
        }
        if !SortKey::for_store().contains(&self.store_sort) {
            self.store_sort = SortKey::Name;
        }
        self
    }

    /// Đọc `settings.json`. Không có file, file hỏng hay không đọc được đều trả cài đặt mặc
    /// định (kèm lý do để ghi nhật ký), không bao giờ làm launcher lỗi.
    pub fn load(path: &Path) -> (Self, Option<String>) {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(v) => (Self::from_value(&v), None),
                Err(e) => (
                    Self::default(),
                    Some(format!("settings.json hỏng ({e}), dùng cài đặt mặc định")),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (
                Self::default(),
                Some(format!("không đọc được settings.json ({e})")),
            ),
        }
    }

    /// Ghi nguyên tử (file tạm rồi đổi tên), như `state.json`.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes =
            serde_json::to_vec_pretty(&self.clone().sanitized()).map_err(std::io::Error::other)?;
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scenario "Nhớ bố cục" và "Đổi chủ đề tối": lưu rồi đọc lại giữ nguyên.
    #[test]
    fn saved_prefs_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let (first, note) = Prefs::load(&path);
        assert_eq!(first, Prefs::default());
        assert!(note.is_none());
        let prefs = Prefs {
            theme: Theme::Dark,
            layout: Layout::List,
            accent: Accent::Orange,
            font_scale: 1.25,
            library_sort: SortKey::RecentlyOpened,
            store_sort: SortKey::Size,
            ..Prefs::default()
        };
        prefs.save(&path).unwrap();
        assert_eq!(Prefs::load(&path).0, prefs);
        assert!(!dir.path().join("settings.json.tmp").exists());
    }

    /// Scenario "File cài đặt hỏng".
    #[test]
    fn broken_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"\x00 day khong phai JSON").unwrap();
        let (prefs, note) = Prefs::load(&path);
        assert_eq!(prefs, Prefs::default());
        assert!(note.unwrap().contains("hỏng"));

        // Trường hỏng về mặc định, trường đúng vẫn giữ, cỡ chữ bị kẹp.
        std::fs::write(
            &path,
            r#"{"theme":"dark","font_scale":9.0,"layout":"bảng","accent":42,"store_sort":"recently_opened","unknown":true}"#,
        )
        .unwrap();
        let (prefs, note) = Prefs::load(&path);
        assert!(note.is_none());
        assert_eq!(prefs.theme, Theme::Dark);
        assert_eq!(prefs.font_scale, FONT_SCALE_MAX);
        assert_eq!(prefs.layout, Layout::Grid);
        assert_eq!(prefs.accent, Accent::Blue);
        assert_eq!(
            prefs.store_sort,
            SortKey::Name,
            "Mở gần đây không có ở Cửa hàng"
        );

        assert_eq!(clamp_font_scale(0.1), FONT_SCALE_MIN);
        assert_eq!(clamp_font_scale(f32::NAN), 1.0);
        assert_eq!(
            Prefs::from_value(&serde_json::json!([1, 2])),
            Prefs::default()
        );
    }

    /// Scenario "Khôi phục mặc định": ghi cài đặt mặc định đè lên cài đặt đã đổi.
    #[test]
    fn reset_restores_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        Prefs {
            font_scale: 1.4,
            accent: Accent::Orange,
            compact: true,
            ..Prefs::default()
        }
        .save(&path)
        .unwrap();
        Prefs::default().save(&path).unwrap();
        let (prefs, _) = Prefs::load(&path);
        assert_eq!(prefs.font_scale, 1.0);
        assert_eq!(prefs.accent, Accent::Blue);
        assert!(!prefs.compact && prefs.show_summary);
    }
}
