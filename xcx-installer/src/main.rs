#![windows_subsystem = "windows"]
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use winreg::enums::*;
use winreg::RegKey;
use std::fs;

const XCX_EXE: &[u8] = include_bytes!("../../target/release/xcx-compiler.exe");
const PAX_XCX: &[u8] = include_bytes!("../../lib/pax.xcx");
const MATH_XCX: &[u8] = include_bytes!("../../lib/math.xcx");
const XCX_ICON: &[u8] = include_bytes!("../../XCX_Ecosystem_v2.2.0/icons/xcx.ico");
const PAX_ICON: &[u8] = include_bytes!("../../XCX_Ecosystem_v2.2.0/icons/pax.ico");
const UNINSTALLER_EXE: &[u8] = include_bytes!("../target/release/xcx-uninstall.exe");
const LICENSE_TXT: &[u8] = include_bytes!("../../XCX_Ecosystem_v2.2.0/LICENSE.txt");
const README_TXT: &[u8] = include_bytes!("../../XCX_Ecosystem_v2.2.0/README.txt");

// ── Kolory ────────────────────────────────────────────────────────────────────
const C_BG:        egui::Color32 = egui::Color32::from_rgb(15,  15,  15);
const C_SURFACE:   egui::Color32 = egui::Color32::from_rgb(24,  24,  24);
const C_SIDEBAR:   egui::Color32 = egui::Color32::from_rgb(20,  20,  20);
const C_BORDER:    egui::Color32 = egui::Color32::from_rgb(45,  45,  45);
const C_RED:       egui::Color32 = egui::Color32::from_rgb(210,  30,  30);
const C_RED_DIM:   egui::Color32 = egui::Color32::from_rgb(140,  20,  20);
const C_WHITE:     egui::Color32 = egui::Color32::from_rgb(240, 240, 240);
const C_MUTED:     egui::Color32 = egui::Color32::from_rgb(130, 130, 130);
const C_MUTED2:    egui::Color32 = egui::Color32::from_rgb(80,   80,  80);
const C_GREEN:     egui::Color32 = egui::Color32::from_rgb( 60, 200, 100);
const C_STEP_DONE: egui::Color32 = egui::Color32::from_rgb( 60, 200, 100);

#[derive(PartialEq, Clone)]
enum Screen {
    Welcome,
    Terms,
    Options,
    Installing,
    Bonus,
    Finished,
    Error(String),
}

impl Screen {
    fn step_index(&self) -> usize {
        match self {
            Screen::Welcome   => 0,
            Screen::Terms     => 1,
            Screen::Options   => 2,
            Screen::Installing => 3,
            Screen::Bonus     => 4,
            Screen::Finished  => 4,
            Screen::Error(_)  => 3,
        }
    }
}

#[derive(Clone)]
struct InstallProgress {
    pub progress: f32,
    pub status_text: String,
    pub log: Vec<String>,
    pub done: bool,
    pub error: Option<String>,
}

impl Default for InstallProgress {
    fn default() -> Self {
        Self {
            progress: 0.0,
            status_text: "Preparing...".to_string(),
            log: Vec::new(),
            done: false,
            error: None,
        }
    }
}

struct XcxInstaller {
    screen: Screen,
    install_pax: bool,
    associate_files: bool,
    terms_accepted: bool,
    install_dir: PathBuf,
    install_progress: Arc<Mutex<InstallProgress>>,
    install_started: bool,
}

impl Default for XcxInstaller {
    fn default() -> Self {
        Self {
            screen: Screen::Welcome,
            install_pax: true,
            associate_files: true,
            terms_accepted: false,
            install_dir: PathBuf::from("C:\\XCX"),
            install_progress: Arc::new(Mutex::new(InstallProgress::default())),
            install_started: false,
        }
    }
}

// ── Logika instalacji ─────────────────────────────────────────────────────────

