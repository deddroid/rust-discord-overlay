use crate::{config::{Anchor, AvatarOrder, Config}, discord::RpcEvent, state::{SharedState, VoiceUser}};
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

// ── Voice rendering ───────────────────────────────────────────────────────────

fn draw_voice(cr: &cairo::Context, state: &SharedState, cfg: &Config) {
    cr.set_operator(cairo::Operator::Clear);
    cr.paint().ok();
    cr.set_operator(cairo::Operator::Over);

    let vcfg = &cfg.voice;

    let mut users: Vec<VoiceUser> = {
        let s = state.lock().unwrap();
        if s.voice_users.is_empty() {
            if s.config.show_test_users { fake_users() } else { vec![] }
        } else {
            s.voice_users.values().cloned().collect()
        }
    };

    if users.is_empty() { return; }

    // Sort by user preference
    match vcfg.order {
        AvatarOrder::Alphabetical => {
            users.sort_by(|a, b| a.username.to_lowercase().cmp(&b.username.to_lowercase()));
        }
        AvatarOrder::Id => {
            users.sort_by(|a, b| a.user_id.cmp(&b.user_id));
        }
        AvatarOrder::LastSpoken => {
            users.sort_by(|a, b| {
                let ta = a.last_spoke.map(|t| t.elapsed().as_millis()).unwrap_or(u128::MAX);
                let tb = b.last_spoke.map(|t| t.elapsed().as_millis()).unwrap_or(u128::MAX);
                ta.cmp(&tb)
            });
        }
    }

    let icon    = vcfg.icon_size as f64;
    let ring_w  = (vcfg.border_width as f64).clamp(1.0, 12.0);
    let name_w  = if vcfg.show_names { 180.0 } else { 0.0 };
    let mut cursor = if vcfg.horizontal {
        vcfg.horz_edge_padding as f64
    } else {
        vcfg.vert_edge_padding as f64
    };

    for user in &users {
        let elapsed = user.last_spoke
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(f64::MAX);

        if vcfg.only_speaking && !user.speaking && elapsed > vcfg.fade_time {
            continue;
        }

        let fade_alpha: f64 = if user.speaking || !vcfg.only_speaking {
            1.0
        } else {
            ((vcfg.fade_time - elapsed) / vcfg.fade_time).clamp(0.0, 1.0)
        };
        let alpha = fade_alpha * vcfg.icon_transparency;

        // Pick colors based on speaking/mute state
        let (pill_bg, ring_col, txt_col) = if user.speaking {
            (vcfg.talking_bg_color, vcfg.talking_border_color, vcfg.talking_color)
        } else if user.muted || user.deafened {
            (vcfg.mute_bg_color, vcfg.idle_border_color, vcfg.mute_color)
        } else {
            (vcfg.idle_bg_color, vcfg.idle_border_color, vcfg.idle_color)
        };

        let av_r    = icon / 2.0;
        let pill_h  = icon + 4.0;

        let (cx, cy, pill_w, pill_x, pill_y): (f64,f64,f64,f64,f64);

        let hpad = vcfg.horz_edge_padding as f64;
        let vpad = vcfg.vert_edge_padding as f64;

        if vcfg.horizontal {
            let col_w = icon + ring_w * 2.0;  // no extra internal padding
            pill_w = col_w;
            pill_x = cursor; pill_y = vpad;
            cx = cursor + col_w / 2.0;
            cy = vpad + ring_w + av_r;
            cursor += col_w + vcfg.icon_spacing as f64;
        } else {
            pill_w = icon + 4.0 + name_w + ring_w * 2.0;
            pill_x = hpad; pill_y = cursor;
            cx = hpad + 2.0 + ring_w + av_r;
            cy = cursor + pill_h / 2.0;
            cursor += pill_h + vcfg.icon_spacing as f64;
        }

        // Background pill
        let [br, bg, bb, ba] = pill_bg;
        let pill_r = (pill_h / 2.0).min(pill_w / 2.0);
        rounded_rect(cr, pill_x, pill_y, pill_w, pill_h, pill_r);
        cr.set_source_rgba(br, bg, bb, ba * alpha);
        cr.fill().ok();

        // Ring around avatar
        let [rr, rg, rb, ra] = ring_col;
        cr.arc(cx, cy, av_r + ring_w * 0.5 + 1.0, 0.0, std::f64::consts::TAU);
        if user.speaking {
            cr.set_source_rgba(rr, rg, rb, ra * alpha);
            cr.set_line_width(ring_w);
        } else {
            cr.set_source_rgba(rr, rg, rb, ra * alpha * 0.4);
            cr.set_line_width((ring_w * 0.4).max(1.0));
        }
        cr.stroke().ok();

        // Avatar
        if vcfg.show_avatar {
            if let Some(px) = &user.avatar_cache {
                blit_avatar(cr, px, user.avatar_size.0, user.avatar_size.1,
                            cx, cy, av_r, alpha, vcfg.square_avatar);
            } else {
                let hue = user.user_id.bytes().fold(0u32, |a, b| a.wrapping_add(b as u32));
                let (pr, pg, pb) = hue_to_rgb(hue as f64 / 255.0);
                let [abr, abg, abb, aba] = vcfg.avatar_bg_color;
                cr.arc(cx, cy, av_r, 0.0, std::f64::consts::TAU);
                if aba > 0.01 {
                    cr.set_source_rgba(abr, abg, abb, aba * alpha);
                } else {
                    cr.set_source_rgba(pr * 0.6, pg * 0.6, pb * 0.6, alpha);
                }
                cr.fill().ok();
                let letter = user.username.chars().next()
                    .unwrap_or('?').to_uppercase().next().unwrap_or('?').to_string();
                cr.set_source_rgba(1.0, 1.0, 1.0, alpha * 0.9);
                cr.move_to(cx - icon * 0.14, cy + icon * 0.17);
                cr.show_text(&letter).ok();
            }
        }

        // Dark overlay on avatar when muted or deafened
        if vcfg.show_avatar && (user.muted || user.deafened) {
            cr.arc(cx, cy, av_r, 0.0, std::f64::consts::TAU);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.45 * alpha);
            cr.fill().ok();
        }

        // Status badges using mute_color from config
        let [mr, mg, mb, _] = vcfg.mute_color;
        if user.deafened {
            draw_badge(cr, cx + av_r * 0.65, cy - av_r * 0.65,
                       (icon * 0.16).clamp(5.0, 11.0), mr, mg, mb, alpha, false);
        }
        if user.muted {
            draw_badge(cr, cx + av_r * 0.65, cy + av_r * 0.65,
                       (icon * 0.16).clamp(5.0, 11.0), mr, mg, mb, alpha, true);
        }

        // Username
        if vcfg.show_names {
            let name: String = user.username.chars().take(vcfg.nick_length as usize).collect();
            let [fr, fg, fb, fa] = txt_col;
            if vcfg.horizontal {
                let ny = cy + av_r + ring_w + 12.0;
                cr.move_to(pill_x + pill_w / 2.0 - 18.0, ny);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.5 * alpha);
                cr.show_text(&name).ok();
                cr.move_to(pill_x + pill_w / 2.0 - 19.0, ny - 1.0);
                cr.set_source_rgba(fr, fg, fb, fa * alpha);
                cr.show_text(&name).ok();
            } else {
                let tx = cx + av_r + 4.0 + ring_w;
                cr.move_to(tx + 1.0, cy + 5.5);
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.5 * alpha);
                cr.show_text(&name).ok();
                cr.move_to(tx, cy + 5.0);
                cr.set_source_rgba(fr, fg, fb, fa * alpha);
                cr.show_text(&name).ok();
            }
        }
    }
}

