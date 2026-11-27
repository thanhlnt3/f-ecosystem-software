//! Design system: two themes (Studio dark, Studio Light), colour tokens, radii and type.
//!
//! The look follows F-Launcher (change f-builder-feco-look): the launcher's light and dark
//! palettes (a pale blue-white canvas with a white sidebar, or deep navy), its "Elegant" corners,
//! the system's own interface font (Segoe UI on Windows), and main buttons filled with F-Eco's
//! colour. Light is the default, as in the launcher.
//! Widgets read [`Tokens::get`] instead of hard-coding colours, so a theme switch applies everywhere.
//! Confidence has its own fixed palette so "how sure are we" reads the same in both themes.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};
use f_builder_engine::evidence::Confidence;
use serde::{Deserialize, Serialize};
use skrifa::MetadataProvider;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeKind {
    Studio,
    /// F-Launcher's default theme is light, so F-Builder's is too.
    #[default]
    StudioLight,
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 2] = [ThemeKind::Studio, ThemeKind::StudioLight];

    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Studio => "Studio (dark)",
            ThemeKind::StudioLight => "Studio Light",
        }
    }

    /// The value stored in `settings.theme`.
    pub fn id(self) -> &'static str {
        match self {
            ThemeKind::Studio => "studio",
            ThemeKind::StudioLight => "studio-light",
        }
    }

    pub fn from_id(s: &str) -> ThemeKind {
        match s.trim().to_ascii_lowercase().replace(['_', ' '], "-").as_str() {
            "studio" | "dark" => ThemeKind::Studio,
            _ => ThemeKind::StudioLight,
        }
    }
}

/// Colour and shape tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub kind: ThemeKind,
    /// Title bar, rails, panels.
    pub chrome: Color32,
    /// The main reading surface behind cards.
    pub canvas: Color32,
    pub card: Color32,
    pub card_border: Color32,
    /// A card a step above `card` (hovered cards, chips).
    pub raised: Color32,
    pub field: Color32,
    pub field_border: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub icon: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    /// Indigo that reads as text on the theme's surfaces.
    pub accent_ink: Color32,
    /// Indigo wash for soft highlights.
    pub accent_soft: Color32,
    /// Text drawn on an `accent` fill.
    pub on_accent: Color32,
    pub separator: Color32,
    pub shadow: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub success: Color32,
    pub code_bg: Color32,
    pub radius_sm: f32,
    pub radius: f32,
    pub radius_lg: f32,
}

/// The app colour: F-Eco's accent (#0f6b5c), as on the F-Eco web pages. White on it passes WCAG
/// AA; on the bright accent (#14957f, used for hover) it doesn't.
pub const BRAND: Color32 = Color32::from_rgb(0x0f, 0x6b, 0x5c);

pub const SEEN_IT: Color32 = Color32::from_rgb(0x3f, 0xb2, 0x7f);
pub const WORKED_OUT: Color32 = Color32::from_rgb(0x5b, 0x8d, 0xef);
pub const BEST_GUESS: Color32 = Color32::from_rgb(0xe0, 0xa2, 0x3a);
pub const UNKNOWN: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x93);

/// The colour for a confidence level (`None` = unknown).
pub fn confidence_color(c: Option<Confidence>) -> Color32 {
    match c {
        Some(Confidence::Observed) => SEEN_IT,
        Some(Confidence::Derived) => WORKED_OUT,
        Some(Confidence::Inferred) => BEST_GUESS,
        None => UNKNOWN,
    }
}

pub fn confidence_label(c: Option<Confidence>) -> &'static str {
    c.map(Confidence::plain_label).unwrap_or("Unknown")
}

