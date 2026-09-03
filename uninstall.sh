#!/usr/bin/env bash
ID="dev.quickshot.QuickShot"
"$HOME/.local/bin/quickshot" hotkey --remove 2>/dev/null || true
rm -f "$HOME/.local/bin/quickshot" "$HOME/.local/share/applications/$ID.desktop" "$HOME/.local/share/icons/hicolor/scalable/apps/$ID.svg"
echo "Đã gỡ quickshot (file cấu hình ~/.config/quickshot vẫn giữ)."
