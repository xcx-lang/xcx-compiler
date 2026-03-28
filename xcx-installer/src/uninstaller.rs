#![windows_subsystem = "windows"]
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use winreg::enums::*;
use winreg::RegKey;
use std::fs;

const C_BG:      egui::Color32 = egui::Color32::from_rgb(15,  15,  15);
const C_SURFACE: egui::Color32 = egui::Color32::from_rgb(24,  24,  24);
const C_SIDEBAR: egui::Color32 = egui::Color32::from_rgb(20,  20,  20);
const C_BORDER:  egui::Color32 = egui::Color32::from_rgb(45,  45,  45);
const C_RED:     egui::Color32 = egui::Color32::from_rgb(210, 30,  30);
const C_RED_DIM: egui::Color32 = egui::Color32::from_rgb(140, 20,  20);
const C_WHITE:   egui::Color32 = egui::Color32::from_rgb(240, 240, 240);
const C_MUTED:   egui::Color32 = egui::Color32::from_rgb(130, 130, 130);
const C_MUTED2:  egui::Color32 = egui::Color32::from_rgb(80,  80,  80);
const C_GREEN:   egui::Color32 = egui::Color32::from_rgb(60,  200, 100);

const XCX_ICON: &[u8] = include_bytes!("../../XCX_Ecosystem_v2.2.0/icons/xcx.ico");

#[derive(PartialEq, Clone)]
enum Screen {
    Welcome,
    Uninstalling,
    Finished,
    Error(String),
}

#[derive(Clone, Default)]
struct Progress {
    pub value: f32,
    pub status: String,
    pub log: Vec<String>,
    pub done: bool,
    pub error: Option<String>,
}

struct XcxUninstaller {
    screen: Screen,
    install_dir: PathBuf,
    progress: Arc<Mutex<Progress>>,
    started: bool,
}

impl Default for XcxUninstaller {
    fn default() -> Self {
        Self {
            screen: Screen::Welcome,
            install_dir: PathBuf::from("C:\\XCX"),
            progress: Arc::new(Mutex::new(Progress::default())),
            started: false,
        }
    }
}

fn remove_from_path(to_remove: &str) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env = hklm
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            KEY_READ | KEY_WRITE,
        )
        .map_err(|e: std::io::Error| e.to_string())?;
    let current: String = env.get_value("Path").unwrap_or_default();
    if current.contains(to_remove) {
        let updated = current
            .split(';')
            .filter(|&p| p != to_remove && !p.is_empty())
            .collect::<Vec<_>>()
            .join(";");
        env.set_value("Path", &updated).map_err(|e: std::io::Error| e.to_string())?;
        unsafe {
            use std::ffi::CString;
            let s = CString::new("Environment").unwrap();
            winapi::um::winuser::SendMessageTimeoutA(
                winapi::um::winuser::HWND_BROADCAST,
                winapi::um::winuser::WM_SETTINGCHANGE,
                0,
                s.as_ptr() as winapi::shared::minwindef::LPARAM,
                winapi::um::winuser::SMTO_ABORTIFHUNG,
                5000,
                std::ptr::null_mut(),
            );
        }
    }
    Ok(())
}

fn remove_associations() {
    if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags("Software\\Classes", KEY_ALL_ACCESS)
    {
        for key in &[".xcx", "XCX.Script", ".pax", "PAX.Package"] {
            let _ = hklm.delete_subkey_all(key);
        }
    }
}

fn run_uninstall(install_dir: PathBuf, arc: Arc<Mutex<Progress>>) {
    macro_rules! step {
        ($val:expr, $txt:expr) => {{
            let mut p = arc.lock().unwrap();
            p.value = $val;
            p.status = $txt.to_string();
            p.log.push($txt.to_string());
        }};
    }

    step!(0.15, "Removing file type associations...");
    remove_associations();

    step!(0.40, "Removing from system PATH...");
    let _ = remove_from_path(install_dir.join("bin").to_str().unwrap_or(""));

    step!(0.65, "Removing Add/Remove Programs entry...");
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let _ = hklm.delete_subkey_all(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\XCX",
    );

    step!(0.85, "Deleting files...");
    let _ = fs::remove_dir_all(&install_dir);

    step!(1.00, "Uninstallation complete.");
    arc.lock().unwrap().done = true;
}

fn styled_btn(ui: &mut egui::Ui, label: &str, primary: bool) -> bool {
    let fill = if primary { C_RED } else { C_SURFACE };
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(C_WHITE).size(13.0))
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, if primary { C_RED } else { C_BORDER }))
            .min_size(egui::vec2(90.0, 32.0))
            .rounding(egui::Rounding::same(4.0)),
    )
    .clicked()
}

