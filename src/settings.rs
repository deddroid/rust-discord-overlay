//! Settings window (libadwaita).
//! Every change is written to config.toml and pushed to the running overlay
//! after a short debounce, so there is no Save/Apply step.

use crate::config::{Anchor, AvatarOrder, Config};
use adw::prelude::*;
use gtk4::{gdk, glib, pango, Align, ColorDialog, ColorDialogButton, FontDialog, FontDialogButton, StringList};
use std::{cell::RefCell, rc::Rc, time::Duration};

/// Delay between the last edit and the write + overlay reload
const SAVE_DEBOUNCE: Duration = Duration::from_millis(350);

const ANCHORS: [(&str, Anchor); 4] = [
    ("Top left", Anchor::TopLeft),
    ("Top right", Anchor::TopRight),
    ("Bottom left", Anchor::BottomLeft),
    ("Bottom right", Anchor::BottomRight),
];

const ORDERS: [(&str, AvatarOrder); 3] = [
    ("Alphabetical", AvatarOrder::Alphabetical),
    ("User ID", AvatarOrder::Id),
    ("Last spoken", AvatarOrder::LastSpoken),
];

fn settings_lock_path() -> std::path::PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("rust-discord-overlay-settings.lock")
}

fn acquire_lock() -> bool {
    let path = settings_lock_path();
    if let Ok(s) = std::fs::read_to_string(&path) {
        if let Ok(pid) = s.trim().parse::<u32>() {
            // Check if alive AND not a zombie
            let proc_status = std::fs::read_to_string(format!("/proc/{}/status", pid))
                .unwrap_or_default();
            let alive = proc_status.contains("State:")
                && !proc_status.contains("State:\tZ")
                && !proc_status.contains("State: Z");
            if alive { return false; }
        }
    }
    let _ = std::fs::write(&path, std::process::id().to_string());
    true
}

fn release_lock() {
    let _ = std::fs::remove_file(settings_lock_path());
}

pub fn open_settings() {
    if !acquire_lock() {
        eprintln!("Settings already open");
        return;
    }
    adw::init().expect("libadwaita init failed");
    let ctx = Ctx::new(Config::load());
    let win = build_window(&ctx);
    win.connect_close_request(move |_| {
        ctx.flush();
        release_lock();
        // _exit bypasses GTK cleanup handlers that can block on Wayland
        unsafe { libc::_exit(0); }
    });
    win.present();
    let ml = glib::MainLoop::new(Some(&glib::MainContext::default()), false);
    ml.run();
    release_lock();
    unsafe { libc::_exit(0); }
}

// ── Live config + debounced save ──────────────────────────────────────────────

#[derive(Clone)]
struct Ctx {
    cfg: Rc<RefCell<Config>>,
    pending: Rc<RefCell<Option<glib::SourceId>>>,
}

impl Ctx {
    fn new(cfg: Config) -> Self {
        Self { cfg: Rc::new(RefCell::new(cfg)), pending: Rc::default() }
    }

    /// Apply an edit to the in-memory config and schedule a save
    fn edit(&self, f: impl FnOnce(&mut Config)) {
        f(&mut self.cfg.borrow_mut());
        if let Some(id) = self.pending.borrow_mut().take() { id.remove(); }
        let this = self.clone();
        let id = glib::timeout_add_local_once(SAVE_DEBOUNCE, move || {
            this.pending.borrow_mut().take();
            this.save_and_reload();
        });
        *self.pending.borrow_mut() = Some(id);
    }

    /// Write immediately if an edit is still waiting for its debounce
    fn flush(&self) {
        if let Some(id) = self.pending.borrow_mut().take() {
            id.remove();
            self.save_and_reload();
        }
    }

    fn save_and_reload(&self) {
        if let Err(e) = self.cfg.borrow().save() {
            eprintln!("Save error: {e}");
            return;
        }
        // The overlay may not be running; a failed send is fine
        let _ = std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let _ = rt.block_on(crate::ipc::send_command(crate::cli::Command::Reload));
        }).join();
    }
}

