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

---

## Installation

### Arch Linux / CachyOS / Manjaro (recommended)

```bash
# Clone and build with makepkg
git clone https://github.com/deddroid/rust-discord-overlay.git
cd rust-discord-overlay
makepkg -si
```

Or manually with the PKGBUILD included in the repo — once published on AUR you will be able to install with:
```bash
yay -S rust-discord-overlay
```

### Other distributions

**Requirements:**

| Distro | Command |
|---|---|
| Debian / Ubuntu | `sudo apt install libgtk-4-dev libgtk4-layer-shell-dev libcairo2-dev pkg-config` |
| Fedora | `sudo dnf install gtk4-devel gtk4-layer-shell-devel cairo-devel pkg-config` |
| Arch | `sudo pacman -S gtk4 gtk4-layer-shell cairo pkg-config` |

You also need Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then build and install:
```bash
git clone https://github.com/deddroid/rust-discord-overlay.git
cd rust-discord-overlay
cargo build --release
bash install.sh
```

`install.sh` installs the binary to `~/.local/bin/`, the icon and `.desktop` file to `~/.local/share/`.

---

## Usage

```bash
# Start the overlay (tray icon appears)
rust-discord-overlay

# Open settings
rust-discord-overlay configure

# Control a running overlay
rust-discord-overlay hide / show
rust-discord-overlay mute / unmute
rust-discord-overlay deaf / undeaf
rust-discord-overlay leave
rust-discord-overlay close
```

You can also use the **system tray icon** to open settings, hide/show the overlay, or quit.

---

## Configuration

Settings are stored at `~/.config/rust-discord-overlay/config.toml` and are created automatically on first run.

Open the settings window with `rust-discord-overlay configure` (or tray → Settings…), change what you need, then click **Save All** and **Apply to Overlay**.

---

## First run — Discord authorisation

On first launch, Discord will show an authorisation popup (same as the original Discover overlay). Accept it — the token is cached at `~/.config/rust-discord-overlay/access_token` so you won't be asked again.

---

## System tray

The tray icon uses the **StatusNotifierItem** D-Bus protocol:

| Desktop | Support |
|---|---|
| KDE Plasma | ✅ Native |
| GNOME | ✅ With [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/) |
| XFCE / LXDE | ✅ Native |
| Sway / Hyprland | ✅ With a compatible bar (e.g. waybar with tray module) |

---

## How it works

```
rust-discord-overlay (daemon)
 ├── discord::run      — WebSocket RPC → Discord (Streamkit protocol)
 ├── ipc::serve        — Unix socket ($XDG_RUNTIME_DIR/rust-discord-overlay.sock)
 ├── audio::run        — pactl subscriber (optional PulseAudio/PipeWire sync)
 ├── tray::spawn       — StatusNotifierItem D-Bus tray icon
 └── overlay::run      — GTK4 layer-shell window (main thread)
```

---

## Credits

- [trigg/Discover](https://github.com/trigg/Discover) — original Python overlay, protocol research
- [Claude](https://claude.ai) (Anthropic) — wrote virtually all the Rust code in this project
- [gtk-rs](https://gtk-rs.org/) — GTK4 Rust bindings
- [ksni](https://github.com/iovxw/ksni) — StatusNotifierItem tray
- [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) — WebSocket client

## License

GPL-3.0 — same as the original Discover project.
