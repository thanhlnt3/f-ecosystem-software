//! Tùy chỉnh giao diện web (spec platform-support, "Tùy chỉnh giao diện web"; thay đổi
//! web-ui-and-japanese, design W3–W4).
//!
//! - Cookie `feco_ui` (`theme=dark&accent=blue&style=starry_night&density=compact&size=110`)
//!   lưu lựa chọn trên trình duyệt; giá trị lạ về mặc định, cỡ chữ kẹp trong 90–125%.
//! - Middleware [`apply`] gắn thuộc tính `data-*` và `--scale` vào thẻ `<html` của mọi trang
//!   HTML, nên tùy chỉnh có hiệu lực từ lần vẽ đầu mà không cần JavaScript.
//! - `/ui.css` sinh từ bảng màu chung `feco_core::ui_palette` (cùng dữ liệu với F-Launcher).
//! - `/preferences` là trang chọn ngôn ngữ, chủ đề, màu nhấn, phong cách, mật độ, cỡ chữ.

use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

use askama::Template;
use axum::{
    body::{Body, Bytes},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use feco_core::ui_palette::{
    contrast, hex, on_color, Palette, Rgb, StyleId, ACCENTS, BASE_DARK, BASE_LIGHT,
};
use serde::Deserialize;

use crate::{csrf, i18n, i18n::Lang, views::Msg, AppState};

pub const COOKIE: &str = "feco_ui";
pub const SIZE_MIN: u8 = 90;
pub const SIZE_MAX: u8 = 125;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub const fn code(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    fn parse(s: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.code() == s)
    }
}

/// Lựa chọn giao diện của một trình duyệt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiPrefs {
    pub theme: Theme,
    /// Mã trong `ACCENTS`; `None` là màu F-Eco mặc định.
    pub accent: Option<&'static str>,
    pub style: StyleId,
    pub compact: bool,
    /// Cỡ chữ, phần trăm.
    pub size: u8,
}

impl Default for UiPrefs {
    fn default() -> Self {
        UiPrefs {
            theme: Theme::System,
            accent: None,
            style: StyleId::Plain,
            compact: false,
            size: 100,
        }
    }
}

fn accent_code(s: &str) -> Option<&'static str> {
    ACCENTS.iter().map(|(c, _)| *c).find(|c| *c == s)
}

impl UiPrefs {
    /// Đọc từ chuỗi `k=v&k=v`; khóa hoặc giá trị lạ bị bỏ qua.
    pub fn parse(value: &str) -> UiPrefs {
        let mut p = UiPrefs::default();
        for (k, v) in form_urlencoded::parse(value.as_bytes()) {
            match k.as_ref() {
                "theme" => p.theme = Theme::parse(&v).unwrap_or_default(),
                "accent" => p.accent = accent_code(&v),
                "style" => p.style = StyleId::parse(&v).unwrap_or_default(),
                "density" => p.compact = v == "compact",
                "size" => {
                    p.size = v
                        .parse::<u32>()
                        .map(|n| n.clamp(SIZE_MIN.into(), SIZE_MAX.into()) as u8)
                        .unwrap_or(100)
                }
                _ => {}
            }
        }
        p
    }

