use crate::config::{Anchor, AvatarOrder, Config};
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, FontDialog, FontDialogButton,
    Grid, HeaderBar, Label, Notebook, Orientation,
    PolicyType, ScrolledWindow, SpinButton,
    Adjustment, DropDown, StringList, Switch, Window,
};

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
    gtk4::init().expect("GTK init failed");
    let win = build_window();
    win.present();
    win.connect_close_request(|_| {
        release_lock();
        // _exit bypasses GTK cleanup handlers that can block on Wayland
        unsafe { libc::_exit(0); }
    });
    let ml = glib::MainLoop::new(Some(&glib::MainContext::default()), false);
    ml.run();
    release_lock();
    unsafe { libc::_exit(0); }
}

// ── Shared state ──────────────────────────────────────────────────────────────
// All widgets captured here; Save All reads them all at once → single write

struct VoiceWidgets {
    anchor: DropDown, x: SpinButton, y: SpinButton,
    show_av: Switch, square: Switch, fancy: Switch,
    av_size: SpinButton, opacity: SpinButton,
    show_names: Switch, nick_len: SpinButton,
    horiz: Switch, spacing: SpinButton, vpad: SpinButton, hpad: SpinButton,
    order: DropDown,
    only_sp: Switch, grace: SpinButton, hl_self: Switch,
    sh_title: Switch, sh_conn: Switch, sh_disc: Switch, fade_t: SpinButton,
    fade_en: Switch, fade_min: SpinButton, inact_t: SpinButton, fade_dur: SpinButton,
}

struct ColorWidgets {
    tk_fg: gtk4::ColorDialogButton, tk_bg: gtk4::ColorDialogButton, tk_bo: gtk4::ColorDialogButton,
    id_fg: gtk4::ColorDialogButton, id_bg: gtk4::ColorDialogButton, id_bo: gtk4::ColorDialogButton,
    mt_fg: gtk4::ColorDialogButton, mt_bg: gtk4::ColorDialogButton,
    bg_pill: gtk4::ColorDialogButton, av_bg: gtk4::ColorDialogButton,
}

struct AdvancedWidgets {
    audio: Switch, font_btn: FontDialogButton, tpad: SpinButton, tadj: SpinButton, bw: SpinButton,
}

struct TextWidgets {
    enabled: Switch, popup: Switch, attach: Switch,
    limit: SpinButton, ptime: SpinButton,
    anch: DropDown, tx: SpinButton, ty: SpinButton,
    ch: gtk4::Entry,
    fg: gtk4::ColorDialogButton, bg: gtk4::ColorDialogButton,
}

