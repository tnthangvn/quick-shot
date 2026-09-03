//! quickshot — chụp & chú thích màn hình kiểu Flameshot cho Ubuntu (GNOME/Wayland), hỗ trợ nhiều màn hình.

mod capture;
mod config;
mod hotkey;
mod model;
mod output;
mod overlay;
mod render;
mod settings;

use clap::{Parser, Subcommand};
use gtk4 as gtk;
use gtk4::prelude::*;
use std::path::PathBuf;

/// Tên file .desktop / app id GTK.
pub const DESKTOP_ID: &str = "dev.quickshot.QuickShot";

const LONG_ABOUT: &str = "\
quickshot — chụp màn hình & chú thích kiểu Flameshot cho Ubuntu (GNOME, Wayland/X11).
Chụp và cắt vùng trên NHIỀU màn hình cùng lúc.

Trong giao diện chọn vùng:
  Chuột trái kéo      chọn vùng (kéo qua cả 2 màn hình được)
  Click (không kéo)   chọn cả màn hình dưới con trỏ;  Space cũng vậy
  Ctrl+A              chọn tất cả màn hình
  Kéo tay cầm         đổi kích thước; kéo trong vùng = di chuyển
  Giữ Shift           khoá vuông 1:1 (hoặc góc 45° khi vẽ mũi tên/đường)
  Tab                 đổi tỉ lệ khung: Tự do → 1:1 → 4:3 → 16:9 → 3:2 → 9:16
  Mũi tên             dịch vùng 1px (Ctrl: 10px, Shift: đổi kích thước)
  Enter / Ctrl+C      copy vào clipboard rồi thoát
  Ctrl+S              lưu vào ~/Pictures/Screenshots;  Ctrl+Shift+S  lưu thành...
  Esc / chuột phải    thoát / bỏ vùng chọn

Công cụ vẽ (bấm phím hoặc chọn trên thanh công cụ):
  S chọn vùng   R chữ nhật   E elip   L đường   A mũi tên   P bút   M dạ quang
  T chữ (gõ tiếng Việt được)   N đánh số   B làm mờ/pixel   C lấy màu   F tô đặc
  1-9, 0        chọn màu nhanh;  lăn chuột hoặc [ ]  đổi độ dày nét
  Ctrl+Z / Ctrl+Shift+Z / Delete   hoàn tác / làm lại / xoá nét cuối

Cấu hình: ~/.config/quickshot/config.toml   (tạo mẫu: quickshot config --init)";

#[derive(Parser)]
#[command(
    name = "quickshot",
    version,
    about = "Chụp màn hình & chú thích kiểu Flameshot cho GNOME/Wayland, hỗ trợ nhiều màn hình",
    long_about = LONG_ABOUT,
    help_template = "{name} {version}\n{about-section}\n{usage-heading} {usage}\n\n{all-args}{after-help}",
    after_help = "Chạy không tham số = mở giao diện chọn vùng. Xem thêm: quickshot <lệnh> --help"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// In thông tin gỡ lỗi (màn hình, kích thước ảnh, nguồn chụp)
    #[arg(long, global = true)]
    debug: bool,

    /// Chờ N giây trước khi chụp (để mở menu, tooltip...)
    #[arg(short = 'd', long, global = true, value_name = "GIÂY", default_value_t = 0.0)]
    delay: f64,
}

