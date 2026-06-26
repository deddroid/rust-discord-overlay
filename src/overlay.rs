use crate::{config::{Anchor, Config}, discord::RpcEvent, state::{SharedState, VoiceUser}};
use gtk4::{cairo, glib, prelude::*, Application, ApplicationWindow, DrawingArea};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::time::Duration;
use tokio::sync::broadcast::Receiver;

const APP_ID: &str = "io.github.rust_discord_overlay";

fn fake_users() -> Vec<VoiceUser> {
    let mut u1 = VoiceUser::new("1", "Daniele"); u1.speaking = true;
    let mut u2 = VoiceUser::new("2", "Marco");   u2.muted = true;
    let mut u3 = VoiceUser::new("3", "Sara");    u3.deafened = true;
    vec![u1, u2, u3]
}

pub async fn run(state: SharedState, mut rx: Receiver<RpcEvent>) {
    let app = Application::builder().application_id(APP_ID).build();
    let state_c = state.clone();
    let (glib_tx, glib_rx) = async_channel::unbounded::<RpcEvent>();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(evt) => { if glib_tx.send(evt).await.is_err() { break; } }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(_) => continue,
            }
        }
    });

    app.connect_activate(move |app| {
        let cfg = state_c.lock().unwrap().config.clone();
        let voice_win = build_voice_window(app, &cfg, state_c.clone());
        let text_win  = cfg.text.enabled.then(|| build_text_window(app, state_c.clone()));

        let vw = voice_win.clone();
        let tw = text_win.clone();
        let st = state_c.clone();
        let rx2 = glib_rx.clone();

        glib::spawn_future_local(async move {
            while let Ok(evt) = rx2.recv().await {
                match &evt {
                    RpcEvent::Control(crate::cli::Command::Hide) => {
                        vw.set_visible(false);
                        tw.iter().for_each(|w| w.set_visible(false));
                    }
                    RpcEvent::Control(crate::cli::Command::Show) => {
                        vw.set_visible(true);
                        tw.iter().for_each(|w| w.set_visible(true));
                    }
                    RpcEvent::Control(crate::cli::Command::Reload) => {
                        let cfg = st.lock().unwrap().config.clone();
                        apply_anchor(&vw, cfg.voice.anchor, cfg.voice.x, cfg.voice.y);
                        re_fetch_avatars(&st);
                    }
                    _ => {}
                }
                vw.queue_draw();
                tw.iter().for_each(|w| w.queue_draw());
            }
        });
    });

    app.run();
}

fn re_fetch_avatars(state: &SharedState) {
    let (icon_size, users): (u32, Vec<(String, Option<String>)>) = {
        let mut s = state.lock().unwrap();
        let size = s.config.voice.icon_size;
        for u in s.voice_users.values_mut() {
            u.avatar_cache = None;
            u.avatar_size  = (0, 0);
        }
        let list = s.voice_users.values()
            .map(|u| (u.user_id.clone(), u.avatar_url.clone()))
            .collect();
        (size, list)
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    for (uid, url) in users {
        if let Some(url) = url {
            crate::avatar::fetch_avatar(state.clone(), tx.clone(), uid, url, icon_size);
        }
    }
}

fn set_passthrough(win: &ApplicationWindow) {
    if let Some(surface) = win.surface() {
        surface.set_input_region(Some(&cairo::Region::create()));
    }
}

// ── Voice window ──────────────────────────────────────────────────────────────

fn build_voice_window(app: &Application, cfg: &Config, state: SharedState) -> ApplicationWindow {
    let win = ApplicationWindow::builder()
        .application(app)
        .title("rust-discord-overlay-voice")
        .default_width(cfg.voice.width)
        .default_height(cfg.voice.height)
        .decorated(false)
        .build();

    let css = gtk4::CssProvider::new();
    css.load_from_string("window, * { background: transparent; }");
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    init_layer_shell(&win, cfg.voice.anchor, cfg.voice.x, cfg.voice.y);

    let draw = DrawingArea::new();
    {
        let st = state.clone();
        draw.set_draw_func(move |_da, cr, _w, _h| {
            let cfg_live = st.lock().unwrap().config.clone();
            draw_voice(cr, &st, &cfg_live);
        });
    }

    let d2 = draw.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        d2.queue_draw();
        glib::ControlFlow::Continue
    });

    win.set_child(Some(&draw));
    win.connect_realize(|w| set_passthrough(w));
    win.connect_map(|w| set_passthrough(w));
    win.set_visible(true);
    win
}

