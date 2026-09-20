//! Tạm ẩn các app "luôn nổi trên cùng" trong lúc chụp và trong lúc mở lớp chọn vùng.
//!
//! Cửa sổ luôn nổi (ví dụ quick-note) nằm đè lên lớp phủ toàn màn hình của
//! quickshot: nó nuốt chuột/bàn phím ở vùng đó, và còn lọt vào ảnh chụp.
//! Các app này quy ước: `SIGUSR1` = ẩn, `SIGUSR2` = hiện lại.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Tên tiến trình (khớp chính xác) sẽ được tạm ẩn.
const APPS: &[&str] = &["quick-note"];

/// Chờ compositor vẽ lại sau khi ẩn, trước khi chụp.
const REPAINT_DELAY: Duration = Duration::from_millis(180);

/// Nhắc lại `SIGUSR1` đều đặn: app bị ẩn sẽ tự hiện lại nếu ta chết giữa chừng.
const HEARTBEAT: Duration = Duration::from_secs(2);

/// Ẩn khi tạo, tự hiện lại khi biến này ra khỏi phạm vi.
pub struct Hidden {
    apps: Vec<&'static str>,
    debug: bool,
    stop: Arc<AtomicBool>,
    beat: Option<JoinHandle<()>>,
}

impl Hidden {
    pub fn hide(debug: bool) -> Self {
        let apps: Vec<&'static str> = APPS.iter().copied().filter(|a| signal(a, "USR1")).collect();
        let stop = Arc::new(AtomicBool::new(false));
        let mut beat = None;
        if !apps.is_empty() {
            if debug {
                eprintln!("[inhibit] đã ẩn: {}", apps.join(", "));
            }
            thread::sleep(REPAINT_DELAY);
            let (apps, stop) = (apps.clone(), stop.clone());
            beat = thread::Builder::new()
                .name("inhibit-heartbeat".into())
                .spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        thread::sleep(HEARTBEAT);
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        for app in &apps {
                            signal(app, "USR1");
                        }
                    }
                })
                .ok();
        }
        Self {
            apps,
            debug,
            stop,
            beat,
        }
    }
}

impl Drop for Hidden {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(beat) = self.beat.take() {
            // Nhịp tim đang ngủ: không chờ nó, chỉ cần cờ dừng đã bật.
            drop(beat);
        }
        for app in &self.apps {
            signal(app, "USR2");
            if self.debug {
                eprintln!("[inhibit] hiện lại: {app}");
            }
        }
    }
}

/// `true` nếu có tiến trình nhận tín hiệu.
fn signal(app: &str, sig: &str) -> bool {
    Command::new("pkill")
        .args([&format!("-{sig}"), "-x", app])
        .status()
        .is_ok_and(|s| s.success())
}
