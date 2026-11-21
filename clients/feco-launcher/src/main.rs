//! Giao diện F-Launcher (egui). Mọi logic nằm trong thư viện `feco_launcher`; tệp này chỉ vẽ
//! giao diện và chạy các thao tác trên một runtime tokio riêng để giao diện không bị treo.
//!
//! Bố cục: thanh bên (Thư viện, Cửa hàng, Tin nhắn, Thông báo, Cài đặt, tài khoản) và vùng chính. Thư viện
//! và Cửa hàng xem theo lưới hoặc danh sách, có tìm, lọc nhóm, sắp xếp; cài đặt giao diện lưu
//! trong `settings.json` (spec launcher của thay đổi improve-launcher-experience).
//!
//! Cài đặt và trang chi tiết app mở trong cửa sổ riêng của hệ điều hành; nút công cụ dùng icon
//! kèm tooltip; ô lưới cùng kích thước (thay đổi launcher-windows-and-web-profile).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod icons;

use eframe::egui;
use feco_launcher::auth::{Auth, AuthError, KeyringStore, PurchaseError, Session, WalletInfo};
use feco_launcher::broker::{Broker, Issuer};
use feco_launcher::chat::{self, ChatError, Conversation, Due, Message, Thread};
use feco_launcher::i18n::{self, t, tf, Language};
use feco_launcher::launch::{command, LaunchEnv};
use feco_launcher::library::{self, SortKey};
use feco_launcher::prefs::{Accent, Layout, Prefs, StartPage, Theme, TileSize};
use feco_launcher::style::{self as ui_style, StyleText, UiStyle};
use feco_launcher::{
    AppStatus, CatalogEntry, Config, Launcher, LauncherError, MyApp, Paths, ISOLATION_NOTICE,
};
use icons::Icon;
use jiff::Timestamp;
use std::future::Future;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Đồng bộ lại mỗi giờ khi có mạng (thu hồi tới launcher trong 1 giờ), mỗi 5 phút khi ngoại tuyến.
const SYNC_ONLINE: Duration = Duration::from_secs(60 * 60);
const SYNC_OFFLINE: Duration = Duration::from_secs(5 * 60);

/// Thời điểm tiến trình bắt đầu, để đo thời gian khởi động (tasks.md 15.2).
static STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Logo của F-Launcher (assets/logos), nhúng trong tệp chạy.
const LAUNCHER_LOGO: &[u8] = include_bytes!("../../../assets/logos/f-launcher-256.png");

/// Ảnh PNG giải mã thành RGBA 8 bit: (rộng, cao, điểm ảnh). Ảnh lớn hơn
/// [`feco_tuf::repo::MAX_ICON_PX`] mỗi chiều bị bỏ qua (logo khai kích thước khổng lồ làm cạn bộ
/// nhớ khi khởi động).
fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    const MAX_PX: u32 = feco_tuf::repo::MAX_ICON_PX;
    let limits = png::Limits {
        bytes: 16 * 1024 * 1024,
    };
    let mut decoder = png::Decoder::new_with_limits(std::io::Cursor::new(bytes), limits);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = reader.info().size();
    if !(1..=MAX_PX).contains(&width) || !(1..=MAX_PX).contains(&height) {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let px = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px
            .chunks(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (rgba.len() == (info.width * info.height * 4) as usize).then_some((
        info.width,
        info.height,
        rgba,
    ))
}

/// Biểu tượng cửa sổ (thanh tác vụ, Alt+Tab) từ logo F-Launcher.
fn window_icon() -> Option<Arc<egui::IconData>> {
    let (width, height, rgba) = decode_png(LAUNCHER_LOGO)?;
    Some(Arc::new(egui::IconData {
        rgba,
        width,
        height,
    }))
}

/// Ảnh logo theo mã app (PNG). Logo app lấy từ metadata TUF đã ký khi đồng bộ.
fn icon_sources() -> &'static std::sync::Mutex<std::collections::HashMap<String, Arc<Vec<u8>>>> {
    static SOURCES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Arc<Vec<u8>>>>,
    > = std::sync::OnceLock::new();
    SOURCES.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        m.insert("f-launcher".to_owned(), Arc::new(LAUNCHER_LOGO.to_vec()));
        std::sync::Mutex::new(m)
    })
}

/// Ghi nhận logo của các app trong danh mục (base64 trong metadata TUF).
fn register_icons(entries: &[CatalogEntry]) {
    use base64::Engine;
    let mut sources = icon_sources().lock().expect("khóa");
    for e in entries {
        if sources.contains_key(&e.app) {
            continue;
        }
        if let Some(bytes) = e
            .info
            .icon
            .as_deref()
            .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok())
        {
            sources.insert(e.app.clone(), Arc::new(bytes));
        }
    }
}

/// Texture logo của app (giải mã một lần, giữ trong bộ nhớ của egui).
fn icon_texture(ctx: &egui::Context, app_id: &str) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique(("app-icon", app_id));
    if let Some(cached) = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return cached;
    }
    let bytes = icon_sources().lock().expect("khóa").get(app_id).cloned()?;
    let texture = decode_png(&bytes).map(|(w, h, rgba)| {
        let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        ctx.load_texture(
            format!("app-icon-{app_id}"),
            image,
            egui::TextureOptions::LINEAR,
        )
    });
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

/// Vị trí để cửa sổ cỡ `size` nằm giữa màn hình hiện tại (None nếu chưa biết cỡ màn hình).
fn centered_position(ctx: &egui::Context, size: egui::Vec2) -> Option<egui::Pos2> {
    let monitor = ctx.input(|i| i.viewport().monitor_size)?;
    centered_in(monitor, size)
}

fn centered_in(monitor: egui::Vec2, size: egui::Vec2) -> Option<egui::Pos2> {
    (monitor.x > 0.0 && monitor.y > 0.0).then(|| {
        egui::pos2(
            ((monitor.x - size.x) / 2.0).max(0.0).round(),
            ((monitor.y - size.y) / 2.0).max(0.0).round(),
        )
    })
}

fn main() -> eframe::Result {
    STARTED.get_or_init(Instant::now);
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("F-Launcher: {e}");
            std::process::exit(2);
        }
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("F-Launcher")
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([720.0, 480.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    // Cửa sổ chính xuất hiện ở giữa màn hình.
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "F-Launcher",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx)?;
            Ok(Box::new(LauncherApp::new(cc.egui_ctx.clone(), config)?))
        }),
    )
}

/// Dùng phông chữ có sẵn của hệ điều hành (có dấu tiếng Việt); không đóng gói phông chữ.
fn install_fonts(ctx: &egui::Context) -> Result<(), String> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("FECO_LAUNCHER_FONT") {
        candidates.push(p.into());
    }
    for name in ["segoeui.ttf", "arial.ttf", "tahoma.ttf"] {
        candidates.push(std::path::Path::new(&windir).join("Fonts").join(name));
    }
    for p in [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    ] {
        candidates.push(p.into());
    }
    let bytes = candidates
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .ok_or(t(
            "không tìm thấy phông chữ hệ thống; đặt FECO_LAUNCHER_FONT=<đường dẫn .ttf>",
        ))?;
    let mut fonts = egui::FontDefinitions::empty();
    fonts
        .font_data
        .insert("ui".into(), Arc::new(egui::FontData::from_owned(bytes)));
    let mut family: Vec<String> = vec!["ui".into()];
    // Chữ tiếng Nhật: phông của hệ điều hành làm phông dự phòng (spec "Tiếng Nhật trong launcher";
    // không đóng gói phông). Không có thì vẫn chạy, chữ Nhật hiện ô trống.
    if let Some(bytes) = japanese_font_candidates(&windir)
        .iter()
        .find_map(|p| std::fs::read(p).ok())
    {
        fonts
            .font_data
            .insert("ja".into(), Arc::new(egui::FontData::from_owned(bytes)));
        family.push("ja".into());
    }
    fonts
        .families
        .insert(egui::FontFamily::Proportional, family.clone());
    fonts.families.insert(egui::FontFamily::Monospace, family);
    ctx.set_fonts(fonts);
    Ok(())
}

/// Phông chữ tiếng Nhật có sẵn: Windows (Yu Gothic, Meiryo, MS Gothic), macOS (Hiragino),
/// Linux (Noto CJK, IPA). `FECO_LAUNCHER_JA_FONT` được ưu tiên.
fn japanese_font_candidates(windir: &str) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("FECO_LAUNCHER_JA_FONT") {
        out.push(p.into());
    }
    for name in ["YuGothM.ttc", "YuGothR.ttc", "meiryo.ttc", "msgothic.ttc"] {
        out.push(std::path::Path::new(windir).join("Fonts").join(name));
    }
    for p in [
        // ヒラギノ角ゴシック W3
        "/System/Library/Fonts/\u{30d2}\u{30e9}\u{30ae}\u{30ce}\u{89d2}\u{30b4}\u{30b7}\u{30c3}\u{30af} W3.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
    ] {
        out.push(p.into());
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SignIn {
    Unknown,
    SignedOut,
    SignedIn,
    /// Có phiên đã lưu nhưng chưa xác nhận được vì ngoại tuyến.
    Offline,
}

/// Phiên đăng nhập, dùng chung giữa giao diện và API cục bộ.
struct Identity {
    auth: Auth,
    session: tokio::sync::Mutex<Option<Session>>,
    sign_in: std::sync::Mutex<SignIn>,
}

impl Identity {
    fn set(&self, s: SignIn) {
        *self.sign_in.lock().expect("khóa") = s;
    }

    /// Access token còn hạn (tự làm mới bằng refresh token khi cần).
    async fn access_token(&self) -> Result<String, AuthError> {
        let mut session = self.session.lock().await;
        if let Some(s) = session.as_ref().filter(|s| s.is_fresh(Timestamp::now())) {
            return Ok(s.access_token.clone());
        }
        match self.auth.restore().await? {
            Some(s) => {
                let token = s.access_token.clone();
                *session = Some(s);
                self.set(SignIn::SignedIn);
                Ok(token)
            }
            None => {
                self.set(SignIn::SignedOut);
                Err(AuthError::Expired)
            }
        }
    }
}

struct Shared {
    /// Trung tâm thông báo (lấy từ máy chủ khi đồng bộ).
    notifications: std::sync::Mutex<Vec<feco_launcher::auth::Notification>>,
    /// Ví F-Coin của người đang đăng nhập (fcoin-wallet-and-marketplace).
    wallet: std::sync::Mutex<Option<WalletInfo>>,
    /// Giá F-Coin của app có giá (danh mục công khai).
    prices: std::sync::Mutex<std::collections::HashMap<String, i64>>,
    config: Config,
    launcher: tokio::sync::Mutex<Launcher>,
    identity: Arc<Identity>,
    broker: Broker,
    broker_url: String,
    /// Phiên bản người dùng đã "Bỏ qua bản này" (bản sao của `prefs.skipped_updates` cho việc chạy
    /// nền: cập nhật tất cả, tự cập nhật; thay đổi app-update-notices design U4).
    skipped: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
}

#[derive(Clone, Default)]
struct View {
    online: bool,
    my_apps: Vec<MyApp>,
    catalog: Vec<CatalogEntry>,
    auto_update: bool,
    user: Option<String>,
    /// Vai trò nền tảng (platform-roles).
    role: Option<feco_launcher::auth::AccountRole>,
    sign_in: Option<SignIn>,
    last_sync: Option<Timestamp>,
    notifications: Vec<feco_launcher::auth::Notification>,
    wallet: Option<WalletInfo>,
    prices: std::collections::HashMap<String, i64>,
}

async fn snapshot(shared: &Shared) -> View {
    let l = shared.launcher.lock().await;
    View {
        online: l.is_online(),
        my_apps: l.my_apps(),
        catalog: l.catalog_latest().into_iter().cloned().collect(),
        auto_update: l.state().auto_update,
        user: l.state().user_name.clone(),
        role: l.state().account_role.clone(),
        sign_in: Some(*shared.identity.sign_in.lock().expect("khóa")),
        last_sync: l.state().last_sync,
        notifications: shared.notifications.lock().expect("khóa").clone(),
        wallet: shared.wallet.lock().expect("khóa").clone(),
        prices: shared.prices.lock().expect("khóa").clone(),
    }
}

/// Nút chính cho app chưa cài (fcoin-wallet-and-marketplace, "Mua app trước khi cài").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Acquire {
    /// Miễn phí hoặc đã mua.
    Install,
    /// App có giá, chưa mua.
    Buy(i64),
    /// App có giá, chưa đăng nhập.
    SignInToBuy(i64),
}

fn acquire(view: &View, app: &str) -> Acquire {
    let signed_in = matches!(view.sign_in, Some(SignIn::SignedIn) | Some(SignIn::Offline));
    let price = view
        .wallet
        .as_ref()
        .map(|w| w.price(app))
        .filter(|p| *p > 0)
        .or_else(|| view.prices.get(app).copied())
        .unwrap_or(0);
    if price <= 0 {
        return Acquire::Install;
    }
    match &view.wallet {
        Some(w) if w.owns(app) => Acquire::Install,
        Some(_) if signed_in => Acquire::Buy(price),
        _ if signed_in => Acquire::Buy(price),
        _ => Acquire::SignInToBuy(price),
    }
}

/// Một bản cập nhật nên gợi ý cho người dùng.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateSuggestion {
    app: String,
    title: String,
    from: String,
    to: String,
    notes: Option<String>,
}

impl UpdateSuggestion {
    fn key(&self) -> String {
        format!("{}@{}", self.app, self.to)
    }
}

/// App đã cài có bản mới hơn trong danh mục, trừ phiên bản người dùng đã bỏ qua.
fn update_suggestions(
    apps: &[MyApp],
    skipped: &std::collections::BTreeMap<String, String>,
) -> Vec<UpdateSuggestion> {
    let mut out: Vec<UpdateSuggestion> = apps
        .iter()
        .filter_map(|a| {
            let u = a.update.as_ref()?;
            if skipped.get(&a.app) == Some(&u.version) {
                return None;
            }
            Some(UpdateSuggestion {
                app: a.app.clone(),
                title: a.title.clone(),
                from: a.version.clone(),
                to: u.version.clone(),
                notes: u.info.release_notes.clone(),
            })
        })
        .collect();
    out.sort_by_key(|a| a.title.to_lowercase());
    out
}

/// Số F-Coin còn thiếu để mua (None: đủ).
fn shortfall(balance: i64, price: i64) -> Option<i64> {
    (balance < price).then_some(price - balance)
}

/// "1.250 F-Coin" theo ngôn ngữ giao diện.
fn coins(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let sep = if i18n::language() == Language::Vi {
        '.'
    } else {
        ','
    };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(c);
    }
    format!("{}{out} F-Coin", if n < 0 { "-" } else { "" })
}

struct Done {
    result: Result<String, String>,
    view: View,
    synced: bool,
}

/// Kết quả một lời gọi `/v1/chat` chạy nền (không chặn giao diện, không chiếm `busy`).
enum ChatDone {
    Unread(Result<usize, ChatError>),
    Conversations(Result<Vec<Conversation>, ChatError>),
    /// Tin sau mốc `poll_after` của hội thoại (lần tải đầu: mọi tin).
    Messages {
        conversation: String,
        result: Result<Vec<Message>, ChatError>,
    },
    Sent {
        conversation: String,
        /// Nội dung đã gửi (trả lại ô soạn tin khi gửi không được).
        body: String,
        result: Result<Message, ChatError>,
    },
    Read {
        conversation: String,
        result: Result<(), ChatError>,
    },
}

/// Trang Tin nhắn (thay đổi chat-and-source-scan).
#[derive(Default)]
struct ChatState {
    /// Tổng tin chưa đọc (huy hiệu ở thanh bên).
    unread: usize,
    conversations: Vec<Conversation>,
    /// Đã tải danh sách ít nhất một lần.
    listed: bool,
    listing: bool,
    selected: Option<String>,
    thread: Option<Thread>,
    draft: String,
    sending: bool,
    error: Option<String>,
    schedule: chat::Schedule,
}

impl ChatState {
    fn selected_conversation(&self) -> Option<&Conversation> {
        let id = self.selected.as_deref()?;
        self.conversations.iter().find(|c| c.id == id)
    }

    fn has_data(&self) -> bool {
        self.unread > 0 || !self.conversations.is_empty() || self.selected.is_some()
    }

    /// Dòng cuối của hội thoại trong danh sách theo tin mới nhất trong luồng.
    fn touch(&mut self, conversation: &str) {
        let Some(last) = self
            .thread
            .as_ref()
            .filter(|t| t.conversation == conversation)
            .and_then(|t| t.messages.last())
        else {
            return;
        };
        let (body, at) = (last.body.chars().take(120).collect(), last.at.clone());
        if let Some(c) = self.conversations.iter_mut().find(|c| c.id == conversation) {
            c.last_message = Some(body);
            c.last_message_at = at;
        }
    }
}

enum Action {
    Login,
    Logout,
    Sync,
    AskInstall(Box<CatalogEntry>),
    Install(Box<CatalogEntry>),
    /// Hỏi xác nhận mua app có giá (giá F-Coin).
    AskBuy(Box<CatalogEntry>, i64),
    /// Mua bằng F-Coin rồi cài ngay.
    Buy(Box<CatalogEntry>, i64),
    /// Mở hộp thoại gợi ý cập nhật.
    ShowUpdates,
    /// Không gợi ý phiên bản này của app nữa.
    SkipUpdate(String, String),
    Update(String),
    UpdateAll,
    Rollback(String),
    AskUninstall(Box<MyApp>),
    Uninstall(String, bool),
    Open(String),
    OpenLink(String),
    SetAutoUpdate(bool),
    CloseDialog,
    /// Mở cửa sổ chi tiết của app (kèm thứ tự đang hiển thị để chuyển trước/sau).
    ShowDetail(Detail),
    /// Chuyển cửa sổ chi tiết `key` sang app khác trong cùng thứ tự.
    MoveDetail(u64, String),
    CloseDetail(u64),
    /// Mở (hoặc đưa lên trước) cửa sổ Cài đặt.
    OpenSettings,
    /// Thu gọn hoặc mở rộng thanh bên (lưu vào cài đặt).
    ToggleSidebar,
}