fn draw_voice(cr: &cairo::Context, state: &SharedState, cfg: &Config) {
    cr.set_operator(cairo::Operator::Clear);
    cr.paint().ok();
    cr.set_operator(cairo::Operator::Over);

    let vcfg = &cfg.voice;

    let users: Vec<VoiceUser> = {
        let s = state.lock().unwrap();
        if s.voice_users.is_empty() { fake_users() }
        else { s.voice_users.values().cloned().collect() }
    };

    if users.is_empty() { return; }

    let icon    = vcfg.icon_size as f64;
    let padding = 8.0;
    let ring_w  = (icon * 0.08).clamp(2.5, 6.0);
    let name_w  = if vcfg.show_names { 180.0 } else { 0.0 };

    // Horizontal: users side by side; Vertical: users stacked
    let mut cursor = padding; // x for horizontal, y for vertical

    for user in &users {
        let elapsed = user.last_spoke
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(f64::MAX);

        if vcfg.only_speaking && !user.speaking && elapsed > vcfg.fade_time {
            continue;
        }

        let alpha: f64 = if user.speaking || !vcfg.only_speaking {
            1.0
        } else {
            ((vcfg.fade_time - elapsed) / vcfg.fade_time).clamp(0.0, 1.0)
        };

        let (cx, cy, row_w, row_h, pill_x, pill_y): (f64,f64,f64,f64,f64,f64);

        if vcfg.horizontal {
            // Horizontal: fixed-width column per user (avatar only, names below)
            let col_w = icon + ring_w * 2.0 + padding * 2.0;
            let col_h = icon + ring_w * 2.0 + padding
                + if vcfg.show_names { 20.0 } else { 0.0 };
            row_w = col_w; row_h = col_h;
            pill_x = cursor; pill_y = 0.0;
            cx = cursor + col_w / 2.0;
            cy = padding + ring_w + icon / 2.0;
            cursor += col_w + vcfg.icon_spacing as f64;
        } else {
            // Vertical: full-width row per user
            row_h = icon + padding;
            row_w = icon + padding * 2.0 + name_w + ring_w * 2.0;
            pill_x = 0.0; pill_y = cursor;
            cx = padding + ring_w + icon / 2.0;
            cy = cursor + row_h / 2.0;
            cursor += row_h + vcfg.icon_spacing as f64;
        }

        let av_r = icon / 2.0;

        // Background pill
        let [br, bg, bb, ba] = vcfg.bg_color;
        rounded_rect(cr, pill_x, pill_y, row_w, row_h, icon / 2.0);
        cr.set_source_rgba(br, bg, bb, ba * alpha);
        cr.fill().ok();

        // Talking ring
        cr.arc(cx, cy, av_r + ring_w * 0.6, 0.0, std::f64::consts::TAU);
        if user.speaking {
            let [r,g,b,a] = vcfg.talking_border_color;
            cr.set_source_rgba(r, g, b, a * alpha);
            cr.set_line_width(ring_w);
        } else {
            let [r,g,b,a] = vcfg.idle_border_color;
            cr.set_source_rgba(r, g, b, a * alpha * 0.5);
            cr.set_line_width(ring_w * 0.4);
        }
        cr.stroke().ok();

        // Avatar
        if vcfg.show_avatar {
            cr.arc(cx, cy, av_r, 0.0, std::f64::consts::TAU);
            if let Some(px) = &user.avatar_cache {
                if vcfg.square_avatar {
                    draw_avatar_square(cr, px, user.avatar_size, cx, cy, av_r, alpha);
                } else {
                    draw_avatar_circle(cr, px, user.avatar_size, cx, cy, av_r, alpha);
                }
            } else {
                // Colored placeholder with initial
                let hue = user.user_id.bytes().fold(0u32, |a, b| a.wrapping_add(b as u32));
                let (pr, pg, pb) = hue_to_rgb(hue as f64 / 255.0);
                cr.set_source_rgba(pr * 0.7, pg * 0.7, pb * 0.7, alpha);
                cr.fill().ok();
                cr.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.9);
                let letter = user.username.chars().next()
                    .unwrap_or('?').to_uppercase().next().unwrap_or('?').to_string();
                cr.move_to(cx - icon * 0.15, cy + icon * 0.18);
                cr.show_text(&letter).ok();
            }
        }

        // Status badges — positioned so they don't overlap
        // Muted mic: bottom-right of avatar
        if user.muted && !user.deafened {
            let bx = cx + av_r * 0.65;
            let by = cy + av_r * 0.65;
            draw_badge_mic_muted(cr, bx, by, (icon * 0.18).clamp(6.0, 12.0), alpha);
        }
        // Deafened: top-right (headphone slash) + bottom-right implied mute
        if user.deafened {
            // Deafen badge top-right
            let bx = cx + av_r * 0.65;
            let by = cy - av_r * 0.65;
            draw_badge_deafened(cr, bx, by, (icon * 0.18).clamp(6.0, 12.0), alpha);
            // Also show mute badge bottom-right
            let bx2 = cx + av_r * 0.65;
            let by2 = cy + av_r * 0.65;
            draw_badge_mic_muted(cr, bx2, by2, (icon * 0.18).clamp(6.0, 12.0), alpha);
        }

        // Username
        if vcfg.show_names {
            let name: String = user.username.chars()
                .take(vcfg.nick_length as usize)
                .collect();
            let [fr, fg, fb, fa] = if user.speaking { vcfg.talking_color } else { vcfg.idle_color };
            if vcfg.horizontal {
                // Name below avatar, centered
                let name_y = cy + av_r + ring_w + 14.0;
                cr.move_to(pill_x + row_w / 2.0 - 20.0, name_y);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.6 * alpha);
                cr.show_text(&name).ok();
                cr.move_to(pill_x + row_w / 2.0 - 21.0, name_y - 1.0);
                cr.set_source_rgba(fr, fg, fb, fa * alpha);
                cr.show_text(&name).ok();
            } else {
                let tx = cx + av_r + padding + ring_w;
                cr.move_to(tx + 1.0, cy + 5.5);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.6 * alpha);
                cr.show_text(&name).ok();
                cr.move_to(tx, cy + 5.0);
                cr.set_source_rgba(fr, fg, fb, fa * alpha);
                cr.show_text(&name).ok();
            }
        }
    }
}