fn run_install(
    install_dir: PathBuf,
    install_pax: bool,
    associate_files: bool,
    progress_arc: Arc<Mutex<InstallProgress>>,
) {
    macro_rules! step {
        ($progress:expr, $text:expr) => {{
            let mut p = progress_arc.lock().unwrap();
            p.progress = $progress;
            p.status_text = $text.to_string();
            p.log.push($text.to_string());
        }};
    }
    macro_rules! bail {
        ($err:expr) => {{
            let mut p = progress_arc.lock().unwrap();
            p.error = Some($err.to_string());
            p.done = true;
            return;
        }};
    }

    step!(0.05, "Creating directory structure...");
    for dir in [&install_dir, &install_dir.join("bin"), &install_dir.join("lib")] {
        if let Err(e) = fs::create_dir_all(dir) {
            bail!(format!("Cannot create {}: {}", dir.display(), e));
        }
    }

    step!(0.20, "Copying XCX compiler binary...");
    let xcx_path = install_dir.join("bin").join("xcx.exe");
    if let Err(e) = fs::write(&xcx_path, XCX_EXE) {
        bail!(format!("Failed to write xcx.exe: {}", e));
    }

    step!(0.35, "Writing uninstaller...");
    let uninstall_path = install_dir.join("uninstall.exe");
    if let Err(e) = fs::write(&uninstall_path, UNINSTALLER_EXE) {
        bail!(format!("Failed to write uninstaller: {}", e));
    }

    if install_pax {
        step!(0.50, "Installing PAX package manager...");
        if let Err(e) = fs::write(install_dir.join("lib").join("pax.xcx"), PAX_XCX) {
            bail!(format!("Failed to write pax.xcx: {}", e));
        }
        step!(0.58, "Installing math standard library...");
        if let Err(e) = fs::write(install_dir.join("lib").join("math.xcx"), MATH_XCX) {
            bail!(format!("Failed to write math.xcx: {}", e));
        }
        step!(0.62, "Writing license and readme...");
        let _ = fs::write(install_dir.join("LICENSE.txt"), LICENSE_TXT);
        let _ = fs::write(install_dir.join("README.txt"), README_TXT);
    }

    step!(0.68, "Configuring system PATH...");
    if let Err(e) = add_to_path(install_dir.join("bin").to_str().unwrap_or("")) {
        bail!(format!("Failed to update PATH: {}", e));
    }

    if associate_files {
        step!(0.80, "Registering file associations...");
        if let Err(e) = setup_associations(&xcx_path, &install_dir) {
            bail!(format!("Failed to set file associations: {}", e));
        }
    }

    step!(0.92, "Registering uninstaller entry...");
    let _ = register_uninstaller(&install_dir);

    step!(1.00, "Installation complete.");
    progress_arc.lock().unwrap().done = true;
}

fn register_uninstaller(install_dir: &Path) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (key, _) = hklm
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\XCX")
        .map_err(|e| e.to_string())?;
    let uninstall_exe = install_dir.join("uninstall.exe");
    key.set_value("DisplayName",    &"XCX Compiler Ecosystem").ok();
    key.set_value("UninstallString",&format!("\"{}\"", uninstall_exe.to_str().unwrap_or(""))).ok();
    key.set_value("DisplayVersion", &"2.2.0").ok();
    key.set_value("Publisher",      &"XCX Team").ok();
    key.set_value("DisplayIcon",    &install_dir.join("xcx.ico").to_str().unwrap_or("")).ok();
    Ok(())
}

