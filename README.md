# quickshot

Ứng dụng chụp màn hình & chú thích kiểu **Flameshot** cho Ubuntu 26 (GNOME, Wayland), viết bằng **Rust + GTK4**.
Điểm khác biệt: chụp và cắt vùng trên **cả 2 (hoặc nhiều) màn hình** cùng lúc — kéo chuột qua ranh giới màn hình thoải mái.

## Cài đặt

Bản trên Google Drive chỉ có mã nguồn (không có file thực thi dựng sẵn), nên cần dựng từ nguồn:

```bash
cd quickshot
chmod +x install.sh uninstall.sh
./install.sh            # cài gói phụ thuộc + bản dựng sẵn + icon + phím Print
```

Máy mới chỉ cần chạy đúng `install.sh`: nó cài hết gói phụ thuộc (hỏi `sudo`), dùng bản dựng sẵn
trong `bin/` — nếu bản đó không chạy được trên máy đó thì tự build lại từ mã nguồn — copy `quickshot`
vào `~/.local/bin`, tạo icon trong menu ứng dụng, gán phím **Print**, rồi in bảng kiểm tra công cụ ngoài.
(Phím Print gốc của GNOME được tắt; `quickshot hotkey --remove` để trả lại.)

Gói được cài (tên theo apt, có ánh xạ sẵn cho dnf/pacman/zypper):

| Gói | Dùng để |
|---|---|
| `libgtk-4-1` | lớp chọn vùng, cửa sổ Cài đặt |
| `xdg-desktop-portal` + backend theo desktop | chụp màn hình trên Wayland |
| `xclip` | copy ảnh mà không mở cửa sổ phụ (GNOME thiếu `zwlr_data_control`) |
| `wl-clipboard` | copy ảnh trên wlroots/KDE và ở chế độ dòng lệnh |
| `libnotify-bin` | `notify-send` — thông báo sau khi chụp |
| `procps` | `pkill` — tạm ẩn app luôn nổi trên cùng lúc chọn vùng |
| `libglib2.0-bin`, `libgtk-3-bin`, `desktop-file-utils` | `gsettings`, `gtk-launch`, cập nhật menu |

Tham số: `--build` ép build lại, `--no-hotkey` không đụng phím Print, `--no-deps` bỏ bước cài gói (khỏi cần sudo).
Rust chỉ cần khi phải build (>= 1.85, edition 2024).

**Lần chạy đầu tiên** GNOME sẽ hỏi *"Cho phép QuickShot chụp màn hình?"* — chọn **Share / Cho phép**.
Nếu mở qua phím tắt hoặc icon trong menu, GNOME nhớ lựa chọn này và không hỏi lại.

## Dùng

Bấm **Print** (hoặc chạy `quickshot`). Màn hình tối đi, kéo chuột để chọn vùng — qua cả 2 màn hình được.

| Thao tác | Kết quả |
|---|---|
| Kéo chuột trái | chọn vùng |
| Click không kéo / **Space** | chọn cả màn hình dưới con trỏ |
| **Ctrl+A** | chọn tất cả màn hình |
| Kéo tay cầm ở cạnh/góc | đổi kích thước; kéo bên trong = di chuyển |
| Giữ **Shift** khi kéo | khoá vuông 1:1 (với mũi tên/đường: khoá góc 45°) |
| **Tab** hoặc nút tỉ lệ | Tự do → 1:1 → 4:3 → 16:9 → 3:2 → 9:16 |
| Phím mũi tên | dịch 1px (Ctrl: 10px; Shift: đổi kích thước) |
| **Enter** / **Ctrl+C** | copy vào clipboard rồi thoát |
| **Ctrl+S** | lưu vào `~/Pictures/Screenshots` (và copy) |
| **Ctrl+Shift+S** | lưu thành… (chọn nơi lưu) |
| **Esc** / chuột phải | thoát / bỏ vùng chọn |

Công cụ vẽ (phím tắt hoặc thanh công cụ):