    /// Đọc từ tiêu đề `Cookie` của yêu cầu.
    pub fn from_headers(headers: &HeaderMap) -> UiPrefs {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|c| c.split(';'))
            .filter_map(|p| p.trim().split_once('='))
            .find(|(k, _)| *k == COOKIE)
            .map(|(_, v)| UiPrefs::parse(v))
            .unwrap_or_default()
    }

    /// Giá trị cookie (chỉ gồm ký tự hợp lệ trong cookie).
    pub fn to_cookie_value(&self) -> String {
        let mut s = format!(
            "theme={}&style={}&density={}&size={}",
            self.theme.code(),
            self.style.code(),
            if self.compact { "compact" } else { "normal" },
            self.size
        );
        if let Some(a) = self.accent {
            let _ = write!(s, "&accent={a}");
        }
        s
    }

    /// Thuộc tính gắn vào `<html`, bắt đầu bằng dấu cách. Mọi giá trị đến từ danh sách cố định.
    pub fn html_attrs(&self) -> String {
        let mut s = format!(
            r#" data-theme="{}" data-style="{}" data-density="{}""#,
            self.theme.code(),
            self.style.code(),
            if self.compact { "compact" } else { "normal" },
        );
        if let Some(a) = self.accent {
            let _ = write!(s, r#" data-accent="{a}""#);
        }
        if self.size != 100 {
            let _ = write!(s, r#" style="--scale:{:.2}""#, f32::from(self.size) / 100.0);
        }
        s
    }
}

/// Middleware: gắn thuộc tính giao diện vào thẻ `<html` của trang HTML.
pub async fn apply(req: Request, next: Next) -> Response {
    let prefs = UiPrefs::from_headers(req.headers());
    let res = next.run(req).await;
    let is_html = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if !is_html {
        return res;
    }
    let (mut parts, body) = res.into_parts();
    let bytes = match axum::body::to_bytes(body, 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "không đọc được trang để gắn tùy chỉnh giao diện");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let out = inject(&bytes, &prefs);
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(out))
}

fn inject(page: &Bytes, prefs: &UiPrefs) -> Bytes {
    let Some(i) = page.windows(5).position(|w| w == b"<html") else {
        return page.clone();
    };
    let mut out = Vec::with_capacity(page.len() + 128);
    out.extend_from_slice(&page[..i + 5]);
    out.extend_from_slice(prefs.html_attrs().as_bytes());
    out.extend_from_slice(&page[i + 5..]);
    Bytes::from(out)
}

// ---------------------------------------------------------------------------------------------
// CSS sinh từ bảng màu chung

fn vars(p: &Palette, ok: Rgb) -> String {
    format!(
        "--bg:{};--panel:{};--card:{};--fg:{};--muted:{};--line:{};--accent:{};--on-accent:{};--warn:{};--err:{};--ok:{};color-scheme:{};",
        hex(p.window),
        hex(p.panel),
        hex(p.card),
        hex(p.text),
        hex(p.weak),
        hex(p.stroke),
        hex(p.accent),
        hex(p.on_accent),
        hex(p.warn),
        hex(p.danger),
        hex(ok),
        if p.dark { "dark" } else { "light" },
    )
}

/// Màu báo thành công: xanh lá nếu đọc rõ trên nền của bảng màu, không thì màu chữ.
fn ok_color(p: &Palette) -> Rgb {
    let green = if p.dark {
        [0x8F, 0xD1, 0x9E]
    } else {
        [0x1D, 0x6B, 0x2F]
    };
    if contrast(green, p.card) >= 4.5 && contrast(green, p.window) >= 4.5 {
        green
    } else {
        p.text
    }
}

/// Màu F-Eco mặc định (khi chưa chọn màu nhấn) trên nền sáng và tối.
const FECO_LIGHT: Rgb = [0x0F, 0x6B, 0x5C];
const FECO_DARK: Rgb = [0x4F, 0xB3, 0xA1];

fn base(dark: bool) -> Palette {
    let mut p = if dark { BASE_DARK } else { BASE_LIGHT };
    p.accent = if dark { FECO_DARK } else { FECO_LIGHT };
    p.on_accent = on_color(p.accent);
    p
}