/// Độ rộng thanh bên đầy đủ và thu gọn; cửa sổ hẹp hơn [`NARROW_WINDOW`] thì thanh bên thu gọn.
const SIDEBAR_FULL: f32 = 210.0;
const SIDEBAR_COMPACT: f32 = 56.0;
const NARROW_WINDOW: f32 = 720.0;

/// Thanh bên đang thu gọn: người dùng đã chọn, hoặc cửa sổ hẹp (không đổi lựa chọn đã lưu).
fn sidebar_collapsed(saved: bool, window_width: f32) -> bool {
    saved || window_width < NARROW_WINDOW
}

fn sidebar_width(collapsed: bool) -> f32 {
    if collapsed {
        SIDEBAR_COMPACT
    } else {
        SIDEBAR_FULL
    }
}

/// Số cửa sổ chi tiết mở cùng lúc; mở thêm thì cửa sổ cũ nhất đóng lại.
const MAX_DETAIL_WINDOWS: usize = 6;

fn settings_viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("feco-settings")
}

struct LauncherApp {
    rt: tokio::runtime::Runtime,
    shared: Arc<Shared>,
    tx: mpsc::Sender<Done>,
    rx: mpsc::Receiver<Done>,
    ctx: egui::Context,
    view: View,
    page: Page,
    prefs: Prefs,
    prefs_path: std::path::PathBuf,
    library_filter: Filter,
    store_filter: Filter,
    /// Các cửa sổ chi tiết app đang mở.
    details: Vec<Detail>,
    next_detail_key: u64,
    settings_open: bool,
    /// Cửa sổ hiện hộp thoại xác nhận (cài, gỡ): cửa sổ nơi người dùng bấm nút.
    dialog_viewport: egui::ViewportId,
    busy: Option<String>,
    notice: Option<(bool, String)>,
    confirm_install: Option<CatalogEntry>,
    /// Vị trí giữa màn hình đã tính cho từng cửa sổ con.
    child_positions: std::collections::HashMap<egui::ViewportId, egui::Pos2>,
    /// Cửa sổ chính đang được focus (để chuyển focus sang cửa sổ con đang chặn).
    root_focused: bool,
    /// Hộp thoại gợi ý cập nhật đang mở.
    update_prompt: bool,
    /// Bản cập nhật đã gợi ý trong phiên này (`app@phiên bản`), để không hỏi lại mỗi lần đồng bộ.
    prompted: std::collections::BTreeSet<String>,
    /// Hộp thoại mua app (app, giá F-Coin).
    confirm_buy: Option<(CatalogEntry, i64)>,
    confirm_uninstall: Option<(MyApp, bool)>,
    next_sync: Instant,
    /// Đã vẽ khung hình đầu tiên.
    probed: bool,
    chat: ChatState,
    chat_tx: mpsc::Sender<ChatDone>,
    chat_rx: mpsc::Receiver<ChatDone>,
}

impl LauncherApp {
    fn new(ctx: egui::Context, config: Config) -> Result<Self, String> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let paths = Paths::new(&config.data_dir);
        let prefs_path = paths.settings_file();
        let (prefs, prefs_note) = Prefs::load(&prefs_path);
        if let Some(note) = &prefs_note {
            eprintln!("{note}");
        }
        apply_prefs(&ctx, &prefs);
        let tuf = feco_tuf::Client::new(
            std::fs::read(&config.trusted_root).map_err(|e| {
                tf(
                    "không đọc được root.json tin cậy {}: {}",
                    &[&config.trusted_root.display(), &e],
                )
            })?,
            config.tuf_metadata.clone(),
            config.tuf_targets.clone(),
            paths.tuf_datastore(),
        );
        let launcher =
            Launcher::open(paths, tuf, config.platform.clone()).map_err(|e| e.to_string())?;
        let events = launcher.events().clone();
        let identity = Arc::new(Identity {
            auth: Auth::new(
                config.hydra.clone(),
                config.server.clone(),
                config.client_id.clone(),
                feco_launcher::auth::REDIRECT_PORT,
                Arc::new(KeyringStore::for_client(&config.client_id)),
            ),
            session: tokio::sync::Mutex::new(None),
            sign_in: std::sync::Mutex::new(SignIn::Unknown),
        });
        let issuer_identity = identity.clone();
        let issuer: Issuer = Arc::new(move |client_id: String| {
            let identity = issuer_identity.clone();
            Box::pin(async move {
                let token = identity.access_token().await.map_err(|e| e.to_string())?;
                identity
                    .auth
                    .launch_code(&token, &client_id)
                    .await
                    .map_err(|e| e.to_string())
            })
        });
        let broker = Broker::new(issuer, events);
        let addr = rt
            .block_on(broker.serve())
            .map_err(|e| tf("không mở được API cục bộ: {}", &[&e]))?;
        let shared = Arc::new(Shared {
            notifications: std::sync::Mutex::new(Vec::new()),
            wallet: std::sync::Mutex::new(None),
            prices: std::sync::Mutex::new(Default::default()),
            config,
            launcher: tokio::sync::Mutex::new(launcher),
            identity,
            broker,
            broker_url: format!("http://{addr}/"),
            skipped: std::sync::Mutex::new(prefs.skipped_updates.clone()),
        });
        let view = rt.block_on(snapshot(&shared));
        register_icons(&view.catalog);
        let (tx, rx) = mpsc::channel();
        let (chat_tx, chat_rx) = mpsc::channel();
        let mut app = Self {
            rt,
            shared,
            tx,
            rx,
            ctx,
            view,
            page: match prefs.start_page {
                StartPage::Library => Page::Library,
                StartPage::Store => Page::Store,
            },
            prefs,
            prefs_path,
            library_filter: Filter::default(),
            store_filter: Filter::default(),
            details: Vec::new(),
            next_detail_key: 1,
            settings_open: false,
            dialog_viewport: egui::ViewportId::ROOT,
            busy: None,
            notice: None,
            confirm_install: None,
            child_positions: Default::default(),
            root_focused: false,
            confirm_buy: None,
            update_prompt: false,
            prompted: Default::default(),
            confirm_uninstall: None,
            next_sync: Instant::now() + SYNC_OFFLINE,
            probed: false,
            chat: ChatState::default(),
            chat_tx,
            chat_rx,
        };
        app.run(t("Đang đăng nhập lại và đồng bộ..."), true, startup);
        Ok(app)
    }

    fn run<F, Fut>(&mut self, label: &str, sync: bool, f: F)
    where
        F: FnOnce(Arc<Shared>) -> Fut,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        self.busy = Some(label.to_owned());
        let shared = self.shared.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let fut = f(shared.clone());
        self.rt.spawn(async move {
            let result = fut.await;
            let view = snapshot(&shared).await;
            let _ = tx.send(Done {
                result,
                view,
                synced: sync,
            });
            ctx.request_repaint();
        });
    }

    fn drain(&mut self) {
        while let Ok(done) = self.rx.try_recv() {
            self.busy = None;
            // Vừa đồng bộ hoặc vừa đăng nhập: cập nhật số tin chưa đọc ngay.
            if done.synced || done.view.sign_in != self.view.sign_in {
                self.chat.schedule.refresh_unread_now(Instant::now());
            }
            self.view = done.view;
            register_icons(&self.view.catalog);
            match done.result {
                Ok(text) if text.is_empty() => {}
                Ok(text) => self.notice = Some((false, text)),
                Err(text) => self.notice = Some((true, text)),
            }
            if done.synced {
                self.next_sync = Instant::now()
                    + if self.view.online {
                        SYNC_ONLINE
                    } else {
                        SYNC_OFFLINE
                    };
                self.maybe_prompt_updates();
            }
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Login => self.run(
                t("Đang chờ đăng nhập trong trình duyệt..."),
                false,
                |s| async move {
                    let session = s
                        .identity
                        .auth
                        .login(|url| open::that(url.as_str()).map_err(|e| e.to_string()))
                        .await
                        .map_err(|e| e.to_string())?;
                    let name = session.name.clone();
                    *s.identity.session.lock().await = Some(session);
                    s.identity.set(SignIn::SignedIn);
                    s.launcher
                        .lock()
                        .await
                        .set_user_name(name.clone())
                        .map_err(|e| e.to_string())?;
                    Ok(tf("Xin chào {}", &[&name.unwrap_or_default()]))
                },
            ),
            Action::Logout => self.run(t("Đang đăng xuất..."), false, |s| async move {
                s.identity.auth.logout().await.map_err(|e| e.to_string())?;
                *s.identity.session.lock().await = None;
                s.identity.set(SignIn::SignedOut);
                *s.wallet.lock().expect("khóa") = None;
                s.launcher
                    .lock()
                    .await
                    .set_user_name(None)
                    .map_err(|e| e.to_string())?;
                Ok(t("Đã đăng xuất").into())
            }),
            Action::Sync => self.run(t("Đang đồng bộ..."), true, sync),
            Action::AskInstall(entry) => self.confirm_install = Some(*entry),
            Action::AskBuy(entry, price) => self.confirm_buy = Some((*entry, price)),
            Action::Buy(entry, price) => {
                self.confirm_buy = None;
                self.run(
                    &tf("Đang mua {}...", &[&entry.title]),
                    false,
                    move |s| async move {
                        let token = s.identity.access_token().await.map_err(|e| e.to_string())?;
                        let balance = s
                            .identity
                            .auth
                            .purchase(&token, &entry.app, price)
                            .await
                            .map_err(|e| match e {
                                PurchaseError::Api { code, .. } if code == "insufficient_funds" => {
                                    t("Không đủ F-Coin. Hãy nạp thêm trong Ví F-Coin.").to_owned()
                                }
                                PurchaseError::Api { code, .. } if code == "price_changed" => {
                                    t("Giá app vừa thay đổi. Hãy đồng bộ rồi thử lại.").to_owned()
                                }
                                PurchaseError::Api { code, .. } if code == "wallet_frozen" => {
                                    t("Ví F-Coin của bạn đang bị khóa.").to_owned()
                                }
                                other => other.to_string(),
                            })?;
                        if let Some(w) = s.wallet.lock().expect("khóa").as_mut() {
                            w.balance = balance;
                            if !w.owns(&entry.app) {
                                w.owned.push(entry.app.clone());
                            }
                        }
                        s.launcher
                            .lock()
                            .await
                            .install(&entry, Timestamp::now())
                            .await
                            .map_err(|e| e.to_string())?;
                        report_install(&s, &entry.app, &entry.version).await;
                        Ok(tf(
                            "Đã mua và cài {} {}. Số dư: {}",
                            &[&entry.title, &entry.version, &coins(balance)],
                        ))
                    },
                )
            }
            Action::Install(entry) => {
                self.confirm_install = None;
                self.run(
                    &tf("Đang cài {} {}...", &[&entry.title, &entry.version]),
                    false,
                    move |s| async move {
                        s.launcher
                            .lock()
                            .await
                            .install(&entry, Timestamp::now())
                            .await
                            .map_err(|e| e.to_string())?;
                        report_install(&s, &entry.app, &entry.version).await;
                        Ok(tf("Đã cài {} {}", &[&entry.title, &entry.version]))
                    },
                )
            }
            Action::Update(app) => self.run(t("Đang cập nhật..."), false, move |s| async move {
                let mut l = s.launcher.lock().await;
                let entry = l
                    .update_for(&app)
                    .cloned()
                    .ok_or(t("không có bản cập nhật"))?;
                l.install(&entry, Timestamp::now())
                    .await
                    .map_err(|e| e.to_string())?;
                drop(l);
                report_install(&s, &entry.app, &entry.version).await;
                Ok(tf("Đã cập nhật {} lên {}", &[&entry.title, &entry.version]))
            }),
            Action::UpdateAll => self.run(t("Đang cập nhật tất cả..."), false, |s| async move {
                let skipped = s.skipped.lock().expect("khóa").clone();
                let mut l = s.launcher.lock().await;
                let results = l.update_all(Timestamp::now(), &skipped).await;
                let installed = l.installed();
                drop(l);
                report_updates(&s, &results, &installed).await;
                summarize(&results)
            }),
            Action::Rollback(app) => self.run(
                t("Đang quay lại bản trước..."),
                false,
                move |s| async move {
                    let mut l = s.launcher.lock().await;
                    l.rollback(&app).map_err(|e| e.to_string())?;
                    let plan = l.prepare_launch(&app).map_err(|e| e.to_string())?;
                    drop(l);
                    report_install(&s, &app, &plan.version).await;
                    Ok(tf("Đã quay lại {} {}", &[&plan.title, &plan.version]))
                },
            ),
            Action::AskUninstall(app) => self.confirm_uninstall = Some((*app, false)),
            Action::Uninstall(app, delete_data) => {
                self.confirm_uninstall = None;
                self.run(t("Đang gỡ..."), false, move |s| async move {
                    s.launcher
                        .lock()
                        .await
                        .uninstall(&app, delete_data)
                        .map_err(|e| e.to_string())?;
                    report_uninstall(&s, &app).await;
                    Ok(tf("Đã gỡ {}. Quyền dữ liệu bạn đã cấp cho app vẫn còn; thu hồi ở trang \"Ứng dụng đã kết nối\".", &[&app]))
                })
            }
            Action::ToggleSidebar => {
                let prefs = Prefs {
                    sidebar_collapsed: !self.prefs.sidebar_collapsed,
                    ..self.prefs.clone()
                };
                self.set_prefs(prefs);
            }
            Action::OpenLink(url) => {
                if let Err(e) = open::that(&url) {
                    self.notice = Some((true, tf("Không mở được trình duyệt: {}", &[&e])));
                }
            }
            Action::Open(app) => self.run(t("Đang mở..."), false, move |s| async move {
                open_app(&s, &app).await
            }),
            Action::SetAutoUpdate(on) => self.run("", false, move |s| async move {
                s.launcher
                    .lock()
                    .await
                    .set_auto_update(on)
                    .map_err(|e| e.to_string())?;
                Ok(String::new())
            }),
            Action::ShowDetail(mut detail) => {
                if let Some(open) = self.details.iter().find(|d| d.app == detail.app) {
                    self.ctx
                        .send_viewport_cmd_to(open.viewport(), egui::ViewportCommand::Focus);
                } else {
                    if self.details.len() >= MAX_DETAIL_WINDOWS {
                        self.details.remove(0);
                    }
                    detail.key = self.next_detail_key;
                    self.next_detail_key += 1;
                    self.details.push(detail);
                }
            }
            Action::MoveDetail(key, app) => {
                if let Some(d) = self.details.iter_mut().find(|d| d.key == key) {
                    d.app = app;
                }
            }
            Action::CloseDetail(key) => {
                if let Some(d) = self.details.iter().find(|d| d.key == key) {
                    if self.dialog_viewport == d.viewport() {
                        self.dialog_viewport = egui::ViewportId::ROOT;
                        self.confirm_install = None;
                        self.confirm_buy = None;
                        self.confirm_uninstall = None;
                    }
                }
                self.details.retain(|d| d.key != key);
            }
            Action::OpenSettings => {
                if self.settings_open {
                    self.ctx
                        .send_viewport_cmd_to(settings_viewport(), egui::ViewportCommand::Focus);
                } else {
                    self.settings_open = true;
                }
            }
            Action::CloseDialog => {
                self.confirm_install = None;
                self.confirm_buy = None;
                self.confirm_uninstall = None;
                self.update_prompt = false;
            }
            Action::ShowUpdates => self.update_prompt = true,
            Action::SkipUpdate(app, version) => {
                self.prefs.skipped_updates.insert(app, version);
                *self.shared.skipped.lock().expect("khóa") = self.prefs.skipped_updates.clone();
                if let Err(e) = self.prefs.save(&self.prefs_path) {
                    self.notice = Some((true, e.to_string()));
                }
                if update_suggestions(&self.view.my_apps, &self.prefs.skipped_updates).is_empty() {
                    self.update_prompt = false;
                }
            }
        }
    }

    /// Trung tâm thông báo: bấm "Mở" để mở đúng màn hình trên web (ví dụ trang báo giá).
    fn notifications(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.heading(t("Thông báo"));
        ui.add_space(6.0);
        if self.view.notifications.is_empty() {
            ui.label(t("Chưa có thông báo nào. Đăng nhập để nhận thông báo về đơn hàng, thanh toán, cập nhật app."));
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for n in &self.view.notifications {
                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if n.read {
                            ui.label(&n.title);
                        } else {
                            ui.strong(&n.title);
                        }
                        // Máy chủ trả thời điểm UTC (RFC 3339): hiện theo giờ của máy.
                        let when = n
                            .created_at
                            .parse::<Timestamp>()
                            .map(|t| {
                                t.to_zoned(jiff::tz::TimeZone::system())
                                    .strftime("%H:%M %d/%m/%Y")
                                    .to_string()
                            })
                            .unwrap_or_else(|_| n.created_at.chars().take(16).collect());
                        ui.weak(when);
                    });
                    ui.label(&n.body);
                    if !n.link.is_empty() && ui.button(t("Mở")).clicked() {
                        actions.push(Action::OpenLink(n.link.clone()));
                    }
                });
                ui.add_space(6.0);
            }
        });
    }

    /// Sau mỗi lần đồng bộ: có bản mới chưa gợi ý thì mở hộp thoại gợi ý cập nhật (trừ khi đang
    /// bật tự động cập nhật, khi đó launcher tự cập nhật).
    fn maybe_prompt_updates(&mut self) {
        if self.view.auto_update {
            return;
        }
        let suggestions = update_suggestions(&self.view.my_apps, &self.prefs.skipped_updates);
        let fresh = suggestions
            .iter()
            .any(|s| !self.prompted.contains(&s.key()));
        if fresh {
            self.update_prompt = true;
            self.prompted
                .extend(suggestions.iter().map(UpdateSuggestion::key));
        }
    }

    /// Hộp thoại "Có bản cập nhật mới".
    fn update_dialog(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if !self.update_prompt || self.open_child().is_some() {
            return;
        }
        let suggestions = update_suggestions(&self.view.my_apps, &self.prefs.skipped_updates);
        if suggestions.is_empty() {
            self.update_prompt = false;
            return;
        }
        let idle = self.busy.is_none() && self.view.online;
        egui::Modal::new(egui::Id::unique("update-suggestions")).show(ctx, |ui| {
            ui.set_max_width(560.0);
            ui.heading(t("Có bản cập nhật mới"));
            ui.weak(t(
                "Bản mới được F-Eco ký và kiểm tra trước khi cài. Dữ liệu của app được giữ nguyên.",
            ));
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for s in &suggestions {
                        ui.horizontal(|ui| {
                            app_icon(ui, &s.app, &s.title, 32.0);
                            ui.vertical(|ui| {
                                ui.strong(&s.title);
                                ui.label(format!("{} \u{2192} {}", s.from, s.to));
                            });
                        });
                        if let Some(notes) = &s.notes {
                            ui.weak(notes);
                        }
                        ui.horizontal(|ui| {
                            if primary_button(ui, &self.prefs, idle, Icon::ArrowUp, t("Cập nhật"))
                            {
                                actions.push(Action::Update(s.app.clone()));
                            }
                            if ui.button(t("Bỏ qua bản này")).clicked() {
                                actions.push(Action::SkipUpdate(s.app.clone(), s.to.clone()));
                            }
                        });
                        ui.separator();
                    }
                });
            ui.horizontal(|ui| {
                if suggestions.len() > 1
                    && primary_button(
                        ui,
                        &self.prefs,
                        idle,
                        Icon::ArrowUp,
                        &tf("Cập nhật tất cả ({})", &[&suggestions.len()]),
                    )
                {
                    actions.push(Action::UpdateAll);
                }
                if ui.button(t("Để sau")).clicked() {
                    actions.push(Action::CloseDialog);
                }
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if let Some(entry) = &self.confirm_install {
            egui::Modal::new(egui::Id::unique("confirm-install")).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    app_icon(ui, &entry.app, &entry.title, 40.0);
                    ui.heading(if entry.is_web() {
                        tf("Thêm {} vào Thư viện?", &[&entry.title])
                    } else {
                        tf("Cài {} {}?", &[&entry.title, &entry.version])
                    });
                });
                if let Some(summary) = &entry.info.summary {
                    ui.label(summary);
                }
                ui.weak(byline(&entry.info));
                if let Some(source) = &entry.info.source {
                    ui.hyperlink_to(t("Mã nguồn"), source);
                }
                if let Some(url) = entry.info.web_url.as_deref().filter(|_| entry.is_web()) {
                    ui.label(tf(
                        "App web mở bằng trình duyệt tại {} và đăng nhập bằng tài khoản F-Eco.",
                        &[&url],
                    ));
                } else {
                    ui.label(tf(
                        "Dung lượng tải về: {} MB. Gói được kiểm tra chữ ký của F-Eco trước khi cài.",
                        &[&format!("{:.1}", entry.length as f64 / 1_048_576.0)],
                    ));
                    ui.colored_label(egui::Color32::from_rgb(200, 120, 0), t(ISOLATION_NOTICE));
                }
                ui.horizontal(|ui| {
                    if ui.button(install_label(entry)).clicked() {
                        actions.push(Action::Install(Box::new(entry.clone())));
                    }
                    if ui.button(t("Hủy")).clicked() {
                        actions.push(Action::CloseDialog);
                    }
                });
            });
        }
        if let Some((entry, price)) = &self.confirm_buy {
            let balance = self.view.wallet.as_ref().map(|w| w.balance).unwrap_or(0);
            let wallet_url = self
                .view
                .wallet
                .as_ref()
                .and_then(|w| w.wallet_url.clone())
                .unwrap_or_else(|| {
                    self.shared
                        .config
                        .server
                        .join("wallet")
                        .map(|u| u.to_string())
                        .unwrap_or_default()
                });
            egui::Modal::new(egui::Id::unique("confirm-buy")).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    app_icon(ui, &entry.app, &entry.title, 40.0);
                    ui.heading(tf("Mua {}?", &[&entry.title]));
                });
                egui::Grid::new("buy-grid").num_columns(2).show(ui, |ui| {
                    ui.label(t("Giá"));
                    ui.strong(coins(*price));
                    ui.end_row();
                    ui.label(t("Số dư hiện tại"));
                    ui.label(coins(balance));
                    ui.end_row();
                    if shortfall(balance, *price).is_none() {
                        ui.label(t("Số dư sau khi mua"));
                        ui.label(coins(balance - price));
                        ui.end_row();
                    }
                });
                match shortfall(balance, *price) {
                    Some(short) => {
                        ui.colored_label(warn_color(ui), tf("Còn thiếu {}", &[&coins(short)]));
                        ui.horizontal(|ui| {
                            if primary_button(ui, &self.prefs, true, Icon::Wallet, t("Nạp F-Coin"))
                            {
                                actions.push(Action::OpenLink(wallet_url.clone()));
                            }
                            if ui.button(t("Hủy")).clicked() {
                                actions.push(Action::CloseDialog);
                            }
                        });
                    }
                    None => {
                        ui.weak(t(
                            "Mua xong, app được cài ngay. Quyền sử dụng gắn với tài khoản của bạn.",
                        ));
                        ui.horizontal(|ui| {
                            if primary_button(ui, &self.prefs, true, Icon::Coin, t("Mua")) {
                                actions.push(Action::Buy(Box::new(entry.clone()), *price));
                            }
                            if ui.button(t("Hủy")).clicked() {
                                actions.push(Action::CloseDialog);
                            }
                        });
                    }
                }
            });
        }
        let server = self.shared.config.server.clone();
        if let Some((app, delete_data)) = &mut self.confirm_uninstall {
            egui::Modal::new(egui::Id::unique("confirm-uninstall")).show(ctx, |ui| {
                ui.heading(tf("Gỡ {}?", &[&app.title]));
                ui.checkbox(delete_data, t("Xóa cả dữ liệu cục bộ của app trên máy này"));
                ui.label(t("Gỡ app không thu hồi quyền dữ liệu bạn đã cấp cho app."));
                ui.hyperlink_to(
                    t("Quản lý quyền ở trang Ứng dụng đã kết nối"),
                    server
                        .join("apps")
                        .map(|u| u.to_string())
                        .unwrap_or_default(),
                );
                ui.horizontal(|ui| {
                    if ui.button(t("Gỡ")).clicked() {
                        actions.push(Action::Uninstall(app.app.clone(), *delete_data));
                    }
                    if ui.button(t("Hủy")).clicked() {
                        actions.push(Action::CloseDialog);
                    }
                });
            });
        }
    }
}

