# Rust Discord Overlay

A Discord voice/text overlay for Linux, written in Rust.

This is a rewrite of [trigg/Discover](https://github.com/trigg/Discover) — all credits to the original authors for the protocol work and design.

> **Note:** I'm not a real programmer, though I do work in IT. This code was written by AI and tested by me. Feel free to take it, modify it, and use it however you like.
>
> *"Non sono un vero programmatore, mi occupo però di informatica. Il codice è stato scritto da IA e testato, potete prenderlo e modificarlo come meglio volete."*

---

## Features

- **Voice overlay** — shows who is in your Discord voice channel, with avatars, talking ring, mute/deafen badges
- **Text overlay** — optional overlay for a text channel
- **System tray** — hide/show overlay and open settings from the tray icon
- **PulseAudio/PipeWire sync** — muting your mic in Discord mutes your system mic too (optional)
- **Wayland native** — uses `gtk4-layer-shell` for a true overlay layer above all windows
- **Click passthrough** — clicks go through the overlay to whatever is below it
- **Low resource usage** — ~5 MB binary, ~20 MB RAM

## Screenshots

| Overlay | Settings |
|---|---|
| Voice channel with avatars and talking ring | Clean GTK4 settings window |

## Requirements

### System dependencies

**Arch / CachyOS / Manjaro**
```bash
sudo pacman -S gtk4 gtk4-layer-shell cairo pkg-config
```

**Debian / Ubuntu**
```bash
sudo apt install libgtk-4-dev libgtk4-layer-shell-dev libcairo2-dev pkg-config
```

**Fedora**
```bash
sudo dnf install gtk4-devel gtk4-layer-shell-devel cairo-devel pkg-config
```

### Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Building

```bash
git clone https://github.com/YOUR_USERNAME/rust-discord-overlay
cd rust-discord-overlay
cargo build --release
```

## Installing

```bash
bash install.sh
```

This installs the binary to `~/.local/bin/` and the icon + `.desktop` file to `~/.local/share/`.

## Usage

```bash
# Start the overlay (runs in background, tray icon appears)
rust-discord-overlay

# Open settings window
rust-discord-overlay configure
# or click "Settings…" in the tray icon

# Control a running overlay
rust-discord-overlay hide
rust-discord-overlay show
rust-discord-overlay mute / unmute
rust-discord-overlay deaf / undeaf
rust-discord-overlay leave
rust-discord-overlay close
```

## Configuration

Stored at `~/.config/rust-discord-overlay/config.toml` — created automatically on first run.

Change settings via the Settings window (`rust-discord-overlay configure` or tray → Settings…), then click **Apply to Overlay**.

## How it works

```
rust-discord-overlay (daemon)
 ├── discord::run      — WebSocket RPC → Discord (Streamkit protocol)
 ├── ipc::serve        — Unix socket at $XDG_RUNTIME_DIR/rust-discord-overlay.sock
 ├── audio::run        — pactl subscriber (optional PulseAudio/PipeWire sync)
 ├── tray::spawn       — StatusNotifierItem D-Bus tray icon
 └── overlay::run      — GTK4 layer-shell window (main thread)
```

## Authentication

On first launch, Discord will ask you to authorise the overlay (same popup as the original Discover). The token is cached at `~/.config/rust-discord-overlay/access_token` for future launches.

## System tray

The tray icon uses the **StatusNotifierItem** D-Bus protocol:
- **KDE Plasma** — works natively
- **GNOME** — requires the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/)
- **XFCE, LXDE, i3, Sway** — works with a compatible status bar (e.g. waybar with the tray module)

## Credits

- [trigg/Discover](https://github.com/trigg/Discover) — original Python overlay, protocol research
- [gtk-rs](https://gtk-rs.org/) — GTK4 Rust bindings
- [ksni](https://github.com/iovxw/ksni) — StatusNotifierItem tray
- [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) — WebSocket client

## License

GPL-3.0 — same as the original Discover project.