fn draw_badge_mic_muted(cr: &cairo::Context, cx: f64, cy: f64, r: f64, alpha: f64) {
    // Red circle
    cr.set_source_rgba(0.9, 0.15, 0.15, alpha);
    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
    cr.fill().ok();
    // White diagonal line (mic slash)
    cr.set_source_rgba(1.0, 1.0, 1.0, alpha);
    cr.set_line_width(r * 0.35);
    cr.move_to(cx - r * 0.55, cy - r * 0.55);
    cr.line_to(cx + r * 0.55, cy + r * 0.55);
    cr.stroke().ok();
}

fn draw_badge_deafened(cr: &cairo::Context, cx: f64, cy: f64, r: f64, alpha: f64) {
    // Dark circle
    cr.set_source_rgba(0.2, 0.2, 0.2, alpha * 0.85);
    cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
    cr.fill().ok();
    // White headphone arc
    cr.set_source_rgba(1.0, 1.0, 1.0, alpha);
    cr.set_line_width(r * 0.28);
    cr.arc(cx, cy + r * 0.1, r * 0.55, std::f64::consts::PI, 0.0);
    cr.stroke().ok();
    // Red diagonal slash
    cr.set_source_rgba(0.9, 0.15, 0.15, alpha);
    cr.set_line_width(r * 0.3);
    cr.move_to(cx - r * 0.6, cy - r * 0.6);
    cr.line_to(cx + r * 0.6, cy + r * 0.6);
    cr.stroke().ok();
}

fn hue_to_rgb(h: f64) -> (f64, f64, f64) {
    let h = h * 6.0;
    let i = h as u32;
    let f = h - i as f64;
    match i % 6 {
        0 => (1.0, f, 0.0),
        1 => (1.0-f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0-f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0-f),
    }
}

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(h / 2.0).min(w / 2.0);
    cr.new_sub_path();
    cr.arc(x+w-r, y+r,   r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x+w-r, y+h-r, r, 0.0,  std::f64::consts::FRAC_PI_2);
    cr.arc(x+r,   y+h-r, r, std::f64::consts::FRAC_PI_2,  std::f64::consts::PI);
    cr.arc(x+r,   y+r,   r, std::f64::consts::PI, -std::f64::consts::FRAC_PI_2);
    cr.close_path();
}

fn draw_avatar_circle(cr: &cairo::Context, pixels: &[u8], (pw, ph): (u32, u32),
                      cx: f64, cy: f64, r: f64, alpha: f64) {
    if pw == 0 || ph == 0 { return; }
    blit_avatar(cr, pixels, pw, ph, cx, cy, r, alpha, false);
}

fn draw_avatar_square(cr: &cairo::Context, pixels: &[u8], (pw, ph): (u32, u32),
                      cx: f64, cy: f64, r: f64, alpha: f64) {
    if pw == 0 || ph == 0 { return; }
    blit_avatar(cr, pixels, pw, ph, cx, cy, r, alpha, true);
}

