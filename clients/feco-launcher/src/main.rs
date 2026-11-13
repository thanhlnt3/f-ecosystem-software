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
use feco_launcher::launch::{command, LaunchEnv};
use feco_launcher::library::{self, SortKey};
use feco_launcher::prefs::{Accent, Layout, Prefs, StartPage, Theme, TileSize};
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
        .ok_or("không tìm thấy phông chữ hệ thống; đặt FECO_LAUNCHER_FONT=<đường dẫn .ttf>")?;
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
                format!(
                    "không đọc được root.json tin cậy {}: {e}",
                    config.trusted_root.display()
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
            .map_err(|e| format!("không mở được API cục bộ: {e}"))?;
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
            busy: None,
            notice: None,
            confirm_install: None,
            confirm_uninstall: None,
            next_sync: Instant::now() + SYNC_OFFLINE,
        };
        app.run("Đang đăng nhập lại và đồng bộ...", true, startup);
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
                "Đang chờ đăng nhập trong trình duyệt...",
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
                    Ok(format!("Xin chào {}", name.unwrap_or_default()))
                },
            ),
            Action::Logout => self.run("Đang đăng xuất...", false, |s| async move {
                s.identity.auth.logout().await.map_err(|e| e.to_string())?;
                *s.identity.session.lock().await = None;
                s.identity.set(SignIn::SignedOut);
                s.launcher
                    .lock()
                    .await
                    .set_user_name(None)
                    .map_err(|e| e.to_string())?;
                Ok("Đã đăng xuất".into())
            }),
            Action::Sync => self.run("Đang đồng bộ...", true, sync),
            Action::AskInstall(entry) => self.confirm_install = Some(*entry),
            Action::Install(entry) => {
                self.confirm_install = None;
                self.run(
                    &format!("Đang cài {} {}...", entry.title, entry.version),
                    false,
                    move |s| async move {
                        s.launcher
                            .lock()
                            .await
                            .install(&entry, Timestamp::now())
                            .await
                            .map_err(|e| e.to_string())?;
                        report_install(&s, &entry.app, &entry.version).await;
                        Ok(format!("Đã cài {} {}", entry.title, entry.version))
                    },
                )
            }
            Action::Update(app) => self.run("Đang cập nhật...", false, move |s| async move {
                let mut l = s.launcher.lock().await;
                let entry = l.update_for(&app).cloned().ok_or("không có bản cập nhật")?;
                l.install(&entry, Timestamp::now())
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("Đã cập nhật {} lên {}", entry.title, entry.version))
            }),
            Action::UpdateAll => self.run("Đang cập nhật tất cả...", false, |s| async move {
                let results = s.launcher.lock().await.update_all(Timestamp::now()).await;
                summarize(&results)
            }),
            Action::Rollback(app) => {
                self.run("Đang quay lại bản trước...", false, move |s| async move {
                    let mut l = s.launcher.lock().await;
                    l.rollback(&app).map_err(|e| e.to_string())?;
                    let plan = l.prepare_launch(&app).map_err(|e| e.to_string())?;
                    Ok(format!("Đã quay lại {} {}", plan.title, plan.version))
                })
            }
            Action::AskUninstall(app) => self.confirm_uninstall = Some((*app, false)),
            Action::Uninstall(app, delete_data) => {
                self.confirm_uninstall = None;
                self.run("Đang gỡ...", false, move |s| async move {
                    s.launcher
                        .lock()
                        .await
                        .uninstall(&app, delete_data)
                        .map_err(|e| e.to_string())?;
                    Ok(format!(
                        "Đã gỡ {app}. Quyền dữ liệu bạn đã cấp cho app vẫn còn; thu hồi ở trang \"Ứng dụng đã kết nối\"."
                    ))
                })
            }
            Action::OpenLink(url) => {
                if let Err(e) = open::that(&url) {
                    self.notice = Some((true, format!("Không mở được trình duyệt: {e}")));
                }
            }
            Action::Open(app) => self.run("Đang mở...", false, move |s| async move {
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
            Action::CloseDialog => {
                self.confirm_install = None;
                self.confirm_uninstall = None;
            }
        }
    }

    /// Trung tâm thông báo: bấm "Mở" để mở đúng màn hình trên web (ví dụ trang báo giá).
    fn notifications(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.heading("Thông báo");
        ui.add_space(6.0);
        if self.view.notifications.is_empty() {
            ui.label("Chưa có thông báo nào. Đăng nhập để nhận thông báo về đơn hàng, thanh toán, cập nhật app.");
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
                    if !n.link.is_empty() && ui.button("Mở").clicked() {
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
                    ui.heading(format!("Cài {} {}?", entry.title, entry.version));
                });
                if let Some(summary) = &entry.info.summary {
                    ui.label(summary);
                }
                ui.weak(byline(&entry.info));
                if let Some(source) = &entry.info.source {
                    ui.hyperlink_to("Mã nguồn", source);
                }
                ui.label(format!(
                    "Dung lượng tải về: {:.1} MB. Gói được kiểm tra chữ ký của F-Eco trước khi cài.",
                    entry.length as f64 / 1_048_576.0
                ));
                ui.colored_label(egui::Color32::from_rgb(200, 120, 0), ISOLATION_NOTICE);
                ui.horizontal(|ui| {
                    if ui.button("Cài").clicked() {
                        actions.push(Action::Install(Box::new(entry.clone())));
                    }
                    if ui.button("Hủy").clicked() {
                        actions.push(Action::CloseDialog);
                    }
                });
            });
        }
        let server = self.shared.config.server.clone();
        if let Some((app, delete_data)) = &mut self.confirm_uninstall {
            egui::Modal::new(egui::Id::unique("confirm-uninstall")).show(ctx, |ui| {
                ui.heading(format!("Gỡ {}?", app.title));
                ui.checkbox(delete_data, "Xóa cả dữ liệu cục bộ của app trên máy này");
                ui.label("Gỡ app không thu hồi quyền dữ liệu bạn đã cấp cho app.");
                ui.hyperlink_to(
                    "Quản lý quyền ở trang Ứng dụng đã kết nối",
                    server
                        .join("apps")
                        .map(|u| u.to_string())
                        .unwrap_or_default(),
                );
                ui.horizontal(|ui| {
                    if ui.button("Gỡ").clicked() {
                        actions.push(Action::Uninstall(app.app.clone(), *delete_data));
                    }
                    if ui.button("Hủy").clicked() {
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
            self.run("Đang đồng bộ...", true, sync);
        }
        ctx.request_repaint_after(Duration::from_secs(30));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions = Vec::new();
        egui::Panel::left("nav")
            .exact_size(210.0)
            .resizable(false)
            .show(ui, |ui| self.sidebar(ui, &mut actions));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(label) = self.busy.as_deref().filter(|l| !l.is_empty()) {
                    ui.spinner();
                    ui.label(label);
                } else if let Some((error, text)) = &self.notice {
                    let color = if *error {
                        DANGER
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.colored_label(color, text);
                } else {
                    ui.weak(" ");
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| match self.page {
            Page::Library => self.library_page(ui, &mut actions),
            Page::Store => self.store_page(ui, &mut actions),
            Page::Notifications => self.notifications(ui, &mut actions),
            Page::Settings => self.settings_page(ui, &mut actions),
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

const WARN: egui::Color32 = egui::Color32::from_rgb(200, 120, 0);
const DANGER: egui::Color32 = egui::Color32::from_rgb(200, 40, 40);

fn accent(prefs: &Prefs) -> egui::Color32 {
    let [r, g, b] = prefs.accent.rgb();
    egui::Color32::from_rgb(r, g, b)
}

/// Áp chủ đề, màu nhấn, cỡ chữ và chế độ gọn lên giao diện (có hiệu lực ngay).
fn apply_prefs(ctx: &egui::Context, prefs: &Prefs) {
    ctx.set_theme(match prefs.theme {
        Theme::System => egui::ThemePreference::System,
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    });
    ctx.set_zoom_factor(prefs.font_scale);
    let color = accent(prefs);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut style = egui::Style {
            visuals: match theme {
                egui::Theme::Dark => egui::Visuals::dark(),
                egui::Theme::Light => egui::Visuals::light(),
            },
            ..egui::Style::default()
        };
        style.visuals.selection.bg_fill = color;
        style.visuals.selection.stroke.color = egui::Color32::WHITE;
        style.visuals.hyperlink_color = color;
        style.visuals.widgets.hovered.bg_stroke.color = color;
        let r = 6;
        style.visuals.widgets.inactive.corner_radius = r.into();
        style.visuals.widgets.hovered.corner_radius = r.into();
        style.visuals.widgets.active.corner_radius = r.into();
        style.visuals.widgets.noninteractive.corner_radius = r.into();
        if prefs.compact {
            style.spacing.item_spacing = egui::vec2(6.0, 3.0);
            style.spacing.button_padding = egui::vec2(6.0, 2.0);
        } else {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 4.0);
        }
        ctx.set_style_of(theme, style);
    }
}

/// Biểu tượng chữ cái đầu của app, màu cố định theo mã app (design E4).
fn app_icon(ui: &mut egui::Ui, app_id: &str, title: &str, size: f32) -> egui::Response {
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
    let button = egui::Button::new(egui::RichText::new(text).color(egui::Color32::WHITE))
        .fill(accent(prefs));
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
        .corner_radius(10.0)
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
            self.notice = Some((true, format!("Không lưu được cài đặt: {e}")));
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            app_icon(ui, "f-launcher", "F Launcher", 28.0);
            ui.heading("F-Launcher");
        });
        ui.add_space(4.0);
        if self.view.online {
            ui.weak("Đã kết nối");
        } else {
            ui.colored_label(WARN, "Đang ngoại tuyến");
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
                    format!("Thư viện  ({} cập nhật)", updates)
                } else {
                    "Thư viện".to_owned()
                },
            ),
            (Page::Store, "Cửa hàng".to_owned()),
            (
                Page::Notifications,
                if unread > 0 {
                    format!("Thông báo  ({unread})")
                } else {
                    "Thông báo".to_owned()
                },
            ),
            (Page::Settings, "Cài đặt".to_owned()),
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
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(8.0);
            let idle = self.busy.is_none();
            if let Some(t) = self.view.last_sync {
                // Giờ địa phương của máy (Timestamp là UTC).
                let local = t.to_zoned(jiff::tz::TimeZone::system());
                ui.weak(format!("Đồng bộ lúc {}", local.strftime("%H:%M %d/%m")));
            }
            if ui.add_enabled(idle, egui::Button::new("Đồng bộ")).clicked() {
                actions.push(Action::Sync);
            }
            ui.separator();
            match self.view.sign_in {
                Some(SignIn::SignedIn) | Some(SignIn::Offline) => {
                    if ui
                        .add_enabled(idle, egui::Button::new("Đăng xuất"))
                        .clicked()
                    {
                        actions.push(Action::Logout);
                    }
                    ui.strong(
                        self.view
                            .user
                            .clone()
                            .unwrap_or_else(|| "Đã đăng nhập".into()),
                    );
                }
                _ => {
                    if primary_button(ui, &self.prefs, idle, "Đăng nhập") {
                        actions.push(Action::Login);
                    }
                    ui.weak("Chưa đăng nhập");
                }
            }
        });
    }

    /// Tiêu đề trang và thanh công cụ: tìm, nhóm, sắp xếp, lưới/danh sách.
    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let store = self.page == Page::Store;
        let (title, count) = if store {
            ("Cửa hàng", self.view.catalog.len())
        } else {
            ("Thư viện", self.view.my_apps.len())
        };
        let categories = if store {
            library::categories(&self.view.catalog)
        } else {
            library::categories(&self.view.my_apps)
        };
        let mut prefs = self.prefs.clone();
        ui.horizontal(|ui| {
            ui.heading(title);
            ui.weak(format!("{count} app"));
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
                            &format!("Cập nhật tất cả ({updates})"),
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
                    .hint_text("Tìm theo tên, mô tả, nhà phát triển...")
                    .desired_width(260.0),
            );
            let selected = filter
                .category
                .as_ref()
                .and_then(|c| categories.iter().find(|(code, _)| code == c))
                .map_or("Mọi nhóm".to_owned(), |(_, label)| {
                    format!("Nhóm: {label}")
                });
            egui::ComboBox::from_id_salt(("category", store))
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut filter.category, None, "Mọi nhóm");
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
                .selected_text(format!("Sắp xếp: {}", sort.label()))
                .show_ui(ui, |ui| {
                    for key in choices {
                        ui.selectable_value(sort, *key, key.label());
                    }
                });
            if (!filter.text.is_empty() || filter.category.is_some())
                && ui.button("Xóa bộ lọc").clicked()
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
            ui.label("Không có app nào khớp");
            if ui.button("Xóa bộ lọc").clicked() {
                if store {
                    self.store_filter = Filter::default();
                } else {
                    self.library_filter = Filter::default();
                }
            }
        });
    }

    /// Lưới: số cột theo chiều rộng cửa sổ và cỡ ô đã chọn.
    fn grid<T>(
        &self,
        ui: &mut egui::Ui,
        items: &[&T],
        mut tile: impl FnMut(&Self, &mut egui::Ui, &T),
    ) {
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
                            card_frame(ui).show(ui, |ui| {
                                ui.set_width(width);
                                ui.vertical(|ui| tile(self, ui, item));
                            });
                        }
                    });
                    ui.add_space(spacing);
                }
            });
    }

    fn list<T>(
        &self,
        ui: &mut egui::Ui,
        items: &[&T],
        mut row: impl FnMut(&Self, &mut egui::Ui, &T),
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for item in items {
                    let frame = if self.prefs.compact {
                        card_frame(ui).inner_margin(6.0)
                    } else {
                        card_frame(ui)
                    };
                    frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| row(self, ui, item));
                    });
                    ui.add_space(if self.prefs.compact { 2.0 } else { 6.0 });
                }
            });
    }

    /// Menu "..." của một app đã cài: cập nhật, quay lại, gỡ, mã nguồn.
    fn app_menu(&self, ui: &mut egui::Ui, app: &MyApp, actions: &mut Vec<Action>) {
        let idle = self.busy.is_none();
        ui.menu_button("...", |ui| {
            if let Some(u) = &app.update {
                if ui
                    .add_enabled(
                        idle,
                        egui::Button::new(format!("Cập nhật lên {}", u.version)),
                    )
                    .clicked()
                {
                    actions.push(Action::Update(app.app.clone()));
                    ui.close();
                }
            }
            if let Some(prev) = &app.previous_version {
                if ui
                    .add_enabled(idle, egui::Button::new(format!("Quay lại {prev}")))
                    .clicked()
                {
                    actions.push(Action::Rollback(app.app.clone()));
                    ui.close();
                }
            }
            if let Some(source) = &app.info.source {
                if ui.button("Xem mã nguồn").clicked() {
                    actions.push(Action::OpenLink(source.clone()));
                    ui.close();
                }
            }
            if ui.add_enabled(idle, egui::Button::new("Gỡ...")).clicked() {
                actions.push(Action::AskUninstall(Box::new(app.clone())));
                ui.close();
            }
        });
    }

    fn open_button(&self, ui: &mut egui::Ui, app: &MyApp, actions: &mut Vec<Action>) {
        let ready = app.status == AppStatus::Ready;
        let label = if app.info.terminal {
            "Mở (terminal)"
        } else {
            "Mở"
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
                ui.label("Chưa cài app nào.");
                if ui.button("Mở Cửa hàng").clicked() {
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
        let show_summary = self.prefs.show_summary;
        match self.prefs.layout {
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
                    ui.colored_label(DANGER, "Đã bị thu hồi");
                } else if let Some(u) = &app.update {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("Có bản {}", u.version)).color(WARN),
                        )
                        .truncate(),
                    );
                } else {
                    ui.add(
                        egui::Label::new(egui::RichText::new(format!("Bản {}", app.version)).weak())
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
                            ui.colored_label(WARN, "có bản cập nhật");
                        }
                    });
                    if show_summary {
                        if let Some(s) = &app.info.summary {
                            ui.add(egui::Label::new(s).truncate());
                        }
                    }
                    if let AppStatus::Revoked { reason } = &app.status {
                        ui.colored_label(
                            DANGER,
                            format!("Phiên bản này đã bị thu hồi: {reason}. Hãy cập nhật, quay lại bản trước hoặc gỡ app."),
                        );
                    } else {
                        ui.weak(byline(&app.info));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    me.app_menu(ui, app, actions);
                    if let Some(u) = &app.update {
                        if ui
                            .add_enabled(me.busy.is_none(), egui::Button::new(format!("Cập nhật lên {}", u.version)))
                            .clicked()
                        {
                            actions.push(Action::Update(app.app.clone()));
                        }
                    }
                    me.open_button(ui, app, actions);
                });
            }),
        }
    }

    /// Nút chính của một app trong Cửa hàng: Cài, Cập nhật, hoặc nhãn "Đã cài".
    fn store_button(&self, ui: &mut egui::Ui, entry: &CatalogEntry, actions: &mut Vec<Action>) {
        let installed = self.view.my_apps.iter().find(|a| a.app == entry.app);
        let label = match installed {
            Some(a) if a.version == entry.version => None,
            Some(_) => Some("Cập nhật"),
            None => Some("Cài"),
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
                ui.weak("Đã cài");
            }
        }
    }

    fn store_page(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if !self.view.online {
            ui.colored_label(
                WARN,
                "Đang ngoại tuyến. Danh mục sẽ cập nhật khi có mạng; các app đã cài vẫn mở được.",
            );
        }
        self.toolbar(ui, actions);
        if self.view.catalog.is_empty() {
            ui.label("Danh mục trống.");
            return;
        }
        let catalog = self.view.catalog.clone();
        let query = self.query(true);
        let shown = library::arrange(&catalog, &query);
        if shown.is_empty() {
            self.empty_result(ui, true);
            return;
        }
        let show_summary = self.prefs.show_summary;
        match self.prefs.layout {
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
                            ui.weak("· chạy trong terminal");
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
                        if ui.button("Mã nguồn").clicked() {
                            actions.push(Action::OpenLink(source.clone()));
                        }
                    }
                    me.store_button(ui, entry, actions);
                });
            }),
        }
    }

    /// Trang Cài đặt (spec launcher, "Cài đặt giao diện của launcher").
    fn settings_page(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let mut prefs = self.prefs.clone();
        let idle = self.busy.is_none();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Cài đặt");
                ui.weak("Cài đặt giao diện lưu trên máy này, không gửi lên máy chủ.");
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong("Giao diện");
                    egui::Grid::new("appearance")
                        .num_columns(2)
                        .spacing([24.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("Chủ đề");
                            ui.horizontal(|ui| {
                                for t in Theme::ALL {
                                    ui.selectable_value(&mut prefs.theme, t, t.label());
                                }
                            });
                            ui.end_row();

                            ui.label("Màu nhấn");
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
                            });
                            ui.end_row();

                            ui.label("Cỡ chữ");
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

                            ui.label("Bố cục mặc định");
                            ui.horizontal(|ui| {
                                for l in [Layout::Grid, Layout::List] {
                                    ui.selectable_value(&mut prefs.layout, l, l.label());
                                }
                            });
                            ui.end_row();

                            ui.label("Cỡ ô lưới");
                            ui.horizontal(|ui| {
                                for s in TileSize::ALL {
                                    ui.selectable_value(&mut prefs.tile_size, s, s.label());
                                }
                            });
                            ui.end_row();

                            ui.label("Trang mở đầu");
                            ui.horizontal(|ui| {
                                for p in [StartPage::Library, StartPage::Store] {
                                    ui.selectable_value(&mut prefs.start_page, p, p.label());
                                }
                            });
                            ui.end_row();

                            ui.label("Hiển thị");
                            ui.vertical(|ui| {
                                ui.checkbox(&mut prefs.show_summary, "Hiện mô tả ngắn của app");
                                ui.checkbox(&mut prefs.compact, "Chế độ gọn (ít khoảng trống)");
                            });
                            ui.end_row();
                        });
                    ui.add_space(6.0);
                    if ui.button("Khôi phục mặc định").clicked() {
                        prefs = Prefs::default();
                    }
                });
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong("Cập nhật");
                    let mut auto = self.view.auto_update;
                    if ui
                        .checkbox(
                            &mut auto,
                            "Tự động cập nhật app (kiểm tra khi mở và mỗi 6 giờ)",
                        )
                        .changed()
                    {
                        actions.push(Action::SetAutoUpdate(auto));
                    }
                    if ui
                        .add_enabled(idle, egui::Button::new("Kiểm tra cập nhật ngay"))
                        .clicked()
                    {
                        actions.push(Action::Sync);
                    }
                });
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong("Tài khoản");
                    match self.view.sign_in {
                        Some(SignIn::SignedIn) | Some(SignIn::Offline) => {
                            ui.label(format!(
                                "Đã đăng nhập: {}",
                                self.view.user.clone().unwrap_or_default()
                            ));
                            ui.horizontal(|ui| {
                                if ui.button("Ứng dụng đã kết nối").clicked() {
                                    if let Ok(url) = self.shared.config.server.join("apps") {
                                        actions.push(Action::OpenLink(url.to_string()));
                                    }
                                }
                                if ui
                                    .add_enabled(idle, egui::Button::new("Đăng xuất"))
                                    .clicked()
                                {
                                    actions.push(Action::Logout);
                                }
                            });
                        }
                        _ => {
                            ui.label("Chưa đăng nhập.");
                            if primary_button(ui, &self.prefs, idle, "Đăng nhập") {
                                actions.push(Action::Login);
                            }
                        }
                    }
                });
                ui.add_space(8.0);

                card_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong("Giới thiệu");
                    egui::Grid::new("about").num_columns(2).show(ui, |ui| {
                        ui.label("Phiên bản");
                        ui.label(env!("CARGO_PKG_VERSION"));
                        ui.end_row();
                        ui.label("Nền tảng");
                        ui.label(&self.shared.config.platform);
                        ui.end_row();
                        ui.label("Thư mục dữ liệu");
                        ui.label(self.shared.config.data_dir.display().to_string());
                        ui.end_row();
                    });
                    ui.weak(format!(
                        "{}. Mọi gói cài đặt được kiểm tra chữ ký của F-Eco trước khi cài.",
                        ISOLATION_NOTICE
                    ));
                });
            });
        self.set_prefs(prefs);
    }
}

fn summarize(results: &[(String, Result<(), LauncherError>)]) -> Result<String, String> {
    if results.is_empty() {
        return Ok("Mọi app đã ở bản mới nhất".into());
    }
    let failed: Vec<String> = results
        .iter()
        .filter_map(|(app, r)| r.as_ref().err().map(|e| format!("{app}: {e}")))
        .collect();
    if failed.is_empty() {
        Ok(format!("Đã cập nhật {} app", results.len()))
    } else {
        Err(format!(
            "Cập nhật lỗi (bản cũ vẫn dùng được): {}",
            failed.join("; ")
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
                    note = format!(" (không lấy được mã đăng nhập: {e})");
                    None
                }
            },
            Err(_) => {
                note = " (chưa đăng nhập: app sẽ tự yêu cầu đăng nhập)".into();
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
        .map_err(|e| format!("không mở được {}: {e}", plan.title))?;
    if let Err(e) = s.launcher.lock().await.mark_opened(app, Timestamp::now()) {
        eprintln!("không ghi được lần mở {app}: {e}");
    }
    Ok(format!("Đã mở {} {}{note}", plan.title, plan.version))
}