impl Tokens {
    pub fn for_kind(kind: ThemeKind) -> Self {
        // Corners of F-Launcher's default "Elegant" style (`feco_core::ui_palette`, `StyleId::Elegant`:
        // 14 for buttons, 16 for cards); small controls a step tighter.
        let (radius_sm, radius, radius_lg) = (10.0, 14.0, 16.0);
        match kind {
            // Dark: F-Launcher's dark palette (`brand_palette` in clients/feco-launcher): window
            // #0d1422, panels #111a2b, cards #172236, text #ebf1f9, muted #a6b3c6, edges #2c3b50.
            ThemeKind::Studio => Tokens {
                kind,
                chrome: Color32::from_rgb(0x11, 0x1a, 0x2b),
                canvas: Color32::from_rgb(0x0d, 0x14, 0x22),
                card: Color32::from_rgb(0x17, 0x22, 0x36),
                card_border: Color32::from_rgb(0x2c, 0x3b, 0x50),
                raised: Color32::from_rgb(0x1e, 0x2b, 0x42),
                field: Color32::from_rgb(0x17, 0x22, 0x36),
                field_border: Color32::from_rgb(0x5c, 0x6c, 0x86),
                hover: Color32::from_rgb(0x1e, 0x2b, 0x42),
                pressed: Color32::from_rgb(0x26, 0x35, 0x50),
                text: Color32::from_rgb(0xeb, 0xf1, 0xf9),
                text_dim: Color32::from_rgb(0xc6, 0xd0, 0xde),
                text_faint: Color32::from_rgb(0xa6, 0xb3, 0xc6),
                icon: Color32::from_rgb(0xa6, 0xb3, 0xc6),
                accent: BRAND,
                accent_hover: Color32::from_rgb(0x14, 0x95, 0x7f),
                accent_ink: Color32::from_rgb(0x5f, 0xd3, 0xbd),
                accent_soft: Color32::from_rgba_unmultiplied(0x14, 0x95, 0x7f, 44),
                on_accent: Color32::WHITE,
                separator: Color32::from_rgb(0x22, 0x30, 0x46),
                shadow: Color32::from_black_alpha(120),
                danger: Color32::from_rgb(0xff, 0x8b, 0x85),
                warning: BEST_GUESS,
                success: Color32::from_rgb(0x5b, 0xbf, 0x8f),
                code_bg: Color32::from_rgb(0x09, 0x0f, 0x1a),
                radius_sm,
                radius,
                radius_lg,
            },
            // Light: F-Launcher's light palette: window #f6f8fc, white panels and cards, text
            // #182234, muted #5b677a, edges #e0e6ef.
            ThemeKind::StudioLight => Tokens {
                kind,
                chrome: Color32::WHITE,
                canvas: Color32::from_rgb(0xf6, 0xf8, 0xfc),
                card: Color32::WHITE,
                card_border: Color32::from_rgb(0xe0, 0xe6, 0xef),
                raised: Color32::from_rgb(0xee, 0xf2, 0xf8),
                field: Color32::WHITE,
                field_border: Color32::from_rgb(0x7d, 0x89, 0x9c),
                hover: Color32::from_rgb(0xee, 0xf2, 0xf8),
                pressed: Color32::from_rgb(0xe2, 0xe8, 0xf1),
                text: Color32::from_rgb(0x18, 0x22, 0x34),
                text_dim: Color32::from_rgb(0x3b, 0x47, 0x5a),
                text_faint: Color32::from_rgb(0x5b, 0x67, 0x7a),
                icon: Color32::from_rgb(0x4d, 0x59, 0x6c),
                accent: BRAND,
                accent_hover: Color32::from_rgb(0x14, 0x95, 0x7f),
                accent_ink: BRAND,
                accent_soft: Color32::from_rgba_unmultiplied(0x14, 0x95, 0x7f, 28),
                on_accent: Color32::WHITE,
                separator: Color32::from_rgb(0xe6, 0xeb, 0xf2),
                shadow: Color32::from_black_alpha(20),
                danger: Color32::from_rgb(0xb2, 0x2a, 0x2a),
                warning: Color32::from_rgb(0x87, 0x4e, 0x00),
                success: Color32::from_rgb(0x12, 0x75, 0x36),
                code_bg: Color32::from_rgb(0xee, 0xf2, 0xf8),
                radius_sm,
                radius,
                radius_lg,
            },
        }
    }

    /// The tokens of the active theme.
    pub fn get(ctx: &egui::Context) -> Tokens {
        ctx.data(|d| d.get_temp::<Tokens>(tokens_id())).unwrap_or_else(|| Tokens::for_kind(ThemeKind::default()))
    }

    pub fn dark(&self) -> bool {
        self.kind == ThemeKind::Studio
    }

    pub fn r_sm(&self) -> CornerRadius {
        CornerRadius::same(self.radius_sm as u8)
    }
    pub fn r(&self) -> CornerRadius {
        CornerRadius::same(self.radius as u8)
    }
    pub fn r_lg(&self) -> CornerRadius {
        CornerRadius::same(self.radius_lg as u8)
    }
}

fn tokens_id() -> egui::Id {
    egui::Id::new("f-builder-theme")
}

// ---- fonts -------------------------------------------------------------------------------------

pub static DM_SANS_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/DMSans-Regular.ttf");
pub static DM_SANS_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/DMSans-Medium.ttf");
pub static DM_SANS_BOLD: &[u8] = include_bytes!("../../../assets/fonts/DMSans-Bold.ttf");
pub static JETBRAINS_MONO: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

/// A font from the optional f-builder-fonts build input (empty unless built with `F_BUILDER_FONTS_DIR`).
pub struct BuilderFont {
    pub family: &'static str,
    pub style: &'static str,
    /// ISO 15924 scripts the font is for, e.g. `"Jpan"`.
    pub scripts: &'static [&'static str],
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/f_builder_fonts.rs"));

/// Install the interface fonts: the system's own sans as F-Launcher uses it (body and labels in its
/// regular face, headings in its bold one), then DM Sans (three weights; it has no static SemiBold),
/// JetBrains Mono (code), plus any f-builder-fonts as fallbacks.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions(system_faces()));
}

/// Only the fonts built into the app (DM Sans, JetBrains Mono, f-builder-fonts), never the
/// system's: the same faces on every computer. UI tests use it, since their sizes and positions
/// must not depend on which fonts a machine has (DejaVu Sans on a Linux runner sets lines shorter
/// than Segoe UI on Windows). The system font itself is tested here, in `theme::tests`.
pub fn install_bundled_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions(None));
}