impl eframe::App for LauncherApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();
        if self.busy.is_none() && Instant::now() >= self.next_sync {
            self.next_sync = Instant::now() + SYNC_OFFLINE;
            self.run(t("Đang đồng bộ..."), true, sync);
        }
        let wake = self.chat_tick().unwrap_or(Duration::MAX);
        ctx.request_repaint_after(wake.min(Duration::from_secs(30)));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Đầu dò khởi động: in thời gian từ lúc chạy tới khung hình đầu tiên rồi thoát
        // (scripts/measure-launcher-startup.ps1).
        if !self.probed {
            self.probed = true;
            // Bản phát hành trên Windows không có console, nên kết quả ghi vào tệp được chỉ định.
            if let Some(out) = std::env::var_os("FECO_LAUNCHER_STARTUP_PROBE") {
                let ms = STARTED.get().map_or(0, |t| t.elapsed().as_millis());
                eprintln!("startup_ms={ms}");
                let _ = std::fs::write(out, format!("{ms}\n"));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        let mut actions = Vec::new();
        // Ctrl+B (Cmd+B trên macOS): ẩn/hiện chữ ở thanh bên.
        if ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::B,
            ))
        }) {
            actions.push(Action::ToggleSidebar);
        }
        let collapsed = sidebar_collapsed(
            self.prefs.sidebar_collapsed,
            ui.ctx().content_rect().width(),
        );
        let mut nav = egui::Panel::left("nav")
            .exact_size(sidebar_width(collapsed))
            .resizable(false);
        if let Some(p) = self.prefs.style.palette() {
            nav = nav.frame(
                egui::Frame::side_top_panel(ui.style())
                    .fill(rgb(p.panel))
                    .stroke(egui::Stroke::new(1.0, rgb(p.stroke))),
            );
        }
        nav.show(ui, |ui| {
            if collapsed {
                self.sidebar_compact(ui, &mut actions)
            } else {
                self.sidebar(ui, &mut actions)
            }
        });
        self.status_panel(ui, "status");
        egui::CentralPanel::default().show(ui, |ui| match self.page {
            Page::Library => self.library_page(ui, &mut actions),
            Page::Store => self.store_page(ui, &mut actions),
            Page::Messages => self.messages_page(ui, &mut actions),
            Page::Notifications => self.notifications(ui, &mut actions),
        });
        let ctx = ui.ctx().clone();
        self.route_dialogs(egui::ViewportId::ROOT, &actions);
        if self.dialog_viewport == egui::ViewportId::ROOT {
            self.dialogs(&ctx, &mut actions);
        }
        self.update_dialog(&ctx, &mut actions);
        // Cửa sổ con là cửa sổ chặn: cửa sổ chính không nhận thao tác cho tới khi nó đóng.
        self.modal_block(&ctx);
        self.settings_window(&ctx, &mut actions);
        self.detail_windows(&ctx, &mut actions);
        // Thao tác có thể mở hoặc đóng cửa sổ: vẽ lại ngay, không chờ sự kiện tiếp theo.
        if !actions.is_empty() {
            ctx.request_repaint();
        }
        for action in actions {
            self.apply(action);
        }
    }
}

impl LauncherApp {
    /// Cửa sổ con đang mở (cửa sổ chi tiết mới nhất, hoặc Cài đặt).
    fn open_child(&self) -> Option<egui::ViewportId> {
        self.details
            .last()
            .map(Detail::viewport)
            .or(self.settings_open.then(settings_viewport))
    }

    /// Vị trí giữa màn hình cho cửa sổ con, tính một lần khi cửa sổ mở (sau đó người dùng kéo
    /// đi đâu thì để đó).
    fn child_position(
        &mut self,
        ctx: &egui::Context,
        id: egui::ViewportId,
        size: egui::Vec2,
    ) -> Option<egui::Pos2> {
        if let Some(pos) = self.child_positions.get(&id) {
            return Some(*pos);
        }
        let pos = centered_position(ctx, size)?;
        self.child_positions.insert(id, pos);
        Some(pos)
    }

    /// Lớp phủ chặn cửa sổ chính khi có cửa sổ con: bấm vào thì đưa cửa sổ con lên trước; cửa
    /// sổ chính được bấm chọn (focus) thì cũng chuyển focus sang cửa sổ con.
    fn modal_block(&mut self, ctx: &egui::Context) {
        let Some(child) = self.open_child() else {
            self.root_focused = false;
            return;
        };
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        if focused && !self.root_focused {
            ctx.send_viewport_cmd_to(child, egui::ViewportCommand::Focus);
        }
        self.root_focused = focused;
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("modal-block"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let response = ui.allocate_rect(screen, egui::Sense::click_and_drag());
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(120));
                ui.painter().text(
                    screen.center(),
                    egui::Align2::CENTER_CENTER,
                    t("Đang mở cửa sổ khác. Đóng cửa sổ đó để tiếp tục."),
                    egui::FontId::proportional(16.0),
                    egui::Color32::WHITE,
                );
                if response.clicked() || response.drag_started() {
                    ctx.send_viewport_cmd_to(child, egui::ViewportCommand::Focus);
                }
            });
    }

    /// Thanh trạng thái dưới cùng: việc đang làm hoặc kết quả gần nhất.
    fn status_panel(&self, ui: &mut egui::Ui, id: &'static str) {
        egui::Panel::bottom(id).show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(label) = self.busy.as_deref().filter(|l| !l.is_empty()) {
                    ui.spinner();
                    ui.label(label);
                } else if let Some((error, text)) = &self.notice {
                    let color = if *error {
                        danger_color(ui)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.colored_label(color, text);
                } else {
                    ui.weak(" ");
                }
            });
        });
    }

    /// Hộp thoại xác nhận hiện ở cửa sổ nơi người dùng bấm Cài hoặc Gỡ.
    fn route_dialogs(&mut self, viewport: egui::ViewportId, actions: &[Action]) {
        if actions.iter().any(|a| {
            matches!(
                a,
                Action::AskInstall(_) | Action::AskBuy(..) | Action::AskUninstall(_)
            )
        }) {
            self.dialog_viewport = viewport;
        }
    }

    /// Cửa sổ Cài đặt (spec "Cửa sổ riêng cho Cài đặt và chi tiết app").
    fn settings_window(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if !self.settings_open {
            return;
        }
        let size = egui::vec2(760.0, 700.0);
        let mut builder = egui::ViewportBuilder::default()
            .with_title(format!("{} · F-Launcher", t("Cài đặt")))
            .with_inner_size(size)
            .with_min_inner_size([520.0, 420.0]);
        if let Some(pos) = self.child_position(ctx, settings_viewport(), size) {
            builder = builder.with_position(pos);
        }
        if let Some(icon) = window_icon() {
            builder = builder.with_icon(icon);
        }
        let mut local = Vec::new();
        let close = ctx.show_viewport_immediate(settings_viewport(), builder, |ui, _| {
            egui::CentralPanel::default().show(ui, |ui| self.settings_page(ui, &mut local));
            let typing = ui.ctx().egui_wants_keyboard_input();
            ui.ctx().input(|i| {
                i.viewport().close_requested() || (!typing && i.key_pressed(egui::Key::Escape))
            })
        });
        if close {
            self.settings_open = false;
        }
        actions.extend(local);
    }

    /// Mỗi app được bấm mở trong một cửa sổ chi tiết riêng.
    fn detail_windows(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        for detail in self.details.clone() {
            let id = detail.viewport();
            let size = egui::vec2(840.0, 720.0);
            let mut builder = egui::ViewportBuilder::default()
                .with_title(format!("{} · F-Launcher", self.title_of(&detail.app)))
                .with_inner_size(size)
                .with_min_inner_size([560.0, 420.0]);
            if let Some(pos) = self.child_position(ctx, id, size) {
                builder = builder.with_position(pos);
            }
            if let Some(icon) = window_icon() {
                builder = builder.with_icon(icon);
            }
            let mut local = Vec::new();
            let close = ctx.show_viewport_immediate(id, builder, |ui, _| {
                self.status_panel(ui, "detail-status");
                egui::CentralPanel::default()
                    .show(ui, |ui| self.detail_page(ui, &detail, &mut local));
                if self.dialog_viewport == id {
                    let ctx = ui.ctx().clone();
                    self.dialogs(&ctx, &mut local);
                }
                ui.ctx().input(|i| i.viewport().close_requested())
            });
            if close {
                local.push(Action::CloseDetail(detail.key));
            }
            self.route_dialogs(id, &local);
            actions.extend(local);
        }
    }
}

// ---- Giao diện (spec launcher: "Bố cục lưới và danh sách", "Tìm, lọc và sắp xếp app",
// ---- "Cài đặt giao diện của launcher", "Thông tin hiển thị của app").

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Library,
    Store,
    Messages,
    Notifications,
}

/// Ô tìm và bộ lọc nhóm của một trang (kiểu sắp xếp nằm trong [`Prefs`] để được nhớ).
#[derive(Default)]
struct Filter {
    text: String,
    category: Option<String>,
}

/// Tên vai trò nền tảng (đã dịch).
fn role_label(role: &str) -> &'static str {
    match role {
        "admin" => t("Quản trị viên"),
        "developer" => t("Lập trình viên cá nhân"),
        "dev_company" => t("Công ty phát triển phần mềm"),
        "customer" => t("Khách hàng cá nhân"),
        "business_customer" => t("Khách hàng doanh nghiệp"),
        _ => t("Người dùng phổ thông"),
    }
}

fn rgb([r, g, b]: ui_style::Rgb) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// Màu cảnh báo theo phong cách và nền sáng/tối hiện tại (đủ tương phản, xem style.rs).
fn warn_color(ui: &egui::Ui) -> egui::Color32 {
    rgb(ui_style::status_palette(ui.visuals().dark_mode).warn)
}

fn danger_color(ui: &egui::Ui) -> egui::Color32 {
    rgb(ui_style::status_palette(ui.visuals().dark_mode).danger)
}

/// Màu nhấn: của phong cách họa sĩ, hoặc màu người dùng chọn.
fn accent(prefs: &Prefs) -> egui::Color32 {
    match prefs.style.palette() {
        Some(p) => rgb(p.accent),
        None => rgb(prefs.accent.rgb()),
    }
}

/// Chữ trên nút dùng màu nhấn.
fn on_accent(prefs: &Prefs) -> egui::Color32 {
    prefs
        .style
        .palette()
        .map_or(egui::Color32::WHITE, |p| rgb(p.on_accent))
}

/// Áp ngôn ngữ, phong cách, chủ đề, màu nhấn, cỡ chữ và chế độ gọn (có hiệu lực ngay).
fn apply_prefs(ctx: &egui::Context, prefs: &Prefs) {
    i18n::set_language(prefs.language);
    ui_style::set_current(prefs.style);
    let shape = prefs.style.shape();
    let palette = prefs.style.palette();
    ctx.set_theme(match (palette, prefs.theme) {
        (Some(p), _) if p.dark => egui::ThemePreference::Dark,
        (Some(_), _) => egui::ThemePreference::Light,
        (None, Theme::System) => egui::ThemePreference::System,
        (None, Theme::Light) => egui::ThemePreference::Light,
        (None, Theme::Dark) => egui::ThemePreference::Dark,
    });
    ctx.set_zoom_factor(prefs.font_scale);
    let color = accent(prefs);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut visuals = match (palette, theme) {
            (Some(p), _) if p.dark => egui::Visuals::dark(),
            (Some(_), _) => egui::Visuals::light(),
            (None, egui::Theme::Dark) => egui::Visuals::dark(),
            (None, egui::Theme::Light) => egui::Visuals::light(),
        };
        if let Some(p) = palette {
            painter_visuals(&mut visuals, &p, shape.stroke);
        } else {
            let base = if theme == egui::Theme::Dark {
                ui_style::BASE_DARK
            } else {
                ui_style::BASE_LIGHT
            };
            visuals.warn_fg_color = rgb(base.warn);
            visuals.error_fg_color = rgb(base.danger);
        }
        visuals.selection.bg_fill = color;
        visuals.selection.stroke.color = on_accent(prefs);
        visuals.hyperlink_color = color;
        visuals.widgets.hovered.bg_stroke.color = color;
        let r: egui::CornerRadius = shape.radius.into();
        for w in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            w.corner_radius = r;
        }
        visuals.window_corner_radius = shape.card_radius.into();
        visuals.menu_corner_radius = shape.card_radius.min(12).into();
        let mut style = egui::Style {
            visuals,
            ..egui::Style::default()
        };
        // Làm tròn tới điểm ảnh nguyên để chữ và viền không bị nhòe.
        let k = |v: f32| (v * shape.spacing).round();
        if prefs.compact {
            style.spacing.item_spacing = egui::vec2(6.0, 3.0);
            style.spacing.button_padding = egui::vec2(6.0, 2.0);
        } else {
            style.spacing.item_spacing = egui::vec2(k(8.0), k(6.0));
            style.spacing.button_padding = egui::vec2(k(10.0), k(4.0));
        }
        ctx.set_style_of(theme, style);
    }
}

