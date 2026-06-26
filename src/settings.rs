//! Modern settings window — clean GTK4 design.

use crate::config::{Anchor, AvatarOrder, Config};
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GBox, Button, FontDialog, FontDialogButton,
    Grid, HeaderBar, Label, Notebook, Orientation,
    PolicyType, ScrolledWindow, SpinButton,
    Adjustment, DropDown, StringList, Switch, Window,
};

pub fn open_settings() {
    gtk4::init().expect("GTK init failed");
    let ml = glib::MainLoop::new(None, false);
    let win = build_window(ml.clone());
    win.present();
    let ml2 = ml.clone();
    win.connect_destroy(move |_| {
        ml2.quit();
        // If opened as subprocess (from tray), exit completely when window closes
        std::process::exit(0);
    });
    ml.run();
}

fn build_window(ml: glib::MainLoop) -> Window {
    let win = Window::builder()
        .title("Rust Discord Overlay — Settings")
        .default_width(580).default_height(620)
        .resizable(true).build();

    // Custom CSS for modern look
    let css = gtk4::CssProvider::new();
    css.load_from_string("
        .section-header { font-size: 11px; font-weight: bold; color: alpha(currentColor, 0.55);
                          text-transform: uppercase; letter-spacing: 1px; margin-top: 16px; }
        .color-circle { border-radius: 50%; min-width: 32px; min-height: 32px; }
        notebook tab { padding: 6px 14px; }
        notebook > header { border-bottom: 1px solid alpha(currentColor, 0.1); }
        .settings-row { padding: 2px 0; }
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

    let (voice_sw,    save_voice)    = build_voice_tab();
    let (colors_sw,   save_colors)   = build_colors_tab();
    let (advanced_sw, save_advanced) = build_advanced_tab();
    let (text_sw,     save_text)     = build_text_tab();

    nb.append_page(&voice_sw,    Some(&tab_label("Voice")));
    nb.append_page(&colors_sw,   Some(&tab_label("Colors")));
    nb.append_page(&advanced_sw, Some(&tab_label("Advanced")));
    nb.append_page(&text_sw,     Some(&tab_label("Text")));

    let outer = GBox::new(Orientation::Vertical, 0);
    outer.set_vexpand(true);
    outer.append(&nb);

    // Save bar
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

    save_btn.connect_clicked(move |_| {
        save_voice(); save_colors(); save_advanced(); save_text();
        println!("All settings saved");
    });
    apply_btn.connect_clicked(|_| {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let _ = rt.block_on(crate::ipc::send_command(crate::cli::Command::Reload));
        });
    });
    let wc = win.clone(); let ml2 = ml.clone();
    close_btn.connect_clicked(move |_| { wc.close(); ml2.quit(); });
    win
}

fn tab_label(text: &str) -> Label {
    let l = Label::new(Some(text));
    l.set_margin_start(4); l.set_margin_end(4);
    l
}

// ── Voice tab ─────────────────────────────────────────────────────────────────

fn build_voice_tab() -> (ScrolledWindow, impl Fn() + 'static) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    section(&g, "POSITION", r); r += 1;
    let anchor_dd = make_dd(&["Bottom Left","Bottom Right","Top Left","Top Right"],
        match cfg.voice.anchor { Anchor::BottomLeft=>0, Anchor::BottomRight=>1,
                                  Anchor::TopLeft=>2, Anchor::TopRight=>3 });
    wide(&g, "Corner", &anchor_dd, r); r += 1;
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
    let order_dd = make_dd(&["Alphabetically","By ID","Last spoken"],
        match cfg.voice.order { AvatarOrder::Alphabetical=>0, AvatarOrder::Id=>1, AvatarOrder::LastSpoken=>2 });
    wide(&g, "Order by", &order_dd, r); r += 1;

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

    let save = move || {
        let mut c = Config::load();
        c.voice.anchor = match anchor_dd.selected() { 0=>Anchor::BottomLeft, 1=>Anchor::BottomRight, 2=>Anchor::TopLeft, _=>Anchor::TopRight };
        c.voice.x = x.value() as i32; c.voice.y = y.value() as i32;
        c.voice.show_avatar = show_av.is_active();
        c.voice.square_avatar = square.is_active();
        c.voice.fancy_border = fancy.is_active();
        c.voice.icon_size = av_size.value() as u32;
        c.voice.icon_transparency = opacity.value() / 100.0;
        c.voice.show_names = show_names.is_active();
        c.voice.nick_length = nick_len.value() as u32;
        c.voice.horizontal = horiz.is_active();
        c.voice.icon_spacing = spacing.value() as i32;
        c.voice.vert_edge_padding = vpad.value() as i32;
        c.voice.horz_edge_padding = hpad.value() as i32;
        c.voice.order = match order_dd.selected() { 0=>AvatarOrder::Alphabetical, 1=>AvatarOrder::Id, _=>AvatarOrder::LastSpoken };
        c.voice.only_speaking = only_sp.is_active();
        c.voice.only_speaking_grace = grace.value() as u32;
        c.voice.highlight_self = hl_self.is_active();
        c.voice.show_title = sh_title.is_active();
        c.voice.show_connection = sh_conn.is_active();
        c.voice.show_disconnected = sh_disc.is_active();
        c.voice.fade_time = fade_t.value();
        c.voice.fade_out_inactive = fade_en.is_active();
        c.voice.fade_out_limit = fade_min.value() / 100.0;
        c.voice.inactive_time = inact_t.value() as u32;
        c.voice.inactive_fade_time = fade_dur.value() as u32;
        write_cfg(c);
    };
    (wrap(g), save)
}

