//! Bấm giờ: app mẫu của F-Eco. Mở từ F-Launcher thì tự đăng nhập bằng tài khoản F-Eco (mã khởi
//! chạy, F-SDK) và chào người dùng; số phiên tập trung lưu trong thư mục dữ liệu của app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use f_timer::{format, FocusTimer, Phase, Stats, Stopwatch};
use feco_sample_common::{data_dir, install_fonts, Account};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bấm giờ")
            .with_inner_size([480.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Bấm giờ",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(TimerApp::new(&cc.egui_ctx)))
        }),
    )
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Mode {
    Stopwatch,
    Focus,
}

struct TimerApp {
    origin: Instant,
    mode: Mode,
    stopwatch: Stopwatch,
    focus: FocusTimer,
    focus_minutes: u64,
    rest_minutes: u64,
    stats: Stats,
    dir: PathBuf,
    account: Account,
    message: Option<String>,
}

impl TimerApp {
    fn new(ctx: &egui::Context) -> Self {
        let dir = data_dir("f-timer");
        Self {
            origin: Instant::now(),
            mode: Mode::Stopwatch,
            stopwatch: Stopwatch::default(),
            focus: FocusTimer::new(25, 5),
            focus_minutes: 25,
            rest_minutes: 5,
            stats: Stats::load(&dir),
            dir,
            account: Account::start(ctx),
            message: None,
        }
    }

    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

fn big(text: String) -> egui::RichText {
    egui::RichText::new(text).size(56.0).monospace()
}

impl eframe::App for TimerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = self.now();
        if let Some(done) = self.focus.tick(now) {
            if done == Phase::Focus {
                self.stats.focus_sessions += 1;
                if let Err(e) = self.stats.save(&self.dir) {
                    self.message = Some(format!("Không lưu được thống kê: {e}"));
                }
            }
            self.message = Some(format!(
                "Hết giờ {}. Bấm Bắt đầu để tiếp tục.",
                done.label()
            ));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Bấm giờ");
                ui.separator();
                ui.weak(self.account.state().describe());
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Stopwatch, "Đồng hồ bấm giờ");
                ui.selectable_value(&mut self.mode, Mode::Focus, "Hẹn giờ tập trung");
            });
            ui.separator();
            match self.mode {
                Mode::Stopwatch => {
                    ui.vertical_centered(|ui| {
                        ui.label(big(format(self.stopwatch.elapsed(now), true)));
                    });
                    ui.horizontal(|ui| {
                        if self.stopwatch.running() {
                            if ui.button("Tạm dừng").clicked() {
                                self.stopwatch.pause(now);
                            }
                            if ui.button("Vòng").clicked() {
                                self.stopwatch.lap(now);
                            }
                        } else if ui.button("Bắt đầu").clicked() {
                            self.stopwatch.start(now);
                        }
                        if ui.button("Đặt lại").clicked() {
                            self.stopwatch.reset();
                        }
                    });
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, lap) in self.stopwatch.laps.iter().enumerate().rev() {
                            ui.monospace(format!("Vòng {:>2}   {}", i + 1, format(*lap, true)));
                        }
                    });
                }
                Mode::Focus => {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(self.focus.phase.label()).strong());
                        ui.label(big(format(self.focus.remaining(now), false)));
                    });
                    ui.horizontal(|ui| {
                        if self.focus.running() {
                            if ui.button("Tạm dừng").clicked() {
                                self.focus.pause(now);
                            }
                        } else if ui.button("Bắt đầu").clicked() {
                            self.message = None;
                            self.focus.start(now);
                        }
                        if ui.button("Đặt lại").clicked() {
                            self.focus.reset();
                        }
                    });
                    ui.add_enabled_ui(!self.focus.running(), |ui| {
                        let mut changed = false;
                        ui.horizontal(|ui| {
                            ui.label("Tập trung");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut self.focus_minutes)
                                        .range(1..=180)
                                        .suffix(" phút"),
                                )
                                .changed();
                            ui.label("Nghỉ");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut self.rest_minutes)
                                        .range(1..=60)
                                        .suffix(" phút"),
                                )
                                .changed();
                        });
                        if changed {
                            self.focus = FocusTimer::new(self.focus_minutes, self.rest_minutes);
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(format!(
                        "Đã hoàn thành {} phiên tập trung",
                        self.stats.focus_sessions
                    ));
                }
            }
            if let Some(m) = &self.message {
                ui.separator();
                ui.label(m);
            }
        });
        if self.stopwatch.running() || self.focus.running() {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
    }
}
