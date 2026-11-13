//! Giao diện F-Launcher (egui). Mọi logic nằm trong thư viện `feco_launcher`; tệp này chỉ vẽ
//! giao diện và chạy các thao tác trên một runtime tokio riêng để giao diện không bị treo.
//!
//! Bố cục: thanh bên (Thư viện, Cửa hàng, Thông báo, Cài đặt, tài khoản) và vùng chính. Thư viện
//! và Cửa hàng xem theo lưới hoặc danh sách, có tìm, lọc nhóm, sắp xếp; cài đặt giao diện lưu
//! trong `settings.json` (spec launcher của thay đổi improve-launcher-experience).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use feco_launcher::auth::{Auth, AuthError, KeyringStore, Session};
use feco_launcher::broker::{Broker, Issuer};
use feco_launcher::i18n::{self, t, tf, Language};
use feco_launcher::launch::{command, LaunchEnv};
use feco_launcher::library::{self, SortKey};
use feco_launcher::prefs::{Accent, Layout, Prefs, StartPage, Theme, TileSize};
use feco_launcher::style::{self as ui_style, UiStyle};
use feco_launcher::{
    AppStatus, CatalogEntry, Config, Launcher, LauncherError, MyApp, Paths, ISOLATION_NOTICE,
};
use jiff::Timestamp;
use std::future::Future;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Đồng bộ lại mỗi giờ khi có mạng (thu hồi tới launcher trong 1 giờ), mỗi 5 phút khi ngoại tuyến.
const SYNC_ONLINE: Duration = Duration::from_secs(60 * 60);
const SYNC_OFFLINE: Duration = Duration::from_secs(5 * 60);

fn main() -> eframe::Result {
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("F-Launcher: {e}");
            std::process::exit(2);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("F-Launcher")
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([720.0, 480.0]),
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
    fonts
        .families
        .insert(egui::FontFamily::Proportional, vec!["ui".into()]);
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["ui".into()]);
    ctx.set_fonts(fonts);
    Ok(())
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
    config: Config,
    launcher: tokio::sync::Mutex<Launcher>,
    identity: Arc<Identity>,
    broker: Broker,
    broker_url: String,
}

#[derive(Clone, Default)]
struct View {
    online: bool,
    my_apps: Vec<MyApp>,
    catalog: Vec<CatalogEntry>,
    auto_update: bool,
    user: Option<String>,
    sign_in: Option<SignIn>,
    last_sync: Option<Timestamp>,
    notifications: Vec<feco_launcher::auth::Notification>,
}

async fn snapshot(shared: &Shared) -> View {
    let l = shared.launcher.lock().await;
    View {
        online: l.is_online(),
        my_apps: l.my_apps(),
        catalog: l.catalog_latest().into_iter().cloned().collect(),
        auto_update: l.state().auto_update,
        user: l.state().user_name.clone(),
        sign_in: Some(*shared.identity.sign_in.lock().expect("khóa")),
        last_sync: l.state().last_sync,
        notifications: shared.notifications.lock().expect("khóa").clone(),
    }
}

struct Done {
    result: Result<String, String>,
    view: View,
    synced: bool,
}