fn add_to_path(new_path: &str) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let env = hklm
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            KEY_READ | KEY_WRITE,
        )
        .map_err(|e: std::io::Error| e.to_string())?;
    let current: String = env.get_value("Path").unwrap_or_default();
    if !current.contains(new_path) {
        let updated = if current.is_empty() {
            new_path.to_string()
        } else if current.ends_with(';') {
            format!("{}{}", current, new_path)
        } else {
            format!("{};{}", current, new_path)
        };
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

fn setup_associations(xcx_exe_path: &Path, install_dir: &Path) -> Result<(), String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let classes = hklm
        .open_subkey_with_flags("Software\\Classes", KEY_ALL_ACCESS)
        .map_err(|e: std::io::Error| e.to_string())?;

    // .xcx
    let (ext, _)  = classes.create_subkey(".xcx").map_err(|e| e.to_string())?;
    ext.set_value("", &"XCX.Script").ok();
    let (typ, _)  = classes.create_subkey("XCX.Script").map_err(|e| e.to_string())?;
    typ.set_value("", &"XCX Script File").ok();
    let ico = install_dir.join("xcx.ico");
    fs::write(&ico, XCX_ICON).ok();
    let (ik, _)   = typ.create_subkey("DefaultIcon").map_err(|e| e.to_string())?;
    ik.set_value("", &ico.to_str().unwrap_or("")).ok();
    let (sh, _)   = typ.create_subkey("shell\\open\\command").map_err(|e| e.to_string())?;
    sh.set_value("", &format!("\"{}\" \"%1\"", xcx_exe_path.to_str().unwrap_or(""))).ok();

    // .pax
    let (pext, _) = classes.create_subkey(".pax").map_err(|e| e.to_string())?;
    pext.set_value("", &"PAX.Package").ok();
    let (ptyp, _) = classes.create_subkey("PAX.Package").map_err(|e| e.to_string())?;
    ptyp.set_value("", &"PAX Package File").ok();
    let pico = install_dir.join("pax.ico");
    fs::write(&pico, PAX_ICON).ok();
    let (pik, _)  = ptyp.create_subkey("DefaultIcon").map_err(|e| e.to_string())?;
    pik.set_value("", &pico.to_str().unwrap_or("")).ok();

    Ok(())
}

fn open_url(url: &str) {
    use std::ffi::CString;
    let url_c = CString::new(url).unwrap_or_default();
    let op_c  = CString::new("open").unwrap_or_default();
    unsafe {
        winapi::um::shellapi::ShellExecuteA(
            std::ptr::null_mut(), op_c.as_ptr(), url_c.as_ptr(),
            std::ptr::null(), std::ptr::null(),
            winapi::um::winuser::SW_SHOWNORMAL,
        );
    }
}

// ── UI helpers ────────────────────────────────────────────────────────────────