// ── Colors tab — circular color swatches ──────────────────────────────────────

fn build_colors_tab() -> (ScrolledWindow, impl Fn() + 'static) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    // Column headers
    for (col, txt) in [(1i32,"Foreground"),(2,"Background"),(3,"Border")] {
        let l = Label::new(Some(txt));
        l.add_css_class("dim-label");
        l.set_halign(Align::Center);
        l.set_margin_bottom(4);
        g.attach(&l, col, r, 1, 1);
    }
    r += 1;

    section(&g, "TALKING", r); r += 1;
    let tk_fg = cdbtn(cfg.voice.talking_color);
    let tk_bg = cdbtn(cfg.voice.talking_bg_color);
    let tk_bo = cdbtn(cfg.voice.talking_border_color);
    color_row3(&g, "Colors", &tk_fg, &tk_bg, &tk_bo, r); r += 1;

    section(&g, "IDLE", r); r += 1;
    let id_fg = cdbtn(cfg.voice.idle_color);
    let id_bg = cdbtn(cfg.voice.idle_bg_color);
    let id_bo = cdbtn(cfg.voice.idle_border_color);
    color_row3(&g, "Colors", &id_fg, &id_bg, &id_bo, r); r += 1;

    section(&g, "MUTED", r); r += 1;
    let mt_fg = cdbtn(cfg.voice.mute_color);
    let mt_bg = cdbtn(cfg.voice.mute_bg_color);
    let ph1   = cdbtn([0.0,0.0,0.0,0.0]); ph1.set_sensitive(false);
    color_row3(&g, "Colors", &mt_fg, &mt_bg, &ph1, r); r += 1;

    section(&g, "OVERLAY BACKGROUND", r); r += 1;
    let bg_pill = cdbtn(cfg.voice.bg_color);
    let av_bg   = cdbtn(cfg.voice.avatar_bg_color);
    let ph2     = cdbtn([0.0,0.0,0.0,0.0]); ph2.set_sensitive(false);
    color_row3(&g, "Colors", &bg_pill, &av_bg, &ph2, r);

    let save = move || {
        let mut c = Config::load();
        c.voice.talking_color        = rgba(&tk_fg);
        c.voice.talking_bg_color     = rgba(&tk_bg);
        c.voice.talking_border_color = rgba(&tk_bo);
        c.voice.idle_color           = rgba(&id_fg);
        c.voice.idle_bg_color        = rgba(&id_bg);
        c.voice.idle_border_color    = rgba(&id_bo);
        c.voice.mute_color           = rgba(&mt_fg);
        c.voice.mute_bg_color        = rgba(&mt_bg);
        c.voice.bg_color             = rgba(&bg_pill);
        c.voice.avatar_bg_color      = rgba(&av_bg);
        write_cfg(c);
    };
    (wrap(g), save)
}

// ── Advanced tab ──────────────────────────────────────────────────────────────

fn build_advanced_tab() -> (ScrolledWindow, impl Fn() + 'static) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    section(&g, "AUDIO", r); r += 1;
    let audio = tog(&g, "Sync with PulseAudio/PipeWire", cfg.audio_assist, r); r += 1;
    let note = Label::new(Some("Requires overlay restart to take effect"));
    note.add_css_class("dim-label"); note.set_halign(Align::Start);
    g.attach(&note, 0, r, 4, 1); r += 1;

    section(&g, "FONT", r); r += 1;
    let fd = FontDialog::builder().build();
    let font_btn = FontDialogButton::new(Some(fd));
    font_btn.set_font_desc(&gtk4::pango::FontDescription::from_string(&cfg.voice.font));
    wide(&g, "Names font", &font_btn, r); r += 1;

    section(&g, "TEXT", r); r += 1;
    let tpad = num(&g, "Padding (px)",        cfg.voice.text_padding as f64, 0.0, 64.0, r); r += 1;
    let tadj = num(&g, "Vertical offset",     cfg.voice.text_baseline_adj as f64, -100.0, 100.0, r); r += 1;

    section(&g, "BORDER", r); r += 1;
    let bw   = num(&g, "Border width (px)",   cfg.voice.border_width as f64, 0.0, 20.0, r);

    let save = move || {
        let mut c = Config::load();
        c.audio_assist = audio.is_active();
        if let Some(fd) = font_btn.font_desc() { c.voice.font = fd.to_string(); }
        c.voice.text_padding = tpad.value() as i32;
        c.voice.text_baseline_adj = tadj.value() as i32;
        c.voice.border_width = bw.value() as u32;
        write_cfg(c);
    };
    (wrap(g), save)
}