enum Action {
    Login,
    Logout,
    Sync,
    AskInstall(Box<CatalogEntry>),
    Install(Box<CatalogEntry>),
    Update(String),
    UpdateAll,
    Rollback(String),
    AskUninstall(Box<MyApp>),
    Uninstall(String, bool),
    Open(String),
    OpenLink(String),
    SetAutoUpdate(bool),
    CloseDialog,
    /// Mở trang chi tiết của app (kèm thứ tự đang hiển thị để chuyển trước/sau).
    ShowDetail(Detail),
    /// Chuyển trang chi tiết sang app khác trong cùng thứ tự.
    MoveDetail(String),
    CloseDetail,
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
    detail: Option<Detail>,
    busy: Option<String>,
    notice: Option<(bool, String)>,
    confirm_install: Option<CatalogEntry>,
    confirm_uninstall: Option<(MyApp, bool)>,
    next_sync: Instant,
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
            config,
            launcher: tokio::sync::Mutex::new(launcher),
            identity,
            broker,
            broker_url: format!("http://{addr}/"),
        });
        let view = rt.block_on(snapshot(&shared));
        let (tx, rx) = mpsc::channel();
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
            detail: None,
            busy: None,
            notice: None,
            confirm_install: None,
            confirm_uninstall: None,
            next_sync: Instant::now() + SYNC_OFFLINE,
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
            self.view = done.view;
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
                s.launcher
                    .lock()
                    .await
                    .set_user_name(None)
                    .map_err(|e| e.to_string())?;
                Ok(t("Đã đăng xuất").into())
            }),
            Action::Sync => self.run(t("Đang đồng bộ..."), true, sync),
            Action::AskInstall(entry) => self.confirm_install = Some(*entry),
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
                Ok(tf("Đã cập nhật {} lên {}", &[&entry.title, &entry.version]))
            }),
            Action::UpdateAll => self.run(t("Đang cập nhật tất cả..."), false, |s| async move {
                let results = s.launcher.lock().await.update_all(Timestamp::now()).await;
                summarize(&results)
            }),
            Action::Rollback(app) => self.run(
                t("Đang quay lại bản trước..."),
                false,
                move |s| async move {
                    let mut l = s.launcher.lock().await;
                    l.rollback(&app).map_err(|e| e.to_string())?;
                    let plan = l.prepare_launch(&app).map_err(|e| e.to_string())?;
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
                    Ok(tf("Đã gỡ {}. Quyền dữ liệu bạn đã cấp cho app vẫn còn; thu hồi ở trang \"Ứng dụng đã kết nối\".", &[&app]))
                })
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
            Action::ShowDetail(detail) => self.detail = Some(detail),
            Action::MoveDetail(app) => {
                if let Some(d) = &mut self.detail {
                    d.app = app;
                }
            }
            Action::CloseDetail => self.detail = None,
            Action::CloseDialog => {
                self.confirm_install = None;
                self.confirm_uninstall = None;
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

    fn dialogs(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if let Some(entry) = &self.confirm_install {
            egui::Modal::new(egui::Id::unique("confirm-install")).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    app_icon(ui, &entry.app, &entry.title, 40.0);
                    ui.heading(tf("Cài {} {}?", &[&entry.title, &entry.version]));
                });
                if let Some(summary) = &entry.info.summary {
                    ui.label(summary);
                }
                ui.weak(byline(&entry.info));
                if let Some(source) = &entry.info.source {
                    ui.hyperlink_to(t("Mã nguồn"), source);
                }
                ui.label(tf(
                    "Dung lượng tải về: {} MB. Gói được kiểm tra chữ ký của F-Eco trước khi cài.",
                    &[&format!("{:.1}", entry.length as f64 / 1_048_576.0)],
                ));
                ui.colored_label(egui::Color32::from_rgb(200, 120, 0), t(ISOLATION_NOTICE));
                ui.horizontal(|ui| {
                    if ui.button(t("Cài")).clicked() {
                        actions.push(Action::Install(Box::new(entry.clone())));
                    }
                    if ui.button(t("Hủy")).clicked() {
                        actions.push(Action::CloseDialog);
                    }
                });
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
        ctx.request_repaint_after(Duration::from_secs(30));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions = Vec::new();
        let mut nav = egui::Panel::left("nav").exact_size(210.0).resizable(false);
        if let Some(p) = self.prefs.style.palette() {
            nav = nav.frame(
                egui::Frame::side_top_panel(ui.style())
                    .fill(rgb(p.panel))
                    .stroke(egui::Stroke::new(1.0, rgb(p.stroke))),
            );
        }
        nav.show(ui, |ui| self.sidebar(ui, &mut actions));
        egui::Panel::bottom("status").show(ui, |ui| {
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
        let detail = self
            .detail
            .clone()
            .filter(|_| matches!(self.page, Page::Library | Page::Store));
        egui::CentralPanel::default().show(ui, |ui| match (&detail, self.page) {
            (Some(d), _) => self.detail_page(ui, d, &mut actions),
            (None, page) => match page {
                Page::Library => self.library_page(ui, &mut actions),
                Page::Store => self.store_page(ui, &mut actions),
                Page::Notifications => self.notifications(ui, &mut actions),
                Page::Settings => self.settings_page(ui, &mut actions),
            },
        });
        let ctx = ui.ctx().clone();
        self.dialogs(&ctx, &mut actions);
        for action in actions {
            self.apply(action);
        }
    }
}

// ---- Giao diện (spec launcher: "Bố cục lưới và danh sách", "Tìm, lọc và sắp xếp app",
// ---- "Cài đặt giao diện của launcher", "Thông tin hiển thị của app").

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Library,
    Store,
    Notifications,
    Settings,
}