/// Rysuje sidebar z krokami — wszystko przez painter, zero rozciagania
fn draw_sidebar(ui: &mut egui::Ui, current_step: usize) {
    let steps = ["Welcome", "Terms", "Options", "Installing", "Finish"];
    let p = ui.painter().clone();
    let r = ui.max_rect();

    p.rect_filled(r, 0.0, C_SIDEBAR);
    p.line_segment(
        [egui::pos2(r.right(), r.top()), egui::pos2(r.right(), r.bottom())],
        egui::Stroke::new(1.0, C_BORDER),
    );

    let font_big   = egui::FontId::proportional(34.0);
    let font_small = egui::FontId::proportional(10.5);
    let font_ver   = egui::FontId::proportional(9.5);
    let font_step  = egui::FontId::proportional(13.0);
    let font_badge = egui::FontId::proportional(11.0);

    let x0 = r.left() + 20.0;
    let mut y = r.top() + 28.0;

    // Logo
    p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP, "XCX", font_big, C_RED);
    y += 42.0;
    p.text(egui::pos2(x0, y), egui::Align2::LEFT_TOP, "Ecosystem Installer", font_small, C_MUTED);
    y += 16.0;
    let ver_rect = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(38.0, 14.0));
    p.rect_filled(ver_rect, egui::Rounding::same(3.0), egui::Color32::from_rgb(35, 10, 10));
    p.text(ver_rect.center(), egui::Align2::CENTER_CENTER, "v2.2.0", font_ver, C_RED);
    y += 24.0;

    // Separator
    p.line_segment(
        [egui::pos2(x0, y), egui::pos2(r.right() - 20.0, y)],
        egui::Stroke::new(1.0, C_BORDER),
    );
    y += 18.0;

    // Kroki
    let step_h   = 38.0;
    let badge_sz = 24.0;
    let badge_x  = x0 + 6.0;
    let text_x   = badge_x + badge_sz + 10.0;

    for (i, label) in steps.iter().enumerate() {
        let is_active = i == current_step;
        let is_done   = i < current_step;
        let row_top = y;
        let row_bot = y + step_h;

        if is_active {
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(r.left(), row_top), egui::pos2(r.right(), row_bot)),
                0.0,
                egui::Color32::from_rgb(28, 10, 10),
            );
            p.rect_filled(
                egui::Rect::from_min_size(egui::pos2(r.left(), row_top), egui::vec2(3.0, step_h)),
                0.0,
                C_RED,
            );
        }

        let badge_center = egui::pos2(badge_x + badge_sz / 2.0, row_top + step_h / 2.0);
        let (badge_bg, badge_fg) = if is_done {
            (C_STEP_DONE, egui::Color32::from_rgb(10, 10, 10))
        } else if is_active {
            (C_RED, C_WHITE)
        } else {
            (egui::Color32::from_rgb(38, 38, 38), C_MUTED2)
        };
        p.rect_filled(
            egui::Rect::from_center_size(badge_center, egui::vec2(badge_sz, badge_sz)),
            egui::Rounding::same(6.0),
            badge_bg,
        );
        let sym = if is_done { "✓".to_string() } else { (i + 1).to_string() };
        p.text(badge_center, egui::Align2::CENTER_CENTER, sym, font_badge.clone(), badge_fg);

        let tc = if is_active { C_WHITE } else if is_done { C_MUTED } else { C_MUTED2 };
        p.text(
            egui::pos2(text_x, row_top + step_h / 2.0),
            egui::Align2::LEFT_CENTER,
            *label,
            font_step.clone(),
            tc,
        );

        y += step_h + 4.0;
    }

    ui.allocate_rect(r, egui::Sense::hover());
}

/// Rysuje dolny pasek z przyciskami
fn draw_footer(ui: &mut egui::Ui, screen: &mut Screen, install_started: &mut bool,
               install_pax: bool, associate_files: bool, terms_accepted: &mut bool, install_dir: &PathBuf,
               install_progress: &Arc<Mutex<InstallProgress>>) {

    // Separator nad stopką
    ui.painter().line_segment(
        [ui.max_rect().left_top(), ui.max_rect().right_top()],
        egui::Stroke::new(1.0, C_BORDER),
    );

    ui.horizontal(|ui| {
        ui.add_space(20.0);

        // Lewa strona: ścieżka instalacji jeśli Options
        if *screen == Screen::Options {
            ui.label(egui::RichText::new("Installing to:").color(C_MUTED).size(11.0));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(install_dir.to_str().unwrap_or(""))
                    .color(C_MUTED)
                    .size(11.0)
                    .monospace(),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(20.0);

            match screen.clone() {
                Screen::Welcome => {
                    if styled_btn(ui, "Next  →", true) {
                        *screen = Screen::Terms;
                    }
                    ui.add_space(8.0);
                    if styled_btn(ui, "Cancel", false) {
                        std::process::exit(0);
                    }
                }
                Screen::Terms => {
                    if styled_btn(ui, "Next  →", *terms_accepted) {
                        if *terms_accepted {
                            *screen = Screen::Options;
                        }
                    }
                    ui.add_space(8.0);
                    if styled_btn(ui, "< Back", false) {
                        *screen = Screen::Welcome;
                    }
                }
                Screen::Options => {
                    if styled_btn(ui, "Install", true) && !*install_started {
                        *install_started = true;
                        *screen = Screen::Installing;
                        {
                            let mut p = install_progress.lock().unwrap();
                            *p = InstallProgress::default();
                        }
                        let arc   = Arc::clone(install_progress);
                        let dir   = install_dir.clone();
                        let pax   = install_pax;
                        let assoc = associate_files;
                        thread::spawn(move || run_install(dir, pax, assoc, arc));
                    }
                    ui.add_space(8.0);
                    if styled_btn(ui, "< Back", false) {
                        *screen = Screen::Terms;
                    }
                }
                Screen::Bonus => {
                    if styled_btn(ui, "Finish", true) {
                        *screen = Screen::Finished;
                    }
                }
                Screen::Finished => {
                    if styled_btn(ui, "Close", true) {
                        std::process::exit(0);
                    }
                }
                Screen::Error(_) => {
                    if styled_btn(ui, "Retry", true) {
                        *install_started = false;
                        *screen = Screen::Options;
                    }
                    ui.add_space(8.0);
                    if styled_btn(ui, "Close", false) {
                        std::process::exit(1);
                    }
                }
                Screen::Installing => {}
            }
        });
    });
}