/// Màu egui từ bảng màu của phong cách họa sĩ.
fn painter_visuals(v: &mut egui::Visuals, p: &ui_style::Palette, stroke: f32) {
    let mix = |a: ui_style::Rgb, b: ui_style::Rgb, t: f32| {
        let m = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
        egui::Color32::from_rgb(m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]))
    };
    v.window_fill = rgb(p.window);
    v.panel_fill = rgb(p.window);
    v.faint_bg_color = rgb(p.card);
    v.extreme_bg_color = rgb(p.card);
    v.text_edit_bg_color = Some(rgb(p.card));
    v.code_bg_color = rgb(p.panel);
    v.weak_text_color = Some(rgb(p.weak));
    v.warn_fg_color = rgb(p.warn);
    v.error_fg_color = rgb(p.danger);
    v.window_stroke = egui::Stroke::new(stroke, rgb(p.stroke));
    let text = rgb(p.text);
    // Nút: nền pha giữa thẻ và viền; di chuột thì đậm hơn.
    let button = mix(p.card, p.stroke, 0.25);
    let hovered = mix(p.card, p.stroke, 0.45);
    let active = mix(p.card, p.stroke, 0.6);
    v.widgets.noninteractive.bg_fill = rgb(p.window);
    v.widgets.noninteractive.weak_bg_fill = rgb(p.card);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(stroke, rgb(p.stroke));
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text);
    for (w, fill) in [
        (&mut v.widgets.inactive, button),
        (&mut v.widgets.hovered, hovered),
        (&mut v.widgets.active, active),
        (&mut v.widgets.open, hovered),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.fg_stroke = egui::Stroke::new(1.0, text);
    }
    v.widgets.inactive.bg_stroke = egui::Stroke::new(stroke * 0.5, rgb(p.stroke));
}

/// Các ô xem trước phong cách trong Cài đặt: bấm để chọn.
fn style_gallery(ui: &mut egui::Ui, prefs: &mut Prefs) {
    let width = 168.0;
    let spacing = 8.0;
    let columns =
        (((ui.available_width() + spacing) / (width + 20.0 + spacing)).floor() as usize).max(1);
    for row in UiStyle::ALL.chunks(columns) {
        ui.horizontal_top(|ui| {
            for style in row {
                let selected = prefs.style == *style;
                let (window, card, text, accent_c) = match style.palette() {
                    Some(p) => (rgb(p.window), rgb(p.card), rgb(p.text), rgb(p.accent)),
                    // Phong cách theo chủ đề: xem trước bằng màu nhấn người dùng đã chọn.
                    None => (
                        ui.visuals().panel_fill,
                        ui.visuals().faint_bg_color,
                        ui.visuals().text_color(),
                        rgb(prefs.accent.rgb()),
                    ),
                };
                let stroke = if selected {
                    egui::Stroke::new(2.5, accent(prefs))
                } else {
                    ui.visuals().widgets.noninteractive.bg_stroke
                };
                let response = ui
                    .scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
                        ui.style_mut().interaction.selectable_labels = false;
                        egui::Frame::new()
                            .fill(window)
                            .stroke(stroke)
                            .corner_radius(f32::from(style.shape().card_radius.max(4)))
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.set_width(width);
                                ui.vertical(|ui| {
                                    // Hình thu nhỏ: một thẻ và một nút theo màu, bo góc của phong cách.
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(width, 44.0),
                                        egui::Sense::hover(),
                                    );
                                    let r = f32::from(style.shape().card_radius);
                                    ui.painter().rect_filled(rect, r.min(10.0), card);
                                    let button = egui::Rect::from_min_size(
                                        rect.left_top() + egui::vec2(10.0, 12.0),
                                        egui::vec2(56.0, 20.0),
                                    );
                                    ui.painter().rect_filled(
                                        button,
                                        f32::from(style.shape().radius).min(10.0),
                                        accent_c,
                                    );
                                    ui.painter().text(
                                        rect.left_top() + egui::vec2(76.0, 22.0),
                                        egui::Align2::LEFT_CENTER,
                                        "Aa",
                                        egui::FontId::proportional(15.0),
                                        text,
                                    );
                                    if style.motif().is_some() {
                                        ui.add_space(4.0);
                                        motif_band(ui, *style, width, 6.0);
                                    }
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(style.label()).strong().color(text),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(style.description())
                                                .small()
                                                .color(text),
                                        )
                                        .wrap(),
                                    );
                                });
                            });
                    })
                    .response
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.clicked() {
                    prefs.style = *style;
                }
            }
        });
        ui.add_space(spacing);
    }
}

/// Dải họa tiết màu của phong cách họa sĩ (thanh bên, ô xem trước).
fn motif_band(ui: &mut egui::Ui, style: UiStyle, width: f32, height: f32) {
    let Some(colors) = style.motif() else {
        return;
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let w = rect.width() / colors.len() as f32;
    let r = f32::from(style.shape().radius.min(4));
    for (i, c) in colors.iter().enumerate() {
        let x = rect.left() + w * i as f32;
        let cell =
            egui::Rect::from_min_max(egui::pos2(x, rect.top()), egui::pos2(x + w, rect.bottom()));
        let radius = egui::CornerRadius {
            nw: if i == 0 { r as u8 } else { 0 },
            sw: if i == 0 { r as u8 } else { 0 },
            ne: if i + 1 == colors.len() { r as u8 } else { 0 },
            se: if i + 1 == colors.len() { r as u8 } else { 0 },
        };
        ui.painter().rect_filled(cell, radius, rgb(*c));
    }
}

/// Biểu tượng chữ cái đầu của app, màu cố định theo mã app (design E4).
fn app_icon(ui: &mut egui::Ui, app_id: &str, title: &str, size: f32) -> egui::Response {
    // Không nhận chuột: bấm vào biểu tượng là bấm vào ô hoặc hàng chứa nó.
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if let Some(texture) = icon_texture(ui.ctx(), app_id) {
        egui::Image::new((texture.id(), egui::vec2(size, size)))
            .corner_radius(size * 0.22)
            .paint_at(ui, rect);
        return response;
    }
    let [r, g, b] = library::tile_color(app_id);
    let painter = ui.painter();
    painter.rect_filled(rect, size * 0.22, egui::Color32::from_rgb(r, g, b));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        library::monogram(title),
        egui::FontId::proportional(size * 0.38),
        egui::Color32::WHITE,
    );
    response
}

/// Nút hành động chính: icon và chữ trên nền màu nhấn.
fn primary_button(ui: &mut egui::Ui, prefs: &Prefs, enabled: bool, icon: Icon, text: &str) -> bool {
    icons::filled(ui, icon, text, accent(prefs), on_accent(prefs), enabled).clicked()
}

fn size_mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

/// Dung lượng gói, hoặc "App web" (gói của app web chỉ có thông tin app, không cài gì lên máy).
fn entry_size(entry: &CatalogEntry) -> String {
    if entry.is_web() {
        t("App web").to_owned()
    } else {
        size_mb(entry.length)
    }
}

/// "Nhà phát triển · Nhóm" (bỏ phần trống).
fn byline(info: &feco_launcher::AppInfo) -> String {
    let category = library::category_label(&library::category_of(info));
    let line = match &info.developer {
        Some(d) => format!("{d} · {category}"),
        None => category,
    };
    // Nhãn "App web" (launcher-sidebar-and-web-apps).
    if info.web {
        format!("{} · {line}", t("App web"))
    } else {
        line
    }
}

/// Chữ của nút cài: "Thêm" cho app web (vào Thư viện, không cài gì lên máy), "Cài" cho app desktop.
fn install_label(entry: &CatalogEntry) -> &'static str {
    if entry.is_web() {
        t("Thêm")
    } else {
        t("Cài")
    }
}

/// Lề trong của ô lưới.
const TILE_MARGIN: f32 = 10.0;

/// Số cột và chiều rộng mỗi ô: càng nhiều cột càng tốt mà ô không hẹp hơn `min_width`; các ô
/// trải đều để hàng lấp đầy chiều rộng.
fn grid_columns(available: f32, min_width: f32, gap: f32) -> (usize, f32) {
    let columns = (((available + gap) / (min_width + gap)).floor() as usize).max(1);
    let width = ((available - gap * (columns - 1) as f32) / columns as f32).floor();
    (columns, width.max(1.0))
}

fn card_frame(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::new()
        .fill(visuals.faint_bg_color)
        .stroke(visuals.widgets.noninteractive.bg_stroke)
        .corner_radius(f32::from(ui_style::current().shape().card_radius))
        .inner_margin(10.0)
}

impl LauncherApp {
    fn set_prefs(&mut self, prefs: Prefs) {
        let prefs = prefs.sanitized();
        if prefs == self.prefs {
            return;
        }
        self.prefs = prefs;
        apply_prefs(&self.ctx, &self.prefs);
        if let Err(e) = self.prefs.save(&self.prefs_path) {
            self.notice = Some((true, tf("Không lưu được cài đặt: {}", &[&e])));
        }
    }

    /// Mục điều hướng: icon, trang, tên, số cần báo (bản cập nhật, thông báo chưa đọc).
    fn nav_entries(&self) -> [(Icon, Page, &'static str, usize); 4] {
        let unread = self.view.notifications.iter().filter(|n| !n.read).count();
        // Không tính bản đã bỏ qua, như hộp thoại "Có bản cập nhật mới" (app-update-notices U4).
        let updates = update_suggestions(&self.view.my_apps, &self.prefs.skipped_updates).len();
        [
            (Icon::Library, Page::Library, t("Thư viện"), updates),
            (Icon::Store, Page::Store, t("Cửa hàng"), 0),
            (Icon::Chat, Page::Messages, t("Tin nhắn"), self.chat.unread),
            (Icon::Bell, Page::Notifications, t("Thông báo"), unread),
        ]
    }

    fn signed_in(&self) -> bool {
        matches!(
            self.view.sign_in,
            Some(SignIn::SignedIn) | Some(SignIn::Offline)
        )
    }

    /// Địa chỉ trang ví trên web.
    fn wallet_url(&self) -> Option<String> {
        let w = self.view.wallet.as_ref()?;
        Some(w.wallet_url.clone().unwrap_or_else(|| {
            self.shared
                .config
                .server
                .join("wallet")
                .map(|u| u.to_string())
                .unwrap_or_default()
        }))
    }

    /// Nút icon dưới cùng: đồng bộ, F-Eco trên web, lối tắt theo vai trò, đăng xuất. Thứ tự gọi
    /// là thứ tự hiện (từ trái sang phải ở chế độ đầy đủ, từ dưới lên ở chế độ thu gọn).
    fn tool_buttons(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let idle = self.busy.is_none();
        let sync_tip = match self.view.last_sync {
            // Giờ địa phương của máy (Timestamp là UTC).
            Some(at) => tf(
                "Đồng bộ lúc {}",
                &[&at
                    .to_zoned(jiff::tz::TimeZone::system())
                    .strftime("%H:%M %d/%m")],
            ),
            None => t("Đồng bộ").to_owned(),
        };
        if icons::button(ui, Icon::Sync, &sync_tip, false, idle).clicked() {
            actions.push(Action::Sync);
        }
        // Tính năng tài khoản trên web (đồng bộ tính năng launcher và web).
        let server = self.shared.config.server.clone();
        let web = icons::button(ui, Icon::Globe, t("F-Eco trên web"), false, true);
        egui::Popup::menu(&web).show(|ui| {
            for (label, path) in [
                (t("Hồ sơ và vai trò"), ""),
                (t("Ví F-Coin"), "wallet"),
                (t("Ứng dụng của tôi"), "library"),
                (t("Dự án phần mềm"), "projects"),
                (t("Đơn xây app"), "orders"),
                (t("Tin nhắn"), "messages"),
                (t("Thông báo"), "notifications"),
            ] {
                if ui.button(label).clicked() {
                    if let Ok(url) = server.join(path) {
                        actions.push(Action::OpenLink(url.to_string()));
                    }
                    ui.close();
                }
            }
        });
        if !self.signed_in() {
            return;
        }
        if let Some(role) = &self.view.role {
            if role.is_admin() {
                if let Some(url) = &role.admin_console {
                    if icons::button(ui, Icon::Shield, t("Bảng quản trị"), false, true).clicked()
                    {
                        actions.push(Action::OpenLink(url.clone()));
                    }
                }
            }
            if role.can_develop() {
                if let Some(url) = &role.developer_portal {
                    if icons::button(ui, Icon::Code, t("Cổng nhà phát triển"), false, true)
                        .clicked()
                    {
                        actions.push(Action::OpenLink(url.clone()));
                    }
                }
            }
        }
        if icons::button(ui, Icon::SignOut, t("Đăng xuất"), false, idle).clicked() {
            actions.push(Action::Logout);
        }
    }

    fn wallet_tip(&self) -> &'static str {
        match &self.view.wallet {
            Some(w) if w.frozen => t("Ví F-Coin đang bị khóa"),
            _ => t("Số dư F-Coin · bấm để nạp hoặc rút"),
        }
    }