fn draw_badge(cr: &cairo::Context, bx: f64, by: f64, r: f64,
              col_r: f64, col_g: f64, col_b: f64, alpha: f64, is_mute: bool) {
    cr.set_source_rgba(col_r, col_g, col_b, alpha);
    cr.arc(bx, by, r, 0.0, std::f64::consts::TAU);
    cr.fill().ok();
    cr.set_source_rgba(1.0, 1.0, 1.0, alpha);
    cr.set_line_width(r * 0.32);
    let d = r * 0.5;
    if is_mute {
        cr.move_to(bx - d, by - d); cr.line_to(bx + d, by + d); cr.stroke().ok();
        cr.move_to(bx + d, by - d); cr.line_to(bx - d, by + d); cr.stroke().ok();
    } else {
        cr.arc(bx, by + r * 0.1, r * 0.5, std::f64::consts::PI, 0.0); cr.stroke().ok();
        cr.move_to(bx - d, by - d); cr.line_to(bx + d, by + d); cr.stroke().ok();
    }
}

fn hue_to_rgb(h: f64) -> (f64, f64, f64) {
    let h = h * 6.0;
    let i = h as u32;
    let f = h - i as f64;
    match i % 6 {
        0 => (1.0, f, 0.0), 1 => (1.0-f, 1.0, 0.0),
        2 => (0.0, 1.0, f), 3 => (0.0, 1.0-f, 1.0),
        4 => (f, 0.0, 1.0), _ => (1.0, 0.0, 1.0-f),
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

fn blit_avatar(cr: &cairo::Context, pixels: &[u8], pw: u32, ph: u32,
               cx: f64, cy: f64, r: f64, alpha: f64, square: bool) {
    if pw == 0 || ph == 0 { return; }
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