/// Ô tìm và bộ lọc nhóm của một trang (kiểu sắp xếp nằm trong [`Prefs`] để được nhớ).
#[derive(Default)]
struct Filter {
    text: String,
    category: Option<String>,
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

fn primary_button(ui: &mut egui::Ui, prefs: &Prefs, enabled: bool, text: &str) -> bool {
    let button =
        egui::Button::new(egui::RichText::new(text).color(on_accent(prefs))).fill(accent(prefs));
    ui.add_enabled(enabled, button).clicked()
}

fn size_mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

/// "Nhà phát triển · Nhóm" (bỏ phần trống).
fn byline(info: &feco_launcher::AppInfo) -> String {
    let category = library::category_label(&library::category_of(info));
    match &info.developer {
        Some(d) => format!("{d} · {category}"),
        None => category,
    }
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

    fn sidebar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            app_icon(ui, "f-launcher", "F Launcher", 28.0);
            ui.heading("F-Launcher");
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
        let unread = self.view.notifications.iter().filter(|n| !n.read).count();
        let updates = self
            .view
            .my_apps
            .iter()
            .filter(|a| a.update.is_some())
            .count();
        let entries = [
            (
                Page::Library,
                if updates > 0 {
                    tf("Thư viện  ({} cập nhật)", &[&updates])
                } else {
                    t("Thư viện").to_owned()
                },
            ),
            (Page::Store, t("Cửa hàng").to_owned()),
            (
                Page::Notifications,
                if unread > 0 {
                    tf("Thông báo  ({})", &[&unread])
                } else {
                    t("Thông báo").to_owned()
                },
            ),
            (Page::Settings, t("Cài đặt").to_owned()),
        ];
        for (page, label) in entries {
            let selected = self.page == page;
            let text = if selected {
                egui::RichText::new(label).strong()
            } else {
                egui::RichText::new(label)
            };
            let button = egui::Button::selectable(selected, text)
                .min_size(egui::vec2(ui.available_width(), 30.0));
            if ui.add(button).clicked() {
                self.page = page;
                self.detail = None;
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(8.0);
            let idle = self.busy.is_none();
            if let Some(t) = self.view.last_sync {
                // Giờ địa phương của máy (Timestamp là UTC).
                let local = t.to_zoned(jiff::tz::TimeZone::system());
                ui.weak(tf("Đồng bộ lúc {}", &[&local.strftime("%H:%M %d/%m")]));
            }
            if ui
                .add_enabled(idle, egui::Button::new(t("Đồng bộ")))
                .clicked()
            {
                actions.push(Action::Sync);
            }
            ui.separator();
            match self.view.sign_in {
                Some(SignIn::SignedIn) | Some(SignIn::Offline) => {
                    if ui
                        .add_enabled(idle, egui::Button::new(t("Đăng xuất")))
                        .clicked()
                    {
                        actions.push(Action::Logout);
                    }
                    ui.strong(
                        self.view
                            .user
                            .clone()
                            .unwrap_or_else(|| t("Đã đăng nhập").into()),
                    );
                }
                _ => {
                    if primary_button(ui, &self.prefs, idle, t("Đăng nhập")) {
                        actions.push(Action::Login);
                    }
                    ui.weak(t("Chưa đăng nhập"));
                }
            }
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
                for layout in [Layout::List, Layout::Grid] {
                    ui.selectable_value(&mut prefs.layout, layout, layout.label());
                }
                if !store {
                    let updates = self
                        .view
                        .my_apps
                        .iter()
                        .filter(|a| a.update.is_some())
                        .count();
                    if updates > 0
                        && primary_button(
                            ui,
                            &self.prefs,
                            self.busy.is_none(),
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
                && ui.button(t("Xóa bộ lọc")).clicked()
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

    /// Lưới: số cột theo chiều rộng cửa sổ và cỡ ô đã chọn.
    fn grid<T: library::Listed>(
        &self,
        ui: &mut egui::Ui,
        items: &[&T],
        mut tile: impl FnMut(&Self, &mut egui::Ui, &T),
    ) -> Option<String> {
        let mut clicked = None;
        let spacing = ui.spacing().item_spacing.x.max(10.0);
        let width = self.prefs.tile_size.width();
        let columns =
            (((ui.available_width() + spacing) / (width + 22.0 + spacing)).floor() as usize).max(1);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in items.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = spacing;
                        for item in row {
                            let card = ui.scope_builder(
                                egui::UiBuilder::new().sense(egui::Sense::click()),
                                |ui| {
                                    // Chữ trong ô không chọn được, để bấm vào đâu cũng mở chi tiết.
                                    ui.style_mut().interaction.selectable_labels = false;
                                    card_frame(ui).show(ui, |ui| {
                                        ui.set_width(width);
                                        ui.vertical(|ui| tile(self, ui, item));
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
                        }
                    });
                    ui.add_space(spacing);
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
        ui.menu_button("...", |ui| {
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
        if primary_button(ui, &self.prefs, self.busy.is_none() && ready, label) {
            actions.push(Action::Open(app.app.clone()));
        }
    }

    fn library_page(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
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
                app,
                store: false,
                order,
            }));
        }
    }

    /// Nút chính của một app trong Cửa hàng: Cài, Cập nhật, hoặc nhãn "Đã cài".
    fn store_button(&self, ui: &mut egui::Ui, entry: &CatalogEntry, actions: &mut Vec<Action>) {
        let installed = self.view.my_apps.iter().find(|a| a.app == entry.app);
        let label = match installed {
            Some(a) if a.version == entry.version => None,
            Some(_) => Some(t("Cập nhật")),
            None => Some(t("Cài")),
        };
        match label {
            Some(label) => {
                if primary_button(
                    ui,
                    &self.prefs,
                    self.busy.is_none() && self.view.online,
                    label,
                ) {
                    actions.push(Action::AskInstall(Box::new(entry.clone())));
                }
            }
            None => {
                ui.weak(t("Đã cài"));
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
                        egui::RichText::new(format!(
                            "{} · {}",
                            entry.version,
                            size_mb(entry.length)
                        ))
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
                    ui.weak(format!(
                        "{} · {}",
                        byline(&entry.info),
                        size_mb(entry.length)
                    ));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(source) = &entry.info.source {
                        if ui.button(t("Mã nguồn")).clicked() {
                            actions.push(Action::OpenLink(source.clone()));
                        }
                    }
                    me.store_button(ui, entry, actions);
                });
            }),
        };
        if let Some(app) = clicked {
            actions.push(Action::ShowDetail(Detail {
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
                            if primary_button(ui, &self.prefs, idle, t("Đăng nhập")) {
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
    app: String,
    store: bool,
    order: Vec<String>,
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

        // Phím tắt: trái/phải chuyển app, Esc quay lại (bỏ qua khi đang gõ vào ô nhập liệu).
        if !ui.ctx().egui_wants_keyboard_input() {
            let (left, right, esc) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                    i.key_pressed(egui::Key::Escape),
                )
            });
            if esc {
                actions.push(Action::CloseDetail);
            } else if let (true, Some(p)) = (left, &prev) {
                actions.push(Action::MoveDetail(p.clone()));
            } else if let (true, Some(n)) = (right, &next) {
                actions.push(Action::MoveDetail(n.clone()));
            }
        }

        // Thanh trên: quay lại danh sách, app trước và sau.
        ui.horizontal(|ui| {
            let back = if detail.store {
                t("‹ Cửa hàng")
            } else {
                t("‹ Thư viện")
            };
            if ui.button(back).on_hover_text("Esc").clicked() {
                actions.push(Action::CloseDetail);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let next_label = match &next {
                    Some(n) => format!("Sau: {} ›", self.title_of(n)),
                    None => "Sau ›".to_owned(),
                };
                if ui
                    .add_enabled(next.is_some(), egui::Button::new(next_label))
                    .on_hover_text(t("Phím mũi tên phải"))
                    .clicked()
                {
                    if let Some(n) = &next {
                        actions.push(Action::MoveDetail(n.clone()));
                    }
                }
                let prev_label = match &prev {
                    Some(p) => tf("‹ Trước: {}", &[&self.title_of(p)]),
                    None => t("‹ Trước").to_owned(),
                };
                if ui
                    .add_enabled(prev.is_some(), egui::Button::new(prev_label))
                    .on_hover_text(t("Phím mũi tên trái"))
                    .clicked()
                {
                    if let Some(p) = &prev {
                        actions.push(Action::MoveDetail(p.clone()));
                    }
                }
                if let Some(i) = detail.order.iter().position(|a| *a == detail.app) {
                    ui.weak(format!("{}/{}", i + 1, detail.order.len()));
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
                                        if ui
                                            .add_enabled(
                                                idle,
                                                egui::Button::new(tf("Quay lại {}", &[&prev])),
                                            )
                                            .clicked()
                                        {
                                            actions.push(Action::Rollback(app.app.clone()));
                                        }
                                    }
                                    if ui
                                        .add_enabled(idle, egui::Button::new(t("Gỡ...")))
                                        .clicked()
                                    {
                                        actions.push(Action::AskUninstall(Box::new(app.clone())));
                                    }
                                }
                                None => {
                                    if let Some(e) = &entry {
                                        if primary_button(
                                            ui,
                                            &self.prefs,
                                            idle && self.view.online,
                                            t("Cài"),
                                        ) {
                                            actions.push(Action::AskInstall(Box::new(e.clone())));
                                        }
                                    }
                                }
                            }
                            if let Some(source) = &info.source {
                                if ui
                                    .button(link_label(source))
                                    .on_hover_text(source)
                                    .clicked()
                                {
                                    actions.push(Action::OpenLink(source.clone()));
                                }
                            }
                            if let Some(site) = &info.website {
                                if ui.button("Website").on_hover_text(site).clicked() {
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
                                row(ui, t("Dung lượng tải về"), size_mb(e.length));
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
                                ui.weak("Website");
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
        }
        Err(AuthError::Offline(_)) => {}
        Err(_) => {
            let _ = l.set_private_apps(Default::default());
            s.notifications.lock().expect("khóa").clear();
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
        let results = l.update_all(now).await;
        if !results.is_empty() {
            return summarize(&results);
        }
    }
    Ok(String::new())
}

/// Báo F-Store đã cài (để được đánh giá app). Không chặn việc cài nếu thất bại.
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
        broker_secret: secret,
    };
    command(&plan, &env)
        .spawn()
        .map_err(|e| tf("không mở được {}: {}", &[&plan.title, &e]))?;
    if let Err(e) = s.launcher.lock().await.mark_opened(app, Timestamp::now()) {
        eprintln!("không ghi được lần mở {app}: {e}");
    }
    Ok(tf("Đã mở {} {}{}", &[&plan.title, &plan.version, &note]))
}
