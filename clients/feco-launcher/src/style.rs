//! Phong cách giao diện của launcher (spec launcher của thay đổi launcher-personalization,
//! "Phong cách giao diện", "Phong cách đọc được và không sao chép tác phẩm"; design P2, P3).
//!
//! Dữ liệu thuần: bảng màu, bo góc, dải họa tiết. `main.rs` chuyển thành `egui::Style`.
//! Phong cách có bảng màu riêng (Giấy da, Bích họa, Trời sao, Hồ sen, Lập thể, Hoa huệ, Phố cổ,
//! Sơn mài) chỉ là bảng màu do F-Eco tự chọn; không dùng hình ảnh hay bản sao tác phẩm nào. Tên
//! là tên phổ thông (thay đổi platform-roles); mã cũ của bản 1.2 vẫn đọc được (serde alias).

use crate::i18n::t;
use std::sync::atomic::{AtomicU8, Ordering};

/// Dữ liệu bảng màu nằm ở `feco_core::ui_palette` (dùng chung với trang web).
pub use feco_core::ui_palette::{
    contrast, luminance, Palette, Rgb, Shape, StyleId as UiStyle, BASE_DARK, BASE_LIGHT,
};

/// Tên và mô tả (đã dịch) của phong cách trong launcher.
pub trait StyleText {
    fn label(self) -> &'static str;
    fn description(self) -> &'static str;
}

impl StyleText for UiStyle {
    /// Tên hiển thị (đã dịch): tên phổ thông, không dùng tên người (thay đổi platform-roles, R6).
    fn label(self) -> &'static str {
        match self {
            UiStyle::Plain => t("Chân phương"),
            UiStyle::Elegant => t("Trang nhã"),
            UiStyle::Parchment => t("Giấy da"),
            UiStyle::Fresco => t("Bích họa"),
            UiStyle::StarryNight => t("Trời sao"),
            UiStyle::LotusPond => t("Hồ sen"),
            UiStyle::Cubist => t("Lập thể"),
            UiStyle::Lily => t("Hoa huệ"),
            UiStyle::OldQuarter => t("Phố cổ"),
            UiStyle::Lacquer => t("Sơn mài"),
        }
    }

    /// Một dòng mô tả (đã dịch): cảm hứng của phong cách.
    fn description(self) -> &'static str {
        match self {
            UiStyle::Plain => t("Phẳng, góc gần vuông; theo chủ đề và màu nhấn bạn chọn"),
            UiStyle::Elegant => {
                t("Nút và thẻ bo tròn, thoáng hơn; theo chủ đề và màu nhấn bạn chọn")
            }
            UiStyle::Parchment => {
                t("Cảm hứng từ bảng màu bản phác thảo: giấy da, nâu sepia, đất nung")
            }
            UiStyle::Fresco => t("Cảm hứng từ bảng màu bích họa: vữa sáng, xanh lam đá, đỏ đất"),
            UiStyle::StarryNight => {
                t("Cảm hứng từ bảng màu trời đêm: xanh thẫm, vàng crôm, xanh coban")
            }
            UiStyle::LotusPond => {
                t("Cảm hứng từ bảng màu mặt nước: lục nhạt, tím oải hương, hồng phấn")
            }
            UiStyle::Cubist => t("Cảm hứng từ tranh lập thể: xanh lam, góc vuông, viền đậm"),
            UiStyle::Lily => t("Cảm hứng từ bảng màu sơn dầu: trắng ngà, lục nhạt, vàng hoa huệ"),
            UiStyle::OldQuarter => t("Cảm hứng từ bảng màu phố cũ: vàng đất, xám ngói, nâu son"),
            UiStyle::Lacquer => t("Cảm hứng từ bảng màu sơn mài: đen then, son đỏ, vàng thếp"),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_have_distinct_translated_names() {
        let mut names = std::collections::HashSet::new();
        for s in UiStyle::ALL {
            assert!(names.insert(s.label()), "trùng tên {s:?}");
            assert!(!s.description().is_empty());
        }
    }
}