#[derive(Subcommand)]
enum Cmd {
    /// Mở giao diện chọn vùng + chú thích (mặc định)
    Gui {
        /// Tỉ lệ khung ban đầu: free, 1:1, 4:3, 16:9, 3:2, 9:16
        #[arg(long, value_name = "TỈ_LỆ")]
        ratio: Option<String>,
        /// Màu vẽ ban đầu, dạng #RRGGBB
        #[arg(long, value_name = "MÀU")]
        color: Option<String>,
        /// Thư mục lưu ảnh (ghi đè cấu hình)
        #[arg(long, value_name = "THƯ_MỤC")]
        dir: Option<PathBuf>,
    },
    /// Chụp ngay không cần giao diện: toàn bộ các màn hình (hoặc một màn hình)
    Full {
        /// Chỉ chụp màn hình số N (xem `quickshot screens`)
        #[arg(short, long, value_name = "N")]
        screen: Option<usize>,
        /// Lưu vào file này (mặc định: thư mục ảnh + tên theo thời gian). Dùng "-" để in PNG ra stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Copy vào clipboard (thay vì / kèm theo lưu file)
        #[arg(short, long)]
        clipboard: bool,
        /// Vùng cắt theo pixel ảnh: X,Y,W,H
        #[arg(short, long, value_name = "X,Y,W,H")]
        region: Option<String>,
    },
    /// Liệt kê các màn hình và vị trí của chúng
    Screens,
    /// Gán / gỡ phím tắt toàn cục trong GNOME
    Hotkey {
        /// Tổ hợp phím, vd: Print, "<Shift>Print", "<Super>s", "<Ctrl><Alt>a"
        #[arg(short, long, default_value = "Print", value_name = "PHÍM")]
        key: String,
        /// Lệnh chạy khi bấm phím (mặc định: đường dẫn của quickshot hiện tại)
        #[arg(short, long, value_name = "LỆNH")]
        command: Option<String>,
        /// Gỡ phím tắt đã gán và trả lại phím Print cho GNOME
        #[arg(long)]
        remove: bool,
    },
    /// Xem đường dẫn file cấu hình / tạo file cấu hình mẫu
    Config {
        /// Tạo file cấu hình mẫu nếu chưa có
        #[arg(long)]
        init: bool,
    },
    /// Mở cửa sổ cài đặt (chỉnh phím tắt, thư mục lưu, màu... bằng giao diện)
    Settings,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.cmd.unwrap_or(Cmd::Gui { ratio: None, color: None, dir: None }) {
        Cmd::Gui { ratio, color, dir } => run_gui(cli.debug, cli.delay, ratio, color, dir),
        Cmd::Full { screen, output, clipboard, region } => {
            run_full(cli.debug, cli.delay, screen, output, clipboard, region)
        }
        Cmd::Screens => run_screens(cli.debug),
        Cmd::Hotkey { key, command, remove } => run_hotkey(key, command, remove),
        Cmd::Config { init } => run_config(init),
        Cmd::Settings => settings::run(),
    };
    std::process::exit(code);
}

fn wait_delay(delay: f64) {
    if delay > 0.0 {
        std::thread::sleep(std::time::Duration::from_secs_f64(delay));
    }
}

fn run_gui(debug: bool, delay: f64, ratio: Option<String>, color: Option<String>, dir: Option<PathBuf>) -> i32 {
    wait_delay(delay);
    let mut cfg = config::Config::load();
    if let Some(r) = ratio {
        if model::Ratio::parse(&r).is_none() {
            eprintln!("quickshot: tỉ lệ không hợp lệ: {r} (dùng free, 1:1, 4:3, 16:9, 3:2, 9:16)");
            return 2;
        }
        cfg.ratio = r;
    }
    if let Some(c) = color {
        if model::Color::from_hex(&c).is_none() {
            eprintln!("quickshot: màu không hợp lệ: {c} (dạng #RRGGBB)");
            return 2;
        }
        cfg.color = c;
    }
    if let Some(d) = dir {
        cfg.save_dir = Some(d.to_string_lossy().to_string());
    }

    // Chụp TRƯỚC khi mở cửa sổ để không dính giao diện của chính mình.
    let cap = match capture::capture_all(debug) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("quickshot: {e}");
            return 1;
        }
    };
    if debug {
        eprintln!("[capture] nguồn: {}", cap.source);
    }
    let ov = overlay::Overlay::new(cap.image, cfg, debug);
    overlay::run(ov)
}

/// Lấy danh sách màn hình (toạ độ logic) qua GDK mà không mở cửa sổ.
fn gdk_monitors() -> Vec<(String, model::Rect, i32)> {
    if gtk::init().is_err() {
        return Vec::new();
    }
    let Some(display) = gtk::gdk::Display::default() else { return Vec::new() };
    let list = display.monitors();
    let mut v = Vec::new();
    for i in 0..list.n_items() {
        if let Some(m) = list.item(i).and_downcast::<gtk::gdk::Monitor>() {
            let g = m.geometry();
            let name = m
                .connector()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("monitor{i}"));
            v.push((
                name,
                model::Rect::new(g.x() as f64, g.y() as f64, g.width() as f64, g.height() as f64),
                m.scale_factor(),
            ));
        }
    }
    v
}

fn run_screens(debug: bool) -> i32 {
    let mons = gdk_monitors();
    if mons.is_empty() {
        eprintln!("quickshot: không thấy màn hình nào (cần chạy trong phiên đồ hoạ)");
        return 1;
    }
    println!("{:<3} {:<12} {:>6} {:>6} {:>7} {:>7} {:>6}", "#", "Tên", "X", "Y", "Rộng", "Cao", "Scale");
    for (i, (name, r, s)) in mons.iter().enumerate() {
        println!("{:<3} {:<12} {:>6} {:>6} {:>7} {:>7} {:>6}", i, name, r.x, r.y, r.w, r.h, s);
    }
    if debug {
        if let Ok(c) = capture::capture_all(true) {
            println!("Ảnh chụp toàn desktop: {}x{} (nguồn {})", c.image.width(), c.image.height(), c.source);
        }
    }
    0
}

