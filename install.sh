#!/usr/bin/env bash
# Cài quickshot cho người dùng hiện tại: gói phụ thuộc + nhị phân + .desktop +
# icon + phím tắt. Máy mới chỉ cần chạy đúng file này.
#
#   ./install.sh                  cài (dùng bin/quickshot dựng sẵn nếu chạy được)
#   ./install.sh --build          ép build lại từ mã nguồn
#   ./install.sh --no-hotkey      không gán phím Print
#   ./install.sh --no-deps        bỏ qua bước cài gói (không cần sudo)
set -e
cd "$(dirname "$0")"

BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
ID="dev.quickshot.QuickShot"
MIN_RUST="1.85" # edition 2024

BUILD=0
HOTKEY=1
DEPS=1
for a in "$@"; do
  case "$a" in
  --build) BUILD=1 ;;
  --no-hotkey) HOTKEY=0 ;;
  --no-deps) DEPS=0 ;;
  -h | --help)
    sed -n '2,8p' "$0" | sed 's/^# \?//'
    exit 0
    ;;
  *)
    echo "Tham số lạ: $a (xem ./install.sh --help)"
    exit 2
    ;;
  esac
done

has() { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------- 1. phụ thuộc
# Nền đồ hoạ quickshot cần:
#   libgtk-4         lớp chọn vùng, cửa sổ Cài đặt
#   xdg-desktop-portal + backend theo desktop   chụp màn hình trên Wayland
#   xclip            copy ảnh mà không mở cửa sổ (GNOME không có zwlr_data_control)
#   wl-clipboard     copy ảnh trên wlroots/KDE và ở chế độ dòng lệnh
#   libnotify-bin    notify-send (thông báo sau khi chụp)
#   procps           pkill (tạm ẩn app luôn nổi trên cùng lúc chọn vùng)
#   libglib2.0-bin   gsettings (gán phím tắt GNOME)
#   libgtk-3-bin     gtk-launch (lệnh phím tắt, giữ quyền portal)
#   desktop-file-utils  update-desktop-database
portal_backend_apt() {
  case "${XDG_CURRENT_DESKTOP,,}" in
  *kde* | *plasma*) echo xdg-desktop-portal-kde ;;
  *gnome* | *unity* | *ubuntu*) echo xdg-desktop-portal-gnome ;;
  *sway* | *hyprland* | *wlroots* | *river*) echo xdg-desktop-portal-wlr ;;
  *) echo xdg-desktop-portal-gtk ;;
  esac
}

install_deps() {
  if has apt-get; then
    local pkgs=(
      libgtk-4-1 libgtk-3-bin libglib2.0-bin desktop-file-utils
      xdg-desktop-portal "$(portal_backend_apt)"
      xclip wl-clipboard libnotify-bin procps
    )
    echo "Gói: ${pkgs[*]}"
    sudo apt-get install -y "${pkgs[@]}" || true
    # Dự phòng khi portal hỏng: gnome-screenshot (GNOME) / grim (wlroots).
    case "$(portal_backend_apt)" in
    *gnome) sudo apt-get install -y gnome-screenshot || true ;;
    *wlr) sudo apt-get install -y grim || true ;;
    esac
  elif has dnf; then
    sudo dnf install -y gtk4 gtk3 glib2 desktop-file-utils \
      xdg-desktop-portal xdg-desktop-portal-gnome \
      xclip wl-clipboard libnotify procps-ng || true
  elif has pacman; then
    sudo pacman -S --needed --noconfirm gtk4 gtk3 glib2 desktop-file-utils \
      xdg-desktop-portal xdg-desktop-portal-gnome \
      xclip wl-clipboard libnotify procps-ng || true
  elif has zypper; then
    sudo zypper install -y gtk4-tools libgtk-4-1 glib2-tools desktop-file-utils \
      xdg-desktop-portal xdg-desktop-portal-gnome \
      xclip wl-clipboard libnotify-tools procps || true
  else
    echo "Không nhận ra trình quản lý gói. Cài tay: gtk4, xdg-desktop-portal (+backend),"
    echo "xclip, wl-clipboard, libnotify (notify-send), procps (pkill), gsettings, gtk-launch."
  fi
}

echo "== 1/4 Gói phụ thuộc =="
if [ "$DEPS" = 1 ]; then
  install_deps
else
  echo "Bỏ qua (--no-deps)."
fi

# ------------------------------------------------------------- 2. file chạy
rust_ok() {
  has cargo || return 1
  local v
  v=$(cargo --version | awk '{print $2}')
  [ "$(printf '%s\n%s\n' "$MIN_RUST" "$v" | sort -V | head -1)" = "$MIN_RUST" ]
}

build_from_source() {
  if ! rust_ok; then
    echo "Cần Rust >= $MIN_RUST (edition 2024) để build. Cài một trong hai cách:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # bản mới nhất"
    echo "  sudo apt install cargo rustc                                    # nếu apt đủ mới"
    exit 1
  fi
  if has apt-get; then
    sudo apt-get install -y build-essential pkg-config libgtk-4-dev || true
  fi
  cargo build --release
  mkdir -p bin && cp target/release/quickshot bin/quickshot
}

echo "== 2/4 File thực thi =="
if [ "$BUILD" = 1 ]; then
  build_from_source
elif [ -x bin/quickshot ] && ./bin/quickshot --version >/dev/null 2>&1; then
  echo "Dùng bản dựng sẵn: $(./bin/quickshot --version)"
else
  echo "Bản dựng sẵn thiếu hoặc không chạy được trên máy này — build lại."
  build_from_source
fi

# ------------------------------------------------------------------ 3. cài đặt
echo "== 3/4 Copy file =="
mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
install -m 755 bin/quickshot "$BIN_DIR/quickshot"
sed "s|^Exec=quickshot |Exec=$BIN_DIR/quickshot |" packaging/$ID.desktop.txt >"$APP_DIR/$ID.desktop"
install -m 644 packaging/$ID.svg "$ICON_DIR/$ID.svg"
update-desktop-database "$APP_DIR" 2>/dev/null || true
gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

case ":$PATH:" in
*":$BIN_DIR:"*) ;;
*) echo "Lưu ý: thêm $BIN_DIR vào PATH (đăng xuất/đăng nhập lại là Ubuntu tự thêm)" ;;
esac

# ----------------------------------------------------------------- 4. phím tắt
echo "== 4/4 Phím tắt =="
if [ "$HOTKEY" = 1 ]; then
  "$BIN_DIR/quickshot" hotkey --key Print ||
    echo "Không gán được phím tắt tự động; vào Settings > Keyboard > Custom Shortcuts, lệnh: gtk-launch $ID"
else
  echo "Bỏ qua gán phím tắt. Gán sau bằng: quickshot hotkey --key Print"
fi

# --------------------------------------------------------------- tự kiểm tra
echo
echo "Kiểm tra công cụ ngoài:"
for t in xclip wl-copy notify-send pkill gsettings gtk-launch; do
  if has "$t"; then
    echo "  [ok]     $t"
  else
    echo "  [thiếu]  $t"
  fi
done
if [ -z "$(pgrep -f xdg-desktop-portal 2>/dev/null)" ]; then
  echo "  [lưu ý]  chưa thấy tiến trình xdg-desktop-portal (đăng xuất/đăng nhập lại nếu chụp lỗi)"
fi

echo
echo "Xong! Chạy thử:  $BIN_DIR/quickshot        (hoặc bấm phím Print)"
echo "Trợ giúp:        quickshot --help"