/// Toàn bộ CSS biến theo phong cách (phục vụ ở `/ui.css`).
pub fn css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let light = base(false);
        let dark = base(true);
        let mut s = String::from("/* Sinh từ feco_core::ui_palette; không sửa tay. */\n");
        let _ = writeln!(s, ":root{{{}}}", vars(&light, ok_color(&light)));
        let _ = writeln!(
            s,
            "@media (prefers-color-scheme: dark){{:root:not([data-theme=light]){{{}}}}}",
            vars(&dark, ok_color(&dark))
        );
        let _ = writeln!(
            s,
            ":root[data-theme=dark]{{{}}}",
            vars(&dark, ok_color(&dark))
        );
        for (code, rgb) in ACCENTS {
            let _ = writeln!(
                s,
                ":root[data-accent={code}]{{--accent:{};--on-accent:{};}}",
                hex(rgb),
                hex(on_color(rgb))
            );
        }
        // Chấm màu ở trang Giao diện.
        let _ = writeln!(
            s,
            "[data-accent-dot=default]{{background:{};}}",
            hex(FECO_LIGHT)
        );
        for (code, rgb) in ACCENTS {
            let _ = writeln!(s, "[data-accent-dot={code}]{{background:{};}}", hex(rgb));
        }
        for style in StyleId::ALL {
            let shape = style.shape();
            let _ = write!(
                s,
                ":root[data-style={}]{{--radius:{}px;--card-radius:{}px;--stroke:{}px;",
                style.code(),
                shape.radius,
                shape.card_radius,
                shape.stroke
            );
            if let Some(p) = style.palette() {
                s.push_str(&vars(&p, ok_color(&p)));
            }
            if let Some(m) = style.motif() {
                let stops: Vec<String> = m.iter().map(|c| hex(*c)).collect();
                let _ = write!(s, "--motif:linear-gradient(90deg,{});", stops.join(","));
            }
            s.push_str("}\n");
        }
        s
    })
}

/// GET /ui.css
pub async fn stylesheet() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        css(),
    )
        .into_response()
}

// ---------------------------------------------------------------------------------------------
// Trang /preferences

pub struct Choice {
    pub code: &'static str,
    pub label: &'static str,
    pub selected: bool,
}

pub struct StyleChoice {
    pub code: &'static str,
    pub label: &'static str,
    pub selected: bool,
    /// Màu nền, thẻ, nhấn (ô xem trước); phong cách theo chủ đề dùng màu chân phương sáng.
    pub swatch: [String; 3],
    pub own_palette: bool,
}

#[derive(Template)]
#[template(path = "preferences.html")]
struct PrefsPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    back: String,
    languages: Vec<Choice>,
    themes: Vec<Choice>,
    accents: Vec<Choice>,
    styles: Vec<StyleChoice>,
    compact: bool,
    size: u8,
}

fn style_label(lang: Lang, s: StyleId) -> &'static str {
    lang.t(match s {
        StyleId::Plain => "prefs.style.plain",
        StyleId::Elegant => "prefs.style.elegant",
        StyleId::Parchment => "prefs.style.parchment",
        StyleId::Fresco => "prefs.style.fresco",
        StyleId::StarryNight => "prefs.style.starry_night",
        StyleId::LotusPond => "prefs.style.lotus_pond",
        StyleId::Cubist => "prefs.style.cubist",
        StyleId::Lily => "prefs.style.lily",
        StyleId::OldQuarter => "prefs.style.old_quarter",
        StyleId::Lacquer => "prefs.style.lacquer",
    })
}

fn accent_label(lang: Lang, code: &str) -> &'static str {
    lang.t(match code {
        "blue" => "prefs.accent.blue",
        "teal" => "prefs.accent.teal",
        "green" => "prefs.accent.green",
        "purple" => "prefs.accent.purple",
        "pink" => "prefs.accent.pink",
        "orange" => "prefs.accent.orange",
        _ => "prefs.accent.default",
    })
}

/// Đường dẫn nội bộ để quay lại: `next` nếu an toàn, không thì trang chủ.
pub fn safe_path(next: Option<&str>) -> String {
    next.filter(|n| n.starts_with('/') && !n.starts_with("//") && !n.contains('\\'))
        .filter(|n| !n.starts_with("/preferences"))
        .unwrap_or("/")
        .to_owned()
}

/// Đường dẫn của trang trước (tiêu đề Referer) nếu cùng nguồn với máy chủ.
pub fn referer_path(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let r = headers.get(header::REFERER)?.to_str().ok()?;
    let u = url::Url::parse(r).ok()?;
    if u.origin() != state.public_url.origin() {
        return None;
    }
    let mut p = u.path().to_owned();
    if let Some(q) = u.query() {
        p.push('?');
        p.push_str(q);
    }
    Some(p)
}

#[derive(Deserialize, Default)]
pub struct PrefsQuery {
    next: Option<String>,
    saved: Option<String>,
}