fn styled_btn(ui: &mut egui::Ui, label: &str, primary: bool) -> bool {
    let fill = if primary { C_RED } else { C_SURFACE };
    let text_color = C_WHITE;
    let btn = egui::Button::new(egui::RichText::new(label).color(text_color).size(13.0))
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, if primary { C_RED } else { C_BORDER }))
        .min_size(egui::vec2(90.0, 32.0))
        .rounding(egui::Rounding::same(4.0));
    ui.add(btn).clicked()
}

/// Bullet z ikoną
fn bullet(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(20, 45, 20))
            .rounding(egui::Rounding::same(3.0))
            .inner_margin(egui::Margin { left: 4.0, right: 4.0, top: 1.0, bottom: 1.0 })
            .show(ui, |ui| {
                ui.label(egui::RichText::new("✓").color(C_GREEN).size(10.0).strong());
            });
        ui.add_space(8.0);
        ui.label(egui::RichText::new(text).color(C_MUTED).size(12.5));
    });
    ui.add_space(5.0);
}

/// Karta opcji z checkboxem
fn option_card(ui: &mut egui::Ui, checked: &mut bool, title: &str, desc: &str, tag: &str) {
    let border_color = if *checked { C_RED_DIM } else { C_BORDER };
    let bg = if *checked {
        egui::Color32::from_rgb(35, 18, 18)
    } else {
        C_SURFACE
    };

    egui::Frame::none()
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border_color))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(checked, "");
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(title).color(C_WHITE).size(14.0).strong());
                        ui.add_space(8.0);
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(40, 40, 40))
                            .rounding(egui::Rounding::same(3.0))
                            .inner_margin(egui::Margin { left: 5.0, right: 5.0, top: 1.0, bottom: 1.0 })
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(tag).color(C_MUTED).size(10.0));
                            });
                    });
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(desc).color(C_MUTED).size(12.0));
                });
            });
        });
    ui.add_space(8.0);
}

// ── Ekrany ────────────────────────────────────────────────────────────────────

