//! Phong cách giao diện của launcher (spec launcher của thay đổi launcher-personalization,
//! "Phong cách giao diện", "Phong cách đọc được và không sao chép tác phẩm"; design P2, P3).
//!
//! Dữ liệu thuần: bảng màu, bo góc, dải họa tiết. `main.rs` chuyển thành `egui::Style`.
//! Phong cách họa sĩ chỉ là bảng màu do F-Eco tự chọn, gợi lại cách dùng màu thường gặp trong
//! tranh của họa sĩ; không dùng hình ảnh hay bản sao tác phẩm nào.

use crate::i18n::t;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

pub type Rgb = [u8; 3];

/// Màu của một phong cách.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Nền tối (chữ sáng) hay nền sáng.
    pub dark: bool,
    pub window: Rgb,
    pub panel: Rgb,
    pub card: Rgb,
    pub text: Rgb,
    pub weak: Rgb,
    pub stroke: Rgb,
    pub accent: Rgb,
    /// Chữ trên nút dùng màu nhấn.
    pub on_accent: Rgb,
    pub warn: Rgb,
    pub danger: Rgb,
}

/// Hình khối của một phong cách.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    /// Bo góc của nút và ô nhập (điểm ảnh).
    pub radius: u8,
    /// Bo góc của thẻ app và khung.
    pub card_radius: u8,
    /// Độ dày viền thẻ.
    pub stroke: f32,
    /// Hệ số khoảng cách so với Chân phương.
    pub spacing: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiStyle {
    #[default]
    Plain,
    Elegant,
    Davinci,
    Michelangelo,
    Vangogh,
    Monet,
    Picasso,
    ToNgocVan,
    BuiXuanPhai,
    NguyenGiaTri,
}

/// Màu trạng thái của Chân phương và Trang nhã khi theo chủ đề sáng hoặc tối.
pub const BASE_LIGHT: Palette = Palette {
    dark: false,
    window: [0xF8, 0xF8, 0xF8],
    panel: [0xF8, 0xF8, 0xF8],
    card: [0xFC, 0xFC, 0xFC],
    text: [0x3C, 0x3C, 0x3C],
    weak: [0x6E, 0x6E, 0x6E],
    stroke: [0xBE, 0xBE, 0xBE],
    accent: [0x25, 0x63, 0xEB],
    on_accent: [0xFF, 0xFF, 0xFF],
    warn: [0x8A, 0x5A, 0x00],
    danger: [0xB4, 0x23, 0x18],
};

pub const BASE_DARK: Palette = Palette {
    dark: true,
    window: [0x1B, 0x1B, 0x1B],
    panel: [0x1B, 0x1B, 0x1B],
    card: [0x20, 0x20, 0x20],
    text: [0xDC, 0xDC, 0xDC],
    weak: [0x9A, 0x9A, 0x9A],
    stroke: [0x3C, 0x3C, 0x3C],
    accent: [0x25, 0x63, 0xEB],
    on_accent: [0xFF, 0xFF, 0xFF],
    warn: [0xF0, 0xB4, 0x4C],
    danger: [0xFF, 0x8A, 0x80],
};

impl UiStyle {
    pub const ALL: [UiStyle; 10] = [
        UiStyle::Plain,
        UiStyle::Elegant,
        UiStyle::Davinci,
        UiStyle::Michelangelo,
        UiStyle::Vangogh,
        UiStyle::Monet,
        UiStyle::Picasso,
        UiStyle::ToNgocVan,
        UiStyle::BuiXuanPhai,
        UiStyle::NguyenGiaTri,
    ];

