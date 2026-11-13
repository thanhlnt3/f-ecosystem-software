//! Tìm, lọc và sắp xếp app trong Thư viện và Cửa hàng; biểu tượng chữ cái đầu (spec launcher,
//! "Tìm, lọc và sắp xếp app", "Thông tin hiển thị của app"; design E2, E4 của
//! improve-launcher-experience). Hàm thuần, giao diện chỉ gọi vào đây.

use crate::catalog::{AppInfo, CatalogEntry};
use crate::install::MyApp;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Kiểu sắp xếp. Mỗi trang chỉ cho chọn những kiểu có nghĩa với nó ([`SortKey::for_library`],
/// [`SortKey::for_store`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    #[default]
    Name,
    RecentlyOpened,
    RecentlyInstalled,
    Category,
    Size,
}

impl SortKey {
    pub const fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Tên (A–Z)",
            SortKey::RecentlyOpened => "Mở gần đây",
            SortKey::RecentlyInstalled => "Cài gần đây",
            SortKey::Category => "Nhóm",
            SortKey::Size => "Dung lượng",
        }
    }

    pub const fn for_library() -> [SortKey; 4] {
        [
            SortKey::Name,
            SortKey::RecentlyOpened,
            SortKey::RecentlyInstalled,
            SortKey::Category,
        ]
    }

    pub const fn for_store() -> [SortKey; 3] {
        [SortKey::Name, SortKey::Category, SortKey::Size]
    }
}

/// Những gì cần để tìm, lọc, sắp xếp một app.
pub trait Listed {
    fn app_id(&self) -> &str;
    fn title(&self) -> &str;
    fn info(&self) -> &AppInfo;
    fn size(&self) -> u64 {
        0
    }
    fn installed_at(&self) -> Option<Timestamp> {
        None
    }
    fn last_opened(&self) -> Option<Timestamp> {
        None
    }
}

impl Listed for MyApp {
    fn app_id(&self) -> &str {
        &self.app
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn info(&self) -> &AppInfo {
        &self.info
    }
    fn installed_at(&self) -> Option<Timestamp> {
        Some(self.installed_at)
    }
    fn last_opened(&self) -> Option<Timestamp> {
        self.last_opened
    }
}

impl Listed for CatalogEntry {
    fn app_id(&self) -> &str {
        &self.app
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn info(&self) -> &AppInfo {
        &self.info
    }
    fn size(&self) -> u64 {
        self.length
    }
}

/// Điều kiện hiển thị của một trang.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// Chuỗi tìm (không phân biệt hoa thường và dấu).
    pub text: String,
    /// Mã nhóm (`None`: mọi nhóm).
    pub category: Option<String>,
    pub sort: SortKey,
}

impl Query {
    pub fn is_filtered(&self) -> bool {
        !self.text.trim().is_empty() || self.category.is_some()
    }
}

/// Mã nhóm của app; app không khai báo thuộc nhóm `other` ("Khác").
pub fn category_of(info: &AppInfo) -> String {
    info.category
        .as_deref()
        .map(|c| c.trim().to_lowercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "other".into())
}

/// Tên hiển thị của nhóm. Nhóm lạ (manifest cho ghi tự do) hiện đúng như khai báo.
pub fn category_label(code: &str) -> String {
    match code {
        "productivity" => "Năng suất",
        "tools" | "utilities" => "Công cụ",
        "system" => "Hệ thống",
        "development" | "developer-tools" => "Lập trình",
        "education" => "Giáo dục",
        "games" => "Trò chơi",
        "graphics" | "design" => "Đồ họa",
        "music" | "audio" => "Âm nhạc",
        "video" => "Video",
        "business" => "Kinh doanh",
        "other" => "Khác",
        other => {
            let mut chars = other.chars();
            return match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => "Khác".into(),
            };
        }
    }
    .into()
}