/// The font set [`install_fonts`] installs, with `system` as the first face when there is one.
fn font_definitions(system: Option<&'static SystemFaces>) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add("DMSans", DM_SANS_REGULAR);
    add("DMSans-Medium", DM_SANS_MEDIUM);
    add("DMSans-Bold", DM_SANS_BOLD);
    add("JetBrainsMono", JETBRAINS_MONO);
    if let Some(s) = system {
        add(SYSTEM_UI, &s.regular);
        if let Some(bold) = &s.bold {
            add(SYSTEM_UI_BOLD, bold);
        }
    }
    let mut fallbacks = Vec::new();
    for f in F_BUILDER_FONTS {
        let name = format!("craft:{}:{}", f.family, f.style);
        fonts.font_data.insert(name.clone(), Arc::new(FontData::from_static(f.bytes)));
        if f.style.eq_ignore_ascii_case("regular") {
            fallbacks.push(name);
        }
    }
    // Sans first: a mincho/serif fallback makes Japanese and Chinese UI text look like prose.
    let fallbacks = sans_first(fallbacks);
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "DMSans".to_owned());
    if system.is_some() {
        proportional.insert(0, SYSTEM_UI.to_owned());
    }
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "JetBrainsMono".to_owned());
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(fam).or_default().extend(fallbacks.iter().cloned());
    }
    let base: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    // A system font that has no bold face keeps its regular one for headings: one face for every
    // letter of a heading reads better than a bolder DM Sans missing half of Vietnamese.
    let system_heading = system.map(|s| if s.bold.is_some() { SYSTEM_UI_BOLD } else { SYSTEM_UI });
    for (fam, first, primary) in [("medium", system.map(|_| SYSTEM_UI), "DMSans-Medium"), ("semibold", system_heading, "DMSans-Bold")] {
        let mut stack: Vec<String> = first.into_iter().chain([primary]).map(str::to_owned).collect();
        let rest: Vec<String> = base.iter().filter(|n| !stack.contains(n)).cloned().collect();
        stack.extend(rest);
        fonts.families.insert(FontFamily::Name(fam.into()), stack);
    }
    fonts
}

/// Font names of the system face in egui's font set.
const SYSTEM_UI: &str = "SystemUI";
const SYSTEM_UI_BOLD: &str = "SystemUI-Bold";

/// The system's interface font, read once: its regular face and, when the family has one, a bold.
pub struct SystemFaces {
    regular: Vec<u8>,
    bold: Option<Vec<u8>>,
    /// The file the regular face came from (for the log).
    pub source: PathBuf,
}

/// The system font [`install_fonts`] uses, or `None` when none on [`system_font_candidates`] is
/// there and usable. Read once per process (fonts are large, and install_fonts runs per window).
pub fn system_faces() -> Option<&'static SystemFaces> {
    static FACES: OnceLock<Option<SystemFaces>> = OnceLock::new();
    FACES
        .get_or_init(|| {
            let found = find_system_faces(&system_font_candidates());
            match &found {
                Some(f) => log::info!("interface font: {}", f.source.display()),
                None => log::info!("no system interface font with Vietnamese; using DM Sans"),
            }
            found
        })
        .as_ref()
}

/// F-Launcher's interface fonts (`install_fonts` in clients/feco-launcher/src/main.rs), in its
/// order, each with its family's bold face. `WINFONTS/` is the Windows fonts folder.
const SYSTEM_UI_FONTS: &[(&str, Option<&str>)] = &[
    ("WINFONTS/segoeui.ttf", Some("WINFONTS/seguisb.ttf")),
    ("WINFONTS/arial.ttf", Some("WINFONTS/arialbd.ttf")),
    ("WINFONTS/tahoma.ttf", Some("WINFONTS/tahomabd.ttf")),
    ("/System/Library/Fonts/Supplemental/Arial.ttf", Some("/System/Library/Fonts/Supplemental/Arial Bold.ttf")),
    ("/Library/Fonts/Arial Unicode.ttf", None),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", Some("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf")),
    ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", Some("/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf")),
    ("/usr/share/fonts/noto/NotoSans-Regular.ttf", Some("/usr/share/fonts/noto/NotoSans-SemiBold.ttf")),
];

/// A font file to try: its regular face and, maybe, its bold one.
struct FontCandidate {
    regular: PathBuf,
    bold: Option<PathBuf>,
}

/// `F_BUILDER_UI_FONT` (a `.ttf` the person names) first, then [`SYSTEM_UI_FONTS`].
fn system_font_candidates() -> Vec<FontCandidate> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("F_BUILDER_UI_FONT").filter(|p| !p.is_empty()) {
        out.push(FontCandidate { regular: PathBuf::from(p), bold: None });
    }
    let fonts_dir = std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from).join("Fonts");
    let path = |f: &str| f.strip_prefix("WINFONTS/").map_or_else(|| PathBuf::from(f), |name| fonts_dir.join(name));
    for &(regular, bold) in SYSTEM_UI_FONTS {
        out.push(FontCandidate { regular: path(regular), bold: bold.map(path) });
    }
    out
}