fn screen_welcome(ui: &mut egui::Ui) {
    ui.add_space(8.0);

    // Duży tytuł
    ui.horizontal(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(30, 10, 10))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("⬡").color(C_RED).size(20.0).strong());
            });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("XCX Ecosystem").color(C_WHITE).size(26.0).strong());
        });
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Professional Installation Wizard").color(C_MUTED).size(14.0));

    ui.add_space(24.0);
    ui.painter().line_segment(
        [ui.min_rect().left_top() + egui::vec2(0.0, ui.min_rect().height()),
         ui.min_rect().right_top() + egui::vec2(0.0, ui.min_rect().height())],
        egui::Stroke::new(1.0, C_BORDER),
    );
    // Zamiast paintować separator ręcznie, użyj separator()
    ui.separator();
    ui.add_space(16.0);

    ui.label(
        egui::RichText::new("This wizard will install the XCX compiler, PAX package manager, and configure your system environment.")
            .color(C_MUTED)
            .size(13.0),
    );

    ui.add_space(24.0);

       // Feature list — 2 pionowe bloki obok siebie, szerokosc przez set_width
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(20, 20, 20))
        .stroke(egui::Stroke::new(1.0, C_BORDER))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            let col_w = (ui.available_width() - 20.0) / 2.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    bullet(ui, "Stack-based VM compiler");
                    bullet(ui, "PAX package manager");
                    bullet(ui, "Math standard library");
                });
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    bullet(ui, "System PATH integration");
                    bullet(ui, ".xcx / .pax file associations");
                    bullet(ui, "Add/Remove Programs entry");
                });
            });
        });

    ui.add_space(20.0);

    // Info box
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(22, 22, 22))
        .stroke(egui::Stroke::new(1.0, C_BORDER))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("ℹ").color(C_MUTED).size(14.0));
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Administrator rights are required to modify system PATH and registry.")
                        .color(C_MUTED)
                        .size(12.0),
                );
            });
        });
}

fn screen_options(ui: &mut egui::Ui, install_pax: &mut bool, associate_files: &mut bool) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(20, 20, 30))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("⚙").color(egui::Color32::from_rgb(120, 140, 200)).size(20.0));
            });
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Choose Components").color(C_WHITE).size(22.0).strong());
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Select which components to install.").color(C_MUTED).size(13.0));
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(16.0);

    option_card(
        ui, install_pax,
        "PAX Package Manager",
        "Package manager and math.xcx standard library. Enables `pax install` command.",
        "Recommended",
    );
    option_card(
        ui, associate_files,
        "File Associations",
        "Register .xcx and .pax extensions with icons in Windows Explorer.",
        "Optional",
    );

    ui.add_space(8.0);
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(20, 20, 20))
        .stroke(egui::Stroke::new(1.0, C_BORDER))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Disk space").color(C_MUTED).size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let size = match (*install_pax, *associate_files) {
                        (true,  true)  => "≈ 28 MB",
                        (true,  false) => "≈ 25 MB",
                        (false, _)     => "≈ 8 MB",
                    };
                    ui.label(egui::RichText::new(size).color(C_WHITE).size(12.0).strong());
                });
            });
        });
}

fn screen_terms(ui: &mut egui::Ui, accepted: &mut bool) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(20, 30, 20))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("📜").color(egui::Color32::from_rgb(100, 200, 120)).size(20.0));
            });
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Terms of Use").color(C_WHITE).size(22.0).strong());
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Please read and accept the license agreement.").color(C_MUTED).size(13.0));
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(16.0);

    egui::Frame::none()
        .fill(egui::Color32::from_rgb(10, 10, 10))
        .show(ui, |ui: &mut egui::Ui| {
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui: &mut egui::Ui| {
                    ui.add_space(10.0);
                    ui.horizontal(|ui: &mut egui::Ui| {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(include_str!("../../XCX_Ecosystem_v2.2.0/LICENSE.txt")).color(C_WHITE).size(11.0).monospace());
                    });
                    ui.add_space(10.0);
                });
        });

    ui.add_space(20.0);
    ui.checkbox(accepted, egui::RichText::new("I accept the terms of the License").color(C_WHITE).size(13.0));
}