    /// Thanh bên đầy đủ: icon và chữ.
    fn sidebar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            app_icon(ui, "f-launcher", "F Launcher", 28.0);
            ui.heading("F-Launcher");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(
                    ui,
                    Icon::PanelLeft,
                    t("Thu gọn thanh bên (Ctrl+B)"),
                    false,
                    true,
                )
                .clicked()
                {
                    actions.push(Action::ToggleSidebar);
                }
            });
        });
        if self.prefs.style.motif().is_some() {
            ui.add_space(6.0);
            let w = ui.available_width();
            motif_band(ui, self.prefs.style, w, 8.0);
        }
        ui.add_space(4.0);
        if self.view.online {
            ui.weak(t("Đã kết nối"));
        } else {
            ui.colored_label(warn_color(ui), t("Đang ngoại tuyến"));
        }
        ui.add_space(12.0);
        let width = ui.available_width();
        for (icon, page, name, count) in self.nav_entries() {
            let label = match (page, count) {
                (_, 0) => name.to_owned(),
                (Page::Library, n) => tf("Thư viện  ({} cập nhật)", &[&n]),
                (Page::Messages, n) => tf("Tin nhắn  ({})", &[&n]),
                (_, n) => tf("Thông báo  ({})", &[&n]),
            };
            let selected = self.page == page;
            let text = if selected {
                egui::RichText::new(label).strong()
            } else {
                egui::RichText::new(label)
            };
            if icons::labeled(ui, icon, text, selected, width).clicked() {
                self.go(page);
            }
        }
        // Cài đặt mở trong cửa sổ riêng.
        if icons::labeled(ui, Icon::Gear, t("Cài đặt"), self.settings_open, width)
            .on_hover_text(t("Mở Cài đặt trong cửa sổ riêng"))
            .clicked()
        {
            actions.push(Action::OpenSettings);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(8.0);
            if let Some(t) = self.view.last_sync {
                // Giờ địa phương của máy (Timestamp là UTC).
                let local = t.to_zoned(jiff::tz::TimeZone::system());
                ui.weak(tf("Đồng bộ lúc {}", &[&local.strftime("%H:%M %d/%m")]));
            }
            // Hàng nút icon: đồng bộ, lối tắt theo vai trò, đăng xuất.
            ui.horizontal(|ui| self.tool_buttons(ui, actions));
            if self.signed_in() {
                if let (Some(w), Some(url)) = (&self.view.wallet, self.wallet_url()) {
                    let label = coins(w.balance);
                    if icons::labeled(ui, Icon::Wallet, &label, false, width)
                        .on_hover_text(self.wallet_tip())
                        .clicked()
                    {
                        actions.push(Action::OpenLink(url));
                    }
                }
            }
            ui.separator();
            if self.signed_in() {
                if let Some(role) = &self.view.role {
                    ui.label(
                        egui::RichText::new(role_label(&role.role))
                            .small()
                            .color(accent(&self.prefs)),
                    );
                }
                ui.strong(
                    self.view
                        .user
                        .clone()
                        .unwrap_or_else(|| t("Đã đăng nhập").into()),
                );
            } else {
                let idle = self.busy.is_none();
                if primary_button(ui, &self.prefs, idle, Icon::SignIn, t("Đăng nhập")) {
                    actions.push(Action::Login);
                }
                ui.weak(t("Chưa đăng nhập"));
            }
        });
    }

    /// Thanh bên thu gọn: chỉ icon, tooltip ghi tên, huy hiệu số (launcher-sidebar-and-web-apps).
    fn sidebar_compact(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let badge_fill = accent(&self.prefs);
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            app_icon(ui, "f-launcher", "F Launcher", 28.0).on_hover_text("F-Launcher");
            ui.add_space(4.0);
            if icons::button(ui, Icon::Menu, t("Mở rộng thanh bên (Ctrl+B)"), false, true).clicked()
            {
                actions.push(Action::ToggleSidebar);
            }
            // Chấm trạng thái kết nối.
            let (color, tip) = if self.view.online {
                (egui::Color32::from_rgb(0x2f, 0xb3, 0x4b), t("Đã kết nối"))
            } else {
                (warn_color(ui), t("Đang ngoại tuyến"))
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, color);
            response.on_hover_text(tip);
            ui.add_space(4.0);
            ui.separator();
            for (icon, page, name, count) in self.nav_entries() {
                let tip = if count > 0 {
                    format!("{name} ({count})")
                } else {
                    name.to_owned()
                };
                let response = icons::button(ui, icon, &tip, self.page == page, true);
                icons::badge(ui, response.rect, count, badge_fill);
                if response.clicked() {
                    self.go(page);
                }
            }
            if icons::button(ui, Icon::Gear, t("Cài đặt"), self.settings_open, true).clicked() {
                actions.push(Action::OpenSettings);
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            if self.signed_in() {
                let name = self
                    .view
                    .user
                    .clone()
                    .unwrap_or_else(|| t("Đã đăng nhập").into());
                let tip = match &self.view.role {
                    Some(role) => format!("{name} · {}", role_label(&role.role)),
                    None => name.clone(),
                };
                app_icon(ui, "account", &name, 28.0).on_hover_text(tip);
            } else if icons::button(ui, Icon::SignIn, t("Đăng nhập"), false, self.busy.is_none())
                .clicked()
            {
                actions.push(Action::Login);
            }
            ui.separator();
            if self.signed_in() {
                if let (Some(w), Some(url)) = (&self.view.wallet, self.wallet_url()) {
                    let tip = format!("{} · {}", coins(w.balance), self.wallet_tip());
                    if icons::button(ui, Icon::Wallet, &tip, false, true).clicked() {
                        actions.push(Action::OpenLink(url));
                    }
                }
            }
            self.tool_buttons(ui, actions);
        });
    }

    /// Tiêu đề trang và thanh công cụ: tìm, nhóm, sắp xếp, lưới/danh sách.
    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let store = self.page == Page::Store;
        let (title, count) = if store {
            (t("Cửa hàng"), self.view.catalog.len())
        } else {
            (t("Thư viện"), self.view.my_apps.len())
        };
        let categories = if store {
            library::categories(&self.view.catalog)
        } else {
            library::categories(&self.view.my_apps)
        };
        let mut prefs = self.prefs.clone();
        ui.horizontal(|ui| {
            ui.heading(title);
            ui.weak(tf("{} app", &[&count]));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (layout, icon) in [(Layout::List, Icon::List), (Layout::Grid, Icon::Grid)] {
                    let selected = prefs.layout == layout;
                    if icons::button(ui, icon, layout.label(), selected, true).clicked() {
                        prefs.layout = layout;
                    }
                }
                if !store {
                    let updates =
                        update_suggestions(&self.view.my_apps, &self.prefs.skipped_updates).len();
                    if updates > 0
                        && primary_button(
                            ui,
                            &self.prefs,
                            self.busy.is_none(),
                            Icon::ArrowUp,
                            &tf("Cập nhật tất cả ({})", &[&updates]),
                        )
                    {
                        actions.push(Action::UpdateAll);
                    }
                }
            });
        });
        ui.add_space(4.0);
        let filter = if store {
            &mut self.store_filter
        } else {
            &mut self.library_filter
        };
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut filter.text)
                    .hint_text(t("Tìm theo tên, mô tả, nhà phát triển..."))
                    .desired_width(260.0),
            );
            let selected = filter
                .category
                .as_ref()
                .and_then(|c| categories.iter().find(|(code, _)| code == c))
                .map_or(t("Mọi nhóm").to_owned(), |(_, label)| {
                    tf("Nhóm: {}", &[&label])
                });
            egui::ComboBox::from_id_salt(("category", store))
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut filter.category, None, t("Mọi nhóm"));
                    for (code, label) in &categories {
                        ui.selectable_value(&mut filter.category, Some(code.clone()), label);
                    }
                });
            let (sort, choices): (&mut SortKey, &[SortKey]) = if store {
                (&mut prefs.store_sort, &SortKey::for_store())
            } else {
                (&mut prefs.library_sort, &SortKey::for_library())
            };
            egui::ComboBox::from_id_salt(("sort", store))
                .selected_text(tf("Sắp xếp: {}", &[&sort.label()]))
                .show_ui(ui, |ui| {
                    for key in choices {
                        ui.selectable_value(sort, *key, key.label());
                    }
                });
            if (!filter.text.is_empty() || filter.category.is_some())
                && icons::button(ui, Icon::Close, t("Xóa bộ lọc"), false, true).clicked()
            {
                *filter = Filter::default();
            }
        });
        self.set_prefs(prefs);
        ui.add_space(6.0);
    }

    fn query(&self, store: bool) -> library::Query {
        let (filter, sort) = if store {
            (&self.store_filter, self.prefs.store_sort)
        } else {
            (&self.library_filter, self.prefs.library_sort)
        };
        library::Query {
            text: filter.text.clone(),
            category: filter.category.clone(),
            sort,
        }
    }

    /// Hiện khi không có app nào khớp bộ lọc (scenario "Không có kết quả").
    fn empty_result(&mut self, ui: &mut egui::Ui, store: bool) {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(t("Không có app nào khớp"));
            if ui.button(t("Xóa bộ lọc")).clicked() {
                if store {
                    self.store_filter = Filter::default();
                } else {
                    self.library_filter = Filter::default();
                }
            }
        });
    }

    /// Chiều cao chung của mọi ô lưới: biểu tượng, tên, dòng phụ, mô tả (nếu bật), phiên bản, nút.
    fn tile_height(&self, ui: &egui::Ui) -> f32 {
        let body = ui.text_style_height(&egui::TextStyle::Body);
        let small = ui.text_style_height(&egui::TextStyle::Small);
        let gap = ui.spacing().item_spacing.y;
        let icon = (self.prefs.tile_size.width() * 0.42).clamp(48.0, 88.0);
        let button = ui.spacing().interact_size.y.max(28.0);
        let mut h = TILE_MARGIN * 2.0 + icon + 4.0 + body * 3.0 + button + gap * 5.0;
        if self.prefs.show_summary {
            h += small + gap;
        }
        h.ceil()
    }

    /// Lưới: mọi ô cùng kích thước, xếp liền nhau và trải đều chiều rộng (spec "Ô lưới đều nhau").
    fn grid<T: library::Listed>(
        &self,
        ui: &mut egui::Ui,
        items: &[&T],
        mut tile: impl FnMut(&Self, &mut egui::Ui, &T),
    ) -> Option<String> {
        let mut clicked = None;
        let gap = if self.prefs.compact { 8.0 } else { 12.0 };
        let min_width = self.prefs.tile_size.width() + TILE_MARGIN * 2.0;
        let height = self.tile_height(ui);
        let radius = f32::from(ui_style::current().shape().card_radius);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (columns, width) = grid_columns(ui.available_width(), min_width, gap);
                for row in items.chunks(columns) {
                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), height),
                        egui::Sense::hover(),
                    );
                    for (i, item) in row.iter().enumerate() {
                        let rect = egui::Rect::from_min_size(
                            row_rect.min + egui::vec2(i as f32 * (width + gap), 0.0),
                            egui::vec2(width, height),
                        );
                        let id = ui.scope_id().with(("tile", item.app_id()));
                        let response = ui.interact(rect, id, egui::Sense::click());
                        let visuals = ui.visuals();
                        let stroke = if response.hovered() {
                            egui::Stroke::new(1.5, visuals.selection.stroke.color)
                        } else {
                            visuals.widgets.noninteractive.bg_stroke
                        };
                        ui.painter().rect(
                            rect,
                            radius,
                            visuals.faint_bg_color,
                            stroke,
                            egui::StrokeKind::Inside,
                        );
                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(rect.shrink(TILE_MARGIN))
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        child.set_clip_rect(rect.intersect(ui.clip_rect()));
                        // Chữ trong ô không chọn được, để bấm vào đâu cũng mở chi tiết.
                        child.style_mut().interaction.selectable_labels = false;
                        tile(self, &mut child, item);
                        let response = response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(t("Xem chi tiết"));
                        if response.clicked() {
                            clicked = Some(item.app_id().to_owned());
                        }
                    }
                    ui.add_space(gap);
                }
            });
        clicked
    }

    fn list<T: library::Listed>(
        &self,
        ui: &mut egui::Ui,
        items: &[&T],
        mut row: impl FnMut(&Self, &mut egui::Ui, &T),
    ) -> Option<String> {
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for item in items {
                    let frame = if self.prefs.compact {
                        card_frame(ui).inner_margin(6.0)
                    } else {
                        card_frame(ui)
                    };
                    let card = ui.scope_builder(
                        egui::UiBuilder::new().sense(egui::Sense::click()),
                        |ui| {
                            ui.style_mut().interaction.selectable_labels = false;
                            frame.show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| row(self, ui, item));
                            });
                        },
                    );
                    let card = card
                        .response
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(t("Xem chi tiết"));
                    if card.clicked() {
                        clicked = Some(item.app_id().to_owned());
                    }
                    ui.add_space(if self.prefs.compact { 2.0 } else { 6.0 });
                }
            });
        clicked
    }

    /// Menu "..." của một app đã cài: cập nhật, quay lại, gỡ, mã nguồn.
    fn app_menu(&self, ui: &mut egui::Ui, app: &MyApp, actions: &mut Vec<Action>) {
        let idle = self.busy.is_none();
        let button = icons::button(ui, Icon::More, t("Thêm thao tác"), false, true);
        egui::Popup::menu(&button).show(|ui| {
            if let Some(u) = &app.update {
                if ui
                    .add_enabled(
                        idle,
                        egui::Button::new(tf("Cập nhật lên {}", &[&u.version])),
                    )
                    .clicked()
                {
                    actions.push(Action::Update(app.app.clone()));
                    ui.close();
                }
            }
            if let Some(prev) = &app.previous_version {
                if ui
                    .add_enabled(idle, egui::Button::new(tf("Quay lại {}", &[&prev])))
                    .clicked()
                {
                    actions.push(Action::Rollback(app.app.clone()));
                    ui.close();
                }
            }
            if let Some(source) = &app.info.source {
                if ui.button(t("Xem mã nguồn")).clicked() {
                    actions.push(Action::OpenLink(source.clone()));
                    ui.close();
                }
            }
            if ui
                .add_enabled(idle, egui::Button::new(t("Gỡ...")))
                .clicked()
            {
                actions.push(Action::AskUninstall(Box::new(app.clone())));
                ui.close();
            }
        });
    }

    fn open_button(&self, ui: &mut egui::Ui, app: &MyApp, actions: &mut Vec<Action>) {
        let ready = app.status == AppStatus::Ready;
        let label = if app.info.terminal {
            t("Mở (terminal)")
        } else {
            t("Mở")
        };
        if primary_button(
            ui,
            &self.prefs,
            self.busy.is_none() && ready,
            Icon::Play,
            label,
        ) {
            actions.push(Action::Open(app.app.clone()));
        }
    }

    fn library_page(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let pending = update_suggestions(&self.view.my_apps, &self.prefs.skipped_updates);
        if !pending.is_empty() {
            egui::Frame::group(ui.style())
                .stroke(egui::Stroke::new(1.0, accent(&self.prefs)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                        icons::paint(ui.painter(), rect, Icon::ArrowUp, accent(&self.prefs));
                        ui.label(tf(
                            "Có {} bản cập nhật mới cho app của bạn",
                            &[&pending.len()],
                        ));
                        if ui.button(t("Xem")).clicked() {
                            actions.push(Action::ShowUpdates);
                        }
                    });
                });
            ui.add_space(6.0);
        }
        self.toolbar(ui, actions);
        if self.view.my_apps.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(t("Chưa cài app nào."));
                if ui.button(t("Mở Cửa hàng")).clicked() {
                    self.page = Page::Store;
                }
            });
            return;
        }
        let apps = self.view.my_apps.clone();
        let query = self.query(false);
        let shown = library::arrange(&apps, &query);
        if shown.is_empty() {
            self.empty_result(ui, false);
            return;
        }
        let order: Vec<String> = shown.iter().map(|a| a.app.clone()).collect();
        let show_summary = self.prefs.show_summary;
        let clicked = match self.prefs.layout {
            Layout::Grid => self.grid(ui, &shown, |me, ui, app| {
                let icon = (me.prefs.tile_size.width() * 0.42).clamp(48.0, 88.0);
                ui.vertical_centered(|ui| app_icon(ui, &app.app, &app.title, icon));
                ui.add_space(4.0);
                ui.add(egui::Label::new(egui::RichText::new(&app.title).strong()).truncate());
                ui.add(egui::Label::new(egui::RichText::new(byline(&app.info)).weak()).truncate());
                if show_summary {
                    if let Some(s) = &app.info.summary {
                        ui.add(egui::Label::new(egui::RichText::new(s).small()).truncate());
                    }
                }
                if let AppStatus::Revoked { .. } = &app.status {
                    ui.colored_label(danger_color(ui), t("Đã bị thu hồi"));
                } else if let Some(u) = &app.update {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(tf("Có bản {}", &[&u.version])).color(warn_color(ui)),
                        )
                        .truncate(),
                    );
                } else {
                    ui.add(
                        egui::Label::new(egui::RichText::new(tf("Bản {}", &[&app.version])).weak())
                            .truncate(),
                    );
                }
                ui.horizontal(|ui| {
                    me.open_button(ui, app, actions);
                    me.app_menu(ui, app, actions);
                });
            }),
            Layout::List => self.list(ui, &shown, |me, ui, app| {
                let icon = if me.prefs.compact { 32.0 } else { 44.0 };
                app_icon(ui, &app.app, &app.title, icon);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&app.title);
                        ui.weak(&app.version);
                        if app.update.is_some() {
                            ui.colored_label(warn_color(ui), t("có bản cập nhật"));
                        }
                    });
                    if show_summary {
                        if let Some(s) = &app.info.summary {
                            ui.add(egui::Label::new(s).truncate());
                        }
                    }
                    if let AppStatus::Revoked { reason } = &app.status {
                        ui.colored_label(
                            danger_color(ui),
                            tf("Phiên bản này đã bị thu hồi: {}. Hãy cập nhật, quay lại bản trước hoặc gỡ app.", &[&reason]),
                        );
                    } else {
                        ui.weak(byline(&app.info));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    me.app_menu(ui, app, actions);
                    if let Some(u) = &app.update {
                        if ui
                            .add_enabled(me.busy.is_none(), egui::Button::new(tf("Cập nhật lên {}", &[&u.version])))
                            .clicked()
                        {
                            actions.push(Action::Update(app.app.clone()));
                        }
                    }
                    me.open_button(ui, app, actions);
                });
            }),
        };
        if let Some(app) = clicked {
            actions.push(Action::ShowDetail(Detail {
                key: 0,
                app,
                store: false,
                order,
            }));
        }
    }

    /// Nút chính của một app trong Cửa hàng: Cài, Cập nhật, hoặc nhãn "Đã cài".
    fn store_button(&self, ui: &mut egui::Ui, entry: &CatalogEntry, actions: &mut Vec<Action>) {
        let installed = self.view.my_apps.iter().find(|a| a.app == entry.app);
        if installed.is_none() {
            self.acquire_button(ui, entry, self.busy.is_none() && self.view.online, actions);
            return;
        }
        let label = match installed {
            Some(a) if a.version == entry.version => None,
            Some(_) => Some((Icon::ArrowUp, t("Cập nhật"))),
            None => Some((Icon::Download, t("Cài"))),
        };
        match label {
            Some((icon, label)) => {
                if primary_button(
                    ui,
                    &self.prefs,
                    self.busy.is_none() && self.view.online,
                    icon,
                    label,
                ) {
                    actions.push(Action::AskInstall(Box::new(entry.clone())));
                }
            }
            None => {
                ui.weak(if entry.is_web() {
                    t("Đã thêm")
                } else {
                    t("Đã cài")
                });
            }
        }
    }

    /// Nút Cài, Mua · N F-Coin, hoặc Đăng nhập để mua cho app chưa cài.
    fn acquire_button(
        &self,
        ui: &mut egui::Ui,
        entry: &CatalogEntry,
        enabled: bool,
        actions: &mut Vec<Action>,
    ) {
        match acquire(&self.view, &entry.app) {
            Acquire::Install => {
                if primary_button(
                    ui,
                    &self.prefs,
                    enabled,
                    Icon::Download,
                    install_label(entry),
                ) {
                    actions.push(Action::AskInstall(Box::new(entry.clone())));
                }
            }
            Acquire::Buy(price) => {
                let label = format!("{} · {}", t("Mua"), coins(price));
                if primary_button(ui, &self.prefs, enabled, Icon::Coin, &label) {
                    actions.push(Action::AskBuy(Box::new(entry.clone()), price));
                }
            }
            Acquire::SignInToBuy(price) => {
                ui.weak(coins(price));
                if primary_button(
                    ui,
                    &self.prefs,
                    enabled,
                    Icon::SignIn,
                    t("Đăng nhập để mua"),
                ) {
                    actions.push(Action::Login);
                }
            }
        }
    }

    fn store_page(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if !self.view.online {
            ui.colored_label(
                warn_color(ui),
                t("Đang ngoại tuyến. Danh mục sẽ cập nhật khi có mạng; các app đã cài vẫn mở được."),
            );
        }
        self.toolbar(ui, actions);
        if self.view.catalog.is_empty() {
            ui.label(t("Danh mục trống."));
            return;
        }
        let catalog = self.view.catalog.clone();
        let query = self.query(true);
        let shown = library::arrange(&catalog, &query);
        if shown.is_empty() {
            self.empty_result(ui, true);
            return;
        }
        let order: Vec<String> = shown.iter().map(|e| e.app.clone()).collect();
        let show_summary = self.prefs.show_summary;
        let clicked = match self.prefs.layout {
            Layout::Grid => self.grid(ui, &shown, |me, ui, entry| {
                let icon = (me.prefs.tile_size.width() * 0.42).clamp(48.0, 88.0);
                ui.vertical_centered(|ui| app_icon(ui, &entry.app, &entry.title, icon));
                ui.add_space(4.0);
                ui.add(egui::Label::new(egui::RichText::new(&entry.title).strong()).truncate());
                ui.add(
                    egui::Label::new(egui::RichText::new(byline(&entry.info)).weak()).truncate(),
                );
                if show_summary {
                    if let Some(s) = &entry.info.summary {
                        ui.add(egui::Label::new(egui::RichText::new(s).small()).truncate());
                    }
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{} · {}", entry.version, entry_size(entry)))
                            .weak(),
                    )
                    .truncate(),
                );
                me.store_button(ui, entry, actions);
            }),
            Layout::List => self.list(ui, &shown, |me, ui, entry| {
                let icon = if me.prefs.compact { 32.0 } else { 44.0 };
                app_icon(ui, &entry.app, &entry.title, icon);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&entry.title);
                        ui.weak(&entry.version);
                        if entry.info.terminal {
                            ui.weak(t("· chạy trong terminal"));
                        }
                    });
                    if show_summary {
                        if let Some(s) = &entry.info.summary {
                            ui.add(egui::Label::new(s).truncate());
                        }
                    }
                    ui.weak(format!("{} · {}", byline(&entry.info), entry_size(entry)));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(source) = &entry.info.source {
                        if icons::button(ui, Icon::Code, link_label(source), false, true).clicked()
                        {
                            actions.push(Action::OpenLink(source.clone()));
                        }
                    }
                    me.store_button(ui, entry, actions);
                });
            }),
        };
        if let Some(app) = clicked {
            actions.push(Action::ShowDetail(Detail {
                key: 0,
                app,
                store: true,
                order,
            }));
        }
    }

    /// Trang Cài đặt (spec launcher, "Cài đặt giao diện của launcher").
    fn settings_page(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let mut prefs = self.prefs.clone();
        let idle = self.busy.is_none();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading(t("Cài đặt"));
                ui.weak(t(
                    "Cài đặt giao diện lưu trên máy này, không gửi lên máy chủ.",
                ));
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong(t("Giao diện"));
                    egui::Grid::new("language")
                        .num_columns(2)
                        .spacing([24.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(t("Ngôn ngữ"));
                            ui.horizontal(|ui| {
                                // Tên ngôn ngữ luôn viết bằng chính ngôn ngữ đó.
                                for lang in Language::ALL {
                                    ui.selectable_value(
                                        &mut prefs.language,
                                        lang,
                                        lang.native_name(),
                                    );
                                }
                            });
                            ui.end_row();
                        });
                    ui.add_space(8.0);
                    ui.label(t("Phong cách"));
                    style_gallery(ui, &mut prefs);
                    ui.add_space(8.0);
                    let follows = prefs.style.follows_theme();
                    if !follows {
                        ui.weak(t(
                            "Phong cách này dùng bảng màu riêng; chủ đề và màu nhấn không áp dụng.",
                        ));
                    }
                    egui::Grid::new("appearance")
                        .num_columns(2)
                        .spacing([24.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(t("Chủ đề"));
                            ui.add_enabled_ui(follows, |ui| {
                                ui.horizontal(|ui| {
                                    for theme in Theme::ALL {
                                        ui.selectable_value(&mut prefs.theme, theme, theme.label());
                                    }
                                });
                            });
                            ui.end_row();

                            ui.label(t("Màu nhấn"));
                            ui.add_enabled_ui(follows, |ui| {
                                ui.horizontal(|ui| {
                                    for a in Accent::ALL {
                                        let [r, g, b] = a.rgb();
                                        let color = egui::Color32::from_rgb(r, g, b);
                                        // Màu đang chọn có viền đậm theo màu chữ (không dùng ký hiệu
                                        // để không phụ thuộc phông chữ hệ thống).
                                        let stroke = if prefs.accent == a {
                                            egui::Stroke::new(3.0, ui.visuals().strong_text_color())
                                        } else {
                                            egui::Stroke::NONE
                                        };
                                        let button = egui::Button::new("")
                                            .fill(color)
                                            .stroke(stroke)
                                            .min_size(egui::vec2(26.0, 26.0));
                                        if ui.add(button).on_hover_text(a.label()).clicked() {
                                            prefs.accent = a;
                                        }
                                    }
                                })
                            });
                            ui.end_row();

                            ui.label(t("Cỡ chữ"));
                            let mut percent = (prefs.font_scale * 100.0).round();
                            let min = feco_launcher::prefs::FONT_SCALE_MIN * 100.0;
                            let max = feco_launcher::prefs::FONT_SCALE_MAX * 100.0;
                            let slider = egui::Slider::new(&mut percent, min..=max)
                                .step_by(10.0)
                                .fixed_decimals(0)
                                .suffix("%");
                            // Đổi cỡ chữ khi thả chuột, để thanh trượt không chạy theo con trỏ.
                            let response = ui.add(slider);
                            if response.drag_stopped()
                                || (response.changed() && !response.dragged())
                            {
                                prefs.font_scale = percent / 100.0;
                            }
                            ui.end_row();

                            ui.label(t("Bố cục mặc định"));
                            ui.horizontal(|ui| {
                                for l in [Layout::Grid, Layout::List] {
                                    ui.selectable_value(&mut prefs.layout, l, l.label());
                                }
                            });
                            ui.end_row();

                            ui.label(t("Cỡ ô lưới"));
                            ui.horizontal(|ui| {
                                for s in TileSize::ALL {
                                    ui.selectable_value(&mut prefs.tile_size, s, s.label());
                                }
                            });
                            ui.end_row();

                            ui.label(t("Trang mở đầu"));
                            ui.horizontal(|ui| {
                                for p in [StartPage::Library, StartPage::Store] {
                                    ui.selectable_value(&mut prefs.start_page, p, p.label());
                                }
                            });
                            ui.end_row();

                            ui.label(t("Hiển thị"));
                            ui.vertical(|ui| {
                                ui.checkbox(&mut prefs.show_summary, t("Hiện mô tả ngắn của app"));
                                ui.checkbox(&mut prefs.compact, t("Chế độ gọn (ít khoảng trống)"));
                            });
                            ui.end_row();
                        });
                    ui.add_space(6.0);
                    if ui.button(t("Khôi phục mặc định")).clicked() {
                        prefs = Prefs::default();
                    }
                });
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong(t("Cập nhật"));
                    let mut auto = self.view.auto_update;
                    if ui
                        .checkbox(
                            &mut auto,
                            t("Tự động cập nhật app (kiểm tra khi mở và mỗi 6 giờ)"),
                        )
                        .changed()
                    {
                        actions.push(Action::SetAutoUpdate(auto));
                    }
                    if ui
                        .add_enabled(idle, egui::Button::new(t("Kiểm tra cập nhật ngay")))
                        .clicked()
                    {
                        actions.push(Action::Sync);
                    }
                });
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong(t("Tài khoản"));
                    match self.view.sign_in {
                        Some(SignIn::SignedIn) | Some(SignIn::Offline) => {
                            ui.label(tf(
                                "Đã đăng nhập: {}",
                                &[&self.view.user.clone().unwrap_or_default()],
                            ));
                            if let Some(role) = &self.view.role {
                                ui.label(tf("Vai trò: {}", &[&role_label(&role.role)]));
                            }
                            ui.horizontal(|ui| {
                                if ui.button(t("Ứng dụng đã kết nối")).clicked() {
                                    if let Ok(url) = self.shared.config.server.join("apps") {
                                        actions.push(Action::OpenLink(url.to_string()));
                                    }
                                }
                                if ui
                                    .add_enabled(idle, egui::Button::new(t("Đăng xuất")))
                                    .clicked()
                                {
                                    actions.push(Action::Logout);
                                }
                            });
                        }
                        _ => {
                            ui.label(t("Chưa đăng nhập."));
                            if primary_button(ui, &self.prefs, idle, Icon::SignIn, t("Đăng nhập"))
                            {
                                actions.push(Action::Login);
                            }
                        }
                    }
                });
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong(t("Giới thiệu"));
                    egui::Grid::new("about").num_columns(2).show(ui, |ui| {
                        ui.label(t("Phiên bản"));
                        ui.label(env!("CARGO_PKG_VERSION"));
                        ui.end_row();
                        ui.label(t("Nền tảng"));
                        ui.label(&self.shared.config.platform);
                        ui.end_row();
                        ui.label(t("Thư mục dữ liệu"));
                        ui.label(self.shared.config.data_dir.display().to_string());
                        ui.end_row();
                    });
                    ui.weak(tf(
                        "{}. Mọi gói cài đặt được kiểm tra chữ ký của F-Eco trước khi cài.",
                        &[&t(ISOLATION_NOTICE)],
                    ));
                });
            });
        self.set_prefs(prefs);
    }
}