fn save_all(vw: &VoiceWidgets, cw: &ColorWidgets, aw: &AdvancedWidgets, tw: &TextWidgets) {
    let mut c = Config::load();

    // Voice
    c.voice.anchor = match vw.anchor.selected() { 0=>Anchor::BottomLeft, 1=>Anchor::BottomRight, 2=>Anchor::TopLeft, _=>Anchor::TopRight };
    c.voice.x = vw.x.value() as i32; c.voice.y = vw.y.value() as i32;
    c.voice.show_avatar = vw.show_av.is_active();
    c.voice.square_avatar = vw.square.is_active();
    c.voice.fancy_border = vw.fancy.is_active();
    c.voice.icon_size = vw.av_size.value() as u32;
    c.voice.icon_transparency = vw.opacity.value() / 100.0;
    c.voice.show_names = vw.show_names.is_active();
    c.voice.nick_length = vw.nick_len.value() as u32;
    c.voice.horizontal = vw.horiz.is_active();
    c.voice.icon_spacing = vw.spacing.value() as i32;
    c.voice.vert_edge_padding = vw.vpad.value() as i32;
    c.voice.horz_edge_padding = vw.hpad.value() as i32;
    c.voice.order = match vw.order.selected() { 0=>AvatarOrder::Alphabetical, 1=>AvatarOrder::Id, _=>AvatarOrder::LastSpoken };
    c.voice.only_speaking = vw.only_sp.is_active();
    c.voice.only_speaking_grace = vw.grace.value() as u32;
    c.voice.highlight_self = vw.hl_self.is_active();
    c.voice.show_title = vw.sh_title.is_active();
    c.voice.show_connection = vw.sh_conn.is_active();
    c.voice.show_disconnected = vw.sh_disc.is_active();
    c.voice.fade_time = vw.fade_t.value();
    c.voice.fade_out_inactive = vw.fade_en.is_active();
    c.voice.fade_out_limit = vw.fade_min.value() / 100.0;
    c.voice.inactive_time = vw.inact_t.value() as u32;
    c.voice.inactive_fade_time = vw.fade_dur.value() as u32;

    // Colors
    c.voice.talking_color        = rgba(&cw.tk_fg);
    c.voice.talking_bg_color     = rgba(&cw.tk_bg);
    c.voice.talking_border_color = rgba(&cw.tk_bo);
    c.voice.idle_color           = rgba(&cw.id_fg);
    c.voice.idle_bg_color        = rgba(&cw.id_bg);
    c.voice.idle_border_color    = rgba(&cw.id_bo);
    c.voice.mute_color           = rgba(&cw.mt_fg);
    c.voice.mute_bg_color        = rgba(&cw.mt_bg);
    c.voice.bg_color             = rgba(&cw.bg_pill);
    c.voice.avatar_bg_color      = rgba(&cw.av_bg);

    // Advanced
    c.audio_assist = aw.audio.is_active();
    if let Some(fd) = aw.font_btn.font_desc() { c.voice.font = fd.to_string(); }
    c.voice.text_padding = aw.tpad.value() as i32;
    c.voice.text_baseline_adj = aw.tadj.value() as i32;
    c.voice.border_width = aw.bw.value() as u32;

    // Text
    c.text.enabled = tw.enabled.is_active();
    c.text.popup_style = tw.popup.is_active();
    c.text.show_attachments = tw.attach.is_active();
    c.text.message_limit = tw.limit.value() as usize;
    c.text.popup_time = tw.ptime.value() as u32;
    c.text.anchor = match tw.anch.selected() { 0=>Anchor::BottomLeft, 1=>Anchor::BottomRight, 2=>Anchor::TopLeft, _=>Anchor::TopRight };
    c.text.x = tw.tx.value() as i32; c.text.y = tw.ty.value() as i32;
    c.text.channel_id = tw.ch.text().to_string();
    c.text.fg_color = rgba(&tw.fg); c.text.bg_color = rgba(&tw.bg);

    match c.save() {
        Ok(_)  => println!("Saved → {:?}", crate::config::Config::config_path()),
        Err(e) => eprintln!("Save error: {e}"),
    }
}

fn build_window() -> Window {
    let win = Window::builder()
        .title("Rust Discord Overlay — Settings")
        .default_width(580).default_height(620)
        .resizable(true).build();

    gtk4::Window::set_default_icon_name("rust-discord-overlay");

    let css = gtk4::CssProvider::new();
    css.load_from_string("
        .section-header { font-size: 11px; font-weight: bold;
                          color: alpha(currentColor, 0.55); margin-top: 16px; }
        .save-bar { background: alpha(currentColor, 0.05);
                    border-top: 1px solid alpha(currentColor, 0.1); }
    ");
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    win.set_titlebar(Some(&HeaderBar::new()));

    let nb = Notebook::new();
    nb.set_vexpand(true); nb.set_hexpand(true);
    nb.set_show_border(false);

    let (voice_sw, vw) = build_voice_tab();
    let (colors_sw, cw) = build_colors_tab();
    let (advanced_sw, aw) = build_advanced_tab();
    let (text_sw, tw) = build_text_tab();

    nb.append_page(&voice_sw,    Some(&tab_label("Voice")));
    nb.append_page(&colors_sw,   Some(&tab_label("Colors")));
    nb.append_page(&advanced_sw, Some(&tab_label("Advanced")));
    nb.append_page(&text_sw,     Some(&tab_label("Text")));

    let outer = GBox::new(Orientation::Vertical, 0);
    outer.set_vexpand(true);
    outer.append(&nb);

    let bar = GBox::new(Orientation::Horizontal, 8);
    bar.add_css_class("save-bar");
    bar.set_margin_start(16); bar.set_margin_end(16);
    bar.set_margin_top(10); bar.set_margin_bottom(10);

    let close_btn = Button::with_label("Close");
    let save_btn  = Button::builder().label("Save All").build();
    let apply_btn = Button::builder().label("Apply to Overlay").build();
    save_btn.add_css_class("suggested-action");
    apply_btn.add_css_class("suggested-action");

    let spacer = GBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bar.append(&close_btn); bar.append(&spacer);
    bar.append(&save_btn); bar.append(&apply_btn);
    outer.append(&bar);
    win.set_child(Some(&outer));

    // Save once — single write to disk
    save_btn.connect_clicked(move |_| save_all(&vw, &cw, &aw, &tw));

    apply_btn.connect_clicked(|_| {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all().build().unwrap();
            let _ = rt.block_on(crate::ipc::send_command(crate::cli::Command::Reload));
        });
    });

    let wc = win.clone();
    close_btn.connect_clicked(move |_| wc.close());
    win
}