// ── Window ────────────────────────────────────────────────────────────────────

fn build_window(ctx: &Ctx) -> adw::Window {
    gtk4::Window::set_default_icon_name("rust-discord-overlay");

    let stack = adw::ViewStack::new();
    stack.add_titled_with_icon(&voice_page(ctx), Some("voice"), "Voice", "audio-input-microphone-symbolic");
    stack.add_titled_with_icon(&colors_page(ctx), Some("colors"), "Colors", "applications-graphics-symbolic");
    stack.add_titled_with_icon(&text_page(ctx), Some("text"), "Text", "chat-symbolic");
    stack.add_titled_with_icon(&advanced_page(ctx), Some("advanced"), "Advanced", "preferences-system-symbolic");

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    let header = adw::HeaderBar::builder().title_widget(&switcher).build();
    let bottom = adw::ViewSwitcherBar::builder().stack(&stack).build();

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&stack));
    view.add_bottom_bar(&bottom);

    let win = adw::Window::builder()
        .title("Rust Discord Overlay")
        .default_width(680)
        .default_height(760)
        .width_request(360)
        .height_request(400)
        .content(&view)
        .build();

    // Narrow window: move the page switcher to the bottom bar
    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth, 550.0, adw::LengthUnit::Sp,
    ));
    narrow.add_setter(&header, "title-widget", Some(&None::<gtk4::Widget>.to_value()));
    narrow.add_setter(&bottom, "reveal", Some(&true.to_value()));
    win.add_breakpoint(narrow);

    win
}

// ── Pages ─────────────────────────────────────────────────────────────────────

