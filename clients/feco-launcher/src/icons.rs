//! Biểu tượng vẽ bằng `egui::Painter` (thay đổi launcher-windows-and-web-profile, design L1).
//!
//! Không dùng phông biểu tượng hay ảnh: mọi icon là nét vẽ đơn giản theo màu chữ hiện hành, nên
//! đổi phong cách hay chủ đề là icon đổi màu theo, và không phát sinh giấy phép tài sản nào. Mọi nút
//! chỉ có icon đều kèm tooltip (đã dịch) để người dùng biết nút làm gì.

use eframe::egui::{
    self, pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Library,
    Store,
    Bell,
    Gear,
    Grid,
    List,
    Sync,
    SignIn,
    SignOut,
    Close,
    ChevronLeft,
    ChevronRight,
    Download,
    Play,
    More,
    Code,
    Globe,
    Shield,
    Trash,
    Undo,
    ArrowUp,
    /// Ví F-Coin.
    Wallet,
    /// Đồng F-Coin (nút Mua).
    Coin,
}

#[cfg(test)]
impl Icon {
    pub const ALL: [Icon; 23] = [
        Icon::Wallet,
        Icon::Coin,
        Icon::Library,
        Icon::Store,
        Icon::Bell,
        Icon::Gear,
        Icon::Grid,
        Icon::List,
        Icon::Sync,
        Icon::SignIn,
        Icon::SignOut,
        Icon::Close,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::Download,
        Icon::Play,
        Icon::More,
        Icon::Code,
        Icon::Globe,
        Icon::Shield,
        Icon::Trash,
        Icon::Undo,
        Icon::ArrowUp,
    ];
}

/// Điểm trong hệ tọa độ 0..1 của ô vẽ.
fn at(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + r.width() * x, r.top() + r.height() * y)
}

fn line(p: &Painter, r: Rect, s: Stroke, pts: &[(f32, f32)]) {
    let pts: Vec<Pos2> = pts.iter().map(|(x, y)| at(r, *x, *y)).collect();
    p.add(Shape::line(pts, s));
}

fn arc(p: &Painter, c: Pos2, radius: f32, from: f32, to: f32, s: Stroke) {
    let n = 20;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * radius
        })
        .collect();
    p.add(Shape::line(pts, s));
}

