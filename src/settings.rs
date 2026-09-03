//! Cửa sổ cài đặt: chỉnh config.toml và phím tắt toàn cục bằng giao diện GTK4.

use crate::DESKTOP_ID;
use crate::config::Config;
use crate::hotkey;
use crate::model;
use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Button, CheckButton, DropDown, Entry, EventControllerKey,
    Grid, Label, Orientation, SpinButton,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Phím bấm một mình chỉ là bổ trợ (Shift/Ctrl/Alt/Super…) — bỏ qua khi bắt phím.
fn is_modifier_key(k: gdk::Key) -> bool {
    matches!(
        k,
        gdk::Key::Shift_L | gdk::Key::Shift_R | gdk::Key::Control_L | gdk::Key::Control_R
            | gdk::Key::Alt_L | gdk::Key::Alt_R | gdk::Key::Meta_L | gdk::Key::Meta_R
            | gdk::Key::Super_L | gdk::Key::Super_R | gdk::Key::Hyper_L | gdk::Key::Hyper_R
            | gdk::Key::ISO_Level3_Shift | gdk::Key::Caps_Lock | gdk::Key::Num_Lock
    )
}

const RATIOS: [&str; 6] = ["free", "1:1", "4:3", "16:9", "3:2", "9:16"];

pub fn run() -> i32 {
    let app = Application::builder()
        .application_id(DESKTOP_ID)
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(build_ui);
    let code = app.run_with_args(&["quickshot"]);
    if code == gtk::glib::ExitCode::SUCCESS { 0 } else { 1 }
}