fn screen_installing(ui: &mut egui::Ui, screen: &mut Screen, progress_arc: &Arc<Mutex<InstallProgress>>, ctx: &egui::Context) {
    let (done, error, progress, status, log) = {
        let p = progress_arc.lock().unwrap();
        (p.done, p.error.clone(), p.progress, p.status_text.clone(), p.log.clone())
    };

    if done {
        *screen = if let Some(err) = error {
            Screen::Error(err)
        } else {
            Screen::Bonus
        };
    }
    ctx.request_repaint();

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(30, 10, 10))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("↓").color(C_RED).size(20.0).strong());
            });
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Installing").color(C_WHITE).size(22.0).strong());
    });
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Please wait while XCX is being installed.").color(C_MUTED).size(13.0));
    ui.add_space(20.0);
    ui.separator();
    ui.add_space(24.0);

    // Progress bar
    let progress_rect = ui.available_rect_before_wrap();
    let bar_h = 6.0;
    let bar_rect = egui::Rect::from_min_size(
        egui::pos2(progress_rect.left(), progress_rect.top()),
        egui::vec2(progress_rect.width(), bar_h),
    );
    ui.painter().rect_filled(bar_rect, egui::Rounding::same(3.0), C_SURFACE);
    let fill_w = bar_rect.width() * progress;
    ui.painter().rect_filled(
        egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_h)),
        egui::Rounding::same(3.0),
        C_RED,
    );
    ui.add_space(bar_h + 4.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&status).color(C_WHITE).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{:.0}%", progress * 100.0))
                    .color(C_MUTED)
                    .size(12.0),
            );
        });
    });

    ui.add_space(24.0);

    // Log ostatnich kroków
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(12, 12, 12))
        .stroke(egui::Stroke::new(1.0, C_BORDER))
        .rounding(egui::Rounding::same(4.0))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ui.set_min_height(100.0);
            let show: Vec<_> = log.iter().rev().take(6).rev().collect();
            for (i, entry) in show.iter().enumerate() {
                let is_last = i + 1 == show.len();
                let color = if is_last { C_WHITE } else { C_MUTED2 };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(if is_last { "›" } else { " " }).color(C_RED).size(12.0));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(*entry).color(color).size(11.0).monospace());
                });
            }
        });
}

fn screen_bonus(ui: &mut egui::Ui) {
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(20, 45, 25))
            .stroke(egui::Stroke::new(1.0, C_GREEN))
            .rounding(egui::Rounding::same(20.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("✓").color(C_GREEN).size(16.0).strong());
            });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("✦  Installation Complete").color(C_WHITE).size(22.0).strong());
            ui.label(egui::RichText::new("XCX is ready to use.").color(C_MUTED).size(13.0));
        });
    });

    ui.add_space(20.0);
    ui.separator();
    ui.add_space(16.0);

    ui.label(egui::RichText::new("Optional extras").color(C_MUTED).size(12.0));
    ui.add_space(10.0);

    // Karta VSCode
    egui::Frame::none()
        .fill(C_SURFACE)
        .stroke(egui::Stroke::new(1.0, C_BORDER))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("VSCode Extension").color(C_WHITE).size(14.0).strong());
                    ui.label(egui::RichText::new("Syntax highlighting, IntelliSense, and run commands for .xcx files.").color(C_MUTED).size(12.0));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if styled_btn(ui, "Get", false) {
                        open_url("https://github.com/xcx-lang/xcx-vscode");
                    }
                });
            });
        });

    ui.add_space(8.0);

    // Karta Docs
    egui::Frame::none()
        .fill(C_SURFACE)
        .stroke(egui::Stroke::new(1.0, C_BORDER))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Documentation").color(C_WHITE).size(14.0).strong());
                    ui.label(egui::RichText::new("Full XCX language reference, math library API, and tutorials.").color(C_MUTED).size(12.0));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if styled_btn(ui, "Open", false) {
                        open_url("https://xcx-lang.dev/docs");
                    }
                });
            });
        });

    ui.add_space(16.0);
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(12, 12, 12))
        .stroke(egui::Stroke::new(1.0, C_BORDER))
        .rounding(egui::Rounding::same(4.0))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Open a new terminal and type  xcx --version  to verify the installation.").color(C_MUTED).size(12.0).monospace());
        });
}