/// Vẽ `icon` trong hình vuông `rect` với màu `color`.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let r = Rect::from_center_size(
        rect.center(),
        egui::Vec2::splat(rect.width().min(rect.height())),
    );
    let w = (r.width() / 11.0).max(1.4);
    let s = Stroke::new(w, color);
    use std::f32::consts::{FRAC_PI_2, PI, TAU};
    match icon {
        Icon::Library => {
            // Ba gáy sách và một cuốn nghiêng.
            for x in [0.2, 0.38] {
                p.rect_stroke(
                    Rect::from_min_max(at(r, x, 0.18), at(r, x + 0.13, 0.84)),
                    1.0,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
            line(
                p,
                r,
                s,
                &[
                    (0.6, 0.24),
                    (0.74, 0.2),
                    (0.88, 0.8),
                    (0.74, 0.84),
                    (0.6, 0.24),
                ],
            );
        }
        Icon::Store => {
            // Túi mua hàng.
            line(
                p,
                r,
                s,
                &[
                    (0.18, 0.36),
                    (0.82, 0.36),
                    (0.76, 0.86),
                    (0.24, 0.86),
                    (0.18, 0.36),
                ],
            );
            arc(p, at(r, 0.5, 0.36), r.width() * 0.17, PI, TAU, s);
        }
        Icon::Bell => {
            arc(p, at(r, 0.5, 0.46), r.width() * 0.24, PI, TAU, s);
            line(
                p,
                r,
                s,
                &[
                    (0.26, 0.46),
                    (0.26, 0.66),
                    (0.16, 0.74),
                    (0.84, 0.74),
                    (0.74, 0.66),
                    (0.74, 0.46),
                ],
            );
            line(p, r, s, &[(0.44, 0.84), (0.56, 0.84)]);
        }
        Icon::Gear => {
            let c = r.center();
            let outer = r.width() * 0.4;
            let inner = r.width() * 0.29;
            let teeth = 8;
            let mut pts = Vec::new();
            for i in 0..teeth * 4 {
                let a = i as f32 / (teeth * 4) as f32 * TAU;
                let rad = if (i / 2) % 2 == 0 { outer } else { inner };
                pts.push(c + vec2(a.cos(), a.sin()) * rad);
            }
            pts.push(pts[0]);
            p.add(Shape::line(pts, s));
            p.circle_stroke(c, r.width() * 0.12, s);
        }
        Icon::Grid => {
            // Bốn ô vuông tô đặc: ở cỡ nhỏ, ô chỉ có viền bo góc trông như hình tròn.
            for (x, y) in [(0.14, 0.14), (0.56, 0.14), (0.14, 0.56), (0.56, 0.56)] {
                p.rect_filled(
                    Rect::from_min_max(at(r, x, y), at(r, x + 0.3, y + 0.3)),
                    1.0,
                    color,
                );
            }
        }
        Icon::List => {
            for y in [0.24, 0.5, 0.76] {
                p.circle_filled(at(r, 0.2, y), w * 0.9, color);
                line(p, r, s, &[(0.36, y), (0.86, y)]);
            }
        }
        Icon::Sync => {
            let c = r.center();
            let rad = r.width() * 0.32;
            arc(p, c, rad, -PI * 0.9, -PI * 0.1, s);
            arc(p, c, rad, PI * 0.1, PI * 0.9, s);
            // Đầu mũi tên.
            let tip = c + vec2((-PI * 0.1).cos(), (-PI * 0.1).sin()) * rad;
            p.add(Shape::line(
                vec![
                    tip + vec2(-w * 2.6, -w * 0.6),
                    tip,
                    tip + vec2(-w * 0.2, -w * 2.6),
                ],
                s,
            ));
            let tip = c + vec2((PI * 0.9).cos(), (PI * 0.9).sin()) * rad;
            p.add(Shape::line(
                vec![
                    tip + vec2(w * 2.6, w * 0.6),
                    tip,
                    tip + vec2(w * 0.2, w * 2.6),
                ],
                s,
            ));
        }
        Icon::SignIn | Icon::SignOut => {
            // Khung cửa bên phải, mũi tên vào (đăng nhập) hoặc ra (đăng xuất).
            line(
                p,
                r,
                s,
                &[(0.52, 0.16), (0.84, 0.16), (0.84, 0.84), (0.52, 0.84)],
            );
            if icon == Icon::SignIn {
                line(p, r, s, &[(0.14, 0.5), (0.64, 0.5)]);
                line(p, r, s, &[(0.48, 0.34), (0.64, 0.5), (0.48, 0.66)]);
            } else {
                line(p, r, s, &[(0.66, 0.5), (0.14, 0.5)]);
                line(p, r, s, &[(0.3, 0.34), (0.14, 0.5), (0.3, 0.66)]);
            }
        }
        Icon::Close => {
            line(p, r, s, &[(0.24, 0.24), (0.76, 0.76)]);
            line(p, r, s, &[(0.76, 0.24), (0.24, 0.76)]);
        }
        Icon::ChevronLeft => line(p, r, s, &[(0.62, 0.2), (0.32, 0.5), (0.62, 0.8)]),
        Icon::ChevronRight => line(p, r, s, &[(0.38, 0.2), (0.68, 0.5), (0.38, 0.8)]),
        Icon::Download => {
            line(p, r, s, &[(0.5, 0.14), (0.5, 0.62)]);
            line(p, r, s, &[(0.3, 0.44), (0.5, 0.64), (0.7, 0.44)]);
            line(
                p,
                r,
                s,
                &[(0.18, 0.66), (0.18, 0.84), (0.82, 0.84), (0.82, 0.66)],
            );
        }
        Icon::Wallet => {
            // Thân ví và nắp cài có chấm tròn.
            p.rect_stroke(
                Rect::from_min_max(at(r, 0.14, 0.26), at(r, 0.86, 0.82)),
                2.0,
                s,
                egui::StrokeKind::Middle,
            );
            p.rect_stroke(
                Rect::from_min_max(at(r, 0.56, 0.44), at(r, 0.86, 0.64)),
                1.0,
                s,
                egui::StrokeKind::Middle,
            );
            p.circle_filled(at(r, 0.68, 0.54), r.width() * 0.035, color);
            line(p, r, s, &[(0.22, 0.26), (0.66, 0.14), (0.72, 0.26)]);
        }
        Icon::Coin => {
            // Đồng xu: vòng ngoài, vòng trong và chữ F.
            p.circle_stroke(at(r, 0.5, 0.5), r.width() * 0.36, s);
            p.circle_stroke(
                at(r, 0.5, 0.5),
                r.width() * 0.25,
                Stroke::new(w * 0.6, color),
            );
            line(p, r, s, &[(0.56, 0.37), (0.45, 0.37), (0.45, 0.64)]);
            line(p, r, s, &[(0.45, 0.5), (0.54, 0.5)]);
        }
        Icon::ArrowUp => {
            line(p, r, s, &[(0.5, 0.84), (0.5, 0.2)]);
            line(p, r, s, &[(0.28, 0.42), (0.5, 0.2), (0.72, 0.42)]);
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![at(r, 0.3, 0.18), at(r, 0.82, 0.5), at(r, 0.3, 0.82)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::More => {
            for x in [0.24, 0.5, 0.76] {
                p.circle_filled(at(r, x, 0.5), w * 1.1, color);
            }
        }
        Icon::Code => {
            line(p, r, s, &[(0.32, 0.26), (0.1, 0.5), (0.32, 0.74)]);
            line(p, r, s, &[(0.68, 0.26), (0.9, 0.5), (0.68, 0.74)]);
            line(p, r, s, &[(0.58, 0.18), (0.42, 0.82)]);
        }
        Icon::Globe => {
            let c = r.center();
            let rad = r.width() * 0.38;
            p.circle_stroke(c, rad, s);
            line(p, r, s, &[(0.12, 0.5), (0.88, 0.5)]);
            // Kinh tuyến: một hình elip hẹp.
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = i as f32 / 24.0 * TAU;
                    c + vec2(a.cos() * rad * 0.42, a.sin() * rad)
                })
                .collect();
            p.add(Shape::line(pts, s));
        }
        Icon::Shield => {
            line(
                p,
                r,
                s,
                &[
                    (0.5, 0.12),
                    (0.82, 0.24),
                    (0.78, 0.56),
                    (0.5, 0.88),
                    (0.22, 0.56),
                    (0.18, 0.24),
                    (0.5, 0.12),
                ],
            );
            line(p, r, s, &[(0.36, 0.5), (0.47, 0.62), (0.66, 0.38)]);
        }
        Icon::Trash => {
            line(p, r, s, &[(0.16, 0.26), (0.84, 0.26)]);
            line(
                p,
                r,
                s,
                &[(0.4, 0.26), (0.4, 0.16), (0.6, 0.16), (0.6, 0.26)],
            );
            line(
                p,
                r,
                s,
                &[(0.24, 0.26), (0.3, 0.86), (0.7, 0.86), (0.76, 0.26)],
            );
        }
        Icon::Undo => {
            let c = at(r, 0.52, 0.56);
            arc(p, c, r.width() * 0.28, PI, TAU + FRAC_PI_2 * 0.6, s);
            let tip = c + vec2(-r.width() * 0.28, 0.0);
            p.add(Shape::line(
                vec![
                    tip + vec2(-w * 2.0, -w * 2.4),
                    tip,
                    tip + vec2(w * 2.4, -w * 1.6),
                ],
                s,
            ));
        }
    }
}

/// Nút chỉ có icon, cỡ `size`, kèm tooltip. `selected` tô nền như mục đang chọn.
pub fn button(ui: &mut Ui, icon: Icon, tooltip: &str, selected: bool, enabled: bool) -> Response {
    let size = ui.spacing().interact_size.y.max(28.0);
    let response = ui
        .add_enabled(
            enabled,
            egui::Button::selectable(selected, "").min_size(vec2(size, size)),
        )
        .on_hover_text(tooltip)
        .on_disabled_hover_text(tooltip);
    let color = if enabled {
        ui.style()
            .interact_selectable(&response, selected)
            .fg_stroke
            .color
    } else {
        ui.visuals().weak_text_color()
    };
    let icon_rect = Rect::from_center_size(response.rect.center(), vec2(size * 0.6, size * 0.6));
    paint(ui.painter(), icon_rect, icon, color);
    response
}

/// Nút hành động chính: nền `fill`, icon và chữ màu `fg`.
pub fn filled(
    ui: &mut Ui,
    icon: Icon,
    text: &str,
    fill: Color32,
    fg: Color32,
    enabled: bool,
) -> Response {
    let h = ui.spacing().interact_size.y.max(28.0);
    let icon_side = h * 0.5;
    let galley = egui::WidgetText::from(egui::RichText::new(text).color(fg)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let width = 10.0 + icon_side + 6.0 + galley.size().x + 12.0;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, h), sense);
    if ui.is_rect_visible(rect) {
        let radius = ui.visuals().widgets.inactive.corner_radius;
        let mut bg = fill;
        let mut color = fg;
        if !enabled {
            bg = bg.gamma_multiply(0.45);
            color = color.gamma_multiply(0.7);
        } else if response.hovered() {
            bg = bg.gamma_multiply(0.88);
        }
        ui.painter().rect_filled(rect, radius, bg);
        let icon_rect = Rect::from_center_size(
            pos2(rect.left() + 10.0 + icon_side / 2.0, rect.center().y),
            vec2(icon_side, icon_side),
        );
        paint(ui.painter(), icon_rect, icon, color);
        let pos = pos2(
            icon_rect.right() + 6.0,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(pos, galley, color);
    }
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// Nút có icon và chữ (mục ở thanh bên, nút hành động chính).
pub fn labeled(
    ui: &mut Ui,
    icon: Icon,
    text: impl Into<egui::WidgetText>,
    selected: bool,
    min_width: f32,
) -> Response {
    let h = ui.spacing().interact_size.y.max(30.0);
    let icon_side = h * 0.55;
    let pad = icon_side + 14.0;
    let text: egui::WidgetText = text.into();
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        min_width - pad - 8.0,
        egui::TextStyle::Button,
    );
    let width = min_width.max(pad + galley.size().x + 12.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        if selected || response.hovered() || response.has_focus() {
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        let icon_rect = Rect::from_center_size(
            pos2(rect.left() + 8.0 + icon_side / 2.0, rect.center().y),
            vec2(icon_side, icon_side),
        );
        paint(ui.painter(), icon_rect, icon, visuals.fg_stroke.color);
        let text_pos = pos2(rect.left() + pad, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(text_pos, galley, visuals.text_color());
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mọi icon vẽ được, tạo hình trong ô và theo đúng màu được truyền vào.
    #[test]
    fn every_icon_paints_inside_its_box() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(24.0, 24.0));
        for icon in Icon::ALL {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                paint(ui.painter(), rect, icon, Color32::RED)
            });
            out.textures_delta.clear();
            assert!(!out.shapes.is_empty(), "{icon:?} không vẽ gì");
            for clipped in &out.shapes {
                let bounds = clipped.shape.visual_bounding_rect();
                assert!(
                    rect.expand(3.0).contains_rect(bounds),
                    "{icon:?} vẽ ra ngoài ô: {bounds:?}"
                );
            }
        }
    }
}
