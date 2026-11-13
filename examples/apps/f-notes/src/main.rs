//! Ghi chú: app mẫu của F-Eco. Mở từ F-Launcher thì tự đăng nhập bằng tài khoản F-Eco (mã khởi
//! chạy, F-SDK) và chào người dùng; ghi chú lưu trong thư mục dữ liệu launcher cấp cho app.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use f_notes::Notebook;
use feco_sample_common::{data_dir, install_fonts, Account};
use jiff::Timestamp;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ghi chú")
            .with_inner_size([820.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Ghi chú",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(NotesApp::new(&cc.egui_ctx)))
        }),
    )
}

struct NotesApp {
    dir: PathBuf,
    book: Notebook,
    selected: Option<u64>,
    search: String,
    account: Account,
    /// Lưu sau khi người dùng ngừng gõ một lúc.
    dirty_since: Option<Instant>,
    error: Option<String>,
}

impl NotesApp {
    fn new(ctx: &egui::Context) -> Self {
        let dir = data_dir("f-notes");
        let (book, error) = match Notebook::load(&dir) {
            Ok(b) => (b, None),
            Err(e) => (
                Notebook::default(),
                Some(format!("Không đọc được ghi chú: {e}")),
            ),
        };
        Self {
            selected: book.notes.first().map(|n| n.id),
            dir,
            book,
            search: String::new(),
            account: Account::start(ctx),
            dirty_since: None,
            error,
        }
    }

    fn save(&mut self) {
        self.dirty_since = None;
        self.error = self
            .book
            .save(&self.dir)
            .err()
            .map(|e| format!("Không lưu được: {e}"));
    }
}

impl eframe::App for NotesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Ghi chú");
                ui.separator();
                ui.weak(self.account.state().describe());
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| match &self.error {
            Some(e) => {
                ui.colored_label(egui::Color32::from_rgb(200, 40, 40), e);
            }
            None => {
                ui.weak(format!(
                    "{} ghi chú · lưu tại {}",
                    self.book.notes.len(),
                    self.dir.display()
                ));
            }
        });
        egui::Panel::left("list")
            .default_size(260.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("+ Ghi chú mới").clicked() {
                        let id = self.book.add(Timestamp::now());
                        self.selected = Some(id);
                        self.save();
                    }
                });
                ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Tìm ghi chú"));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let found: Vec<(u64, String, String)> = self
                        .book
                        .search(&self.search)
                        .iter()
                        .map(|n| {
                            (
                                n.id,
                                n.display_title(),
                                n.updated_at
                                    .to_zoned(jiff::tz::TimeZone::system())
                                    .strftime("%H:%M %d/%m")
                                    .to_string(),
                            )
                        })
                        .collect();
                    for (id, title, when) in found {
                        let selected = self.selected == Some(id);
                        let text = format!("{title}\n{when}");
                        if ui
                            .add(
                                egui::Button::selectable(selected, text)
                                    .min_size(egui::vec2(ui.available_width(), 0.0)),
                            )
                            .clicked()
                        {
                            self.selected = Some(id);
                        }
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(id) = self.selected else {
                ui.label("Chọn hoặc tạo một ghi chú.");
                return;
            };
            let mut delete = false;
            let mut changed = false;
            if let Some(note) = self.book.get_mut(id) {
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut note.title)
                            .hint_text("Tiêu đề")
                            .font(egui::TextStyle::Heading)
                            .desired_width(f32::INFINITY),
                    )
                    .changed();
                changed |= ui
                    .add_sized(
                        [
                            ui.available_width(),
                            (ui.available_height() - 40.0).max(80.0),
                        ],
                        egui::TextEdit::multiline(&mut note.body).hint_text("Nội dung"),
                    )
                    .changed();
                if changed {
                    note.updated_at = Timestamp::now();
                }
                delete = ui.button("Xóa ghi chú").clicked();
            }
            if changed && self.dirty_since.is_none() {
                self.dirty_since = Some(Instant::now());
            }
            if delete {
                self.book.remove(id);
                self.selected = self.book.notes.first().map(|n| n.id);
                self.save();
            }
        });
        if self
            .dirty_since
            .is_some_and(|t| t.elapsed() > Duration::from_millis(800))
        {
            self.save();
        } else if self.dirty_since.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(300));
        }
    }

    fn on_exit(&mut self) {
        if self.dirty_since.is_some() {
            self.save();
        }
    }
}