fn run_full(
    debug: bool,
    delay: f64,
    screen: Option<usize>,
    output: Option<PathBuf>,
    clipboard: bool,
    region: Option<String>,
) -> i32 {
    wait_delay(delay);
    let cfg = config::Config::load();
    let cap = match capture::capture_all(debug) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("quickshot: {e}");
            return 1;
        }
    };
    let mut img = cap.image;
    let bounds = model::Rect::new(0.0, 0.0, img.width() as f64, img.height() as f64);

    let mut crop: Option<model::Rect> = None;
    if let Some(n) = screen {
        let mons = gdk_monitors();
        if mons.is_empty() {
            eprintln!("quickshot: không lấy được danh sách màn hình");
            return 1;
        }
        let Some((_, r, _)) = mons.get(n) else {
            eprintln!("quickshot: không có màn hình số {n} (có {} màn hình)", mons.len());
            return 2;
        };
        let minx = mons.iter().map(|m| m.1.x).fold(f64::MAX, f64::min);
        let miny = mons.iter().map(|m| m.1.y).fold(f64::MAX, f64::min);
        let maxx = mons.iter().map(|m| m.1.right()).fold(f64::MIN, f64::max);
        let maxy = mons.iter().map(|m| m.1.bottom()).fold(f64::MIN, f64::max);
        let rx = bounds.w / (maxx - minx);
        let ry = bounds.h / (maxy - miny);
        crop = Some(model::Rect::new((r.x - minx) * rx, (r.y - miny) * ry, r.w * rx, r.h * ry));
    }
    if let Some(s) = region {
        let parts: Vec<f64> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if parts.len() != 4 {
            eprintln!("quickshot: --region cần dạng X,Y,W,H");
            return 2;
        }
        crop = Some(model::Rect::new(parts[0], parts[1], parts[2], parts[3]));
    }
    if let Some(r) = crop {
        let Some(r) = r.rounded().intersect(&bounds) else {
            eprintln!("quickshot: vùng cắt nằm ngoài ảnh");
            return 2;
        };
        let r = r.rounded();
        img = image::imageops::crop_imm(&img, r.x as u32, r.y as u32, r.w as u32, r.h as u32).to_image();
    }

    let to_stdout = output.as_ref().map(|p| p.as_os_str() == "-").unwrap_or(false);
    let mut code = 0;
    if to_stdout {
        match output::encode_png(&img) {
            Ok(png) => {
                use std::io::Write;
                let _ = std::io::stdout().write_all(&png);
            }
            Err(e) => {
                eprintln!("quickshot: {e}");
                code = 1;
            }
        }
    } else if !clipboard || output.is_some() {
        let path = output.unwrap_or_else(|| output::default_save_path(&cfg));
        match output::save_png(&img, &path) {
            Ok(()) => {
                println!("{}", path.display());
                if cfg.notify {
                    output::notify("Đã lưu ảnh", &path.display().to_string(), Some(&path));
                }
            }
            Err(e) => {
                eprintln!("quickshot: {e}");
                code = 1;
            }
        }
    }
    if clipboard {
        match output::encode_png(&img) {
            Ok(png) => match output::copy_png_external(&png) {
                output::ClipResult::Done => {
                    if cfg.notify {
                        output::notify("Đã copy ảnh vào clipboard", &format!("{}×{}", img.width(), img.height()), None);
                    }
                }
                output::ClipResult::NeedGtk => {
                    eprintln!("quickshot: cần 'wl-copy' (gói wl-clipboard) hoặc 'xclip' để copy ở chế độ dòng lệnh");
                    code = 1;
                }
            },
            Err(e) => {
                eprintln!("quickshot: {e}");
                code = 1;
            }
        }
    }
    code
}

fn run_hotkey(key: String, command: Option<String>, remove: bool) -> i32 {
    let res = if remove {
        hotkey::remove()
    } else {
        let cmd = command.unwrap_or_else(|| hotkey::default_command(DESKTOP_ID));
        hotkey::install(&key, &cmd)
    };
    match res {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(e) => {
            eprintln!("quickshot: {e}");
            1
        }
    }
}

fn run_config(init: bool) -> i32 {
    if init {
        match config::Config::write_default() {
            Ok(p) => println!("File cấu hình: {}", p.display()),
            Err(e) => {
                eprintln!("quickshot: {e}");
                return 1;
            }
        }
    } else {
        match config::Config::path() {
            Some(p) => println!(
                "File cấu hình: {} ({})",
                p.display(),
                if p.exists() { "đã có" } else { "chưa có — tạo bằng: quickshot config --init" }
            ),
            None => eprintln!("quickshot: không xác định được thư mục cấu hình"),
        }
        let cfg = config::Config::load();
        println!("Thư mục lưu ảnh: {}", cfg.save_dir().display());
        println!("{}", toml::to_string_pretty(&cfg).unwrap_or_default());
    }
    0
}
