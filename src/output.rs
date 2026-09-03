//! Lưu file, copy clipboard, thông báo.

use crate::capture::which;
use image::RgbaImage;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

pub fn save_png(img: &RgbaImage, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("tạo thư mục {}: {e}", parent.display()))?;
    }
    img.save(path).map_err(|e| format!("ghi {}: {e}", path.display()))
}

pub fn default_save_path(cfg: &crate::config::Config) -> PathBuf {
    let name = chrono::Local::now().format(&cfg.filename).to_string();
    cfg.save_dir().join(name)
}

/// Kết quả copy: đã xong bằng công cụ ngoài, hay cần GTK giữ clipboard.
pub enum ClipResult {
    Done,
    NeedGtk,
}

/// Copy PNG vào clipboard bằng wl-copy (Wayland) hoặc xclip/xsel (X11).
/// Các công cụ này tự fork để giữ nội dung sau khi ta thoát.
pub fn copy_png_external(png: &[u8]) -> ClipResult {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let mut candidates: Vec<(&str, Vec<&str>)> = Vec::new();
    if wayland {
        candidates.push(("wl-copy", vec!["--type", "image/png"]));
    }
    candidates.push(("xclip", vec!["-selection", "clipboard", "-t", "image/png", "-i"]));
    if wayland {
        // vẫn thử nếu WAYLAND_DISPLAY có nhưng wl-copy không có
    } else {
        candidates.insert(0, ("wl-copy", vec!["--type", "image/png"]));
    }
    for (cmd, args) in candidates {
        if which(cmd).is_none() {
            continue;
        }
        let child = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { continue };
        if let Some(mut stdin) = child.stdin.take() {
            if stdin.write_all(png).is_err() {
                continue;
            }
        }
        match child.wait() {
            Ok(s) if s.success() => {
                return ClipResult::Done
            }
            _ => continue,
        }
    }
    ClipResult::NeedGtk
}

/// Copy chuỗi văn bản bằng wl-copy / xclip. Trả về false nếu không có công cụ.
pub fn copy_text_external(text: &str) -> bool {
    let candidates: [(&str, Vec<&str>); 2] = [
        ("wl-copy", vec![]),
        ("xclip", vec!["-selection", "clipboard", "-i"]),
    ];
    for (cmd, args) in candidates {
        if which(cmd).is_none() {
            continue;
        }
        let child = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { continue };
        if let Some(mut stdin) = child.stdin.take() {
            if stdin.write_all(text.as_bytes()).is_err() {
                continue;
            }
        }
        if matches!(child.wait(), Ok(s) if s.success()) {
            return true;
        }
    }
    false
}

pub fn notify(title: &str, body: &str, icon_path: Option<&Path>) {
    if which("notify-send").is_none() {
        return;
    }
    let mut c = Command::new("notify-send");
    c.arg("-a").arg("quickshot");
    if let Some(p) = icon_path {
        c.arg("-i").arg(p);
    }
    c.arg(title).arg(body);
    let _ = c.stdout(Stdio::null()).stderr(Stdio::null()).spawn();
}