fn blit_avatar(cr: &cairo::Context, pixels: &[u8], pw: u32, ph: u32,
               cx: f64, cy: f64, r: f64, alpha: f64, square: bool) {
    let stride = cairo::Format::ARgb32.stride_for_width(pw).unwrap_or(0) as usize;
    let mut argb = vec![0u8; ph as usize * stride];
    for row in 0..ph as usize {
        for col in 0..pw as usize {
            let src = (row * pw as usize + col) * 4;
            let dst =  row * stride + col * 4;
            if src+3 >= pixels.len() || dst+3 >= argb.len() { break; }
            let a  = (pixels[src+3] as f64 * alpha) as u8;
            let pm = |c: u8| ((c as u32 * a as u32) / 255) as u8;
            argb[dst+3]=a; argb[dst+2]=pm(pixels[src]);
            argb[dst+1]=pm(pixels[src+1]); argb[dst]=pm(pixels[src+2]);
        }
    }
    if let Ok(surf) = cairo::ImageSurface::create_for_data(
        argb, cairo::Format::ARgb32, pw as i32, ph as i32, stride as i32) {
        cr.save().ok();
        if square {
            cr.rectangle(cx-r, cy-r, r*2.0, r*2.0);
        } else {
            cr.arc(cx, cy, r, 0.0, std::f64::consts::TAU);
        }
        cr.clip();
        cr.set_source_surface(&surf, cx-r, cy-r).ok();
        cr.paint().ok();
        cr.restore().ok();
    }
}

// ── Text window ───────────────────────────────────────────────────────────────

fn build_text_window(app: &Application, state: SharedState) -> ApplicationWindow {
    let cfg = state.lock().unwrap().config.clone();
    let win = ApplicationWindow::builder()
        .application(app).title("rust-discord-overlay-text")
        .default_width(cfg.text.width).default_height(cfg.text.height)
        .decorated(false).build();
    init_layer_shell(&win, cfg.text.anchor, cfg.text.x, cfg.text.y);
    let draw = DrawingArea::new();
    draw.set_draw_func(move |_da, cr, _w, h| {
        let cfg_live = state.lock().unwrap().config.clone();
        let msgs: Vec<_> = state.lock().unwrap().text_messages.clone();
        cr.set_operator(cairo::Operator::Clear); cr.paint().ok();
        cr.set_operator(cairo::Operator::Over);
        if msgs.is_empty() { return; }
        let [br,bg,bb,ba] = cfg_live.text.bg_color;
        cr.set_source_rgba(br,bg,bb,ba); cr.paint().ok();
        let [fr,fg,fb,fa] = cfg_live.text.fg_color;
        cr.set_source_rgba(fr,fg,fb,fa);
        let mut y = h as f64 - 6.0;
        for msg in msgs.iter().rev() {
            y -= 18.0; if y < 0.0 { break; }
            cr.move_to(6.0, y);
            cr.show_text(&format!("{}: {}", msg.author, msg.content)).ok();
        }
    });
    win.set_child(Some(&draw));
    win.connect_realize(|w| set_passthrough(w));
    win.connect_map(|w| set_passthrough(w));
    win.set_visible(true);
    win
}

// ── Layer shell ───────────────────────────────────────────────────────────────

fn init_layer_shell(win: &ApplicationWindow, anchor: Anchor, x: i32, y: i32) {
    win.init_layer_shell();
    win.set_layer(Layer::Overlay);
    win.set_namespace(Some("rust-discord-overlay"));
    win.set_exclusive_zone(-1);
    win.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
    apply_anchor(win, anchor, x, y);
}

pub fn apply_anchor(win: &ApplicationWindow, anchor: Anchor, x: i32, y: i32) {
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        win.set_anchor(edge, false);
        win.set_margin(edge, 0);
    }
    match anchor {
        Anchor::BottomLeft  => { win.set_anchor(Edge::Bottom,true); win.set_anchor(Edge::Left,true);
                                  win.set_margin(Edge::Bottom,y);   win.set_margin(Edge::Left,x); }
        Anchor::BottomRight => { win.set_anchor(Edge::Bottom,true); win.set_anchor(Edge::Right,true);
                                  win.set_margin(Edge::Bottom,y);   win.set_margin(Edge::Right,x); }
        Anchor::TopLeft     => { win.set_anchor(Edge::Top,true);    win.set_anchor(Edge::Left,true);
                                  win.set_margin(Edge::Top,y);      win.set_margin(Edge::Left,x); }
        Anchor::TopRight    => { win.set_anchor(Edge::Top,true);    win.set_anchor(Edge::Right,true);
                                  win.set_margin(Edge::Top,y);      win.set_margin(Edge::Right,x); }
    }
}
