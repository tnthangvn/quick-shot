//! Chụp toàn bộ desktop (mọi màn hình) thành một ảnh RGBA.
//!
//! Thứ tự thử:
//! 1. xdg-desktop-portal (org.freedesktop.portal.Screenshot) — cách chuẩn trên Wayland/GNOME,
//!    trả về ảnh ghép của tất cả màn hình.
//! 2. Các lệnh ngoài nếu có: gnome-screenshot, grim, import (ImageMagick), scrot — dùng cho X11
//!    hoặc máy không có portal.

use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Captured {
    pub image: RgbaImage,
    pub source: &'static str,
}

pub fn capture_all(debug: bool) -> Result<Captured, String> {
    let mut errors = Vec::new();

    // Dùng để thử nghiệm: QUICKSHOT_FAKE_IMAGE=/path/anh.png thay cho chụp thật
    if let Some(p) = std::env::var_os("QUICKSHOT_FAKE_IMAGE") {
        let img = load_png(Path::new(&p))?;
        return Ok(Captured { image: img, source: "fake" });
    }

    match capture_portal() {
        Ok(img) => {
            if debug {
                eprintln!("[capture] portal OK: {}x{}", img.width(), img.height());
            }
            return Ok(Captured { image: img, source: "portal" });
        }
        Err(e) => {
            if debug {
                eprintln!("[capture] portal lỗi: {e}");
            }
            errors.push(format!("portal: {e}"));
        }
    }

    for (name, args) in external_commands() {
        if which(name).is_none() {
            continue;
        }
        match capture_command(name, &args) {
            Ok(img) => {
                if debug {
                    eprintln!("[capture] {name} OK: {}x{}", img.width(), img.height());
                }
                return Ok(Captured { image: img, source: name });
            }
            Err(e) => {
                if debug {
                    eprintln!("[capture] {name} lỗi: {e}");
                }
                errors.push(format!("{name}: {e}"));
            }
        }
    }

    Err(format!(
        "Không chụp được màn hình. Chi tiết:\n  {}\n\
         Gợi ý: trên GNOME/Wayland cần gói xdg-desktop-portal-gnome; nếu đã từng từ chối quyền,\n\
         chạy: flatpak permission-reset screenshot  (hoặc xoá mục 'screenshot' trong Settings > Apps).",
        errors.join("\n  ")
    ))
}

fn capture_portal() -> Result<RgbaImage, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let uri = rt.block_on(async {
        use ashpd::desktop::screenshot::Screenshot;
        let resp = Screenshot::request()
            .interactive(false)
            .modal(false)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .response()
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(resp.uri().clone())
    })?;
    let path = uri
        .to_file_path()
        .map_err(|_| format!("URI không phải file: {uri}"))?;
    let img = load_png(&path)?;
    // Portal tạo file tạm cho riêng ta; dọn đi sau khi đọc.
    let _ = std::fs::remove_file(&path);
    Ok(img)
}

fn external_commands() -> Vec<(&'static str, Vec<String>)> {
    let out = tmp_path();
    let o = out.to_string_lossy().to_string();
    vec![
        ("gnome-screenshot", vec!["-f".into(), o.clone()]),
        ("grim", vec![o.clone()]),
        ("import", vec!["-window".into(), "root".into(), o.clone()]),
        ("scrot", vec!["-o".into(), o.clone()]),
        ("spectacle", vec!["-b".into(), "-n".into(), "-f".into(), "-o".into(), o.clone()]),
    ]
}

fn capture_command(cmd: &str, args: &[String]) -> Result<RgbaImage, String> {
    let out = args.last().cloned().unwrap_or_default();
    let status = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("thoát với mã {status}"));
    }
    let img = load_png(Path::new(&out))?;
    let _ = std::fs::remove_file(&out);
    Ok(img)
}

fn tmp_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join(format!("quickshot-{}.png", std::process::id()))
}

fn load_png(path: &Path) -> Result<RgbaImage, String> {
    let img = image::open(path).map_err(|e| format!("đọc {}: {e}", path.display()))?;
    Ok(img.into_rgba8())
}

pub fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}