// ---- Trang chi tiết app (spec launcher: "Trang chi tiết app trong launcher", "Bảng thông tin và
// ---- nút hành động của trang chi tiết", "Chuyển sang app trước và sau").

/// Trang chi tiết đang mở: app, danh sách gốc (Cửa hàng hay Thư viện) và thứ tự app đang hiển
/// thị lúc mở (sau tìm, lọc, sắp xếp) để chuyển app trước và sau.
#[derive(Clone, Debug)]
struct Detail {
    /// Mã cửa sổ (không đổi khi chuyển sang app trước/sau trong cùng cửa sổ).
    key: u64,
    app: String,
    store: bool,
    order: Vec<String>,
}

impl Detail {
    fn viewport(&self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("feco-detail", self.key))
    }
}

fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(text).strong().size(16.0));
    ui.add_space(2.0);
}

fn link_label(url: &str) -> &'static str {
    if url.contains("github.com") {
        t("Mã nguồn (GitHub)")
    } else {
        t("Mã nguồn")
    }
}

impl LauncherApp {
    /// Tên hiển thị của một app (trong danh mục hoặc đã cài), cho nút Trước/Sau.
    fn title_of(&self, app: &str) -> String {
        self.view
            .catalog
            .iter()
            .find(|e| e.app == app)
            .map(|e| e.title.clone())
            .or_else(|| {
                self.view
                    .my_apps
                    .iter()
                    .find(|a| a.app == app)
                    .map(|a| a.title.clone())
            })
            .unwrap_or_else(|| app.to_owned())
    }

    fn detail_page(&mut self, ui: &mut egui::Ui, detail: &Detail, actions: &mut Vec<Action>) {
        let entry = self
            .view
            .catalog
            .iter()
            .find(|e| e.app == detail.app)
            .cloned();
        let installed = self
            .view
            .my_apps
            .iter()
            .find(|a| a.app == detail.app)
            .cloned();
        let (prev, next) = library::neighbors(&detail.order, &detail.app);
        let (prev, next) = (prev.map(str::to_owned), next.map(str::to_owned));

        // Phím tắt: trái/phải chuyển app, Esc đóng cửa sổ (bỏ qua khi đang gõ vào ô nhập liệu).
        if !ui.ctx().egui_wants_keyboard_input() {
            let (left, right, esc) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                    i.key_pressed(egui::Key::Escape),
                )
            });
            if esc {
                actions.push(Action::CloseDetail(detail.key));
            } else if let (true, Some(p)) = (left, &prev) {
                actions.push(Action::MoveDetail(detail.key, p.clone()));
            } else if let (true, Some(n)) = (right, &next) {
                actions.push(Action::MoveDetail(detail.key, n.clone()));
            }
        }

        // Thanh trên: nguồn (Cửa hàng hay Thư viện), app trước và sau.
        ui.horizontal(|ui| {
            ui.weak(if detail.store {
                t("Cửa hàng")
            } else {
                t("Thư viện")
            });
            if let Some(i) = detail.order.iter().position(|a| *a == detail.app) {
                ui.weak(format!("{}/{}", i + 1, detail.order.len()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let next_tip = match &next {
                    Some(n) => tf("Sau: {} ›", &[&self.title_of(n)]),
                    None => t("Sau ›").to_owned(),
                };
                if icons::button(
                    ui,
                    Icon::ChevronRight,
                    &format!("{next_tip} ({})", t("Phím mũi tên phải")),
                    false,
                    next.is_some(),
                )
                .clicked()
                {
                    if let Some(n) = &next {
                        actions.push(Action::MoveDetail(detail.key, n.clone()));
                    }
                }
                let prev_tip = match &prev {
                    Some(p) => tf("‹ Trước: {}", &[&self.title_of(p)]),
                    None => t("‹ Trước").to_owned(),
                };
                if icons::button(
                    ui,
                    Icon::ChevronLeft,
                    &format!("{prev_tip} ({})", t("Phím mũi tên trái")),
                    false,
                    prev.is_some(),
                )
                .clicked()
                {
                    if let Some(p) = &prev {
                        actions.push(Action::MoveDetail(detail.key, p.clone()));
                    }
                }
            });
        });
        ui.separator();

        let (title, info) = match (&entry, &installed) {
            (Some(e), _) => (e.title.clone(), e.info.clone()),
            (None, Some(a)) => (a.title.clone(), a.info.clone()),
            (None, None) => {
                ui.add_space(24.0);
                ui.label(t("App này không còn trong danh mục."));
                return;
            }
        };
        let idle = self.busy.is_none();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Phần đầu: biểu tượng, tên, nhà phát triển, mô tả ngắn, nút hành động.
                ui.horizontal_top(|ui| {
                    app_icon(ui, &detail.app, &title, 96.0);
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&title).size(26.0).strong());
                        ui.weak(byline(&info));
                        if let Some(s) = &info.summary {
                            ui.label(s);
                        }
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            match &installed {
                                Some(app) => {
                                    self.open_button(ui, app, actions);
                                    if let Some(u) = &app.update {
                                        if ui
                                            .add_enabled(
                                                idle && self.view.online,
                                                egui::Button::new(tf(
                                                    "Cập nhật lên {}",
                                                    &[&u.version],
                                                )),
                                            )
                                            .clicked()
                                        {
                                            actions.push(Action::Update(app.app.clone()));
                                        }
                                    }
                                    if let Some(prev) = &app.previous_version {
                                        if icons::button(
                                            ui,
                                            Icon::Undo,
                                            &tf("Quay lại {}", &[&prev]),
                                            false,
                                            idle,
                                        )
                                        .clicked()
                                        {
                                            actions.push(Action::Rollback(app.app.clone()));
                                        }
                                    }
                                    if icons::button(ui, Icon::Trash, t("Gỡ..."), false, idle)
                                        .clicked()
                                    {
                                        actions.push(Action::AskUninstall(Box::new(app.clone())));
                                    }
                                }
                                None => {
                                    if let Some(e) = &entry {
                                        self.acquire_button(
                                            ui,
                                            e,
                                            idle && self.view.online,
                                            actions,
                                        );
                                    }
                                }
                            }
                            if let Some(source) = &info.source {
                                if icons::button(
                                    ui,
                                    Icon::Code,
                                    &format!("{}: {source}", link_label(source)),
                                    false,
                                    true,
                                )
                                .clicked()
                                {
                                    actions.push(Action::OpenLink(source.clone()));
                                }
                            }
                            if let Some(site) = &info.website {
                                if icons::button(
                                    ui,
                                    Icon::Globe,
                                    &format!("{}: {site}", t("Website")),
                                    false,
                                    true,
                                )
                                .clicked()
                                {
                                    actions.push(Action::OpenLink(site.clone()));
                                }
                            }
                        });
                        if let Some(app) = &installed {
                            if let AppStatus::Revoked { reason } = &app.status {
                                ui.colored_label(
                                    danger_color(ui),
                                    tf("Phiên bản đã cài bị thu hồi: {}", &[&reason]),
                                );
                            }
                        }
                    });
                });

                // Giới thiệu và tính năng.
                if let Some(d) = info
                    .description
                    .as_ref()
                    .filter(|d| Some(*d) != info.summary.as_ref())
                {
                    section_title(ui, t("Giới thiệu"));
                    ui.label(d);
                }
                if !info.features.is_empty() {
                    section_title(ui, t("Tính năng"));
                    for f in &info.features {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("•");
                            ui.label(f);
                        });
                    }
                }

                // Có gì mới trong bản mới nhất.
                if let Some(notes) = &info.release_notes {
                    let version = entry
                        .as_ref()
                        .map(|e| e.version.clone())
                        .or_else(|| installed.as_ref().map(|a| a.version.clone()))
                        .unwrap_or_default();
                    section_title(ui, &tf("Có gì mới trong bản {}", &[&version]));
                    if let Some(date) = info.released_at.as_deref().and_then(library::release_date)
                    {
                        ui.weak(tf("Phát hành ngày {}", &[&date]));
                    }
                    ui.label(notes);
                }

                // Bảng thông tin.
                section_title(ui, t("Thông tin"));
                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::Grid::new(("detail-info", &detail.app))
                        .num_columns(2)
                        .spacing([32.0, 8.0])
                        .show(ui, |ui| {
                            let row = |ui: &mut egui::Ui, k: &str, v: String| {
                                ui.weak(k);
                                ui.label(v);
                                ui.end_row();
                            };
                            if let Some(e) = &entry {
                                row(ui, t("Phiên bản mới nhất"), e.version.clone());
                            }
                            match &installed {
                                Some(a) => row(ui, t("Đã cài"), a.version.clone()),
                                None => row(ui, t("Đã cài"), t("Chưa cài").into()),
                            }
                            if let Some(date) =
                                info.released_at.as_deref().and_then(library::release_date)
                            {
                                row(ui, t("Ngày phát hành"), date);
                            }
                            if let Some(e) = &entry {
                                match e.info.web_url.as_deref().filter(|_| e.is_web()) {
                                    Some(url) => row(ui, t("App web"), url.to_owned()),
                                    None => row(ui, t("Dung lượng tải về"), size_mb(e.length)),
                                }
                                let platforms = if e.platforms.is_empty() {
                                    library::platform_label(&e.platform)
                                } else {
                                    e.platforms
                                        .iter()
                                        .map(|p| library::platform_label(p))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                };
                                row(ui, t("Chạy trên"), platforms);
                            }
                            if let Some(d) = &info.developer {
                                row(ui, t("Nhà phát triển"), d.clone());
                            }
                            row(
                                ui,
                                t("Nhóm"),
                                library::category_label(&library::category_of(&info)),
                            );
                            if let Some(l) = &info.license {
                                row(ui, t("Giấy phép"), l.clone());
                            }
                            let terminal = if info.terminal {
                                t("Trong cửa sổ terminal")
                            } else {
                                t("Cửa sổ riêng của app")
                            };
                            row(ui, t("Cách chạy"), terminal.into());
                            let sso = entry
                                .as_ref()
                                .map(|e| e.client_id.is_some())
                                .unwrap_or(false);
                            row(
                                ui,
                                t("Tài khoản F-Eco"),
                                if sso {
                                    t("Đăng nhập một lần qua F-Launcher").into()
                                } else {
                                    t("Không dùng").into()
                                },
                            );
                            if let Some(source) = &info.source {
                                ui.weak(t("Mã nguồn"));
                                ui.hyperlink_to(source, source);
                                ui.end_row();
                            }
                            if let Some(site) = &info.website {
                                ui.weak(t("Website"));
                                ui.hyperlink_to(site, site);
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(6.0);
                ui.weak(tf(
                    "{}. Gói được kiểm tra chữ ký của F-Eco trước khi cài.",
                    &[&t(ISOLATION_NOTICE)],
                ));
            });
    }
}