/// The first candidate whose regular face is a font egui can read and has every letter of
/// Vietnamese. A bold face that isn't is left out; a candidate that isn't is skipped, so a broken
/// or unsuitable file never stops the app (egui panics on a font it can't parse).
fn find_system_faces(candidates: &[FontCandidate]) -> Option<SystemFaces> {
    candidates.iter().find_map(|c| {
        let regular = read_face(&c.regular)?;
        let bold = c.bold.as_deref().and_then(read_face);
        Some(SystemFaces { regular, bold, source: c.regular.clone() })
    })
}

/// A font file's bytes, when it's small enough to keep, parses, and has [`VIETNAMESE`] and the
/// arrows the interface uses.
fn read_face(path: &std::path::Path) -> Option<Vec<u8>> {
    /// Arial Unicode is 23 MB; nothing an interface needs is bigger.
    const MAX_FONT_BYTES: u64 = 40 * 1024 * 1024;
    if std::fs::metadata(path).ok()?.len() > MAX_FONT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    covers(&bytes, VIETNAMESE).then_some(bytes)
}

/// Whether `bytes` is a font egui can read (face 0) with a glyph for every letter of `text`.
fn covers(bytes: &[u8], text: &str) -> bool {
    let Ok(font) = skrifa::FontRef::from_index(bytes, 0) else {
        return false;
    };
    let map = font.charmap();
    text.chars().filter(|c| !c.is_whitespace()).all(|c| map.map(c).is_some())
}

/// All 12 Vietnamese vowels with each of the six tones, and đ, in both cases, plus the arrows the
/// interface writes ("Settings → AI"): what a system face must have to be used.
const VIETNAMESE: &str = "aàáảãạăằắẳẵặâầấẩẫậeèéẻẽẹêềếểễệiìíỉĩịoòóỏõọôồốổỗộơờớởỡợuùúủũụưừứửữựyỳýỷỹỵđ\
AÀÁẢÃẠĂẰẮẲẴẶÂẦẤẨẪẬEÈÉẺẼẸÊỀẾỂỄỆIÌÍỈĨỊOÒÓỎÕỌÔỒỐỔỖỘƠỜỚỞỠỢUÙÚỦŨỤƯỪỨỬỮỰYỲÝỶỸỴĐ→←↑↓";