fn tab_label(text: &str) -> Label {
    let l = Label::new(Some(text));
    l.set_margin_start(4); l.set_margin_end(4);
    l
}

fn build_voice_tab() -> (ScrolledWindow, VoiceWidgets) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    section(&g, "POSITION", r); r += 1;
    let anchor = make_dd(&["Bottom Left","Bottom Right","Top Left","Top Right"],
        match cfg.voice.anchor { Anchor::BottomLeft=>0, Anchor::BottomRight=>1, Anchor::TopLeft=>2, Anchor::TopRight=>3 });
    wide(&g, "Corner", &anchor, r); r += 1;
    let x = num(&g, "Offset X (px)", cfg.voice.x as f64, 0.0, 3840.0, r); r += 1;
    let y = num(&g, "Offset Y (px)", cfg.voice.y as f64, 0.0, 2160.0, r); r += 1;

    section(&g, "AVATAR", r); r += 1;
    let show_av  = tog(&g, "Show avatar",      cfg.voice.show_avatar,   r); r += 1;
    let square   = tog(&g, "Square avatar",    cfg.voice.square_avatar, r); r += 1;
    let fancy    = tog(&g, "Fancy border",     cfg.voice.fancy_border,  r); r += 1;
    let av_size  = num(&g, "Size (px)",        cfg.voice.icon_size as f64, 16.0, 128.0, r); r += 1;
    let opacity  = num(&g, "Opacity %",        cfg.voice.icon_transparency*100.0, 0.0, 100.0, r); r += 1;

    section(&g, "NAMES", r); r += 1;
    let show_names = tog(&g, "Show names",     cfg.voice.show_names,    r); r += 1;
    let nick_len = num(&g, "Max length",       cfg.voice.nick_length as f64, 1.0, 64.0, r); r += 1;

    section(&g, "LAYOUT", r); r += 1;
    let horiz    = tog(&g, "Horizontal layout",cfg.voice.horizontal,    r); r += 1;
    let spacing  = num(&g, "User spacing (px)",cfg.voice.icon_spacing as f64, 0.0, 64.0, r); r += 1;
    let vpad     = num(&g, "Vert. edge padding",  cfg.voice.vert_edge_padding as f64, 0.0, 400.0, r); r += 1;
    let hpad     = num(&g, "Horiz. edge padding", cfg.voice.horz_edge_padding as f64, 0.0, 400.0, r); r += 1;
    let order    = make_dd(&["Alphabetically","By ID","Last spoken"],
        match cfg.voice.order { AvatarOrder::Alphabetical=>0, AvatarOrder::Id=>1, AvatarOrder::LastSpoken=>2 });
    wide(&g, "Order by", &order, r); r += 1;

    section(&g, "VISIBILITY", r); r += 1;
    let only_sp  = tog(&g, "Speakers only",    cfg.voice.only_speaking,     r); r += 1;
    let grace    = num(&g, "Grace period (s)", cfg.voice.only_speaking_grace as f64, 0.0, 360.0, r); r += 1;
    let hl_self  = tog(&g, "Highlight self",   cfg.voice.highlight_self,    r); r += 1;
    let sh_title = tog(&g, "Show title",       cfg.voice.show_title,        r); r += 1;
    let sh_conn  = tog(&g, "Show connection",  cfg.voice.show_connection,   r); r += 1;
    let sh_disc  = tog(&g, "Show disconnected",cfg.voice.show_disconnected, r); r += 1;
    let fade_t   = num(&g, "Fade timeout (s)", cfg.voice.fade_time, 1.0, 60.0, r); r += 1;

    section(&g, "INACTIVITY FADE", r); r += 1;
    let fade_en  = tog(&g, "Enable fade-out",  cfg.voice.fade_out_inactive,  r); r += 1;
    let fade_min = num(&g, "Min opacity %",    cfg.voice.fade_out_limit*100.0, 0.0, 100.0, r); r += 1;
    let inact_t  = num(&g, "Inactive after (s)",cfg.voice.inactive_time as f64, 1.0, 300.0, r); r += 1;
    let fade_dur = num(&g, "Fade duration (s)",cfg.voice.inactive_fade_time as f64, 1.0, 300.0, r);

    let w = VoiceWidgets { anchor, x, y, show_av, square, fancy, av_size, opacity,
        show_names, nick_len, horiz, spacing, vpad, hpad, order, only_sp, grace,
        hl_self, sh_title, sh_conn, sh_disc, fade_t, fade_en, fade_min, inact_t, fade_dur };
    (wrap(g), w)
}