fn voice_page(ctx: &Ctx) -> adw::PreferencesPage {
    let c = ctx.cfg.borrow().voice.clone();
    let page = adw::PreferencesPage::new();

    let g = group(&page, "Position", Some("Where the voice overlay sits on screen"));
    g.add(&combo(ctx, "Corner", &ANCHORS, c.anchor, |cfg, v| cfg.voice.anchor = v));
    g.add(&spin(ctx, "Horizontal offset", "px", c.x as f64, 0.0, 3840.0, |cfg, v| cfg.voice.x = v as i32));
    g.add(&spin(ctx, "Vertical offset", "px", c.y as f64, 0.0, 2160.0, |cfg, v| cfg.voice.y = v as i32));

    let g = group(&page, "Avatars", None);
    g.add(&switch(ctx, "Show avatars", None, c.show_avatar, |cfg, v| cfg.voice.show_avatar = v));
    g.add(&switch(ctx, "Square avatars", None, c.square_avatar, |cfg, v| cfg.voice.square_avatar = v));
    g.add(&switch(ctx, "Fancy border", None, c.fancy_border, |cfg, v| cfg.voice.fancy_border = v));
    g.add(&spin(ctx, "Size", "px", c.icon_size as f64, 16.0, 128.0, |cfg, v| cfg.voice.icon_size = v as u32));
    g.add(&spin(ctx, "Opacity", "%", c.icon_transparency * 100.0, 0.0, 100.0,
        |cfg, v| cfg.voice.icon_transparency = v / 100.0));

    let g = group(&page, "Names", None);
    g.add(&switch(ctx, "Show names", None, c.show_names, |cfg, v| cfg.voice.show_names = v));
    g.add(&spin(ctx, "Maximum length", "characters", c.nick_length as f64, 1.0, 64.0,
        |cfg, v| cfg.voice.nick_length = v as u32));

    let g = group(&page, "Layout", None);
    g.add(&switch(ctx, "Horizontal layout", Some("Place users side by side"), c.horizontal,
        |cfg, v| cfg.voice.horizontal = v));
    g.add(&combo(ctx, "Order", &ORDERS, c.order, |cfg, v| cfg.voice.order = v));
    g.add(&spin(ctx, "Spacing between users", "px", c.icon_spacing as f64, 0.0, 64.0,
        |cfg, v| cfg.voice.icon_spacing = v as i32));
    g.add(&spin(ctx, "Vertical edge padding", "px", c.vert_edge_padding as f64, 0.0, 400.0,
        |cfg, v| cfg.voice.vert_edge_padding = v as i32));
    g.add(&spin(ctx, "Horizontal edge padding", "px", c.horz_edge_padding as f64, 0.0, 400.0,
        |cfg, v| cfg.voice.horz_edge_padding = v as i32));

    let g = group(&page, "Visibility", None);
    g.add(&switch(ctx, "Speakers only", Some("Hide users who are not talking"), c.only_speaking,
        |cfg, v| cfg.voice.only_speaking = v));
    g.add(&spin(ctx, "Fade timeout", "seconds after they stop talking", c.fade_time, 1.0, 60.0,
        |cfg, v| cfg.voice.fade_time = v));
    g.add(&spin(ctx, "Grace period", "seconds", c.only_speaking_grace as f64, 0.0, 360.0,
        |cfg, v| cfg.voice.only_speaking_grace = v as u32));
    g.add(&switch(ctx, "Highlight self", None, c.highlight_self, |cfg, v| cfg.voice.highlight_self = v));
    g.add(&switch(ctx, "Show channel title", None, c.show_title, |cfg, v| cfg.voice.show_title = v));
    g.add(&switch(ctx, "Show connection status", None, c.show_connection,
        |cfg, v| cfg.voice.show_connection = v));
    g.add(&switch(ctx, "Show when disconnected", None, c.show_disconnected,
        |cfg, v| cfg.voice.show_disconnected = v));

    let g = group(&page, "Inactivity Fade", Some("Dim users who have been quiet for a while"));
    let fade = adw::ExpanderRow::builder()
        .title("Fade out inactive users")
        .show_enable_switch(true)
        .enable_expansion(c.fade_out_inactive)
        .build();
    {
        let ctx = ctx.clone();
        fade.connect_enable_expansion_notify(move |r| {
            let v = r.enables_expansion();
            ctx.edit(|cfg| cfg.voice.fade_out_inactive = v);
        });
    }
    fade.add_row(&spin(ctx, "Inactive after", "seconds", c.inactive_time as f64, 1.0, 300.0,
        |cfg, v| cfg.voice.inactive_time = v as u32));
    fade.add_row(&spin(ctx, "Fade duration", "seconds", c.inactive_fade_time as f64, 1.0, 300.0,
        |cfg, v| cfg.voice.inactive_fade_time = v as u32));
    fade.add_row(&spin(ctx, "Minimum opacity", "%", c.fade_out_limit * 100.0, 0.0, 100.0,
        |cfg, v| cfg.voice.fade_out_limit = v / 100.0));
    g.add(&fade);

    page
}

fn colors_page(ctx: &Ctx) -> adw::PreferencesPage {
    let c = ctx.cfg.borrow().voice.clone();
    let page = adw::PreferencesPage::new();

    let g = group(&page, "Talking", None);
    g.add(&color(ctx, "Name", c.talking_color, |cfg, v| cfg.voice.talking_color = v));
    g.add(&color(ctx, "Background", c.talking_bg_color, |cfg, v| cfg.voice.talking_bg_color = v));
    g.add(&color(ctx, "Border", c.talking_border_color, |cfg, v| cfg.voice.talking_border_color = v));

    let g = group(&page, "Idle", None);
    g.add(&color(ctx, "Name", c.idle_color, |cfg, v| cfg.voice.idle_color = v));
    g.add(&color(ctx, "Background", c.idle_bg_color, |cfg, v| cfg.voice.idle_bg_color = v));
    g.add(&color(ctx, "Border", c.idle_border_color, |cfg, v| cfg.voice.idle_border_color = v));

    let g = group(&page, "Muted", None);
    g.add(&color(ctx, "Name and badges", c.mute_color, |cfg, v| cfg.voice.mute_color = v));
    g.add(&color(ctx, "Background", c.mute_bg_color, |cfg, v| cfg.voice.mute_bg_color = v));

    let g = group(&page, "Overlay", None);
    g.add(&color(ctx, "Window background", c.bg_color, |cfg, v| cfg.voice.bg_color = v));
    g.add(&color(ctx, "Avatar placeholder", c.avatar_bg_color, |cfg, v| cfg.voice.avatar_bg_color = v));

    page
}