fn screen_finished(ui: &mut egui::Ui) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("✓").color(C_GREEN).size(48.0).strong());
        ui.add_space(16.0);
        ui.label(egui::RichText::new("XCX is Ready").color(C_WHITE).size(26.0).strong());
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Open a new terminal window and run:").color(C_MUTED).size(13.0));
        ui.add_space(12.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(12, 12, 12))
            .stroke(egui::Stroke::new(1.0, C_BORDER))
            .rounding(egui::Rounding::same(4.0))
            .inner_margin(egui::Margin { left: 20.0, right: 20.0, top: 8.0, bottom: 8.0 })
            .show(ui, |ui| {
                ui.label(egui::RichText::new("xcx --version").color(C_WHITE).size(14.0).monospace());
            });
    });
}

fn screen_error(ui: &mut egui::Ui, err: &str) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(50, 15, 15))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 40, 40)))
            .rounding(egui::Rounding::same(20.0))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("✕").color(egui::Color32::from_rgb(255, 80, 80)).size(16.0).strong());
            });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Installation Failed").color(C_WHITE).size(22.0).strong());
            ui.label(egui::RichText::new("An error occurred during installation.").color(C_MUTED).size(13.0));
        });
    });

    ui.add_space(20.0);
    ui.separator();
    ui.add_space(16.0);

    egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 12, 12))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 30, 30)))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(255, 120, 120)).size(12.0).monospace());
        });

    ui.add_space(16.0);
    ui.label(egui::RichText::new("Check that you have administrator rights and that C:\\XCX is not locked by another process.").color(C_MUTED).size(12.0));
}

// ── App ───────────────────────────────────────────────────────────────────────

impl eframe::App for XcxInstaller {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Styl globalny
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(C_WHITE);
        visuals.panel_fill = C_BG;
        visuals.window_fill = C_BG;
        visuals.widgets.noninteractive.bg_fill = C_SURFACE;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, C_BORDER);
        visuals.widgets.inactive.bg_fill = C_SURFACE;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, C_BORDER);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(35, 35, 35);
        visuals.widgets.active.bg_fill = C_RED_DIM;
        visuals.selection.bg_fill = C_RED_DIM;
        visuals.window_rounding = egui::Rounding::same(0.0);
        ctx.set_visuals(visuals);

        // Sidebar
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(210.0)
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                draw_sidebar(ui, self.screen.step_index());
            });

        // Stopka z przyciskami
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::none()
                    .fill(C_SURFACE)
                    .inner_margin(egui::Margin { left: 0.0, right: 0.0, top: 10.0, bottom: 10.0 }),
            )
            .exact_height(52.0)
            .show(ctx, |ui| {
                draw_footer(
                    ui,
                    &mut self.screen,
                    &mut self.install_started,
                    self.install_pax,
                    self.associate_files,
                    &mut self.terms_accepted,
                    &self.install_dir,
                    &self.install_progress,
                );
            });

        // Główna treść
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(C_BG)
                    .inner_margin(egui::Margin::same(28.0)),
            )
            .show(ctx, |ui| {
                match self.screen.clone() {
                    Screen::Welcome    => screen_welcome(ui),
                    Screen::Terms      => screen_terms(ui, &mut self.terms_accepted),
                    Screen::Options    => screen_options(ui, &mut self.install_pax, &mut self.associate_files),
                    Screen::Installing => screen_installing(ui, &mut self.screen, &self.install_progress, ctx),
                    Screen::Bonus      => screen_bonus(ui),
                    Screen::Finished   => screen_finished(ui),
                    Screen::Error(e)   => screen_error(ui, &e),
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
            .with_inner_size([600.0, 500.0])
            .with_min_inner_size([600.0, 500.0])
            .with_resizable(true)
            .with_title("XCX Ecosystem Installer")
            .with_icon(icon_data.unwrap_or_default()),
        ..Default::default()
    };

    eframe::run_native(
        "XCX Installer",
        native_options,
        Box::new(|_cc| Box::new(XcxInstaller::default())),
    )
    .expect("Failed to run installer");
}