impl eframe::App for XcxUninstaller {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(C_WHITE);
        visuals.panel_fill = C_BG;
        visuals.window_fill = C_BG;
        visuals.widgets.noninteractive.bg_fill = C_SURFACE;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, C_BORDER);
        visuals.widgets.inactive.bg_fill = C_SURFACE;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, C_BORDER);
        visuals.selection.bg_fill = C_RED_DIM;
        ctx.set_visuals(visuals);

        // Sidebar
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(210.0)
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let p = ui.painter().clone();
                let r = ui.max_rect();
                p.rect_filled(r, 0.0, C_SIDEBAR);
                p.line_segment(
                    [egui::pos2(r.right(), r.top()), egui::pos2(r.right(), r.bottom())],
                    egui::Stroke::new(1.0, C_BORDER),
                );
                let x0 = r.left() + 20.0;
                let mut y = r.top() + 28.0;
                p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP,
                    "XCX", egui::FontId::proportional(34.0), C_RED);
                y += 42.0;
                p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP,
                    "Ecosystem Uninstaller", egui::FontId::proportional(10.5), C_MUTED);
                y += 16.0;
                let ver_rect = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(38.0, 14.0));
                p.rect_filled(ver_rect, egui::Rounding::same(3.0), egui::Color32::from_rgb(35, 10, 10));
                p.text(ver_rect.center(), egui::Align2::CENTER_CENTER,
                    "v2.2.0", egui::FontId::proportional(9.5), C_RED);
                y += 24.0;
                p.line_segment(
                    [egui::pos2(x0, y), egui::pos2(r.right() - 20.0, y)],
                    egui::Stroke::new(1.0, C_BORDER),
                );
                y += 20.0;
                // Ostrzezenie
                let warn_rect = egui::Rect::from_min_size(
                    egui::pos2(x0, y), egui::vec2(r.width() - 40.0, 60.0));
                p.rect_filled(warn_rect, egui::Rounding::same(6.0), egui::Color32::from_rgb(35, 10, 10));
                p.text(egui::pos2(x0 + 8.0, y + 8.0), egui::Align2::LEFT_TOP,
                    "WARNING", egui::FontId::proportional(10.0), C_RED);
                p.text(egui::pos2(x0 + 8.0, y + 24.0), egui::Align2::LEFT_TOP,
                    "This will remove all", egui::FontId::proportional(9.5), C_MUTED2);
                p.text(egui::pos2(x0 + 8.0, y + 36.0), egui::Align2::LEFT_TOP,
                    "XCX files from", egui::FontId::proportional(9.5), C_MUTED2);
                p.text(egui::pos2(x0 + 8.0, y + 48.0), egui::Align2::LEFT_TOP,
                    "your system.", egui::FontId::proportional(9.5), C_MUTED2);
                ui.allocate_rect(r, egui::Sense::hover());
            });

        // Footer
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::none()
                    .fill(C_SURFACE)
                    .inner_margin(egui::Margin { left: 0.0, right: 0.0, top: 10.0, bottom: 10.0 }),
            )
            .exact_height(52.0)
            .show(ctx, |ui| {
                // Separator
                ui.painter().line_segment(
                    [ui.max_rect().left_top(), ui.max_rect().right_top()],
                    egui::Stroke::new(1.0, C_BORDER),
                );
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(20.0);
                        match self.screen.clone() {
                            Screen::Welcome => {
                                if styled_btn(ui, "Uninstall", true) && !self.started {
                                    self.started = true;
                                    self.screen = Screen::Uninstalling;
                                    let arc = Arc::clone(&self.progress);
                                    let dir = self.install_dir.clone();
                                    thread::spawn(move || run_uninstall(dir, arc));
                                }
                                ui.add_space(8.0);
                                if styled_btn(ui, "Cancel", false) {
                                    std::process::exit(0);
                                }
                            }
                            Screen::Finished | Screen::Error(_) => {
                                if styled_btn(ui, "Close", true) {
                                    std::process::exit(0);
                                }
                            }
                            Screen::Uninstalling => {}
                        }
                    });
                });
            });

        // Content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(C_BG).inner_margin(egui::Margin::same(28.0)))
            .show(ctx, |ui| {
                match self.screen.clone() {
                    Screen::Welcome => {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(35, 10, 10))
                                .rounding(egui::Rounding::same(8.0))
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("⚠").color(C_RED).size(18.0));
                                });
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("Uninstall XCX").color(C_WHITE).size(22.0).strong());
                        });
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Remove the XCX compiler and all associated files.").color(C_MUTED).size(13.0));
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // What will be removed
                        ui.label(egui::RichText::new("The following will be removed:").color(C_MUTED).size(12.0));
                        ui.add_space(10.0);
                        egui::Frame::none()
                            .fill(C_SURFACE)
                            .stroke(egui::Stroke::new(1.0, C_BORDER))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(14.0))
                            .show(ui, |ui| {
                                for item in &[
                                    "C:\\XCX  —  all compiler files",
                                    "System PATH entry for C:\\XCX\\bin",
                                    ".xcx and .pax file associations",
                                    "Add/Remove Programs registry entry",
                                ] {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("—").color(C_RED).size(12.0));
                                        ui.add_space(6.0);
                                        ui.label(egui::RichText::new(*item).color(C_MUTED).size(12.0).monospace());
                                    });
                                    ui.add_space(4.0);
                                }
                            });

                        ui.add_space(16.0);
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(35, 18, 12))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 50, 20)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(12.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("⚠").color(egui::Color32::from_rgb(220, 140, 40)).size(14.0));
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("This action cannot be undone. Your .xcx source files will not be deleted.").color(C_MUTED).size(12.0));
                                });
                            });
                    }

                    Screen::Uninstalling => {
                        let (done, error, value, status, log) = {
                            let p = self.progress.lock().unwrap();
                            (p.done, p.error.clone(), p.value, p.status.clone(), p.log.clone())
                        };
                        if done {
                            self.screen = if let Some(e) = error { Screen::Error(e) } else { Screen::Finished };
                        }
                        ctx.request_repaint();

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Uninstalling").color(C_WHITE).size(22.0).strong());
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Please wait while XCX is being removed.").color(C_MUTED).size(13.0));
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(24.0);

                        // Progress bar custom
                        let r = ui.available_rect_before_wrap();
                        let bar = egui::Rect::from_min_size(egui::pos2(r.left(), r.top()), egui::vec2(r.width(), 6.0));
                        ui.painter().rect_filled(bar, egui::Rounding::same(3.0), C_SURFACE);
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * value, 6.0)),
                            egui::Rounding::same(3.0), C_RED,
                        );
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&status).color(C_WHITE).size(12.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(format!("{:.0}%", value * 100.0)).color(C_MUTED).size(12.0));
                            });
                        });
                        ui.add_space(24.0);

                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(12, 12, 12))
                            .stroke(egui::Stroke::new(1.0, C_BORDER))
                            .rounding(egui::Rounding::same(4.0))
                            .inner_margin(egui::Margin::same(10.0))
                            .show(ui, |ui| {
                                ui.set_min_height(80.0);
                                for (i, entry) in log.iter().rev().take(5).rev().enumerate() {
                                    let last = i + 1 == log.len().min(5);
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(if last { "›" } else { " " }).color(C_RED).size(12.0));
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(entry).color(if last { C_WHITE } else { C_MUTED2 }).size(11.0).monospace());
                                    });
                                }
                            });
                    }

                    Screen::Finished => {
                        ui.add_space(50.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("✓").color(C_GREEN).size(48.0).strong());
                            ui.add_space(16.0);
                            ui.label(egui::RichText::new("XCX Removed").color(C_WHITE).size(26.0).strong());
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("The XCX ecosystem has been completely removed from your system.").color(C_MUTED).size(13.0));
                        });
                    }

                    Screen::Error(err) => {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Uninstall Failed").color(C_WHITE).size(22.0).strong());
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(30, 12, 12))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 30, 30)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(14.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(&err).color(egui::Color32::from_rgb(255, 120, 120)).size(12.0).monospace());
                            });
                    }
                }
            });
    }
}

fn main() {
    let icon_data = if let Ok(image) = image::load_from_memory(XCX_ICON) {
        let image = image.to_rgba8();
        let (width, height) = image.dimensions();
        Some(egui::IconData {
            rgba: image.into_raw(),
            width,
            height,
        })
    } else {
        None
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 400.0])
            .with_min_inner_size([600.0, 400.0])
            .with_resizable(true)
            .with_title("XCX Uninstaller")
            .with_icon(icon_data.unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "XCX Uninstaller",
        native_options,
        Box::new(|_cc| Box::new(XcxUninstaller::default())),
    )
    .expect("Failed to run uninstaller");
}