/// Bỏ dấu tiếng Việt và chuyển về chữ thường ("Ghi chú" → "ghi chu", "Đồ họa" → "do hoa").
pub fn fold(text: &str) -> String {
    const GROUPS: [(&str, char); 14] = [
        ("àáảãạăằắẳẵặâầấẩẫậ", 'a'),
        ("èéẻẽẹêềếểễệ", 'e'),
        ("ìíỉĩị", 'i'),
        ("òóỏõọôồốổỗộơờớởỡợ", 'o'),
        ("ùúủũụưừứửữự", 'u'),
        ("ỳýỷỹỵ", 'y'),
        ("đ", 'd'),
        ("ÀÁẢÃẠĂẰẮẲẴẶÂẦẤẨẪẬ", 'a'),
        ("ÈÉẺẼẸÊỀẾỂỄỆ", 'e'),
        ("ÌÍỈĨỊ", 'i'),
        ("ÒÓỎÕỌÔỒỐỔỖỘƠỜỚỞỠỢ", 'o'),
        ("ÙÚỦŨỤƯỪỨỬỮỰ", 'u'),
        ("ỲÝỶỸỴ", 'y'),
        ("Đ", 'd'),
    ];
    text.chars()
        .map(|c| {
            GROUPS
                .iter()
                .find(|(set, _)| set.contains(c))
                .map(|(_, base)| *base)
                .unwrap_or(c)
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn matches<T: Listed>(item: &T, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let info = item.info();
    let haystack = fold(&format!(
        "{} {} {} {} {}",
        item.title(),
        info.summary.as_deref().unwrap_or(""),
        info.developer.as_deref().unwrap_or(""),
        category_label(&category_of(info)),
        item.app_id(),
    ));
    words.iter().all(|w| haystack.contains(w.as_str()))
}

fn by_name<T: Listed>(a: &T, b: &T) -> Ordering {
    fold(a.title())
        .cmp(&fold(b.title()))
        .then_with(|| a.app_id().cmp(b.app_id()))
}

/// Mới nhất trước; không có thời điểm thì xếp sau, theo tên.
fn newest_first<T: Listed>(a: &T, b: &T, at: impl Fn(&T) -> Option<Timestamp>) -> Ordering {
    match (at(a), at(b)) {
        (Some(x), Some(y)) => y.cmp(&x).then_with(|| by_name(a, b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => by_name(a, b),
    }
}

/// Thứ tự hiển thị của `items` theo `query`.
pub fn arrange<'a, T: Listed>(items: &'a [T], query: &Query) -> Vec<&'a T> {
    let words: Vec<String> = fold(&query.text)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut out: Vec<&T> = items
        .iter()
        .filter(|i| {
            query
                .category
                .as_ref()
                .is_none_or(|c| category_of(i.info()) == *c)
        })
        .filter(|i| matches(*i, &words))
        .collect();
    out.sort_by(|a, b| match query.sort {
        SortKey::Name => by_name(*a, *b),
        SortKey::RecentlyOpened => newest_first(*a, *b, |x| x.last_opened()),
        SortKey::RecentlyInstalled => newest_first(*a, *b, |x| x.installed_at()),
        SortKey::Category => fold(&category_label(&category_of(a.info())))
            .cmp(&fold(&category_label(&category_of(b.info()))))
            .then_with(|| by_name(*a, *b)),
        SortKey::Size => b.size().cmp(&a.size()).then_with(|| by_name(*a, *b)),
    });
    out
}

/// Các nhóm có trong `items` (mã nhóm, tên hiển thị), theo tên hiển thị.
pub fn categories<T: Listed>(items: &[T]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for item in items {
        let code = category_of(item.info());
        if !out.iter().any(|(c, _)| *c == code) {
            let label = category_label(&code);
            out.push((code, label));
        }
    }
    out.sort_by_key(|(_, label)| fold(label));
    out
}

/// 1–2 chữ cái đầu của tên app, viết hoa ("Ghi chú" → "GC", "kibi" → "K", "eframe template" →
/// "ET").
pub fn monogram(title: &str) -> String {
    let words: Vec<&str> = title
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter(|w| w.chars().next().is_some_and(char::is_alphanumeric))
        .collect();
    let initials: String = words
        .iter()
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if initials.is_empty() {
        "?".into()
    } else {
        initials
    }
}

/// Bảng màu nền của biểu tượng: đủ tương phản với chữ trắng ở cả giao diện sáng và tối.
pub const TILE_COLORS: [[u8; 3]; 12] = [
    [0x2E, 0x6F, 0xD8],
    [0x1F, 0x8A, 0x70],
    [0x8E, 0x44, 0xAD],
    [0xC0, 0x39, 0x2B],
    [0xD3, 0x54, 0x00],
    [0x16, 0x7D, 0x9E],
    [0x6D, 0x4C, 0x41],
    [0x2C, 0x3E, 0x50],
    [0xAD, 0x1A, 0x72],
    [0x3D, 0x7A, 0x1E],
    [0x5B, 0x4C, 0xC4],
    [0x9E, 0x5B, 0x00],
];

/// Màu nền cố định của biểu tượng một app: băm FNV-1a của mã app.
pub fn tile_color(app_id: &str) -> [u8; 3] {
    let mut hash: u32 = 0x811c_9dc5;
    for b in app_id.bytes() {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    TILE_COLORS[(hash % TILE_COLORS.len() as u32) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::AppStatus;

    fn app(id: &str, title: &str, category: &str, opened: Option<i64>, installed: i64) -> MyApp {
        MyApp {
            app: id.into(),
            title: title.into(),
            version: "1.0.0".into(),
            previous_version: None,
            status: AppStatus::Ready,
            update: None,
            info: AppInfo {
                summary: Some(format!("Mô tả của {title}")),
                category: Some(category.into()),
                developer: Some("F-Eco".into()),
                ..AppInfo::default()
            },
            installed_at: Timestamp::from_second(installed).unwrap(),
            last_opened: opened.map(|s| Timestamp::from_second(s).unwrap()),
        }
    }

    fn library() -> Vec<MyApp> {
        vec![
            app("kibi", "kibi", "tools", None, 300),
            app("f-notes", "Ghi chú", "productivity", Some(2_000), 100),
            app("f-timer", "Bấm giờ", "productivity", Some(1_000), 200),
            app("bottom", "bottom", "system", None, 400),
        ]
    }

    fn titles(items: Vec<&MyApp>) -> Vec<&str> {
        items.iter().map(|a| a.title.as_str()).collect()
    }

    /// Scenario "Tìm không dấu".
    #[test]
    fn search_ignores_case_and_accents() {
        let apps = library();
        for text in ["ghi chu", "GHI CHÚ", "  ghi   chu ", "chú ghi"] {
            let q = Query {
                text: text.into(),
                ..Query::default()
            };
            assert_eq!(titles(arrange(&apps, &q)), ["Ghi chú"], "{text}");
        }
        // Tìm theo mô tả, nhóm và nhà phát triển.
        let q = |t: &str| Query {
            text: t.into(),
            ..Query::default()
        };
        assert_eq!(arrange(&apps, &q("nang suat")).len(), 2);
        assert_eq!(arrange(&apps, &q("f-eco")).len(), 4);
        assert_eq!(titles(arrange(&apps, &q("mo ta cua bottom"))), ["bottom"]);
        // Scenario "Không có kết quả".
        let none = q("khong co app nay");
        assert!(arrange(&apps, &none).is_empty());
        assert!(none.is_filtered());
    }

    /// Scenario "Sắp xếp theo lần mở gần nhất": mở gần nhất trước, chưa mở xếp cuối theo tên.
    #[test]
    fn sort_by_recently_opened() {
        let apps = library();
        let q = Query {
            sort: SortKey::RecentlyOpened,
            ..Query::default()
        };
        assert_eq!(
            titles(arrange(&apps, &q)),
            ["Ghi chú", "Bấm giờ", "bottom", "kibi"]
        );
    }

    #[test]
    fn sort_by_name_installed_and_category() {
        let apps = library();
        let sorted = |sort| {
            titles(arrange(
                &apps,
                &Query {
                    sort,
                    ..Query::default()
                },
            ))
        };
        assert_eq!(
            sorted(SortKey::Name),
            ["Bấm giờ", "bottom", "Ghi chú", "kibi"]
        );
        assert_eq!(
            sorted(SortKey::RecentlyInstalled),
            ["bottom", "kibi", "Bấm giờ", "Ghi chú"]
        );
        // Công cụ < Hệ thống < Năng suất
        assert_eq!(
            sorted(SortKey::Category),
            ["kibi", "bottom", "Bấm giờ", "Ghi chú"]
        );
    }

    /// Scenario "Lọc theo nhóm".
    #[test]
    fn filter_by_category() {
        let apps = library();
        let q = Query {
            category: Some("productivity".into()),
            ..Query::default()
        };
        assert_eq!(titles(arrange(&apps, &q)), ["Bấm giờ", "Ghi chú"]);
        assert!(q.is_filtered());
        let cats = categories(&apps);
        assert_eq!(
            cats.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>(),
            ["Công cụ", "Hệ thống", "Năng suất"]
        );
    }

    #[test]
    fn store_sorts_by_size_and_missing_info_is_other() {
        let entry = |id: &str, length: u64, info: AppInfo| CatalogEntry {
            app: id.into(),
            title: id.into(),
            version: "1.0.0".into(),
            platform: "windows-x64".into(),
            target: format!("{id}.zip"),
            length,
            sha256: String::new(),
            entry: "a.exe".into(),
            client_id: None,
            private: false,
            info,
        };
        let items = vec![
            entry("small", 10, AppInfo::default()),
            entry("big", 1_000, AppInfo::default()),
        ];
        let q = Query {
            sort: SortKey::Size,
            ..Query::default()
        };
        let order: Vec<&str> = arrange(&items, &q).iter().map(|e| e.app.as_str()).collect();
        assert_eq!(order, ["big", "small"]);
        assert_eq!(categories(&items), [("other".into(), "Khác".into())]);
        assert_eq!(category_label("Âm nhạc"), "Âm nhạc");
        assert_eq!(category_label("trò chơi"), "Trò chơi");
    }

    /// Scenario "Biểu tượng ổn định".
    #[test]
    fn monogram_and_color_are_stable() {
        assert_eq!(monogram("Ghi chú"), "GC");
        assert_eq!(monogram("kibi"), "K");
        assert_eq!(monogram("eframe template"), "ET");
        assert_eq!(monogram("đồng hồ bấm giờ"), "ĐH");
        assert_eq!(monogram("  "), "?");
        assert_eq!(tile_color("f-notes"), tile_color("f-notes"));
        assert!(TILE_COLORS.contains(&tile_color("kibi")));
        // Các app khác nhau thường khác màu.
        let colors: std::collections::HashSet<_> = [
            "f-notes",
            "f-timer",
            "kibi",
            "bottom",
            "eframe-template",
            "feco-demo-app",
        ]
        .iter()
        .map(|a| tile_color(a))
        .collect();
        assert!(colors.len() >= 4, "{colors:?}");
    }

    #[test]
    fn fold_vietnamese() {
        assert_eq!(fold("Đồ họa Ứng dụng Ỷ Ì"), "do hoa ung dung y i");
        assert_eq!(fold("Bấm giờ"), "bam gio");
    }
}