| Phím | Công cụ | Phím | Công cụ |
|---|---|---|---|
| S | chọn vùng | T | chữ (gõ tiếng Việt qua ibus/fcitx bình thường, Enter để xong) |
| R | hình chữ nhật | N | đánh số 1, 2, 3… |
| E | hình elip | B | làm mờ / pixel hoá (che thông tin nhạy cảm) |
| L | đường thẳng | C | lấy màu tại điểm click (copy mã #RRGGBB) |
| A | mũi tên | F | bật/tắt tô đặc cho chữ nhật, elip |
| P | bút vẽ | 1–9, 0 | chọn màu nhanh |
| M | bút dạ quang | lăn chuột, `[` `]` | độ dày nét / cỡ chữ |
| Ctrl+Z | hoàn tác | Ctrl+Shift+Z | làm lại |

## Dòng lệnh

```
quickshot                      mở giao diện chọn vùng
quickshot gui --ratio 16:9     mở với tỉ lệ khung 16:9 (cũng có --color, --dir)
quickshot full                 chụp ngay toàn bộ các màn hình → file
quickshot full -c              chụp ngay → clipboard
quickshot full --screen 1      chỉ chụp màn hình số 1
quickshot full --region 0,0,800,600 -o anh.png
quickshot full -o - > anh.png  in PNG ra stdout (dùng trong script)
quickshot --delay 3            chờ 3 giây rồi mới chụp (mở menu trước)
quickshot screens              liệt kê màn hình
quickshot hotkey --key Print   gán phím tắt (--remove để gỡ)
quickshot config --init        tạo file cấu hình mẫu
quickshot --help               trợ giúp đầy đủ
quickshot --debug              in thông tin màn hình / ảnh chụp khi có lỗi
```

## Cấu hình `~/.config/quickshot/config.toml`

```toml
save_dir = "~/Pictures/Screenshots"
filename = "Screenshot_%Y-%m-%d_%H-%M-%S.png"
color = "#FF0000"
thickness = 4
copy_on_save = true
notify = true
dim = 0.45
ratio = "free"
enter_copies = true
```

## Cách hoạt động trên Wayland

Wayland không cho ứng dụng tự đọc màn hình, nên quickshot chụp qua **xdg-desktop-portal**
(cùng cơ chế GNOME dùng) — portal trả về một ảnh ghép của tất cả màn hình. Sau đó quickshot mở một
cửa sổ toàn màn hình trên **từng** màn hình, cùng chia sẻ một vùng chọn, nên kéo chọn qua lại giữa các màn hình
được và ảnh xuất ra giữ đúng độ phân giải gốc (kể cả khi bật scale 200%).

Trên X11 (Xorg) app vẫn chạy; nếu không có portal sẽ tự dùng `gnome-screenshot`, `grim`, `import` hoặc `scrot`.

## Khắc phục sự cố

- **Không chụp được / "Access denied"**: cài `xdg-desktop-portal-gnome`, đăng xuất rồi vào lại.
  Nếu lỡ bấm *Deny*: mở Settings → Apps → QuickShot → bật Screenshot, hoặc `flatpak permission-reset screenshot`.
- **Ảnh chỉ có 1 màn hình / lệch vị trí**: chạy `quickshot screens` và `quickshot --debug`, gửi kết quả để chỉnh.
- **Copy xong dán không được**: cài `xclip` (`sudo apt install xclip`); thiếu cả `xclip` lẫn
  `wl-copy` thì quickshot phải tự giữ clipboard nên tiến trình ở lại nền tới 3 giờ.
- **GNOME hiện thông báo lạ `"wl-clipboard" is ready`**: cài `xclip`. Trên GNOME `wl-copy` phải tự mở
  một cửa sổ ẩn để xin focus, Shell coi đó là cửa sổ đòi chú ý; quickshot né bằng `xclip` hoặc clipboard GTK.
- **Phím Print vẫn mở trình chụp của GNOME**: chạy lại `quickshot hotkey --key Print`.

## Dựng từ mã nguồn

```bash
sudo apt install cargo rustc libgtk-4-dev build-essential pkg-config
cargo build --release      # → target/release/quickshot
```

Mã nguồn: `src/capture.rs` (chụp qua portal), `src/overlay.rs` (giao diện & công cụ), `src/render.rs` (vẽ chú thích),
`src/output.rs` (clipboard/lưu), `src/hotkey.rs` (gsettings), `src/config.rs`, `src/model.rs`.

## Ẩn app luôn nổi trên cùng khi chụp

Trước khi chụp, quickshot gửi `SIGUSR1` cho các app "luôn nổi trên cùng" (hiện tại: `quick-note`) để chúng ẩn đi, nhắc lại `SIGUSR1` mỗi 2 giây trong lúc lớp chọn vùng còn mở, và gửi `SIGUSR2` khi đóng. App bị ẩn nên tự hiện lại nếu ngừng nhận nhắc vài giây (phòng khi quickshot bị kill). Lý do: cửa sổ luôn nổi vừa lọt vào ảnh, vừa nằm đè lên lớp chọn vùng nên nuốt mất chuột và bàn phím.

Thêm app khác vào danh sách: sửa `APPS` trong `src/inhibit.rs` (app đó cần tự ẩn khi nhận `SIGUSR1` và hiện lại khi nhận `SIGUSR2`).
