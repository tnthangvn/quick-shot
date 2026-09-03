//! Cấu hình đọc từ ~/.config/quickshot/config.toml (mọi trường đều tuỳ chọn).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Thư mục lưu ảnh. Mặc định: ~/Pictures/Screenshots
    pub save_dir: Option<String>,
    /// Mẫu tên file (theo cú pháp strftime của chrono). Mặc định: Screenshot_%Y-%m-%d_%H-%M-%S.png
    pub filename: String,
    /// Màu vẽ mặc định, dạng #RRGGBB
    pub color: String,
    /// Độ dày nét mặc định (1..40)
    pub thickness: f64,
    /// Sau khi bấm Lưu có đồng thời copy vào clipboard không
    pub copy_on_save: bool,
    /// Hiện thông báo (notify-send) sau khi lưu / copy
    pub notify: bool,
    /// Độ mờ của lớp phủ ngoài vùng chọn (0..1)
    pub dim: f64,
    /// Tỉ lệ khung mặc định: free, 1:1, 4:3, 16:9, 3:2, 9:16
    pub ratio: String,
    /// Tự động copy vào clipboard khi bấm Enter (true) hay chỉ khi bấm Ctrl+C (false)
    pub enter_copies: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            save_dir: None,
            filename: "Screenshot_%Y-%m-%d_%H-%M-%S.png".into(),
            color: "#FF0000".into(),
            thickness: 4.0,
            copy_on_save: true,
            notify: true,
            dim: 0.45,
            ratio: "free".into(),
            enter_copies: true,
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("quickshot").join("config.toml"))
    }

    pub fn load() -> Config {
        let Some(p) = Self::path() else {
            return Config::default();
        };
        match std::fs::read_to_string(&p) {
            Ok(s) => match toml::from_str::<Config>(&s) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("quickshot: lỗi trong {}: {e} — dùng cấu hình mặc định", p.display());
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    pub fn save_dir(&self) -> PathBuf {
        if let Some(d) = &self.save_dir {
            return expand_tilde(d);
        }
        if let Ok(d) = std::env::var("QUICKSHOT_DIR") {
            return expand_tilde(&d);
        }
        dirs::picture_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("Screenshots")
    }

    /// Ghi file cấu hình mẫu (đầy đủ chú thích) nếu chưa có.
    pub fn write_default() -> Result<PathBuf, String> {
        let p = Self::path().ok_or("không xác định được thư mục cấu hình")?;
        if p.exists() {
            return Ok(p);
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = r##"# Cấu hình quickshot — mọi dòng đều tuỳ chọn, bỏ dấu # để bật.

# Thư mục lưu ảnh (mặc định ~/Pictures/Screenshots)
# save_dir = "~/Pictures/Screenshots"

# Mẫu tên file, theo cú pháp strftime
# filename = "Screenshot_%Y-%m-%d_%H-%M-%S.png"

# Màu vẽ mặc định
# color = "#FF0000"

# Độ dày nét mặc định
# thickness = 4

# Bấm Lưu thì cũng copy vào clipboard
# copy_on_save = true

# Hiện thông báo sau khi lưu / copy
# notify = true

# Độ tối lớp phủ ngoài vùng chọn (0..1)
# dim = 0.45

# Tỉ lệ khung mặc định: free, 1:1, 4:3, 16:9, 3:2, 9:16
# ratio = "free"

# Enter = copy vào clipboard rồi thoát (false: Enter chỉ lưu file)
# enter_copies = true
"##;
        std::fs::write(&p, text).map_err(|e| e.to_string())?;
        Ok(p)
    }
}

pub fn expand_tilde(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(h) = dirs::home_dir() {
            return h.join(rest);
        }
    }
    PathBuf::from(s)
}
