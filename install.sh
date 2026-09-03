#!/usr/bin/env bash
# Cài quickshot cho người dùng hiện tại (không cần sudo, trừ bước apt).
set -e
cd "$(dirname "$0")"

BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
ID="dev.quickshot.QuickShot"

echo "== 1/4 Cài gói phụ thuộc (cần sudo) =="
if command -v apt-get >/dev/null; then
  sudo apt-get install -y libgtk-4-1 wl-clipboard xdg-desktop-portal xdg-desktop-portal-gnome libnotify-bin || true
fi

echo "== 2/4 Chuẩn bị file thực thi =="
if [ "$1" = "--build" ] || [ ! -x bin/quickshot ]; then
  if ! command -v cargo >/dev/null; then
    echo "Chưa có cargo (Rust). Cài: sudo apt install cargo libgtk-4-dev  hoặc  curl https://sh.rustup.rs -sSf | sh"
    exit 1
  fi
  sudo apt-get install -y libgtk-4-dev build-essential pkg-config || true
  cargo build --release
  mkdir -p bin && cp target/release/quickshot bin/quickshot
fi

echo "== 3/4 Copy file =="
mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
install -m 755 bin/quickshot "$BIN_DIR/quickshot"
sed "s|^Exec=quickshot gui|Exec=$BIN_DIR/quickshot gui|" packaging/$ID.desktop.txt > "$APP_DIR/$ID.desktop"
install -m 644 packaging/$ID.svg "$ICON_DIR/$ID.svg"
update-desktop-database "$APP_DIR" 2>/dev/null || true
gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Lưu ý: thêm $BIN_DIR vào PATH (đăng xuất/đăng nhập lại là Ubuntu tự thêm)";;
esac

echo "== 4/4 Phím tắt =="
if [ "$1" = "--no-hotkey" ] || [ "$2" = "--no-hotkey" ]; then
  echo "Bỏ qua gán phím tắt. Gán sau bằng: quickshot hotkey --key Print"
else
  "$BIN_DIR/quickshot" hotkey --key Print || echo "Không gán được phím tắt tự động; vào Settings > Keyboard > Custom Shortcuts, lệnh: gtk-launch $ID"
fi

echo
echo "Xong! Chạy thử:  $BIN_DIR/quickshot        (hoặc bấm phím Print)"
echo "Trợ giúp:        quickshot --help"