fn build_ui(app: &Application) {
    let cfg = Config::load();

    let win = ApplicationWindow::builder()
        .application(app)
        .title("QuickShot — Cài đặt")
        .default_width(480)
        .resizable(false)
        .build();

    let grid = Grid::builder()
        .row_spacing(8)
        .column_spacing(10)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let mut row = 0i32;
    let add_label = |grid: &Grid, text: &str, row: i32| {
        let l = Label::builder().label(text).halign(gtk::Align::Start).build();
        grid.attach(&l, 0, row, 1, 1);
    };

    // --- Phím tắt toàn cục (bắt phím thật, không gõ text) ---
    add_label(&grid, "Phím tắt mở QuickShot", row);
    let hotkey_val = Rc::new(RefCell::new(hotkey::current_binding().unwrap_or_default()));
    let capturing = Rc::new(RefCell::new(false));
    let btn_label = |v: &str| {
        if v.is_empty() { "(chưa gán) — bấm để đặt".to_string() } else { v.to_string() }
    };
    let hotkey_btn = Button::builder()
        .label(btn_label(&hotkey_val.borrow()))
        .hexpand(true)
        .tooltip_text("Bấm rồi nhấn tổ hợp phím muốn dùng (Esc huỷ)")
        .build();
    grid.attach(&hotkey_btn, 1, row, 1, 1);
    let hotkey_remove = Button::with_label("Gỡ");
    grid.attach(&hotkey_remove, 2, row, 1, 1);
    row += 1;

    // Click nút = vào chế độ bắt phím.
    {
        let capturing = capturing.clone();
        let hotkey_btn2 = hotkey_btn.clone();
        hotkey_btn.connect_clicked(move |_| {
            *capturing.borrow_mut() = true;
            hotkey_btn2.set_label("Nhấn tổ hợp phím…  (Esc huỷ)");
        });
    }
    // Controller bắt phím ở pha Capture để không lọt vào các ô Entry khác.
    {
        let capturing = capturing.clone();
        let hotkey_val = hotkey_val.clone();
        let hotkey_btn = hotkey_btn.clone();
        let keys = EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, keyval, _keycode, state| {
            if !*capturing.borrow() {
                return gtk::glib::Propagation::Proceed;
            }
            if keyval == gdk::Key::Escape {
                *capturing.borrow_mut() = false;
                hotkey_btn.set_label(&btn_label(&hotkey_val.borrow()));
                return gtk::glib::Propagation::Stop;
            }
            if is_modifier_key(keyval) {
                return gtk::glib::Propagation::Stop; // đợi phím chính
            }
            let mods = state
                & (gdk::ModifierType::CONTROL_MASK
                    | gdk::ModifierType::SHIFT_MASK
                    | gdk::ModifierType::ALT_MASK
                    | gdk::ModifierType::SUPER_MASK
                    | gdk::ModifierType::META_MASK);
            let accel = gtk::accelerator_name(keyval, mods).to_string();
            *hotkey_val.borrow_mut() = accel.clone();
            *capturing.borrow_mut() = false;
            hotkey_btn.set_label(&btn_label(&accel));
            gtk::glib::Propagation::Stop
        });
        win.add_controller(keys);
    }

    // --- Thư mục lưu ---
    add_label(&grid, "Thư mục lưu ảnh", row);
    let dir_entry = Entry::builder()
        .text(cfg.save_dir.clone().unwrap_or_default())
        .placeholder_text(cfg.save_dir().to_string_lossy().to_string())
        .hexpand(true)
        .build();
    grid.attach(&dir_entry, 1, row, 1, 1);
    let dir_browse = Button::with_label("Chọn…");
    grid.attach(&dir_browse, 2, row, 1, 1);
    row += 1;

    // --- Mẫu tên file ---
    add_label(&grid, "Mẫu tên file", row);
    let filename_entry = Entry::builder()
        .text(&cfg.filename)
        .tooltip_text("Cú pháp strftime, vd Screenshot_%Y-%m-%d_%H-%M-%S.png")
        .hexpand(true)
        .build();
    grid.attach(&filename_entry, 1, row, 2, 1);
    row += 1;

    // --- Màu vẽ ---
    add_label(&grid, "Màu vẽ mặc định", row);
    let color_entry = Entry::builder().text(&cfg.color).tooltip_text("Dạng #RRGGBB").build();
    grid.attach(&color_entry, 1, row, 2, 1);
    row += 1;

    // --- Độ dày nét ---
    add_label(&grid, "Độ dày nét", row);
    let thickness = SpinButton::with_range(1.0, 40.0, 1.0);
    thickness.set_value(cfg.thickness);
    grid.attach(&thickness, 1, row, 2, 1);
    row += 1;

    // --- Độ tối lớp phủ ---
    add_label(&grid, "Độ tối lớp phủ (0–1)", row);
    let dim = SpinButton::with_range(0.0, 1.0, 0.05);
    dim.set_digits(2);
    dim.set_value(cfg.dim);
    grid.attach(&dim, 1, row, 2, 1);
    row += 1;

    // --- Tỉ lệ khung mặc định ---
    add_label(&grid, "Tỉ lệ khung mặc định", row);
    let ratio = DropDown::from_strings(&RATIOS);
    if let Some(i) = RATIOS.iter().position(|r| *r == cfg.ratio) {
        ratio.set_selected(i as u32);
    }
    grid.attach(&ratio, 1, row, 2, 1);
    row += 1;

    // --- Các tuỳ chọn bật/tắt ---
    let enter_copies = CheckButton::with_label("Enter = copy clipboard rồi thoát (tắt: Enter lưu file)");
    enter_copies.set_active(cfg.enter_copies);
    grid.attach(&enter_copies, 0, row, 3, 1);
    row += 1;
    let copy_on_save = CheckButton::with_label("Khi Lưu thì copy luôn vào clipboard");
    copy_on_save.set_active(cfg.copy_on_save);
    grid.attach(&copy_on_save, 0, row, 3, 1);
    row += 1;
    let notify = CheckButton::with_label("Hiện thông báo sau khi lưu / copy");
    notify.set_active(cfg.notify);
    grid.attach(&notify, 0, row, 3, 1);
    row += 1;

    // --- Trạng thái + nút ---
    let status = Label::builder().label("").halign(gtk::Align::Start).wrap(true).build();
    grid.attach(&status, 0, row, 3, 1);
    row += 1;

    let buttons = gtk::Box::builder().orientation(Orientation::Horizontal).spacing(8).halign(gtk::Align::End).build();
    let btn_close = Button::with_label("Đóng");
    let btn_save = Button::with_label("Lưu");
    btn_save.add_css_class("suggested-action");
    buttons.append(&btn_close);
    buttons.append(&btn_save);
    grid.attach(&buttons, 0, row, 3, 1);

    win.set_child(Some(&grid));

    // Chọn thư mục qua hộp thoại.
    {
        let dir_entry = dir_entry.clone();
        let win = win.clone();
        dir_browse.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder().title("Chọn thư mục lưu ảnh").build();
            let dir_entry = dir_entry.clone();
            dialog.select_folder(Some(&win), gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(p) = file.path() {
                        dir_entry.set_text(&p.to_string_lossy());
                    }
                }
            });
        });
    }

    // Gỡ phím tắt.
    {
        let hotkey_val = hotkey_val.clone();
        let hotkey_btn = hotkey_btn.clone();
        let status = status.clone();
        hotkey_remove.connect_clicked(move |_| match hotkey::remove() {
            Ok(msg) => {
                hotkey_val.borrow_mut().clear();
                hotkey_btn.set_label("(chưa gán) — bấm để đặt");
                status.set_text(&msg);
            }
            Err(e) => status.set_text(&format!("Lỗi gỡ phím tắt: {e}")),
        });
    }

    // Đóng.
    {
        let win = win.clone();
        btn_close.connect_clicked(move |_| win.close());
    }

    // Lưu cấu hình + áp phím tắt.
    btn_save.connect_clicked(move |_| {
        // Xác thực màu.
        let color = color_entry.text().to_string();
        if model::Color::from_hex(&color).is_none() {
            status.set_text(&format!("Màu không hợp lệ: {color} (cần dạng #RRGGBB)"));
            return;
        }
        let filename = filename_entry.text().to_string();
        if filename.trim().is_empty() {
            status.set_text("Mẫu tên file không được để trống.");
            return;
        }
        let dir_text = dir_entry.text().to_string();
        let ratio_str = RATIOS
            .get(ratio.selected() as usize)
            .copied()
            .unwrap_or("free")
            .to_string();

        let cfg = Config {
            save_dir: if dir_text.trim().is_empty() { None } else { Some(dir_text) },
            filename,
            color,
            thickness: thickness.value(),
            copy_on_save: copy_on_save.is_active(),
            notify: notify.is_active(),
            dim: dim.value(),
            ratio: ratio_str,
            enter_copies: enter_copies.is_active(),
        };

        let mut msg = match cfg.save() {
            Ok(p) => format!("Đã lưu cấu hình: {}", p.display()),
            Err(e) => {
                status.set_text(&format!("Lỗi lưu cấu hình: {e}"));
                return;
            }
        };

        // Áp phím tắt (chưa gán = không đụng phím tắt).
        let key = hotkey_val.borrow().clone();
        if !key.trim().is_empty() {
            match hotkey::install(&key, &hotkey::default_command(DESKTOP_ID)) {
                Ok(m) => {
                    msg.push('\n');
                    msg.push_str(&m.lines().next().unwrap_or(&m));
                }
                Err(e) => msg.push_str(&format!("\nLỗi gán phím tắt: {e}")),
            }
        }
        status.set_text(&msg);
    });

    win.present();
}