/// Fallback faces, most wanted first. A stable sort keeps the manifest's order inside a rank.
fn sans_first(fallbacks: Vec<String>) -> Vec<String> {
    let mut ranked: Vec<(FallbackRank, String)> = fallbacks.into_iter().map(|n| (fallback_rank(&n), n)).collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    ranked.into_iter().map(|(_, name)| name).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FallbackRank {
    /// A face made for the interface's language (BIZ UDPGothic for Japanese).
    Local,
    /// One pan-CJK sans.
    Cjk,
    /// Any other sans or gothic.
    Sans,
    /// Nothing known about it.
    Other,
    /// A book face: last, because interface text set in one reads like prose.
    Serif,
}

/// Which face a f-builder-fonts name looks like, from the family name: a face made for the interface's
/// language first, a book face last.
fn fallback_rank(name: &str) -> FallbackRank {
    let n = name.to_ascii_lowercase();
    if n.contains("udpgothic") {
        FallbackRank::Local
    } else if n.contains("sans cjk") {
        FallbackRank::Cjk
    } else if n.contains("gothic") || n.contains("sans") {
        FallbackRank::Sans
    } else if n.contains("mincho") || n.contains("serif") {
        FallbackRank::Serif
    } else {
        FallbackRank::Other
    }
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
/// The type scale: every text size in the interface is one of these.
/// Headings (`heading`, DM Sans bold), largest first: a screen's title, a step or a dialog's
/// title, a card's or a list item's title.
pub const TITLE: f32 = 26.0;
pub const SECTION: f32 = 18.0;
pub const CARD: f32 = 16.0;
/// Text: the line under a title (and Simple mode's main field), reading text, secondary text
/// and list rows, fine print (paths, counts, notes), and tags painted in tight spots.
pub const LEAD: f32 = 17.0;
pub const BODY: f32 = 15.0;
pub const SMALL: f32 = 14.0;
pub const CAPTION: f32 = 12.0;
pub const TINY: f32 = 11.0;

/// A heading: DM Sans bold, at one of the heading sizes above.
pub fn heading(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text).font(semibold(size))
}

/// `heading` laid out for painting, wrapped at `wrap`.
pub fn heading_galley(painter: &egui::Painter, text: impl Into<String>, size: f32, color: Color32, wrap: f32) -> Arc<egui::Galley> {
    painter.layout(text.into(), semibold(size), color, wrap)
}
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// `a` blended toward `b` by `t` (0..=1), channel by channel (alpha included).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

/// A short label (captions, buttons, the title bar) in sentence case. `size` is the old
/// uppercase-mono size; mixed-case DM Sans needs a point and a half more to read the same.
pub fn label_text(text: &str, size: f32) -> egui::RichText {
    egui::RichText::new(text).font(medium(size + 1.5))
}

/// Scroll bars that can always be seen and grabbed. egui's thin preset paints the thumb in the
/// button fill and its track in the field colour, which here are the same, and narrows it to 2 px
/// at rest, so it showed only on hover. The thumb is the text colour, see-through enough to stay
/// quiet but 3:1 against the surfaces it sits on (WCAG 1.4.11), 6 px at rest and 10 px on hover.
pub fn scroll_style() -> egui::style::ScrollStyle {
    egui::style::ScrollStyle {
        floating: true,
        bar_width: 10.0,
        floating_width: 6.0,
        floating_allocated_width: 8.0,
        foreground_color: true,
        dormant_background_opacity: 0.0,
        active_background_opacity: 0.5,
        interact_background_opacity: 0.8,
        dormant_handle_opacity: SCROLL_THUMB_AT_REST,
        active_handle_opacity: 0.65,
        interact_handle_opacity: 0.85,
        ..egui::style::ScrollStyle::solid()
    }
}

/// How opaque a scroll bar's thumb is at rest (over the text colour).
const SCROLL_THUMB_AT_REST: f32 = 0.5;

/// Apply a theme. `roomy` = Beginner mode (bigger type, more air).
pub fn apply(ctx: &egui::Context, kind: ThemeKind, roomy: bool) {
    let t = Tokens::for_kind(kind);
    ctx.data_mut(|d| d.insert_temp(tokens_id(), t));
    ctx.set_theme(if t.dark() { egui::Theme::Dark } else { egui::Theme::Light });
    let mut v = if t.dark() { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.chrome;
    v.window_fill = t.card;
    v.window_stroke = Stroke::new(1.0, t.card_border);
    v.extreme_bg_color = t.field;
    v.faint_bg_color = t.raised;
    v.code_bg_color = t.code_bg;
    v.override_text_color = Some(t.text);
    v.hyperlink_color = t.accent_ink;
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.danger;
    v.window_corner_radius = t.r_lg();
    v.menu_corner_radius = t.r();
    v.window_shadow = egui::Shadow { offset: [0, 10], blur: 28, spread: 0, color: t.shadow };
    v.popup_shadow = egui::Shadow { offset: [0, 4], blur: 14, spread: 0, color: t.shadow };
    v.text_edit_bg_color = Some(t.field);
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.handle_shape = egui::style::HandleShape::Circle;
    v.striped = false;
    v.indent_has_left_vline = false;
    let r = t.r_sm();
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.card;
    w.noninteractive.weak_bg_fill = t.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.separator);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.text_dim);
    w.noninteractive.corner_radius = r;
    // Buttons and combo boxes are hairline boxes on the surface (weak fill transparent at rest).
    for (wv, bg, weak, stroke) in [
        (&mut w.inactive, t.field, Color32::TRANSPARENT, t.field_border),
        (&mut w.hovered, t.hover, t.hover, t.text_faint),
        (&mut w.active, t.pressed, t.pressed, t.accent),
        (&mut w.open, t.hover, t.hover, t.field_border),
    ] {
        wv.bg_fill = bg;
        wv.weak_bg_fill = weak;
        wv.bg_stroke = Stroke::new(1.0, stroke);
        wv.fg_stroke = Stroke::new(1.0, t.text);
        wv.corner_radius = r;
        wv.expansion = 0.0;
    }
    ctx.set_visuals(v);
    // Simple mode is for anyone, including people who find small text hard to read.
    let body = if roomy { 16.0 } else { 13.0 };
    ctx.global_style_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(if roomy { 13.5 } else { 11.0 })),
            (TextStyle::Body, FontId::proportional(body)),
            (TextStyle::Button, FontId::proportional(body)),
            (TextStyle::Heading, semibold(if roomy { SECTION } else { CARD })),
            (TextStyle::Monospace, FontId::monospace(if roomy { 13.5 } else { 12.5 })),
        ]
        .into();
        s.spacing.item_spacing = if roomy { egui::vec2(10.0, 8.0) } else { egui::vec2(8.0, 5.0) };
        s.spacing.button_padding = if roomy { egui::vec2(12.0, 6.0) } else { egui::vec2(9.0, 4.0) };
        s.spacing.interact_size = egui::vec2(24.0, if roomy { 28.0 } else { 24.0 });
        s.spacing.menu_margin = egui::Margin::same(6);
        s.spacing.window_margin = egui::Margin::same(18);
        s.spacing.icon_width = 14.0;
        s.spacing.scroll = scroll_style();
        s.spacing.tooltip_width = 320.0;
        s.interaction.tooltip_delay = 0.3;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The type scale is the only place a size is written down. Screens outside the workbench
    /// use its names (`CAPTION`, `BODY`, …) or a literal of 12 pt or more; the workbench's dense
    /// lists may use `TINY`. This walks the source so a new screen can't quietly go smaller.
    #[test]
    fn no_literal_text_size_under_12_outside_the_workbench() {
        fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    rust_files(&path, out);
                } else if path.extension().is_some_and(|x| x == "rs") {
                    out.push(path);
                }
            }
        }

        // Markers whose next argument is a text size. `.size` on a `RichText` counts; the
        // `.size` of a spinner or a galley does not.
        let texts = ["label_text(", "heading(", "medium(", "semibold(", "mono(", "FontId::proportional(", "FontId::monospace("];

        let mut files = Vec::new();
        rust_files(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        assert!(files.len() > 10, "the source scan found only {} files", files.len());

        let mut checked = 0;
        for path in files {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            // `theme.rs` is the scale itself, and `workbench.rs` is the dense workbench.
            if name == "theme.rs" || name == "workbench.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                let mut markers: Vec<&str> = texts.iter().copied().filter(|m| code.contains(m)).collect();
                if code.contains("RichText") && code.contains(".size(") && !code.contains("Spinner") {
                    markers.push(".size(");
                }
                for marker in markers {
                    let mut from = 0;
                    while let Some(rel) = code[from..].find(marker) {
                        let at = from + rel;
                        let open = at + marker.len() - 1;
                        if let Some(call) = call_text(code, open) {
                            let arg = if marker == "label_text(" || marker == "heading(" { top_level_parts(call).last().copied().unwrap_or("") } else { call };
                            for size in size_literals(arg) {
                                assert!(
                                    size >= 12.0,
                                    "{}:{}: a literal text size of {size} pt is below 12; use the theme scale: {}",
                                    path.display(),
                                    n + 1,
                                    code
                                );
                            }
                            checked += 1;
                        }
                        from = at + marker.len();
                    }
                }
            }
        }
        assert!(checked > 40, "the source scan checked only {checked} text sizes");
    }

    /// The text between `open`'s parenthesis and its match, or `None` when it isn't closed on
    /// this line.
    fn call_text(line: &str, open: usize) -> Option<&str> {
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        for (i, c) in line.get(open..)?.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return line.get(open + 1..open + i);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Split at commas that aren't inside brackets, braces or a string.
    fn top_level_parts(s: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut start = 0;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        for (i, c) in s.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(&s[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        out.push(&s[start..]);
        out
    }

    /// The literals a size argument is written as: one bare number, or every number in an
    /// `if`/`else`. An expression such as `body.size + 2.0` isn't a size of its own and is left
    /// to the reviewer.
    fn size_literals(arg: &str) -> Vec<f64> {
        let arg = arg.trim();
        if arg.starts_with("if ") {
            return floats_outside_strings(arg);
        }
        let bare = arg.strip_suffix("f32").unwrap_or(arg).trim();
        bare.parse::<f64>().ok().into_iter().collect()
    }

    /// Float literals on a line, ignoring digits inside string literals (a version or a count).
    fn floats_outside_strings(line: &str) -> Vec<f64> {
        let mut out = Vec::new();
        let mut in_string = false;
        let mut escaped = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }
            if c == '"' {
                in_string = true;
                continue;
            }
            if c.is_ascii_digit() {
                let mut number = String::from(c);
                while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                    if let Some(d) = chars.next() {
                        number.push(d);
                    }
                }
                if chars.peek() == Some(&'.') {
                    number.push('.');
                    chars.next();
                    while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                        if let Some(d) = chars.next() {
                            number.push(d);
                        }
                    }
                    if let Ok(v) = number.parse::<f64>() {
                        out.push(v);
                    }
                }
            }
        }
        out
    }

    /// WCAG relative luminance of an opaque colour.
    fn luminance(c: Color32) -> f64 {
        let ch = |v: u8| {
            let s = f64::from(v) / 255.0;
            if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }

    /// A Japanese sans comes before a serif, so interface text is not set like a book page.
    #[test]
    fn fallback_fonts_are_sans_first() {
        let names = |list: &[&str]| list.iter().map(|f| format!("craft:{f}:Regular")).collect::<Vec<_>>();
        let ordered = sans_first(names(&["Shippori Mincho", "Noto Sans CJK SC", "BIZ UDPGothic", "NanumGothic", "Unknown Face"]));
        assert_eq!(ordered[0], "craft:BIZ UDPGothic:Regular");
        assert_eq!(ordered[1], "craft:Noto Sans CJK SC:Regular");
        assert_eq!(ordered[2], "craft:NanumGothic:Regular");
        assert_eq!(ordered[3], "craft:Unknown Face:Regular");
        assert_eq!(ordered[4], "craft:Shippori Mincho:Regular");
        assert_eq!(fallback_rank("craft:Unknown Face:Regular"), FallbackRank::Other);
        assert_eq!(fallback_rank("craft:BIZ UDMincho:Regular"), FallbackRank::Serif);
        assert_eq!(fallback_rank("craft:BIZ UDPGothic:Regular"), FallbackRank::Local);
    }

    /// Releases are built with f-builder-fonts (#19), and so is CI; a build without them has only
    /// DM Sans, and shows other scripts as boxes.
    #[test]
    fn with_craft_fonts_every_script_people_type_shows() {
        if F_BUILDER_FONTS.is_empty() {
            return;
        }
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        let texts = [
            ("Korean", "한국어 사용자"),
            ("Georgian", "ქართული ენა"),
            ("Chinese", "你好，世界"),
            ("Japanese", "こんにちは、カタカナ"),
            ("Thai", "ภาษาไทย"),
            ("Arabic", "مرحبا بالعالم"),
            ("Hebrew", "שלום עולם"),
            ("Hindi", "नमस्ते दुनिया"),
            // All 12 Vietnamese vowels with each of the six tones, and đ, in both cases (146 letters):
            // people type any of them, not just the ones the catalog uses.
            (
                "Vietnamese",
                "Tiếng Việt a à á ả ã ạ ă ằ ắ ẳ ẵ ặ â ầ ấ ẩ ẫ ậ e è é ẻ ẽ ẹ ê ề ế ể ễ ệ i ì í ỉ ĩ ị o ò ó ỏ õ ọ ô ồ ố ổ ỗ ộ ơ ờ ớ ở ỡ ợ u ù ú ủ ũ ụ ư ừ ứ ử ữ ự y ỳ ý ỷ ỹ ỵ đ A À Á Ả Ã Ạ Ă Ằ Ắ Ẳ Ẵ Ặ Â Ầ Ấ Ẩ Ẫ Ậ E È É Ẻ Ẽ Ẹ Ê Ề Ế Ể Ễ Ệ I Ì Í Ỉ Ĩ Ị O Ò Ó Ỏ Õ Ọ Ô Ồ Ố Ổ Ỗ Ộ Ơ Ờ Ớ Ở Ỡ Ợ U Ù Ú Ủ Ũ Ụ Ư Ừ Ứ Ử Ữ Ự Y Ỳ Ý Ỷ Ỹ Ỵ Đ",
            ),
            ("Russian", "Привет"),
            ("Greek", "Γειά σου"),
            ("arrows", "Settings → AI ← ↑ ↓"),
        ];
        for (script, text) in texts {
            for font in [FontId::proportional(14.0), FontId::monospace(14.0), medium(14.0), semibold(14.0)] {
                // egui's bundled Hack (monospace) draws Georgian itself, and it is also the face egui
                // takes as its "no glyph" marker (it has U+25FB), so `has_glyphs` reports boxes for
                // it. The f-builder-fonts face is checked through the other three families.
                if script == "Georgian" && font.family == FontFamily::Monospace {
                    continue;
                }
                assert!(ctx.fonts_mut(|f| f.has_glyphs(&font, text)), "{script} ({text}) shows as boxes in {font:?}");
            }
        }
    }

    #[test]
    fn a_scroll_thumb_at_rest_stands_out_from_every_surface_in_every_theme() {
        let style = scroll_style();
        assert!(style.floating_width >= 6.0 && style.foreground_color, "visible and wide enough to grab at rest");
        for k in ThemeKind::ALL {
            let t = Tokens::for_kind(k);
            for (name, surface) in [("chrome", t.chrome), ("canvas", t.canvas), ("card", t.card)] {
                // The thumb is the text colour at that opacity, over the surface.
                let a = f64::from(style.dormant_handle_opacity);
                let mix = |f: u8, b: u8| (a * f64::from(f) + (1.0 - a) * f64::from(b)).round() as u8;
                let thumb = Color32::from_rgb(mix(t.text.r(), surface.r()), mix(t.text.g(), surface.g()), mix(t.text.b(), surface.b()));
                let (l1, l2) = (luminance(thumb), luminance(surface));
                let ratio = (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05);
                assert!(ratio >= 3.0, "{k:?} {name}: the thumb at rest is only {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn theme_ids_round_trip() {
        for k in ThemeKind::ALL {
            assert_eq!(ThemeKind::from_id(k.id()), k);
        }
        assert_eq!(ThemeKind::from_id("dark"), ThemeKind::Studio);
        // Light, as in F-Launcher: the default, and what an unknown value means.
        assert_eq!(ThemeKind::default(), ThemeKind::StudioLight);
        assert_eq!(ThemeKind::from_id("nonsense"), ThemeKind::StudioLight);
        assert_eq!(ThemeKind::from_id(&f_builder_engine::settings::Settings::default().theme), ThemeKind::StudioLight);
    }

    fn asset(name: &str) -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts").join(name)
    }

    /// The bundled DM Sans lacks most of Vietnamese (ả ạ ơ ư…) and the arrows: why the interface
    /// takes the system's font first. If this starts failing, DM Sans gained them.
    #[test]
    fn dm_sans_alone_cannot_show_vietnamese() {
        assert!(!covers(DM_SANS_REGULAR, VIETNAMESE));
        assert!(covers(DM_SANS_REGULAR, "Settings"), "the check reads DM Sans itself");
    }

    /// Scenario "Phông chỉ định bị hỏng": a file that isn't a font, a missing file and a font
    /// without Vietnamese are passed over, and the next usable font is taken (JetBrains Mono here,
    /// which has every letter), with no panic.
    #[test]
    fn a_broken_or_unsuitable_font_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("not-a-font.ttf");
        std::fs::write(&junk, b"this is not a font file").unwrap();
        let candidates = [
            FontCandidate { regular: junk.clone(), bold: None },
            FontCandidate { regular: dir.path().join("missing.ttf"), bold: None },
            FontCandidate { regular: asset("DMSans-Regular.ttf"), bold: None },
            FontCandidate { regular: asset("JetBrainsMono-Regular.ttf"), bold: Some(junk) },
        ];
        let found = find_system_faces(&candidates).expect("JetBrains Mono has every letter");
        assert_eq!(found.source, asset("JetBrainsMono-Regular.ttf"));
        assert!(found.bold.is_none(), "a broken bold face is left out, not loaded");
        assert!(find_system_faces(&candidates[..3]).is_none());
    }

    /// Scenario "Chữ tiếng Việt đủ dấu": with a system font, every family starts with it, headings
    /// with its bold face, and that one face has every Vietnamese letter and arrow, so no letter
    /// falls through to another font or a box.
    #[test]
    fn vietnamese_comes_from_one_system_face() {
        let faces: &'static SystemFaces = Box::leak(Box::new(
            find_system_faces(&[FontCandidate { regular: asset("JetBrainsMono-Regular.ttf"), bold: Some(asset("JetBrainsMono-Regular.ttf")) }])
                .expect("a usable face"),
        ));
        let fonts = font_definitions(Some(faces));
        let first = |fam: FontFamily| fonts.families.get(&fam).and_then(|s| s.first()).cloned().unwrap_or_default();
        assert_eq!(first(FontFamily::Proportional), SYSTEM_UI);
        assert_eq!(first(FontFamily::Name("medium".into())), SYSTEM_UI);
        assert_eq!(first(FontFamily::Name("semibold".into())), SYSTEM_UI_BOLD);
        assert_eq!(first(FontFamily::Monospace), "JetBrainsMono");
        for name in [SYSTEM_UI, SYSTEM_UI_BOLD] {
            let data = fonts.font_data.get(name).expect("installed");
            assert!(covers(&data.font, VIETNAMESE), "{name}");
        }
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        assert!(ctx.fonts_mut(|f| f.has_glyphs(&FontId::proportional(14.0), "Cài đặt → AI: Tiếng Việt")));
    }

    /// Without a system font the set is what it was: DM Sans first, no system names.
    #[test]
    fn without_a_system_font_dm_sans_leads() {
        let fonts = font_definitions(None);
        assert_eq!(fonts.families.get(&FontFamily::Proportional).and_then(|s| s.first()).map(String::as_str), Some("DMSans"));
        assert_eq!(fonts.families.get(&FontFamily::Name("semibold".into())).and_then(|s| s.first()).map(String::as_str), Some("DMSans-Bold"));
        assert!(!fonts.font_data.contains_key(SYSTEM_UI));
    }

    /// On this computer, when F-Launcher's list finds a font: it is the one installed. Windows has
    /// Segoe UI, so there it must be found.
    #[test]
    fn the_system_font_is_found_where_the_launcher_finds_one() {
        let found = system_faces();
        if cfg!(windows) {
            let f = found.expect("Windows has Segoe UI, Arial or Tahoma");
            assert!(f.bold.is_some(), "and its bold face");
        }
        if let Some(f) = found {
            assert!(covers(&f.regular, VIETNAMESE));
        }
    }

    /// Main buttons: white on F-Eco's colour, and still AA when hovered or pressed (darker).
    #[test]
    fn main_buttons_keep_their_contrast_in_every_state() {
        use crate::widgets::{PRIMARY_HOVER, PRIMARY_PRESSED};
        for k in ThemeKind::ALL {
            let t = Tokens::for_kind(k);
            for amount in [0.0, PRIMARY_HOVER, PRIMARY_PRESSED] {
                let fill = mix(t.accent, Color32::BLACK, amount);
                assert!(contrast(t.on_accent, fill) >= 4.5, "{k:?} at {amount}: {:.2}", contrast(t.on_accent, fill));
            }
        }
    }

    /// WCAG 2.x contrast ratio between two opaque colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let s = v as f32 / 255.0;
                if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn contrast_meets_wcag_aa_in_both_themes() {
        for k in ThemeKind::ALL {
            let t = Tokens::for_kind(k);
            // `success` is text too: the Clean room and Ready pills.
            for (name, fg) in [("text", t.text), ("text_dim", t.text_dim), ("text_faint", t.text_faint), ("accent_ink", t.accent_ink), ("success", t.success)] {
                for bg in [t.canvas, t.card, t.chrome, t.raised] {
                    assert!(contrast(fg, bg) >= 4.5, "{k:?} {name} on {bg:?}: {:.2}", contrast(fg, bg));
                }
            }
            assert!(contrast(t.on_accent, t.accent) >= 4.5, "{k:?} text on the app colour: {:.2}", contrast(t.on_accent, t.accent));
            // Send's icon on its hover colour (an icon, so 3:1).
            assert!(contrast(t.on_accent, t.accent_hover) >= 3.0, "{k:?} icon on the hover colour: {:.2}", contrast(t.on_accent, t.accent_hover));
            assert!(contrast(t.field_border, t.canvas) >= 3.0, "{k:?} control borders: {:.2}", contrast(t.field_border, t.canvas));
            assert!(contrast(t.canvas, t.text) >= 4.5, "{k:?} text on the canvas");
            assert_eq!(t.accent, BRAND);
        }
    }

    #[test]
    fn confidence_has_four_distinct_colours() {
        let cs = [
            confidence_color(Some(Confidence::Observed)),
            confidence_color(Some(Confidence::Derived)),
            confidence_color(Some(Confidence::Inferred)),
            confidence_color(None),
        ];
        for (i, a) in cs.iter().enumerate() {
            for b in cs.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
        assert_eq!(confidence_label(None), "Unknown");
    }
}