fn build_colors_tab() -> (ScrolledWindow, ColorWidgets) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    for (col, txt) in [(1i32,"Foreground"),(2,"Background"),(3,"Border")] {
        let l = Label::new(Some(txt));
        l.add_css_class("dim-label"); l.set_halign(Align::Center);
        g.attach(&l, col, r, 1, 1);
    }
    r += 1;

    section(&g, "TALKING", r); r += 1;
    let tk_fg = cdbtn(cfg.voice.talking_color);
    let tk_bg = cdbtn(cfg.voice.talking_bg_color);
    let tk_bo = cdbtn(cfg.voice.talking_border_color);
    cr3(&g, "Colors", &tk_fg, &tk_bg, &tk_bo, r); r += 1;

    section(&g, "IDLE", r); r += 1;
    let id_fg = cdbtn(cfg.voice.idle_color);
    let id_bg = cdbtn(cfg.voice.idle_bg_color);
    let id_bo = cdbtn(cfg.voice.idle_border_color);
    cr3(&g, "Colors", &id_fg, &id_bg, &id_bo, r); r += 1;

    section(&g, "MUTED", r); r += 1;
    let mt_fg = cdbtn(cfg.voice.mute_color);
    let mt_bg = cdbtn(cfg.voice.mute_bg_color);
    let ph1   = cdbtn([0.0,0.0,0.0,0.0]); ph1.set_sensitive(false);
    cr3(&g, "Colors", &mt_fg, &mt_bg, &ph1, r); r += 1;

    section(&g, "OVERLAY", r); r += 1;
    let bg_pill = cdbtn(cfg.voice.bg_color);
    let av_bg   = cdbtn(cfg.voice.avatar_bg_color);
    let ph2     = cdbtn([0.0,0.0,0.0,0.0]); ph2.set_sensitive(false);
    cr3(&g, "Colors", &bg_pill, &av_bg, &ph2, r);

    let w = ColorWidgets { tk_fg, tk_bg, tk_bo, id_fg, id_bg, id_bo, mt_fg, mt_bg, bg_pill, av_bg };
    (wrap(g), w)
}

fn build_advanced_tab() -> (ScrolledWindow, AdvancedWidgets) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    section(&g, "AUDIO", r); r += 1;
    let audio = tog(&g, "Sync with PulseAudio/PipeWire", cfg.audio_assist, r); r += 1;
    let note = Label::new(Some("Requires overlay restart"));
    note.add_css_class("dim-label"); note.set_halign(Align::Start);
    g.attach(&note, 0, r, 4, 1); r += 1;

    section(&g, "FONT", r); r += 1;
    let font_btn = FontDialogButton::new(Some(FontDialog::builder().build()));
    font_btn.set_font_desc(&gtk4::pango::FontDescription::from_string(&cfg.voice.font));
    wide(&g, "Names font", &font_btn, r); r += 1;

    section(&g, "TEXT", r); r += 1;
    let tpad = num(&g, "Padding (px)",    cfg.voice.text_padding as f64, 0.0, 64.0, r); r += 1;
    let tadj = num(&g, "Vertical offset", cfg.voice.text_baseline_adj as f64, -100.0, 100.0, r); r += 1;

    section(&g, "BORDER", r); r += 1;
    let bw = num(&g, "Width (px)", cfg.voice.border_width as f64, 0.0, 20.0, r);

    let w = AdvancedWidgets { audio, font_btn, tpad, tadj, bw };
    (wrap(g), w)
}