fn text_page(ctx: &Ctx) -> adw::PreferencesPage {
    let c = ctx.cfg.borrow().text.clone();
    let page = adw::PreferencesPage::new();

    let g = group(&page, "Text Channel Overlay", Some("Show messages from a text channel"));
    g.add(&switch(ctx, "Enabled", Some("Requires an overlay restart"), c.enabled,
        |cfg, v| cfg.text.enabled = v));
    let ch = adw::EntryRow::builder().title("Channel ID").text(c.channel_id.as_str()).build();
    {
        let ctx = ctx.clone();
        ch.connect_changed(move |e| {
            let v = e.text().trim().to_string();
            ctx.edit(|cfg| cfg.text.channel_id = v);
        });
    }
    g.add(&ch);
    g.add(&spin(ctx, "Message limit", "messages", c.message_limit as f64, 1.0, 200.0,
        |cfg, v| cfg.text.message_limit = v as usize));
    g.add(&switch(ctx, "Popup style", None, c.popup_style, |cfg, v| cfg.text.popup_style = v));
    g.add(&spin(ctx, "Popup time", "seconds", c.popup_time as f64, 1.0, 300.0,
        |cfg, v| cfg.text.popup_time = v as u32));
    g.add(&switch(ctx, "Show attachments", None, c.show_attachments, |cfg, v| cfg.text.show_attachments = v));

    let g = group(&page, "Position", None);
    g.add(&combo(ctx, "Corner", &ANCHORS, c.anchor, |cfg, v| cfg.text.anchor = v));
    g.add(&spin(ctx, "Horizontal offset", "px", c.x as f64, 0.0, 3840.0, |cfg, v| cfg.text.x = v as i32));
    g.add(&spin(ctx, "Vertical offset", "px", c.y as f64, 0.0, 2160.0, |cfg, v| cfg.text.y = v as i32));

    let g = group(&page, "Colors", None);
    g.add(&color(ctx, "Text", c.fg_color, |cfg, v| cfg.text.fg_color = v));
    g.add(&color(ctx, "Background", c.bg_color, |cfg, v| cfg.text.bg_color = v));

    page
}

fn advanced_page(ctx: &Ctx) -> adw::PreferencesPage {
    let c = ctx.cfg.borrow().clone();
    let page = adw::PreferencesPage::new();

    let g = group(&page, "Audio", None);
    g.add(&switch(ctx, "Sync with PulseAudio/PipeWire",
        Some("Mirror system mic/speaker mute to Discord · requires restart"),
        c.audio_assist, |cfg, v| cfg.audio_assist = v));

    let g = group(&page, "Font", None);
    let font_btn = FontDialogButton::new(Some(FontDialog::new()));
    font_btn.set_font_desc(&pango::FontDescription::from_string(&c.voice.font));
    font_btn.set_valign(Align::Center);
    {
        let ctx = ctx.clone();
        font_btn.connect_font_desc_notify(move |b| {
            if let Some(fd) = b.font_desc() {
                let v = fd.to_string();
                ctx.edit(|cfg| cfg.voice.font = v);
            }
        });
    }
    let row = adw::ActionRow::builder().title("Names font").activatable_widget(&font_btn).build();
    row.add_suffix(&font_btn);
    g.add(&row);
    g.add(&spin(ctx, "Text padding", "px", c.voice.text_padding as f64, 0.0, 64.0,
        |cfg, v| cfg.voice.text_padding = v as i32));
    g.add(&spin(ctx, "Text vertical offset", "px", c.voice.text_baseline_adj as f64, -100.0, 100.0,
        |cfg, v| cfg.voice.text_baseline_adj = v as i32));

    let g = group(&page, "Border", None);
    g.add(&spin(ctx, "Border width", "px", c.voice.border_width as f64, 0.0, 20.0,
        |cfg, v| cfg.voice.border_width = v as u32));

    let g = group(&page, "Debug", None);
    g.add(&switch(ctx, "Show test users", Some("Preview avatars when you are not in a call"),
        c.show_test_users, |cfg, v| cfg.show_test_users = v));

    let g = group(&page, "Configuration", Some(&Config::config_path().display().to_string()));
    let open = gtk4::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Open config folder")
        .valign(Align::Center)
        .css_classes(["flat"])
        .build();
    open.connect_clicked(|_| {
        let uri = glib::filename_to_uri(Config::config_dir(), None).map(|u| u.to_string());
        if let Ok(uri) = uri {
            gtk4::UriLauncher::new(&uri).launch(None::<&gtk4::Window>, None::<&gtk4::gio::Cancellable>, |_| {});
        }
    });
    let row = adw::ActionRow::builder().title("Config file").activatable_widget(&open).build();
    row.add_suffix(&open);
    g.add(&row);

    page
}