    /// Tên hiển thị (đã dịch). Tên họa sĩ giữ nguyên.
    pub fn label(self) -> &'static str {
        match self {
            UiStyle::Plain => t("Chân phương"),
            UiStyle::Elegant => t("Trang nhã"),
            UiStyle::Davinci => "Leonardo da Vinci",
            UiStyle::Michelangelo => "Michelangelo",
            UiStyle::Vangogh => "Vincent van Gogh",
            UiStyle::Monet => "Claude Monet",
            UiStyle::Picasso => "Pablo Picasso",
            UiStyle::ToNgocVan => "Tô Ngọc Vân",
            UiStyle::BuiXuanPhai => "Bùi Xuân Phái",
            UiStyle::NguyenGiaTri => "Nguyễn Gia Trí",
        }
    }

    /// Một dòng mô tả (đã dịch): cảm hứng của phong cách.
    pub fn description(self) -> &'static str {
        match self {
            UiStyle::Plain => t("Phẳng, góc gần vuông; theo chủ đề và màu nhấn bạn chọn"),
            UiStyle::Elegant => {
                t("Nút và thẻ bo tròn, thoáng hơn; theo chủ đề và màu nhấn bạn chọn")
            }
            UiStyle::Davinci => {
                t("Cảm hứng từ bảng màu bản phác thảo: giấy da, nâu sepia, đất nung")
            }
            UiStyle::Michelangelo => {
                t("Cảm hứng từ bảng màu bích họa: vữa sáng, xanh lam đá, đỏ đất")
            }
            UiStyle::Vangogh => {
                t("Cảm hứng từ bảng màu trời đêm: xanh thẫm, vàng crôm, xanh coban")
            }
            UiStyle::Monet => {
                t("Cảm hứng từ bảng màu mặt nước: lục nhạt, tím oải hương, hồng phấn")
            }
            UiStyle::Picasso => {
                t("Cảm hứng từ thời kỳ Xanh và tranh lập thể: xanh lam, góc vuông, viền đậm")
            }
            UiStyle::ToNgocVan => {
                t("Cảm hứng từ bảng màu sơn dầu: trắng ngà, lục nhạt, vàng hoa huệ")
            }
            UiStyle::BuiXuanPhai => t("Cảm hứng từ bảng màu phố cũ: vàng đất, xám ngói, nâu son"),
            UiStyle::NguyenGiaTri => t("Cảm hứng từ bảng màu sơn mài: đen then, son đỏ, vàng thếp"),
        }
    }

    /// Phong cách theo chủ đề sáng/tối và màu nhấn của người dùng (Chân phương, Trang nhã).
    pub fn follows_theme(self) -> bool {
        matches!(self, UiStyle::Plain | UiStyle::Elegant)
    }

    /// Bảng màu cố định của phong cách họa sĩ; `None` với phong cách theo chủ đề.
    pub fn palette(self) -> Option<Palette> {
        let p = match self {
            UiStyle::Plain | UiStyle::Elegant => return None,
            UiStyle::Davinci => Palette {
                dark: false,
                window: [0xED, 0xE0, 0xC4],
                panel: [0xE3, 0xD2, 0xAE],
                card: [0xF6, 0xED, 0xD9],
                text: [0x3A, 0x28, 0x17],
                weak: [0x6E, 0x55, 0x39],
                stroke: [0xB3, 0x9A, 0x72],
                accent: [0x7A, 0x4A, 0x1E],
                on_accent: [0xFF, 0xF8, 0xEC],
                warn: [0x7A, 0x4E, 0x00],
                danger: [0x9E, 0x2A, 0x1C],
            },
            UiStyle::Michelangelo => Palette {
                dark: false,
                window: [0xF1, 0xE7, 0xD8],
                panel: [0xE6, 0xD6, 0xBF],
                card: [0xFA, 0xF4, 0xEA],
                text: [0x33, 0x29, 0x1F],
                weak: [0x6B, 0x5B, 0x49],
                stroke: [0xC2, 0xAB, 0x8E],
                accent: [0x2F, 0x5E, 0x8E],
                on_accent: [0xFF, 0xFF, 0xFF],
                warn: [0x7A, 0x4E, 0x00],
                danger: [0xA1, 0x2C, 0x1E],
            },
            UiStyle::Vangogh => Palette {
                dark: true,
                window: [0x0F, 0x1E, 0x3D],
                panel: [0x0B, 0x17, 0x30],
                card: [0x17, 0x30, 0x5C],
                text: [0xF4, 0xE9, 0xC1],
                weak: [0xB7, 0xB0, 0x8E],
                stroke: [0x2E, 0x4F, 0x8A],
                accent: [0xF2, 0xC2, 0x30],
                on_accent: [0x1A, 0x1A, 0x1A],
                warn: [0xFF, 0xC0, 0x70],
                danger: [0xFF, 0x9C, 0x90],
            },
            UiStyle::Monet => Palette {
                dark: false,
                window: [0xE7, 0xEF, 0xE8],
                panel: [0xD7, 0xE5, 0xDB],
                card: [0xF5, 0xF9, 0xF4],
                text: [0x26, 0x35, 0x2F],
                weak: [0x55, 0x6B, 0x62],
                stroke: [0xA9, 0xC4, 0xB5],
                accent: [0x5B, 0x4B, 0x9A],
                on_accent: [0xFF, 0xFF, 0xFF],
                warn: [0x6E, 0x4A, 0x00],
                danger: [0xA3, 0x26, 0x3B],
            },
            UiStyle::Picasso => Palette {
                dark: false,
                window: [0xE4, 0xEC, 0xF4],
                panel: [0xCA, 0xD8, 0xE8],
                card: [0xF4, 0xF7, 0xFB],
                text: [0x14, 0x23, 0x3A],
                weak: [0x44, 0x56, 0x6F],
                stroke: [0x1F, 0x3B, 0x63],
                accent: [0x1F, 0x4E, 0x8C],
                on_accent: [0xFF, 0xFF, 0xFF],
                warn: [0x6E, 0x4A, 0x00],
                danger: [0xB0, 0x18, 0x2E],
            },
            UiStyle::ToNgocVan => Palette {
                dark: false,
                window: [0xF5, 0xF2, 0xE8],
                panel: [0xEA, 0xE6, 0xD8],
                card: [0xFF, 0xFD, 0xF7],
                text: [0x2E, 0x3A, 0x2C],
                weak: [0x5E, 0x6B, 0x57],
                stroke: [0xCF, 0xC9, 0xB3],
                accent: [0x4F, 0x7A, 0x3E],
                on_accent: [0xFF, 0xFF, 0xFF],
                warn: [0x6E, 0x4A, 0x00],
                danger: [0xA3, 0x3A, 0x2A],
            },
            UiStyle::BuiXuanPhai => Palette {
                dark: false,
                window: [0xE8, 0xDF, 0xCF],
                panel: [0xD9, 0xCC, 0xB6],
                card: [0xF2, 0xEB, 0xDF],
                text: [0x34, 0x29, 0x1E],
                weak: [0x5E, 0x50, 0x43],
                stroke: [0x9C, 0x8E, 0x79],
                accent: [0x8E, 0x4A, 0x24],
                on_accent: [0xFF, 0xF8, 0xF0],
                warn: [0x6E, 0x46, 0x00],
                danger: [0x9E, 0x2A, 0x1C],
            },
            UiStyle::NguyenGiaTri => Palette {
                dark: true,
                window: [0x12, 0x0C, 0x09],
                panel: [0x0B, 0x07, 0x05],
                card: [0x22, 0x16, 0x0F],
                text: [0xEB, 0xD9, 0xAE],
                weak: [0xB4, 0x9A, 0x6C],
                stroke: [0x5A, 0x3B, 0x1F],
                accent: [0xB3, 0x30, 0x2A],
                on_accent: [0xFF, 0xF3, 0xDD],
                warn: [0xE0, 0xA9, 0x3B],
                danger: [0xFF, 0x7B, 0x6B],
            },
        };
        Some(p)
    }

    pub fn shape(self) -> Shape {
        let s = |radius, card_radius, stroke, spacing| Shape {
            radius,
            card_radius,
            stroke,
            spacing,
        };
        match self {
            UiStyle::Plain => s(3, 6, 1.0, 1.0),
            UiStyle::Elegant => s(14, 16, 1.0, 1.25),
            UiStyle::Davinci => s(4, 8, 1.0, 1.1),
            UiStyle::Michelangelo => s(6, 10, 1.0, 1.1),
            UiStyle::Vangogh => s(12, 14, 1.0, 1.15),
            UiStyle::Monet => s(16, 18, 1.0, 1.25),
            UiStyle::Picasso => s(0, 0, 1.5, 1.0),
            UiStyle::ToNgocVan => s(12, 16, 1.0, 1.2),
            UiStyle::BuiXuanPhai => s(3, 4, 1.2, 1.05),
            UiStyle::NguyenGiaTri => s(2, 4, 1.0, 1.1),
        }
    }

    /// Dải họa tiết 5 màu (thanh bên và ô xem trước); `None` với phong cách theo chủ đề.
    pub fn motif(self) -> Option<[Rgb; 5]> {
        Some(match self {
            UiStyle::Plain | UiStyle::Elegant => return None,
            UiStyle::Davinci => [
                [0xF6, 0xED, 0xD9],
                [0xD9, 0xC2, 0x9A],
                [0xA9, 0x81, 0x4F],
                [0x7A, 0x4A, 0x1E],
                [0x3A, 0x28, 0x17],
            ],
            UiStyle::Michelangelo => [
                [0xE8, 0xC9, 0xA6],
                [0xC2, 0x78, 0x5B],
                [0x8A, 0x9A, 0x5B],
                [0x2F, 0x5E, 0x8E],
                [0x5B, 0x7F, 0xB0],
            ],
            UiStyle::Vangogh => [
                [0x0B, 0x17, 0x30],
                [0x2E, 0x4F, 0x8A],
                [0x5D, 0x8A, 0xC9],
                [0xF2, 0xC2, 0x30],
                [0xF7, 0xE2, 0x7A],
            ],
            UiStyle::Monet => [
                [0xF3, 0xC6, 0xD3],
                [0xC9, 0xB8, 0xE8],
                [0x9C, 0xC9, 0xB6],
                [0x6F, 0xA3, 0xA8],
                [0x5B, 0x4B, 0x9A],
            ],
            UiStyle::Picasso => [
                [0x1F, 0x3B, 0x63],
                [0x3D, 0x6A, 0xA8],
                [0xE3, 0xB2, 0x3C],
                [0xC2, 0x45, 0x2D],
                [0x14, 0x23, 0x3A],
            ],
            UiStyle::ToNgocVan => [
                [0xFF, 0xFD, 0xF7],
                [0xF3, 0xE6, 0xB3],
                [0xCF, 0xE0, 0xC0],
                [0x8F, 0xB3, 0x7A],
                [0x4F, 0x7A, 0x3E],
            ],
            UiStyle::BuiXuanPhai => [
                [0xD8, 0xB6, 0x7A],
                [0xB9, 0xA5, 0x8A],
                [0x7D, 0x74, 0x68],
                [0x8E, 0x4A, 0x24],
                [0x3E, 0x3A, 0x36],
            ],
            UiStyle::NguyenGiaTri => [
                [0x0B, 0x07, 0x05],
                [0x5A, 0x1D, 0x14],
                [0xB3, 0x30, 0x2A],
                [0xC9, 0xA2, 0x4A],
                [0xEB, 0xD9, 0xAE],
            ],
        })
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Phong cách đang dùng (đặt khi áp cài đặt; các hàm vẽ đọc để lấy màu trạng thái, bo góc).
pub fn set_current(style: UiStyle) {
    let i = UiStyle::ALL.iter().position(|s| *s == style).unwrap_or(0);
    CURRENT.store(i as u8, Ordering::Relaxed);
}

pub fn current() -> UiStyle {
    UiStyle::ALL
        .get(CURRENT.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or_default()
}

/// Bảng màu dùng cho màu trạng thái: của phong cách họa sĩ, hoặc sáng/tối của chủ đề.
pub fn status_palette(dark_mode: bool) -> Palette {
    current()
        .palette()
        .unwrap_or(if dark_mode { BASE_DARK } else { BASE_LIGHT })
}

/// Độ sáng tương đối (WCAG 2.1).
pub fn luminance([r, g, b]: Rgb) -> f64 {
    let ch = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
}

/// Tỉ lệ tương phản giữa hai màu (1–21).
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palettes() -> Vec<(String, Palette)> {
        let mut out = vec![
            ("chân phương sáng".to_owned(), BASE_LIGHT),
            ("chân phương tối".to_owned(), BASE_DARK),
        ];
        for s in UiStyle::ALL {
            if let Some(p) = s.palette() {
                out.push((format!("{s:?}"), p));
            }
        }
        out
    }

    /// Scenario "Kiểm tra tương phản".
    #[test]
    fn every_style_is_readable() {
        for (name, p) in palettes() {
            for (bg_name, bg) in [
                ("cửa sổ", p.window),
                ("thẻ", p.card),
                ("thanh bên", p.panel),
            ] {
                let c = contrast(p.text, bg);
                assert!(c >= 4.5, "{name}: chữ trên {bg_name} chỉ {c:.2}");
                let c = contrast(p.weak, bg);
                assert!(c >= 3.0, "{name}: chữ mờ trên {bg_name} chỉ {c:.2}");
                for (what, color) in [("cảnh báo", p.warn), ("lỗi", p.danger)] {
                    let c = contrast(color, bg);
                    assert!(c >= 4.5, "{name}: màu {what} trên {bg_name} chỉ {c:.2}");
                }
            }
            let c = contrast(p.on_accent, p.accent);
            assert!(c >= 4.5, "{name}: chữ trên nút nhấn chỉ {c:.2}");
            assert_eq!(p.dark, luminance(p.window) < 0.2, "{name}: cờ nền tối sai");
        }
    }

    #[test]
    fn styles_are_distinct_and_described() {
        let mut names = std::collections::HashSet::new();
        for s in UiStyle::ALL {
            assert!(names.insert(s.label()), "trùng tên {s:?}");
            assert!(!s.description().is_empty());
            assert_eq!(s.follows_theme(), s.palette().is_none());
            assert_eq!(s.motif().is_some(), s.palette().is_some());
        }
        assert!(UiStyle::Elegant.shape().radius > UiStyle::Plain.shape().radius);
        assert_eq!(UiStyle::Picasso.shape().radius, 0);
        assert_eq!(UiStyle::default(), UiStyle::Plain);
        // Tên lưu trong settings.json.
        assert_eq!(
            serde_json::to_string(&UiStyle::ToNgocVan).unwrap(),
            "\"to_ngoc_van\""
        );
        assert_eq!(
            serde_json::to_string(&UiStyle::Vangogh).unwrap(),
            "\"vangogh\""
        );
    }

    #[test]
    fn contrast_math() {
        assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 0.01);
        assert!((contrast([7, 7, 7], [7, 7, 7]) - 1.0).abs() < 1e-9);
    }
}
