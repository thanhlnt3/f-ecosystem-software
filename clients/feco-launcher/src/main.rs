//! Giao diện F-Launcher (egui). Mọi logic nằm trong thư viện `feco_launcher`; tệp này chỉ vẽ
//! giao diện và chạy các thao tác trên một runtime tokio riêng để giao diện không bị treo.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use feco_launcher::auth::{Auth, AuthError, KeyringStore, Session};
use feco_launcher::broker::{Broker, Issuer};
use feco_launcher::launch::{command, LaunchEnv};
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
            .with_inner_size([920.0, 640.0]),
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
    }
}

struct Done {
    result: Result<String, String>,
    view: View,
    synced: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    MyApps,
    Catalog,
}

enum Action {
    Login,
    Logout,
    Sync,
    AskInstall(CatalogEntry),
    Install(CatalogEntry),
    Update(String),
    UpdateAll,
    Rollback(String),
    AskUninstall(MyApp),
    Uninstall(String, bool),
    Open(String),
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
    tab: Tab,
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
            tab: Tab::MyApps,
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
            Action::AskInstall(entry) => self.confirm_install = Some(entry),
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
            Action::AskUninstall(app) => self.confirm_uninstall = Some((app, false)),
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

    fn top_bar(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.heading("F-Launcher");
            ui.separator();
            if self.view.online {
                ui.label("Đã kết nối");
            } else {
                ui.colored_label(egui::Color32::from_rgb(200, 120, 0), "Đang ngoại tuyến");
            }
            if let Some(t) = self.view.last_sync {
                ui.weak(format!("đồng bộ lần cuối {}", t.strftime("%H:%M %d/%m")));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let idle = self.busy.is_none();
                match self.view.sign_in {
                    Some(SignIn::SignedIn) | Some(SignIn::Offline) => {
                        if ui
                            .add_enabled(idle, egui::Button::new("Đăng xuất"))
                            .clicked()
                        {
                            actions.push(Action::Logout);
                        }
                        ui.label(
                            self.view
                                .user
                                .clone()
                                .unwrap_or_else(|| "Đã đăng nhập".into()),
                        );
                    }
                    _ => {
                        if ui
                            .add_enabled(idle, egui::Button::new("Đăng nhập"))
                            .clicked()
                        {
                            actions.push(Action::Login);
                        }
                    }
                }
                if ui.add_enabled(idle, egui::Button::new("Đồng bộ")).clicked() {
                    actions.push(Action::Sync);
                }
            });
        });
    }

    fn my_apps(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let idle = self.busy.is_none();
        ui.horizontal(|ui| {
            let mut auto = self.view.auto_update;
            if ui.checkbox(&mut auto, "Tự động cập nhật").changed() {
                actions.push(Action::SetAutoUpdate(auto));
            }
            let any_update = self.view.my_apps.iter().any(|a| a.update.is_some());
            if ui
                .add_enabled(idle && any_update, egui::Button::new("Cập nhật tất cả"))
                .clicked()
            {
                actions.push(Action::UpdateAll);
            }
        });
        ui.separator();
        if self.view.my_apps.is_empty() {
            ui.label("Chưa cài app nào. Mở tab Danh mục để cài.");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for app in &self.view.my_apps {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&app.title);
                        ui.label(&app.version);
                    });
                    if let AppStatus::Revoked { reason } = &app.status {
                        ui.colored_label(
                            egui::Color32::from_rgb(200, 40, 40),
                            format!("Phiên bản này đã bị thu hồi: {reason}. Hãy cập nhật, quay lại bản trước hoặc gỡ app."),
                        );
                    }
                    ui.horizontal(|ui| {
                        let ready = app.status == AppStatus::Ready;
                        if ui.add_enabled(idle && ready, egui::Button::new("Mở")).clicked() {
                            actions.push(Action::Open(app.app.clone()));
                        }
                        if let Some(u) = &app.update {
                            if ui
                                .add_enabled(idle, egui::Button::new(format!("Cập nhật lên {}", u.version)))
                                .clicked()
                            {
                                actions.push(Action::Update(app.app.clone()));
                            }
                        }
                        if let Some(prev) = &app.previous_version {
                            if ui
                                .add_enabled(idle, egui::Button::new(format!("Quay lại {prev}")))
                                .clicked()
                            {
                                actions.push(Action::Rollback(app.app.clone()));
                            }
                        }
                        if ui.add_enabled(idle, egui::Button::new("Gỡ")).clicked() {
                            actions.push(Action::AskUninstall(app.clone()));
                        }
                    });
                });
            }
        });
    }

    fn catalog(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if !self.view.online {
            ui.colored_label(
                egui::Color32::from_rgb(200, 120, 0),
                "Đang ngoại tuyến. Danh mục sẽ cập nhật khi có mạng; các app đã cài vẫn mở được.",
            );
            ui.separator();
        }
        if self.view.catalog.is_empty() {
            ui.label("Danh mục trống.");
            return;
        }
        let idle = self.busy.is_none();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in &self.view.catalog {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&entry.title);
                        ui.label(&entry.version);
                        ui.weak(format!("{:.1} MB", entry.length as f64 / 1_048_576.0));
                    });
                    ui.weak(ISOLATION_NOTICE);
                    let installed = self.view.my_apps.iter().find(|a| a.app == entry.app);
                    let label = match installed {
                        Some(a) if a.version == entry.version => None,
                        Some(_) => Some("Cập nhật"),
                        None => Some("Cài"),
                    };
                    match label {
                        Some(label) => {
                            if ui
                                .add_enabled(idle && self.view.online, egui::Button::new(label))
                                .clicked()
                            {
                                actions.push(Action::AskInstall(entry.clone()));
                            }
                        }
                        None => {
                            ui.label("Đã cài");
                        }
                    }
                });
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if let Some(entry) = &self.confirm_install {
            egui::Modal::new(egui::Id::unique("confirm-install")).show(ctx, |ui| {
                ui.heading(format!("Cài {} {}?", entry.title, entry.version));
                ui.label(format!(
                    "Dung lượng tải về: {:.1} MB. Gói được kiểm tra chữ ký của F-Eco trước khi cài.",
                    entry.length as f64 / 1_048_576.0
                ));
                ui.colored_label(egui::Color32::from_rgb(200, 120, 0), ISOLATION_NOTICE);
                ui.horizontal(|ui| {
                    if ui.button("Cài").clicked() {
                        actions.push(Action::Install(entry.clone()));
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
        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui, &mut actions));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(label) = self.busy.as_deref().filter(|l| !l.is_empty()) {
                    ui.spinner();
                    ui.label(label);
                } else if let Some((error, text)) = &self.notice {
                    let color = if *error {
                        egui::Color32::from_rgb(200, 40, 40)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.colored_label(color, text);
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::MyApps, "App của tôi");
                ui.selectable_value(&mut self.tab, Tab::Catalog, "Danh mục");
            });
            ui.separator();
            match self.tab {
                Tab::MyApps => self.my_apps(ui, &mut actions),
                Tab::Catalog => self.catalog(ui, &mut actions),
            }
        });
        let ctx = ui.ctx().clone();
        self.dialogs(&ctx, &mut actions);
        for action in actions {
            self.apply(action);
        }
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
    if l.state().auto_update && l.due_for_update_check(now) {
        let results = l.update_all(now).await;
        if !results.is_empty() {
            return summarize(&results);
        }
    }
    Ok(String::new())
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
        broker_url: secret.as_ref().map(|_| s.broker_url.clone()),
        broker_secret: secret,
    };
    command(&plan, &env)
        .spawn()
        .map_err(|e| format!("không mở được {}: {e}", plan.title))?;
    Ok(format!("Đã mở {} {}{note}", plan.title, plan.version))
}