fn build_text_tab() -> (ScrolledWindow, TextWidgets) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    section(&g, "TEXT CHANNEL OVERLAY", r); r += 1;
    let enabled = tog(&g, "Enabled",           cfg.text.enabled,          r); r += 1;
    let popup   = tog(&g, "Popup style",       cfg.text.popup_style,      r); r += 1;
    let attach  = tog(&g, "Show attachments",  cfg.text.show_attachments, r); r += 1;
    let limit   = num(&g, "Message limit",     cfg.text.message_limit as f64, 1.0, 200.0, r); r += 1;
    let ptime   = num(&g, "Popup time (s)",    cfg.text.popup_time as f64, 1.0, 300.0, r); r += 1;

    section(&g, "POSITION", r); r += 1;
    let anch = make_dd(&["Bottom Left","Bottom Right","Top Left","Top Right"],
        match cfg.text.anchor { Anchor::BottomLeft=>0, Anchor::BottomRight=>1, Anchor::TopLeft=>2, Anchor::TopRight=>3 });
    wide(&g, "Corner", &anch, r); r += 1;
    let tx = num(&g, "Offset X", cfg.text.x as f64, 0.0, 3840.0, r); r += 1;
    let ty = num(&g, "Offset Y", cfg.text.y as f64, 0.0, 2160.0, r); r += 1;

    section(&g, "CHANNEL", r); r += 1;
    let ch = gtk4::Entry::builder()
        .text(cfg.text.channel_id.as_str())
        .placeholder_text("Paste Discord text channel ID here")
        .hexpand(true).build();
    g.attach(&ch, 0, r, 4, 1); r += 1;

    section(&g, "COLORS", r); r += 1;
    let fg = cdbtn(cfg.text.fg_color);
    let bg = cdbtn(cfg.text.bg_color);
    let ph = cdbtn([0.0,0.0,0.0,0.0]); ph.set_sensitive(false);
    cr3(&g, "Text / Background", &fg, &bg, &ph, r);

    let w = TextWidgets { enabled, popup, attach, limit, ptime, anch, tx, ty, ch, fg, bg };
    (wrap(g), w)
}

// ── Widget helpers ────────────────────────────────────────────────────────────

fn make_grid() -> Grid {
    let g = Grid::new();
    g.set_row_spacing(2); g.set_column_spacing(12);
    g.set_margin_top(16); g.set_margin_bottom(16);
    g.set_margin_start(20); g.set_margin_end(20);
    g.set_hexpand(true); g.set_vexpand(true);
    g
}

fn section(g: &Grid, text: &str, row: i32) {
    let l = Label::new(Some(text));
    l.add_css_class("section-header");
    l.set_halign(Align::Start);
    l.set_margin_top(14); l.set_margin_bottom(2);
    g.attach(&l, 0, row, 4, 1);
}

fn num(g: &Grid, label: &str, val: f64, min: f64, max: f64, row: i32) -> SpinButton {
    let lbl = Label::new(Some(label));
    lbl.set_halign(Align::Start); lbl.set_hexpand(true);
    let sp = SpinButton::new(Some(&Adjustment::new(val,min,max,1.0,10.0,0.0)),1.0,0);
    sp.set_width_request(110);
    g.attach(&lbl, 0, row, 3, 1);
    g.attach(&sp,  3, row, 1, 1);
    sp
}

fn tog(g: &Grid, label: &str, active: bool, row: i32) -> Switch {
    let lbl = Label::new(Some(label));
    lbl.set_halign(Align::Start); lbl.set_hexpand(true);
    let sw = Switch::new();
    sw.set_active(active); sw.set_valign(Align::Center); sw.set_halign(Align::End);
    g.attach(&lbl, 0, row, 3, 1);
    g.attach(&sw,  3, row, 1, 1);
    sw
}

fn wide(g: &Grid, label: &str, w: &impl IsA<gtk4::Widget>, row: i32) {
    let lbl = Label::new(Some(label));
    lbl.set_halign(Align::Start); lbl.set_hexpand(true);
    g.attach(&lbl, 0, row, 2, 1);
    g.attach(w,    2, row, 2, 1);
}

fn cr3(g: &Grid, label: &str, w1: &gtk4::ColorDialogButton,
       w2: &gtk4::ColorDialogButton, w3: &gtk4::ColorDialogButton, row: i32) {
    let lbl = Label::new(Some(label));
    lbl.set_halign(Align::Start); lbl.set_hexpand(true);
    g.attach(&lbl, 0, row, 1, 1);
    g.attach(w1, 1, row, 1, 1);
    g.attach(w2, 2, row, 1, 1);
    g.attach(w3, 3, row, 1, 1);
}

fn make_dd(opts: &[&str], active: u32) -> DropDown {
    let d = DropDown::new(Some(StringList::new(opts)), gtk4::Expression::NONE);
    d.set_selected(active); d
}

fn cdbtn(c: [f64;4]) -> gtk4::ColorDialogButton {
    let d = gtk4::ColorDialog::builder().with_alpha(true).build();
    let b = gtk4::ColorDialogButton::new(Some(d));
    b.set_rgba(&gtk4::gdk::RGBA::new(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32));
    b.set_halign(Align::Center);
    b
}

fn rgba(b: &gtk4::ColorDialogButton) -> [f64;4] {
    let c = b.rgba();
    [c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64]
}

fn wrap(g: Grid) -> ScrolledWindow {
    ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .hexpand(true).vexpand(true)
        .child(&g).build()
}