fn render(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    prefs: &UiPrefs,
    back: String,
    messages: Vec<Msg>,
) -> Response {
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    let mut accents = vec![Choice {
        code: "default",
        label: accent_label(lang, "default"),
        selected: prefs.accent.is_none(),
    }];
    accents.extend(ACCENTS.iter().map(|(c, _)| Choice {
        code: c,
        label: accent_label(lang, c),
        selected: prefs.accent == Some(*c),
    }));
    let page = PrefsPage {
        i18n: lang,
        messages,
        csrf: token,
        back,
        languages: Lang::ALL
            .iter()
            .map(|l| Choice {
                code: l.code(),
                label: l.native_name(),
                selected: *l == lang,
            })
            .collect(),
        themes: Theme::ALL
            .iter()
            .map(|t| Choice {
                code: t.code(),
                label: lang.t(match t {
                    Theme::System => "prefs.theme.system",
                    Theme::Light => "prefs.theme.light",
                    Theme::Dark => "prefs.theme.dark",
                }),
                selected: *t == prefs.theme,
            })
            .collect(),
        accents,
        styles: StyleId::ALL
            .iter()
            .map(|s| {
                let p = s.palette().unwrap_or_else(|| base(false));
                StyleChoice {
                    code: s.code(),
                    label: style_label(lang, *s),
                    selected: *s == prefs.style,
                    swatch: [hex(p.window), hex(p.card), hex(p.accent)],
                    own_palette: s.palette().is_some(),
                }
            })
            .collect(),
        compact: prefs.compact,
        size: prefs.size,
    };
    let mut res = match page.render() {
        Ok(h) => Html(h).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang");
            (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
        }
    };
    if let Some(c) = set {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

/// GET /preferences
pub async fn show(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<PrefsQuery>,
) -> Response {
    let back = safe_path(
        q.next
            .as_deref()
            .or(referer_path(&state, &headers).as_deref()),
    );
    let messages = if q.saved.is_some() {
        vec![Msg::success(lang.t("prefs.saved"))]
    } else {
        Vec::new()
    };
    render(
        &state,
        lang,
        &headers,
        &UiPrefs::from_headers(&headers),
        back,
        messages,
    )
}

/// POST /preferences: lưu cookie `feco_ui` (và `feco_lang`), rồi mở lại trang Giao diện.
pub async fn save(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    let get = |k: &str| form.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    if !csrf::verify(&headers, get(csrf::FIELD)) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let back = safe_path(get("next"));
    let secure = if state.public_url.scheme() == "https" {
        "; Secure"
    } else {
        ""
    };
    let (prefs, chosen_lang) = if get("reset").is_some() {
        (UiPrefs::default(), lang)
    } else {
        let mut raw = String::new();
        for k in ["theme", "accent", "style", "density", "size"] {
            if let Some(v) = get(k) {
                let _ = write!(raw, "{k}={}&", urlencode(v));
            }
        }
        (
            UiPrefs::parse(&raw),
            get("lang").and_then(Lang::parse).unwrap_or(lang),
        )
    };
    let ui = format!(
        "{COOKIE}={}; Path=/; Max-Age=31536000; SameSite=Lax{secure}",
        prefs.to_cookie_value()
    );
    let lc = format!(
        "{}={}; Path=/; Max-Age=31536000; SameSite=Lax{secure}",
        i18n::COOKIE,
        chosen_lang.code()
    );
    let to = format!("/preferences?saved=1&next={}", urlencode(&back));
    let mut res = Redirect::to(&to).into_response();
    for c in [ui, lc] {
        if let Ok(v) = HeaderValue::from_str(&c) {
            res.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    res
}

fn urlencode(s: &str) -> String {
    form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scenario "Cookie hỏng".
    #[test]
    fn broken_cookie_falls_back() {
        let p = UiPrefs::parse("style=khong-ton-tai&size=900");
        assert_eq!(p.style, StyleId::Plain);
        assert_eq!(p.size, 125);
        assert_eq!(UiPrefs::parse("size=-3&theme=xanh&accent=red").size, 100);
        assert_eq!(UiPrefs::parse("size=10").size, 90);
        assert_eq!(UiPrefs::parse("%%%"), UiPrefs::default());
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; feco_ui=theme=dark&style=starry_night&size=110"),
        );
        let p = UiPrefs::from_headers(&h);
        assert_eq!(
            (p.theme, p.style, p.size),
            (Theme::Dark, StyleId::StarryNight, 110)
        );
    }

    #[test]
    fn cookie_round_trip_and_attrs() {
        let p = UiPrefs {
            theme: Theme::Dark,
            accent: Some("pink"),
            style: StyleId::Lacquer,
            compact: true,
            size: 115,
        };
        assert_eq!(UiPrefs::parse(&p.to_cookie_value()), p);
        assert!(HeaderValue::from_str(&p.to_cookie_value()).is_ok());
        let a = p.html_attrs();
        assert!(a.contains(r#"data-style="lacquer""#), "{a}");
        assert!(a.contains(r#"data-theme="dark""#));
        assert!(a.contains(r#"data-accent="pink""#));
        assert!(a.contains("--scale:1.15"));
        // Mã cũ của bản 1.2 vẫn đọc được.
        assert_eq!(UiPrefs::parse("style=vangogh").style, StyleId::StarryNight);
    }

    #[test]
    fn injects_into_html_tag() {
        let page = Bytes::from_static(b"<!doctype html>\n<html lang=\"vi\"><body></body></html>");
        let p = UiPrefs {
            style: StyleId::StarryNight,
            ..UiPrefs::default()
        };
        let out = String::from_utf8(inject(&page, &p).to_vec()).unwrap();
        assert!(
            out.contains(r#"<html data-theme="system" data-style="starry_night""#),
            "{out}"
        );
        assert!(out.contains(r#" lang="vi">"#));
        let plain = Bytes::from_static(b"no html here");
        assert_eq!(inject(&plain, &p), plain);
    }

    /// Scenario "CSS khớp bảng màu": mỗi phong cách có bảng màu riêng mang đúng màu nền, chữ, nhấn.
    #[test]
    fn css_matches_the_shared_palette() {
        let css = css();
        for s in StyleId::ALL {
            let rule = format!(":root[data-style={}]{{", s.code());
            let start = css.find(&rule).unwrap_or_else(|| panic!("thiếu {rule}"));
            let body = &css[start..start + css[start..].find('}').unwrap()];
            if let Some(p) = s.palette() {
                for (var, c) in [
                    ("--bg", p.window),
                    ("--card", p.card),
                    ("--fg", p.text),
                    ("--muted", p.weak),
                    ("--accent", p.accent),
                    ("--on-accent", p.on_accent),
                ] {
                    assert!(
                        body.contains(&format!("{var}:{};", hex(c))),
                        "{}: {var}",
                        s.code()
                    );
                }
                assert!(body.contains("--motif:"));
            } else {
                assert!(!body.contains("--bg:"), "{} theo chủ đề", s.code());
            }
            assert!(body.contains(&format!("--radius:{}px;", s.shape().radius)));
        }
        assert!(css.contains(&format!("--bg:{};", hex(BASE_LIGHT.window))));
        assert!(css.contains(&format!("--bg:{};", hex(BASE_DARK.window))));
        for (code, rgb) in ACCENTS {
            assert!(css.contains(&format!(
                ":root[data-accent={code}]{{--accent:{};",
                hex(rgb)
            )));
        }
        // Màu thành công đọc rõ ở mọi bảng màu.
        for p in StyleId::ALL
            .iter()
            .filter_map(|s| s.palette())
            .chain([base(false), base(true)])
        {
            assert!(contrast(ok_color(&p), p.card) >= 4.5);
            assert!(contrast(p.on_accent, p.accent) >= 4.5);
        }
    }

    #[test]
    fn return_paths_stay_local() {
        assert_eq!(safe_path(Some("/store?q=a")), "/store?q=a");
        assert_eq!(safe_path(Some("//evil.example")), "/");
        assert_eq!(safe_path(Some("https://evil.example")), "/");
        assert_eq!(safe_path(Some("/\\evil")), "/");
        assert_eq!(safe_path(Some("/preferences")), "/");
        assert_eq!(safe_path(None), "/");
    }
}
