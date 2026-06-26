#!/usr/bin/env bash
# Install rust-discord-overlay to user directories (no root needed)
set -e

BINARY="target/release/rust-discord-overlay"
ICON_SRC="assets/icon.svg"
DESKTOP_SRC="assets/rust-discord-overlay.desktop"

if [ ! -f "$BINARY" ]; then
    echo "Binary not found. Run 'cargo build --release' first."
    exit 1
fi

# Install binary
install -Dm755 "$BINARY" "$HOME/.local/bin/rust-discord-overlay"

# Install icon (multiple sizes via rsvg-convert if available, else raw SVG)
ICON_DIR="$HOME/.local/share/icons/hicolor"
install -Dm644 "$ICON_SRC" "$ICON_DIR/scalable/apps/rust-discord-overlay.svg"

if command -v rsvg-convert &>/dev/null; then
    for SIZE in 16 32 48 64 128 256; do
        mkdir -p "$ICON_DIR/${SIZE}x${SIZE}/apps"
        rsvg-convert -w $SIZE -h $SIZE "$ICON_SRC" \
            > "$ICON_DIR/${SIZE}x${SIZE}/apps/rust-discord-overlay.png"
    done
    echo "Icons installed at multiple sizes"
else
    echo "rsvg-convert not found — only SVG icon installed (works on most DEs)"
fi

gtk-update-icon-cache "$ICON_DIR" 2>/dev/null || true

# Install .desktop file
install -Dm644 "$DESKTOP_SRC" \
    "$HOME/.local/share/applications/rust-discord-overlay.desktop"

echo ""
echo "✓ Installed to ~/.local/bin/rust-discord-overlay"
echo "  Add ~/.local/bin to your PATH if not already there."
echo "  Run: rust-discord-overlay"
