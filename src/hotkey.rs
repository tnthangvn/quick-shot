//! Gán phím tắt toàn cục qua gsettings của GNOME (Settings > Keyboard > Custom Shortcuts).

use std::process::Command;

const SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const KEY: &str = "custom-keybindings";
const BASE: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/";
const NAME: &str = "quickshot";

fn gsettings(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| format!("không chạy được gsettings: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn list_paths() -> Result<Vec<String>, String> {
    let raw = gsettings(&["get", SCHEMA, KEY])?;
    // dạng: @as []  hoặc  ['/org/.../custom0/', '/org/.../custom1/']
    let mut v = Vec::new();
    for part in raw.split('\'') {
        if part.starts_with('/') {
            v.push(part.to_string());
        }
    }
    Ok(v)
}

fn entry_name(path: &str) -> String {
    gsettings(&["get", &format!("{SCHEMA}.custom-keybinding:{path}"), "name"])
        .map(|s| s.trim_matches('\'').to_string())
        .unwrap_or_default()
}

/// Gán `binding` (vd "Print", "<Shift>Print", "<Super>s") chạy lệnh `command`.
pub fn install(binding: &str, command: &str) -> Result<String, String> {
    let mut paths = list_paths()?;
    let path = match paths.iter().find(|p| entry_name(p) == NAME) {
        Some(p) => p.clone(),
        None => {
            let mut i = 0;
            loop {
                let p = format!("{BASE}custom{i}/");
                if !paths.contains(&p) {
                    break p;
                }
                i += 1;
            }
        }
    };
    if !paths.contains(&path) {
        paths.push(path.clone());
        let list = format!(
            "[{}]",
            paths.iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(", ")
        );
        gsettings(&["set", SCHEMA, KEY, &list])?;
    }
    let sub = format!("{SCHEMA}.custom-keybinding:{path}");
    gsettings(&["set", &sub, "name", &format!("'{NAME}'")])?;
    gsettings(&["set", &sub, "command", &format!("'{command}'")])?;
    gsettings(&["set", &sub, "binding", &format!("'{binding}'")])?;

    let mut msg = format!("Đã gán phím [{binding}] → {command}");
    // GNOME dùng sẵn Print/Shift+Print/Alt+Print cho screenshot UI của nó — gỡ ra để không xung đột.
    let lower = binding.to_ascii_lowercase();
    if lower.contains("print") {
        let shell_keys = [
            ("show-screenshot-ui", "print"),
            ("screenshot", "<shift>print"),
            ("screenshot-window", "<alt>print"),
        ];
        for (k, b) in shell_keys {
            if lower == b {
                let _ = gsettings(&["set", "org.gnome.shell.keybindings", k, "[]"]);
                msg.push_str(&format!(
                    "\nĐã tắt phím tắt GNOME '{k}' để nhường [{binding}] cho quickshot.\n\
                     Khôi phục: gsettings reset org.gnome.shell.keybindings {k}"
                ));
            }
        }
    }
    Ok(msg)
}

pub fn remove() -> Result<String, String> {
    let paths = list_paths()?;
    let Some(path) = paths.iter().find(|p| entry_name(p) == NAME).cloned() else {
        return Ok("Chưa có phím tắt quickshot nào.".into());
    };
    let rest: Vec<&String> = paths.iter().filter(|p| **p != path).collect();
    let list = if rest.is_empty() {
        "@as []".to_string()
    } else {
        format!("[{}]", rest.iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(", "))
    };
    gsettings(&["set", SCHEMA, KEY, &list])?;
    let sub = format!("{SCHEMA}.custom-keybinding:{path}");
    let _ = gsettings(&["reset", &sub, "name"]);
    let _ = gsettings(&["reset", &sub, "command"]);
    let _ = gsettings(&["reset", &sub, "binding"]);
    for k in ["show-screenshot-ui", "screenshot", "screenshot-window"] {
        let _ = gsettings(&["reset", "org.gnome.shell.keybindings", k]);
    }
    Ok("Đã gỡ phím tắt quickshot và khôi phục phím Print của GNOME.".into())
}