// ── Text tab ──────────────────────────────────────────────────────────────────

fn build_text_tab() -> (ScrolledWindow, impl Fn() + 'static) {
    let cfg = Config::load();
    let g = make_grid();
    let mut r = 0i32;

    section(&g, "TEXT CHANNEL OVERLAY", r); r += 1;
    let enabled = tog(&g, "Enabled",            cfg.text.enabled,          r); r += 1;
    let popup   = tog(&g, "Popup style",        cfg.text.popup_style,      r); r += 1;
    let attach  = tog(&g, "Show attachments",   cfg.text.show_attachments, r); r += 1;
    let limit   = num(&g, "Message limit",      cfg.text.message_limit as f64, 1.0, 200.0, r); r += 1;
    let ptime   = num(&g, "Popup time (s)",     cfg.text.popup_time as f64, 1.0, 300.0, r); r += 1;

    section(&g, "POSITION", r); r += 1;
    let anch_dd = make_dd(&["Bottom Left","Bottom Right","Top Left","Top Right"],
        match cfg.text.anchor { Anchor::BottomLeft=>0, Anchor::BottomRight=>1, Anchor::TopLeft=>2, Anchor::TopRight=>3 });
    wide(&g, "Corner", &anch_dd, r); r += 1;
    let tx = num(&g, "Offset X", cfg.text.x as f64, 0.0, 3840.0, r); r += 1;
    let ty = num(&g, "Offset Y", cfg.text.y as f64, 0.0, 2160.0, r); r += 1;

    section(&g, "CHANNEL", r); r += 1;
    let ch = gtk4::Entry::builder()
        .text(cfg.text.channel_id.as_str())
        .placeholder_text("Paste the Discord text channel ID here")
        .hexpand(true).build();
    g.attach(&ch, 0, r, 4, 1); r += 1;

    section(&g, "COLORS", r); r += 1;
    let fg = cdbtn(cfg.text.fg_color);
    let bg = cdbtn(cfg.text.bg_color);
    let ph = cdbtn([0.0,0.0,0.0,0.0]); ph.set_sensitive(false);
    color_row3(&g, "Text / Background", &fg, &bg, &ph, r);

    let save = move || {
        let mut c = Config::load();
        c.text.enabled = enabled.is_active();
        c.text.popup_style = popup.is_active();
        c.text.show_attachments = attach.is_active();
        c.text.message_limit = limit.value() as usize;
        c.text.popup_time = ptime.value() as u32;
        c.text.anchor = match anch_dd.selected() { 0=>Anchor::BottomLeft, 1=>Anchor::BottomRight, 2=>Anchor::TopLeft, _=>Anchor::TopRight };
        c.text.x = tx.value() as i32; c.text.y = ty.value() as i32;
        c.text.channel_id = ch.text().to_string();
        c.text.fg_color = rgba(&fg); c.text.bg_color = rgba(&bg);
        write_cfg(c);
    };
    (wrap(g), save)
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
    lbl.add_css_class("settings-row");
    let sp = SpinButton::new(Some(&Adjustment::new(val,min,max,1.0,10.0,0.0)),1.0,0);
    sp.set_width_request(110);
    g.attach(&lbl, 0, row, 3, 1);
    g.attach(&sp,  3, row, 1, 1);
    sp
}

fn tog(g: &Grid, label: &str, active: bool, row: i32) -> Switch {
    let lbl = Label::new(Some(label));
    lbl.set_halign(Align::Start); lbl.set_hexpand(true);
    lbl.add_css_class("settings-row");
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

fn color_row3(g: &Grid, label: &str,
              w1: &gtk4::ColorDialogButton,
              w2: &gtk4::ColorDialogButton,
              w3: &gtk4::ColorDialogButton,
              row: i32) {
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

fn write_cfg(c: Config) {
    match c.save() {
        Ok(_)  => println!("Saved → {:?}", Config::config_path()),
        Err(e) => eprintln!("Save error: {e}"),
    }
}