// ---- Tin nhắn (thay đổi chat-and-source-scan: spec launcher "Tin nhắn trong F-Launcher").

/// Giờ địa phương của thời điểm RFC 3339 máy chủ trả: hôm nay thì chỉ giờ, ngày khác kèm ngày.
fn chat_time(at: &str) -> String {
    match at.parse::<Timestamp>() {
        Ok(ts) => {
            let tz = jiff::tz::TimeZone::system();
            let local = ts.to_zoned(tz.clone());
            if local.date() == Timestamp::now().to_zoned(tz).date() {
                local.strftime("%H:%M").to_string()
            } else {
                local.strftime("%H:%M %d/%m/%Y").to_string()
            }
        }
        Err(_) => at.chars().take(16).collect(),
    }
}

fn kind_label(kind: chat::Kind) -> &'static str {
    match kind {
        chat::Kind::Project => t("Dự án"),
        chat::Kind::Direct => t("Trực tiếp"),
    }
}

async fn chat_token(s: &Shared) -> Result<String, ChatError> {
    s.identity.access_token().await.map_err(ChatError::Auth)
}

/// Một tin trong luồng: của mình bên phải (màu nhấn), của bên kia bên trái, của quản trị có viền
/// và nhãn riêng, tin hệ thống ở giữa (chữ nghiêng).
fn chat_bubble(ui: &mut egui::Ui, m: &Message, prefs: &Prefs, actions: &mut Vec<Action>) {
    let time = chat_time(&m.at);
    if m.side == chat::Side::System {
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(&m.body).italics().weak());
            ui.label(egui::RichText::new(time).small().weak());
        });
        ui.add_space(6.0);
        return;
    }
    let max = (ui.available_width() * 0.72).max(160.0);
    let admin = m.side == chat::Side::Admin;
    let warn = warn_color(ui);
    let (fill, color, align) = if m.mine {
        (accent(prefs), on_accent(prefs), egui::Align::Max)
    } else if admin {
        (
            warn.gamma_multiply(0.18),
            ui.visuals().text_color(),
            egui::Align::Min,
        )
    } else {
        (
            ui.visuals().widgets.inactive.weak_bg_fill,
            ui.visuals().text_color(),
            egui::Align::Min,
        )
    };
    let stroke = if admin {
        egui::Stroke::new(1.0, warn)
    } else {
        egui::Stroke::NONE
    };
    ui.with_layout(egui::Layout::top_down(align), |ui| {
        egui::Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(10.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_max_width(max);
                if admin {
                    ui.label(
                        egui::RichText::new(t("Quản trị F-Eco"))
                            .small()
                            .strong()
                            .color(warn),
                    );
                }
                ui.add(egui::Label::new(egui::RichText::new(&m.body).color(color)).wrap());
                if let Some(a) = &m.attachment {
                    chat_attachment(ui, a, color, actions);
                }
                ui.label(
                    egui::RichText::new(time)
                        .small()
                        .color(color.gamma_multiply(0.75)),
                );
            });
    });
    ui.add_space(6.0);
}

/// Tệp đính kèm: tên, dung lượng, trạng thái quét; chỉ tệp sạch mới mở được (bằng trình duyệt).
fn chat_attachment(
    ui: &mut egui::Ui,
    a: &chat::Attachment,
    color: egui::Color32,
    actions: &mut Vec<Action>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!("{} · {}", a.name, chat::file_size(a.size)))
                .strong()
                .color(color),
        );
        ui.label(
            egui::RichText::new(chat::scan_label(a.status))
                .small()
                .color(color),
        );
        if let Some(url) = a.open_url() {
            if ui
                .button(t("Mở tệp"))
                .on_hover_text(t("Mở bằng trình duyệt"))
                .clicked()
            {
                actions.push(Action::OpenLink(url.to_owned()));
            }
        }
    });
}

impl LauncherApp {
    /// Chuyển trang; vào trang Tin nhắn thì tải lại danh sách hội thoại và số chưa đọc.
    fn go(&mut self, page: Page) {
        if page == Page::Messages && self.page != Page::Messages {
            self.chat_load_list();
            self.chat.schedule.refresh_unread_now(Instant::now());
        }
        self.page = page;
    }

    /// Chạy một lời gọi chat trên runtime nền; kết quả về qua `chat_rx`.
    fn chat_task<Fut>(&self, fut: Fut)
    where
        Fut: Future<Output = ChatDone> + Send + 'static,
    {
        let tx = self.chat_tx.clone();
        let ctx = self.ctx.clone();
        self.rt.spawn(async move {
            let _ = tx.send(fut.await);
            ctx.request_repaint();
        });
    }

    fn chat_load_list(&mut self) {
        if self.chat.listing || !self.signed_in() {
            return;
        }
        self.chat.listing = true;
        let s = self.shared.clone();
        self.chat_task(async move {
            let result = async {
                let token = chat_token(&s).await?;
                s.identity.auth.chat_conversations(&token).await
            }
            .await;
            ChatDone::Conversations(result)
        });
    }

    /// Lịch hỏi máy chủ (design C3): số chưa đọc mỗi 60 giây khi đã đăng nhập, tin mới mỗi 5 giây
    /// khi đang mở hội thoại. Trả thời gian tới lần hỏi kế tiếp.
    fn chat_tick(&mut self) -> Option<Duration> {
        self.chat_drain();
        let now = Instant::now();
        let signed_in = self.signed_in();
        if !signed_in && self.chat.has_data() {
            // Đăng xuất: bỏ tin của tài khoản cũ.
            self.chat = ChatState::default();
        }
        let open = if self.page == Page::Messages {
            self.chat.selected.clone()
        } else {
            None
        };
        self.chat.schedule.set_open(open.as_deref(), now);
        for due in self.chat.schedule.due(now, signed_in) {
            match due {
                Due::Unread => {
                    let s = self.shared.clone();
                    self.chat_task(async move {
                        let result = async {
                            let token = chat_token(&s).await?;
                            s.identity.auth.chat_unread(&token).await
                        }
                        .await;
                        ChatDone::Unread(result)
                    });
                    if self.page == Page::Messages {
                        self.chat_load_list();
                    }
                }
                Due::Poll(id) => {
                    let after = self
                        .chat
                        .thread
                        .as_ref()
                        .filter(|t| t.conversation == id)
                        .map_or(0, Thread::poll_after);
                    let s = self.shared.clone();
                    self.chat_task(async move {
                        let result = async {
                            let token = chat_token(&s).await?;
                            s.identity
                                .auth
                                .chat_messages_after(&token, &id, after)
                                .await
                        }
                        .await;
                        ChatDone::Messages {
                            conversation: id,
                            result,
                        }
                    });
                }
            }
        }
        self.chat.schedule.wake_in(now, signed_in)
    }

    fn chat_drain(&mut self) {
        while let Ok(done) = self.chat_rx.try_recv() {
            let now = Instant::now();
            match done {
                ChatDone::Unread(result) => {
                    self.chat.schedule.unread_done(now);
                    match result {
                        Ok(n) => self.chat.unread = n,
                        Err(e) => eprintln!("không lấy được số tin chưa đọc: {e}"),
                    }
                }
                ChatDone::Conversations(result) => {
                    self.chat.listing = false;
                    match result {
                        Ok(list) => {
                            self.chat.conversations = list;
                            self.chat.listed = true;
                            if let Some(id) = self.chat.selected.clone() {
                                self.chat.touch(&id);
                            }
                        }
                        Err(e) => self.chat.error = Some(e.message()),
                    }
                }
                ChatDone::Messages {
                    conversation,
                    result,
                } => {
                    self.chat.schedule.poll_done(&conversation, now);
                    let Some(thread) = self
                        .chat
                        .thread
                        .as_mut()
                        .filter(|t| t.conversation == conversation)
                    else {
                        continue;
                    };
                    match result {
                        Ok(page) => {
                            thread.apply(page);
                            let due = thread.read_due();
                            self.chat.touch(&conversation);
                            if let Some(up_to) = due {
                                self.chat_mark_read(conversation, up_to);
                            }
                        }
                        Err(e) => self.chat.error = Some(e.message()),
                    }
                }
                ChatDone::Sent {
                    conversation,
                    body,
                    result,
                } => {
                    self.chat.sending = false;
                    match result {
                        Ok(message) => {
                            if let Some(thread) = self
                                .chat
                                .thread
                                .as_mut()
                                .filter(|t| t.conversation == conversation)
                            {
                                thread.push_sent(message);
                            }
                            self.chat.error = None;
                            self.chat.touch(&conversation);
                            // Hội thoại vừa có tin lên đầu danh sách (mới nhất trước).
                            if let Some(i) = self
                                .chat
                                .conversations
                                .iter()
                                .position(|c| c.id == conversation)
                            {
                                let c = self.chat.conversations.remove(i);
                                self.chat.conversations.insert(0, c);
                            }
                        }
                        Err(e) => {
                            if matches!(&e, ChatError::Api { code, .. } if code == "chat_blocked") {
                                if let Some(c) = self
                                    .chat
                                    .conversations
                                    .iter_mut()
                                    .find(|c| c.id == conversation)
                                {
                                    c.blocked = true;
                                }
                            }
                            // Gửi không được: trả lại nội dung vào ô soạn tin.
                            if self.chat.selected.as_deref() == Some(conversation.as_str())
                                && self.chat.draft.is_empty()
                            {
                                self.chat.draft = body;
                            }
                            self.chat.error = Some(e.message());
                        }
                    }
                }
                ChatDone::Read {
                    conversation,
                    result,
                } => match result {
                    Ok(()) => {
                        if let Some(c) = self
                            .chat
                            .conversations
                            .iter_mut()
                            .find(|c| c.id == conversation)
                        {
                            c.unread = 0;
                        }
                        self.chat.schedule.refresh_unread_now(now);
                    }
                    Err(e) => eprintln!("không đánh dấu đã đọc được: {e}"),
                },
            }
        }
    }

    fn chat_mark_read(&mut self, conversation: String, up_to: i64) {
        if let Some(thread) = self.chat.thread.as_mut() {
            thread.read_up_to = thread.read_up_to.max(up_to);
        }
        let s = self.shared.clone();
        self.chat_task(async move {
            let result = async {
                let token = chat_token(&s).await?;
                s.identity
                    .auth
                    .chat_read(&token, &conversation, up_to)
                    .await
            }
            .await;
            ChatDone::Read {
                conversation,
                result,
            }
        });
    }

    fn chat_select(&mut self, id: &str) {
        if self.chat.selected.as_deref() == Some(id) {
            return;
        }
        self.chat.selected = Some(id.to_owned());
        self.chat.thread = Some(Thread::new(id));
        self.chat.draft.clear();
        self.chat.error = None;
        self.chat.sending = false;
    }

    fn chat_send(&mut self) {
        let Some(id) = self.chat.selected.clone() else {
            return;
        };
        if self.chat.sending {
            return;
        }
        let body = match chat::validate_body(&self.chat.draft) {
            Ok(body) => body,
            Err(code) => {
                self.chat.error = Some(chat::error_message(code).to_owned());
                return;
            }
        };
        self.chat.draft.clear();
        self.chat.sending = true;
        self.chat.error = None;
        let s = self.shared.clone();
        self.chat_task(async move {
            let result = async {
                let token = chat_token(&s).await?;
                s.identity.auth.chat_send(&token, &id, &body).await
            }
            .await;
            ChatDone::Sent {
                conversation: id,
                body,
                result,
            }
        });
    }

    /// Trang Tin nhắn: danh sách hội thoại bên trái, luồng tin của hội thoại đang chọn bên phải.
    fn messages_page(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if !self.signed_in() {
            ui.heading(t("Tin nhắn"));
            ui.add_space(6.0);
            ui.label(t(
                "Đăng nhập để nhắn tin với nhà phát triển và bên nhận dự án của bạn.",
            ));
            ui.add_space(8.0);
            if primary_button(
                ui,
                &self.prefs,
                self.busy.is_none(),
                Icon::SignIn,
                t("Đăng nhập"),
            ) {
                actions.push(Action::Login);
            }
            return;
        }
        if !self.chat.listed && !self.chat.listing && self.chat.error.is_none() {
            self.chat_load_list();
        }
        egui::Panel::left("chat_list")
            .resizable(true)
            .default_size(280.0)
            .size_range(200.0..=440.0)
            .show(ui, |ui| self.conversation_list(ui));
        self.thread_view(ui, actions);
    }

    fn conversation_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(t("Tin nhắn"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, Icon::Sync, t("Tải lại"), false, !self.chat.listing).clicked()
                {
                    self.chat.error = None;
                    self.chat_load_list();
                    self.chat.schedule.refresh_unread_now(Instant::now());
                }
            });
        });
        ui.add_space(4.0);
        if self.chat.conversations.is_empty() {
            if self.chat.listing {
                ui.spinner();
            } else {
                ui.weak(t("Chưa có hội thoại nào. Bấm \"Nhắn nhà phát triển\" ở trang app trên web để bắt đầu."));
            }
            return;
        }
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for c in &self.chat.conversations {
                    if self.conversation_row(ui, c) {
                        clicked = Some(c.id.clone());
                    }
                }
            });
        if let Some(id) = clicked {
            self.chat_select(&id);
        }
    }

    /// Một dòng của danh sách: tiêu đề, loại và tên bên kia, dòng cuối, số chưa đọc, thời gian.
    fn conversation_row(&self, ui: &mut egui::Ui, c: &Conversation) -> bool {
        let selected = self.chat.selected.as_deref() == Some(c.id.as_str());
        let fill = if selected {
            ui.visuals().selection.bg_fill.gamma_multiply(0.35)
        } else {
            ui.visuals().faint_bg_color
        };
        let inner = egui::Frame::new()
            .fill(fill)
            .corner_radius(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if c.unread > 0 {
                            ui.label(
                                egui::RichText::new(format!(" {} ", c.unread))
                                    .small()
                                    .strong()
                                    .color(on_accent(&self.prefs))
                                    .background_color(accent(&self.prefs)),
                            );
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let title = egui::RichText::new(&c.title);
                            let title = if c.unread > 0 { title.strong() } else { title };
                            ui.add(egui::Label::new(title).truncate());
                        });
                    });
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "{} · {}",
                                kind_label(c.kind),
                                c.peer_name
                            ))
                            .small()
                            .weak(),
                        )
                        .truncate(),
                    );
                });
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(chat_time(&c.last_message_at))
                                .small()
                                .weak(),
                        );
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let last = c.last_message.clone().unwrap_or_default();
                            ui.add(egui::Label::new(egui::RichText::new(last).weak()).truncate());
                        });
                    });
                });
            });
        let response = ui
            .interact(
                inner.response.rect,
                ui.scope_id().with(("chat", &c.id)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.add_space(4.0);
        response.clicked()
    }

    fn thread_view(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(conv) = self.chat.selected_conversation().cloned() else {
            if let Some(e) = &self.chat.error {
                ui.colored_label(danger_color(ui), e.as_str());
            }
            ui.centered_and_justified(|ui| {
                ui.weak(t("Chọn một hội thoại để xem tin nhắn."));
            });
            return;
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.strong(&conv.title);
                ui.weak(format!("{} · {}", kind_label(conv.kind), conv.peer_name));
            });
            if let Some(url) = &conv.web_url {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(t("Mở trên web"))
                        .on_hover_text(t("Mở hội thoại trên web, nơi gửi được tệp đính kèm"))
                        .clicked()
                    {
                        actions.push(Action::OpenLink(url.clone()));
                    }
                });
            }
        });
        ui.separator();
        egui::Panel::bottom("chat_composer")
            .resizable(false)
            .show(ui, |ui| self.composer(ui, &conv, actions));
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| match &self.chat.thread {
                Some(thread) if thread.loaded => {
                    if thread.messages.is_empty() {
                        ui.weak(t("Chưa có tin nhắn nào. Hãy gửi lời chào."));
                    }
                    for m in &thread.messages {
                        chat_bubble(ui, m, &self.prefs, actions);
                    }
                }
                _ => {
                    ui.spinner();
                }
            });
    }

    /// Ô soạn tin nhiều dòng và nút Gửi (Ctrl+Enter cũng gửi). Tệp đính kèm chỉ gửi trên web.
    fn composer(&mut self, ui: &mut egui::Ui, conv: &Conversation, actions: &mut Vec<Action>) {
        ui.add_space(4.0);
        if let Some(e) = &self.chat.error {
            ui.colored_label(danger_color(ui), e.as_str());
        }
        if conv.send_blocked() {
            ui.colored_label(warn_color(ui), chat::error_message("chat_blocked"));
            ui.add_space(4.0);
            return;
        }
        let id = ui.make_persistent_id("chat_draft");
        let shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter);
        let mut send =
            ui.memory(|m| m.has_focus(id)) && ui.input_mut(|i| i.consume_shortcut(&shortcut));
        ui.add(
            egui::TextEdit::multiline(&mut self.chat.draft)
                .id(id)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text(t("Nhập tin nhắn... (Ctrl+Enter để gửi)")),
        );
        ui.horizontal(|ui| {
            let ready = !self.chat.sending && !self.chat.draft.trim().is_empty();
            if primary_button(ui, &self.prefs, ready, Icon::Send, t("Gửi")) {
                send = true;
            }
            if self.chat.sending {
                ui.spinner();
            }
            if let Some(url) = &conv.web_url {
                if ui
                    .link(t("Gửi tệp đính kèm trên web"))
                    .on_hover_text(t("Tệp đính kèm chỉ gửi được trên web"))
                    .clicked()
                {
                    actions.push(Action::OpenLink(url.clone()));
                }
            }
        });
        ui.add_space(4.0);
        if send {
            self.chat_send();
        }
    }
}