// ── Row helpers ───────────────────────────────────────────────────────────────

fn group(page: &adw::PreferencesPage, title: &str, desc: Option<&str>) -> adw::PreferencesGroup {
    let g = adw::PreferencesGroup::builder().title(title).build();
    if let Some(d) = desc { g.set_description(Some(d)); }
    page.add(&g);
    g
}

fn switch(ctx: &Ctx, title: &str, subtitle: Option<&str>, active: bool,
          set: impl Fn(&mut Config, bool) + 'static) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder().title(title).active(active).build();
    if let Some(s) = subtitle { row.set_subtitle(s); }
    let ctx = ctx.clone();
    row.connect_active_notify(move |r| {
        let v = r.is_active();
        ctx.edit(|cfg| set(cfg, v));
    });
    row
}

#[allow(clippy::too_many_arguments)]
fn spin(ctx: &Ctx, title: &str, unit: &str, value: f64, min: f64, max: f64,
        set: impl Fn(&mut Config, f64) + 'static) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(min, max, 1.0);
    row.set_title(title);
    row.set_subtitle(unit);
    row.set_value(value);
    let ctx = ctx.clone();
    row.connect_value_notify(move |r| {
        let v = r.value();
        ctx.edit(|cfg| set(cfg, v));
    });
    row
}

fn combo<T: Copy + PartialEq + 'static>(ctx: &Ctx, title: &str, options: &'static [(&str, T)],
                                        current: T, set: impl Fn(&mut Config, T) + 'static) -> adw::ComboRow {
    let labels: Vec<&str> = options.iter().map(|(l, _)| *l).collect();
    let row = adw::ComboRow::builder().title(title).model(&StringList::new(&labels)).build();
    row.set_selected(options.iter().position(|(_, v)| *v == current).unwrap_or(0) as u32);
    let ctx = ctx.clone();
    row.connect_selected_notify(move |r| {
        if let Some((_, v)) = options.get(r.selected() as usize) {
            let v = *v;
            ctx.edit(|cfg| set(cfg, v));
        }
    });
    row
}

fn color(ctx: &Ctx, title: &str, c: [f64; 4],
         set: impl Fn(&mut Config, [f64; 4]) + 'static) -> adw::ActionRow {
    let btn = ColorDialogButton::new(Some(ColorDialog::builder().with_alpha(true).build()));
    btn.set_rgba(&gdk::RGBA::new(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32));
    btn.set_valign(Align::Center);
    let ctx = ctx.clone();
    btn.connect_rgba_notify(move |b| {
        let c = b.rgba();
        let v = [c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64];
        ctx.edit(|cfg| set(cfg, v));
    });
    let row = adw::ActionRow::builder().title(title).activatable_widget(&btn).build();
    row.add_suffix(&btn);
    row
}