fn summarize(results: &[(String, Result<(), LauncherError>)]) -> Result<String, String> {
    if results.is_empty() {
        return Ok(t("Mọi app đã ở bản mới nhất").into());
    }
    let failed: Vec<String> = results
        .iter()
        .filter_map(|(app, r)| r.as_ref().err().map(|e| format!("{app}: {e}")))
        .collect();
    if failed.is_empty() {
        Ok(tf("Đã cập nhật {} app", &[&results.len()]))
    } else {
        Err(tf(
            "Cập nhật lỗi (bản cũ vẫn dùng được): {}",
            &[&failed.join("; ")],
        ))
    }
}

async fn startup(s: Arc<Shared>) -> Result<String, String> {
    let mut notice = String::new();
    match s.identity.auth.restore().await {
        Ok(Some(session)) => {
            let name = session.name.clone();
            *s.identity.session.lock().await = Some(session);
            s.identity.set(SignIn::SignedIn);
            if name.is_some() {
                let _ = s.launcher.lock().await.set_user_name(name);
            }
        }
        Ok(None) => s.identity.set(SignIn::SignedOut),
        Err(AuthError::Offline(_)) => s.identity.set(SignIn::Offline),
        Err(e) => {
            s.identity.set(SignIn::SignedOut);
            notice = e.to_string();
        }
    }
    let synced = sync(s).await;
    if notice.is_empty() {
        synced
    } else {
        Err(notice)
    }
}

async fn sync(s: Arc<Shared>) -> Result<String, String> {
    let now = Timestamp::now();
    let mut l = s.launcher.lock().await;
    match l.sync(now).await {
        Ok(()) => {}
        // Ngoại tuyến: thanh trên cùng đã báo, không cần thông báo lỗi.
        Err(LauncherError::Offline(_)) => return Ok(String::new()),
        Err(e) => return Err(e.to_string()),
    }
    // Giá F-Coin công khai (hiện giá cả khi chưa đăng nhập); lỗi thì giữ lần trước.
    match s.identity.auth.catalog_prices().await {
        Ok(p) => *s.prices.lock().expect("khóa") = p,
        Err(e) => eprintln!("không lấy được giá app: {e}"),
    }
    // App riêng tư (bản build của đơn xây app): chỉ hiện những app máy chủ liệt kê cho người
    // dùng đang đăng nhập; chưa đăng nhập thì ẩn hết. Không hỏi được thì giữ danh sách cũ.
    match s.identity.access_token().await {
        Ok(token) => {
            match s.identity.auth.private_apps(&token).await {
                Ok(apps) => {
                    let _ = l.set_private_apps(apps.into_iter().collect());
                }
                Err(e) => eprintln!("không lấy được app riêng tư: {e}"),
            }
            match s.identity.auth.notifications(&token).await {
                Ok(n) => *s.notifications.lock().expect("khóa") = n,
                Err(e) => eprintln!("không lấy được thông báo: {e}"),
            }
            // Ví F-Coin: số dư, app đã mua, giá; không hỏi được thì giữ lần trước.
            match s.identity.auth.wallet(&token).await {
                Ok(w) => *s.wallet.lock().expect("khóa") = Some(w),
                Err(e) => eprintln!("không lấy được ví F-Coin: {e}"),
            }
            // Vai trò nền tảng; không hỏi được thì giữ lần trước.
            match s.identity.auth.account_role(&token).await {
                Ok(role) => {
                    let _ = l.set_account_role(Some(role));
                }
                Err(e) => eprintln!("không lấy được vai trò: {e}"),
            }
        }
        Err(AuthError::Offline(_)) => {}
        Err(_) => {
            let _ = l.set_private_apps(Default::default());
            s.notifications.lock().expect("khóa").clear();
            *s.wallet.lock().expect("khóa") = None;
        }
    }
    // Phát hành dần: hỏi máy chủ thiết bị này được nhận bản nào của từng app đã cài.
    if let Ok(device) = l.device_id() {
        for (app, _) in l.installed() {
            match s
                .identity
                .auth
                .rollout_version(&app, &s.config.platform, &device)
                .await
            {
                Ok(version) => {
                    let _ = l.set_rollout(&app, version);
                }
                // Không hỏi được thì giữ câu trả lời lần trước.
                Err(e) => eprintln!("không hỏi được bản phát hành của {app}: {e}"),
            }
        }
    }
    if l.state().auto_update && l.due_for_update_check(now) {
        let skipped = s.skipped.lock().expect("khóa").clone();
        let results = l.update_all(now, &skipped).await;
        let installed = l.installed();
        drop(l);
        report_updates(&s, &results, &installed).await;
        if !results.is_empty() {
            return summarize(&results);
        }
    }
    Ok(String::new())
}

/// Báo máy chủ phiên bản mới của các app vừa cập nhật thành công (thay đổi app-update-notices U3).
async fn report_updates(
    s: &Shared,
    results: &[(String, Result<(), LauncherError>)],
    installed: &[(String, String)],
) {
    for (app, result) in results {
        if result.is_err() {
            continue;
        }
        if let Some((_, version)) = installed.iter().find(|(a, _)| a == app) {
            report_install(s, app, version).await;
        }
    }
}

/// Báo F-Store đã gỡ app (rời "Ứng dụng của tôi", không nhận thông báo cập nhật nữa). Không chặn
/// việc gỡ nếu thất bại.
async fn report_uninstall(s: &Shared, app: &str) {
    if let Ok(token) = s.identity.access_token().await {
        if let Err(e) = s.identity.auth.remove_install(&token, app).await {
            eprintln!("không báo được lượt gỡ {app}: {e}");
        }
    }
}

/// Báo F-Store phiên bản đang cài sau khi cài, cập nhật hay quay lại bản trước (để được đánh giá
/// app, và để "Ứng dụng của tôi" trên web và di động biết đã có bản mới). Không chặn việc cài nếu
/// thất bại.
async fn report_install(s: &Shared, app: &str, version: &str) {
    if let Ok(token) = s.identity.access_token().await {
        if let Err(e) = s.identity.auth.record_install(&token, app, version).await {
            eprintln!("không báo được lượt cài {app}: {e}");
        }
    }
}

async fn open_app(s: &Shared, app: &str) -> Result<String, String> {
    let plan = s
        .launcher
        .lock()
        .await
        .prepare_launch(app)
        .map_err(|e| e.to_string())?;
    if let Some(url) = &plan.web_url {
        // App web: mở bằng trình duyệt, app tự đăng nhập F-Eco bằng OIDC (không cấp mã khởi chạy).
        open::that(url).map_err(|e| tf("Không mở được trình duyệt: {}", &[&e]))?;
        if let Err(e) = s.launcher.lock().await.mark_opened(app, Timestamp::now()) {
            eprintln!("không ghi được lần mở {app}: {e}");
        }
        return Ok(tf("Đã mở {} trong trình duyệt", &[&plan.title]));
    }
    let mut note = String::new();
    let code = match &plan.client_id {
        Some(client_id) => match s.identity.access_token().await {
            Ok(token) => match s.identity.auth.launch_code(&token, client_id).await {
                Ok(code) => Some(code.code),
                Err(e) => {
                    note = tf(" (không lấy được mã đăng nhập: {})", &[&e]);
                    None
                }
            },
            Err(_) => {
                note = t(" (chưa đăng nhập: app sẽ tự yêu cầu đăng nhập)").into();
                None
            }
        },
        None => None,
    };
    let secret = plan.client_id.as_deref().map(|c| s.broker.register(c));
    let env = LaunchEnv {
        launch_code: code,
        server_url: s.config.server.to_string(),
        issuer_url: s.config.hydra.to_string(),
        broker_url: secret.as_ref().map(|_| s.broker_url.clone()),
        broker_secret: secret.clone(),
    };
    let spawned = command(&plan, &env).spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            if let Some(secret) = &secret {
                s.broker.unregister(secret);
            }
            return Err(tf("không mở được {}: {}", &[&plan.title, &e]));
        }
    };
    // Hủy bí mật của lần chạy khi app thoát. App terminal được mở qua `cmd /C start` hay
    // `open -a Terminal` (thoát ngay sau khi mở cửa sổ), nên giữ bí mật tới khi launcher đóng.
    if let Some(secret) = secret.filter(|_| !plan.terminal) {
        let broker = s.broker.clone();
        std::thread::spawn(move || {
            let _ = child.wait();
            broker.unregister(&secret);
        });
    }
    if let Err(e) = s.launcher.lock().await.mark_opened(app, Timestamp::now()) {
        eprintln!("không ghi được lần mở {app}: {e}");
    }
    Ok(tf("Đã mở {} {}{}", &[&plan.title, &plan.version, &note]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallet_view(balance: i64, owned: &[&str], signed_in: bool) -> View {
        let body = serde_json::json!({
            "balance": balance, "rate_vnd": 1000, "frozen": false,
            "owned": owned, "prices": {"so-tay-pro": 49, "free-app": 0},
            "wallet_url": "http://127.0.0.1:8080/wallet"
        });
        View {
            sign_in: Some(if signed_in {
                SignIn::SignedIn
            } else {
                SignIn::SignedOut
            }),
            wallet: signed_in.then(|| WalletInfo::from_json(&body)),
            prices: [("so-tay-pro".to_owned(), 49)].into_iter().collect(),
            ..Default::default()
        }
    }

    /// Scenario "Mua rồi cài": app có giá chưa mua thì nút chính là Mua; đủ F-Coin thì không thiếu.
    #[test]
    fn paid_app_needs_purchase_before_install() {
        let v = wallet_view(120, &[], true);
        assert_eq!(acquire(&v, "so-tay-pro"), Acquire::Buy(49));
        assert_eq!(acquire(&v, "free-app"), Acquire::Install);
        assert_eq!(acquire(&v, "khong-co"), Acquire::Install);
        assert_eq!(shortfall(120, 49), None);
        assert_eq!(120 - 49, 71, "số dư sau khi mua");
        let owned = wallet_view(71, &["so-tay-pro"], true);
        assert_eq!(acquire(&owned, "so-tay-pro"), Acquire::Install);
        let anon = wallet_view(0, &[], false);
        assert_eq!(acquire(&anon, "so-tay-pro"), Acquire::SignInToBuy(49));
        assert_eq!(
            v.wallet.as_ref().unwrap().wallet_url.as_deref(),
            Some("http://127.0.0.1:8080/wallet")
        );
    }

    fn my_app(app: &str, version: &str, update: Option<&str>) -> MyApp {
        let entry = |v: &str| CatalogEntry {
            app: app.into(),
            title: app.to_uppercase(),
            version: v.into(),
            platform: "windows-x64".into(),
            target: format!("{app}-{v}.zip"),
            length: 1,
            sha256: String::new(),
            entry: "bin/app.exe".into(),
            client_id: None,
            private: false,
            info: feco_launcher::AppInfo {
                release_notes: Some(format!("Ghi chú bản {v}")),
                ..Default::default()
            },
            platforms: Vec::new(),
        };
        MyApp {
            app: app.into(),
            title: app.to_uppercase(),
            version: version.into(),
            previous_version: None,
            status: AppStatus::Ready,
            update: update.map(entry),
            info: Default::default(),
            installed_at: Timestamp::UNIX_EPOCH,
            last_opened: None,
        }
    }

    /// Scenario "Phát hiện bản mới" và "Bỏ qua phiên bản": gợi ý app có bản mới, trừ bản đã bỏ qua.
    #[test]
    fn suggests_updates_except_skipped_versions() {
        let apps = vec![
            my_app("kibi", "0.3.2", Some("0.3.3")),
            my_app("bottom", "0.10.2", None),
            my_app("f-notes", "1.5.0", Some("1.6.0")),
        ];
        let s = update_suggestions(&apps, &Default::default());
        assert_eq!(
            s.iter()
                .map(|u| (u.app.as_str(), u.from.as_str(), u.to.as_str()))
                .collect::<Vec<_>>(),
            [("f-notes", "1.5.0", "1.6.0"), ("kibi", "0.3.2", "0.3.3")]
        );
        assert_eq!(s[1].notes.as_deref(), Some("Ghi chú bản 0.3.3"));
        let skipped = [("kibi".to_owned(), "0.3.3".to_owned())]
            .into_iter()
            .collect();
        let s = update_suggestions(&apps, &skipped);
        assert_eq!(s.len(), 1, "bỏ qua kibi 0.3.3");
        // Có bản mới hơn bản đã bỏ qua thì gợi ý lại.
        let newer = vec![my_app("kibi", "0.3.2", Some("0.3.4"))];
        assert_eq!(update_suggestions(&newer, &skipped).len(), 1);
    }

    /// launcher-sidebar-and-web-apps, scenario "Thu gọn thanh bên", "Cửa sổ hẹp".
    #[test]
    fn sidebar_collapses_when_chosen_or_narrow() {
        assert_eq!(sidebar_width(sidebar_collapsed(false, 1000.0)), 210.0);
        assert_eq!(sidebar_width(sidebar_collapsed(true, 1000.0)), 56.0);
        // Cửa sổ 700 px: thu gọn mà không đổi lựa chọn đã lưu; rộng lại thì đầy đủ.
        assert!(sidebar_collapsed(false, 700.0));
        assert!(!sidebar_collapsed(false, 1000.0));
        assert!(sidebar_collapsed(true, 700.0));
    }

    /// Scenario "Cửa sổ ở giữa màn hình".
    #[test]
    fn child_windows_are_centered() {
        assert_eq!(
            centered_in(egui::vec2(1920.0, 1080.0), egui::vec2(760.0, 700.0)),
            Some(egui::pos2(580.0, 190.0))
        );
        assert_eq!(
            centered_in(egui::vec2(800.0, 600.0), egui::vec2(840.0, 720.0)),
            Some(egui::pos2(0.0, 0.0)),
            "màn hình nhỏ hơn cửa sổ: đặt ở góc trên"
        );
        assert_eq!(
            centered_in(egui::vec2(0.0, 0.0), egui::vec2(10.0, 10.0)),
            None
        );
    }

    /// Logo F-Launcher và logo app (PNG trong metadata TUF) giải mã được.
    #[test]
    fn logos_decode() {
        let (w, h, rgba) = decode_png(LAUNCHER_LOGO).expect("logo F-Launcher");
        assert_eq!((w, h, rgba.len()), (256, 256, 256 * 256 * 4));
        assert!(window_icon().is_some());
        use base64::Engine;
        let kibi = include_bytes!("../../../assets/logos/kibi-128.png");
        let mut entry = my_app("kibi", "0.3.3", Some("0.3.3")).update.unwrap();
        entry.info.icon = Some(base64::engine::general_purpose::STANDARD.encode(kibi));
        register_icons(&[entry]);
        let bytes = icon_sources().lock().unwrap().get("kibi").cloned().unwrap();
        assert_eq!(decode_png(&bytes).unwrap().0, 128);
        assert!(decode_png(b"not a png").is_none());
    }

    /// Logo khai kích thước 65535×65535 trong IHDR bị bỏ qua, không cấp phát bộ đệm khổng lồ.
    #[test]
    fn huge_png_header_is_rejected() {
        fn crc32(data: &[u8]) -> u32 {
            let mut c = !0u32;
            for &b in data {
                c ^= u32::from(b);
                for _ in 0..8 {
                    c = if c & 1 != 0 {
                        0xEDB8_8320 ^ (c >> 1)
                    } else {
                        c >> 1
                    };
                }
            }
            !c
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let start = out.len();
            out.extend_from_slice(kind);
            out.extend_from_slice(data);
            let crc = crc32(&out[start..]);
            out.extend_from_slice(&crc.to_be_bytes());
        }
        let png = |w: u32, h: u32| {
            let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
            let mut ihdr = Vec::new();
            ihdr.extend_from_slice(&w.to_be_bytes());
            ihdr.extend_from_slice(&h.to_be_bytes());
            ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
            chunk(&mut out, b"IHDR", &ihdr);
            chunk(
                &mut out,
                b"IDAT",
                &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
            );
            chunk(&mut out, b"IEND", &[]);
            out
        };
        assert!(decode_png(&png(65535, 65535)).is_none());
        assert!(decode_png(&png(1025, 1)).is_none());
    }

    /// Scenario "Không đủ F-Coin trong launcher": còn thiếu 39 F-Coin.
    #[test]
    fn shortfall_when_balance_is_low() {
        assert_eq!(shortfall(10, 49), Some(39));
        i18n::set_language(Language::Vi);
        assert_eq!(coins(1250), "1.250 F-Coin");
        assert_eq!(tf("Còn thiếu {}", &[&coins(39)]), "Còn thiếu 39 F-Coin");
    }

    /// Scenario "Ô lưới đều nhau": các ô cùng chiều rộng, lấp đầy hàng, không hẹp hơn cỡ đã chọn.
    #[test]
    fn grid_tiles_fill_rows_evenly() {
        for (available, min, gap) in [
            (900.0, 180.0, 12.0),
            (500.0, 230.0, 8.0),
            (120.0, 180.0, 12.0),
        ] {
            let (columns, width) = grid_columns(available, min, gap);
            assert!(columns >= 1);
            if columns > 1 {
                assert!(width >= min, "{available}: ô {width} < {min}");
            }
            let used = width * columns as f32 + gap * (columns - 1) as f32;
            assert!(
                used <= available + 0.5 && available - used < columns as f32,
                "{available}: {used}"
            );
        }
        assert_eq!(grid_columns(900.0, 180.0, 12.0).0, 4);
    }
}
