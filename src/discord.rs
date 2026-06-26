use crate::{
    cli::Command,
    state::{SharedState, TextMessage, VoiceUser},
};
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};
use tracing::{debug, info, warn};

const CLIENT_ID: &str = "207646673902501888";
const STREAMKIT_URL: &str = "https://streamkit.discord.com/overlay/token";

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RpcEvent {
    Connected,
    Disconnected,
    VoiceStateUpdate(VoiceUser),
    VoiceStateRemove(String),
    SpeakingStart(String),
    SpeakingStop(String),
    MessageCreate(TextMessage),
    Control(Command),
}

pub type EventTx = tokio::sync::broadcast::Sender<RpcEvent>;

// ── Entry point ───────────────────────────────────────────────────────────────

pub async fn run(state: SharedState, tx: EventTx) {
    let mut cached_token = load_cached_token();
    loop {
        match connect_rpc(&state, &tx, &mut cached_token).await {
            Ok(_) => info!("Discord RPC disconnesso"),
            Err(e) => warn!("Discord RPC errore: {e}"),
        }
        {
            let mut s = state.lock().unwrap();
            s.connected = false;
            s.voice_users.clear();
        }
        let _ = tx.send(RpcEvent::Disconnected);
        info!("Riconnessione tra 5 secondi...");
        sleep(Duration::from_secs(5)).await;
    }
}

// ── Connessione principale ────────────────────────────────────────────────────

async fn connect_rpc(
    state: &SharedState,
    tx: &EventTx,
    cached_token: &mut Option<String>,
) -> Result<()> {
    // Trova porta Discord
    let mut ws_opt = None;
    for port in 6463u16..=6472 {
        let url = format!("ws://127.0.0.1:{port}/?v=1&client_id={CLIENT_ID}");
        let mut req = url.into_client_request()?;
        req.headers_mut()
            .insert("Origin", HeaderValue::from_static("http://localhost:3000"));
        if let Ok((stream, _)) = connect_async_with_config(req, None, false).await {
            info!("Connesso a Discord RPC sulla porta {port}");
            ws_opt = Some(stream);
            break;
        }
    }
    let mut ws = ws_opt.ok_or_else(|| anyhow!("Discord non trovato (porte 6463-6472)"))?;

    // Aspetta READY
    loop {
        let v = recv_json(&mut ws).await?;
        if v["cmd"] == "DISPATCH" && v["evt"] == "READY" {
            info!("Discord READY");
            break;
        }
    }

    // Autenticazione
    if let Some(token) = cached_token.clone() {
        send(&mut ws, json!({
            "cmd": "AUTHENTICATE",
            "args": { "access_token": token },
            "nonce": "x"
        })).await?;
        let v = recv_json(&mut ws).await?;
        if v["cmd"] == "AUTHENTICATE" && v["evt"] != "ERROR" {
            info!("Autenticato (cache): {}", v["data"]["user"]["username"].as_str().unwrap_or("?"));
            state.lock().unwrap().self_user_id =
                Some(v["data"]["user"]["id"].as_str().unwrap_or("").to_string());
        } else {
            warn!("Token scaduto, riautorizzo...");
            *cached_token = None;
            authorize_full(&mut ws, cached_token, state).await?;
        }
    } else {
        authorize_full(&mut ws, cached_token, state).await?;
    }

    // Sottoscrizioni globali
    for evt in &[
        "VOICE_CHANNEL_SELECT",
        "VOICE_SETTINGS_UPDATE",
        "VOICE_CONNECTION_STATUS",
        "GUILD_CREATE",
        "CHANNEL_CREATE",
    ] {
        send(&mut ws, json!({
            "cmd": "SUBSCRIBE", "evt": evt, "args": {}, "nonce": evt
        })).await?;
    }

    // Chiedi canale corrente
    send(&mut ws, json!({
        "cmd": "GET_SELECTED_VOICE_CHANNEL", "args": {}, "nonce": "init"
    })).await?;

    state.lock().unwrap().connected = true;
    let _ = tx.send(RpcEvent::Connected);

    let mut ctrl_rx = tx.subscribe();
    let mut current_voice: Option<String> = None;

    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            handle_event(state, tx, &v, &mut current_voice, &mut ws).await;
                        }
                    }
                    Some(Ok(_)) => {}
                    _ => { warn!("WebSocket chiuso"); break; }
                }
            }
            ctrl = ctrl_rx.recv() => {
                if let Ok(RpcEvent::Control(cmd)) = ctrl {
                    let _ = send_control(&mut ws, &cmd).await;
                }
            }
        }
    }
    Ok(())
}

// ── Flusso autorizzazione completo ────────────────────────────────────────────

async fn authorize_full(
    ws: &mut (impl SinkExt<Message, Error = impl std::fmt::Debug>
              + StreamExt<Item = Result<Message, impl std::fmt::Debug>>
              + Unpin),
    cached_token: &mut Option<String>,
    state: &SharedState,
) -> Result<String> {
    send(ws, json!({
        "cmd": "AUTHORIZE",
        "args": {
            "client_id": CLIENT_ID,
            "scopes": ["rpc", "messages.read", "rpc.notifications.read"],
            "prompt": "none"
        },
        "nonce": "auth"
    })).await?;

    let code = loop {
        let v = recv_json(ws).await?;
        if v["cmd"] == "AUTHORIZE" {
            break v["data"]["code"]
                .as_str()
                .ok_or_else(|| anyhow!("AUTHORIZE rifiutato dall'utente"))?
                .to_string();
        }
    };

    info!("Auth code ottenuto, scambio con Streamkit...");
    let resp = reqwest::Client::new()
        .post(STREAMKIT_URL)
        .json(&json!({ "code": code }))
        .send().await?
        .json::<Value>().await?;

    let token = resp["access_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Nessun token da Streamkit: {resp}"))?
        .to_string();

    info!("Access token ottenuto");
    save_cached_token(&token);
    *cached_token = Some(token.clone());

    send(ws, json!({
        "cmd": "AUTHENTICATE",
        "args": { "access_token": token },
        "nonce": "x"
    })).await?;

    let v = loop {
        let v = recv_json(ws).await?;
        if v["cmd"] == "AUTHENTICATE" { break v; }
    };

    if v["evt"] == "ERROR" {
        return Err(anyhow!("AUTHENTICATE fallito: {}", v["data"]["message"]));
    }

    info!("Loggato come {}", v["data"]["user"]["username"].as_str().unwrap_or("?"));
    state.lock().unwrap().self_user_id =
        Some(v["data"]["user"]["id"].as_str().unwrap_or("").to_string());

    Ok(token)
}

// ── Gestore eventi ────────────────────────────────────────────────────────────

async fn handle_event(
    state: &SharedState,
    tx: &EventTx,
    v: &Value,
    current_voice: &mut Option<String>,
    ws: &mut (impl SinkExt<Message, Error = impl std::fmt::Debug> + Unpin),
) {
    let cmd  = v["cmd"].as_str().unwrap_or("");
    let evt  = v["evt"].as_str().unwrap_or("");
    let data = &v["data"];

    match (cmd, evt) {

        // Utente si sposta di canale
        ("DISPATCH", "VOICE_CHANNEL_SELECT") => {
            if let Some(old) = current_voice.take() {
                unsub_voice(ws, &old).await;
                state.lock().unwrap().voice_users.clear();
            }
            if let Some(ch_id) = data["channel_id"].as_str() {
                sub_voice(ws, ch_id).await;
                *current_voice = Some(ch_id.to_string());
                let _ = send(ws, json!({
                    "cmd": "GET_SELECTED_VOICE_CHANNEL", "args": {}, "nonce": "refresh"
                })).await;
            }
        }

        // Risposta a GET_SELECTED_VOICE_CHANNEL
        ("GET_SELECTED_VOICE_CHANNEL", _) => {
            if data.is_null() || data["id"].is_null() {
                return;
            }
            let ch_id   = data["id"].as_str().unwrap_or("").to_string();
            let ch_name = data["name"].as_str().unwrap_or("").to_string();
            info!("In canale: {ch_name} (id={ch_id})");

            if current_voice.as_deref() != Some(&ch_id) {
                sub_voice(ws, &ch_id).await;
                *current_voice = Some(ch_id.clone());
            }

            let icon_size = state.lock().unwrap().config.voice.icon_size;
            let mut to_fetch: Vec<(String, String)> = Vec::new();

            {
                let mut s = state.lock().unwrap();
                s.voice_users.clear();
                s.channel_name = Some(ch_name);

                if let Some(states) = data["voice_states"].as_array() {
                    if let Some(first) = states.first() {
                        debug!("voice_state sample: {first}");
                    }
                    for vs in states {
                        let user = parse_voice_user(vs);
                        info!("Utente: {} | avatar: {:?}", user.username, user.avatar_url);
                        if let Some(url) = &user.avatar_url {
                            to_fetch.push((user.user_id.clone(), url.clone()));
                        }
                        s.voice_users.insert(user.user_id.clone(), user);
                    }
                }
            }

            for (uid, url) in to_fetch {
                crate::avatar::fetch_avatar(state.clone(), tx.clone(), uid, url, icon_size);
            }
            let _ = tx.send(RpcEvent::VoiceStateUpdate(VoiceUser::new("", "")));
        }

        // Utente entra / aggiorna stato
        ("DISPATCH", "VOICE_STATE_CREATE") | ("DISPATCH", "VOICE_STATE_UPDATE") => {
            let user      = parse_voice_user(data);
            let uid       = user.user_id.clone();
            let avatar    = user.avatar_url.clone();
            let icon_size = state.lock().unwrap().config.voice.icon_size;

            let needs_avatar = state.lock().unwrap()
                .voice_users.get(&uid)
                .map(|u| u.avatar_cache.is_none())
                .unwrap_or(true);

            state.lock().unwrap().voice_users.insert(uid.clone(), user.clone());

            if needs_avatar {
                if let Some(url) = avatar {
                    crate::avatar::fetch_avatar(state.clone(), tx.clone(), uid, url, icon_size);
                }
            }
            let _ = tx.send(RpcEvent::VoiceStateUpdate(user));
        }

        // Utente lascia il canale
        ("DISPATCH", "VOICE_STATE_DELETE") => {
            if let Some(uid) = data["user"]["id"].as_str() {
                state.lock().unwrap().voice_users.remove(uid);
                let _ = tx.send(RpcEvent::VoiceStateRemove(uid.to_string()));
            }
        }

        // Inizia a parlare
        ("DISPATCH", "SPEAKING_START") => {
            if let Some(uid) = data["user_id"].as_str() {
                if let Some(u) = state.lock().unwrap().voice_users.get_mut(uid) {
                    u.speaking   = true;
                    u.last_spoke = Some(Instant::now());
                }
                let _ = tx.send(RpcEvent::SpeakingStart(uid.to_string()));
            }
        }

        // Smette di parlare
        ("DISPATCH", "SPEAKING_STOP") => {
            if let Some(uid) = data["user_id"].as_str() {
                if let Some(u) = state.lock().unwrap().voice_users.get_mut(uid) {
                    u.speaking = false;
                }
                let _ = tx.send(RpcEvent::SpeakingStop(uid.to_string()));
            }
        }

        // Messaggio testo
        ("DISPATCH", "MESSAGE_CREATE") => {
            let watched = state.lock().unwrap().config.text.channel_id.clone();
            if data["channel_id"].as_str() == Some(&watched) || watched.is_empty() {
                let msg = TextMessage {
                    author:    data["message"]["author"]["username"].as_str().unwrap_or("?").to_string(),
                    content:   data["message"]["content"].as_str().unwrap_or("").to_string(),
                    timestamp: std::time::SystemTime::now(),
                    avatar_url: None,
                };
                let mut s = state.lock().unwrap();
                let lim = s.config.text.message_limit;
                s.text_messages.push(msg.clone());
                if s.text_messages.len() > lim { s.text_messages.remove(0); }
                let _ = tx.send(RpcEvent::MessageCreate(msg));
            }
        }

        _ => { debug!("Evento non gestito: cmd={cmd} evt={evt}"); }
    }
}

// ── Parsing utente vocale ─────────────────────────────────────────────────────

fn parse_voice_user(vs: &Value) -> VoiceUser {
    let user_obj = &vs["user"];
    let uid = user_obj["id"].as_str().unwrap_or("").to_string();

    let name = vs["nick"].as_str().filter(|s| !s.is_empty())
        .or_else(|| user_obj["global_name"].as_str())
        .or_else(|| user_obj["username"].as_str())
        .unwrap_or("?")
        .to_string();

    let avatar_url = user_obj["avatar"].as_str()
        .filter(|s| !s.is_empty())
        .map(|hash| {
            let ext = if hash.starts_with("a_") { "gif" } else { "png" };
            format!("https://cdn.discordapp.com/avatars/{uid}/{hash}.{ext}?size=128")
        });

    let voice = &vs["voice_state"];
    let mut u = VoiceUser::new(uid, name);
    u.avatar_url = avatar_url;
    u.muted    = voice["mute"].as_bool().unwrap_or(false)
               || voice["self_mute"].as_bool().unwrap_or(false)
               || voice["suppress"].as_bool().unwrap_or(false);
    u.deafened = voice["deaf"].as_bool().unwrap_or(false)
               || voice["self_deaf"].as_bool().unwrap_or(false);
    u
}

// ── Helper sottoscrizioni voce ────────────────────────────────────────────────

async fn sub_voice(
    ws: &mut (impl SinkExt<Message, Error = impl std::fmt::Debug> + Unpin),
    ch_id: &str,
) {
    for e in &["VOICE_STATE_CREATE","VOICE_STATE_UPDATE","VOICE_STATE_DELETE","SPEAKING_START","SPEAKING_STOP"] {
        let _ = send(ws, json!({
            "cmd": "SUBSCRIBE", "evt": e,
            "args": { "channel_id": ch_id }, "nonce": e
        })).await;
    }
}

async fn unsub_voice(
    ws: &mut (impl SinkExt<Message, Error = impl std::fmt::Debug> + Unpin),
    ch_id: &str,
) {
    for e in &["VOICE_STATE_CREATE","VOICE_STATE_UPDATE","VOICE_STATE_DELETE","SPEAKING_START","SPEAKING_STOP"] {
        let _ = send(ws, json!({
            "cmd": "UNSUBSCRIBE", "evt": e,
            "args": { "channel_id": ch_id }, "nonce": e
        })).await;
    }
}

// ── Helper comandi controllo ──────────────────────────────────────────────────

async fn send_control(
    ws: &mut (impl SinkExt<Message, Error = impl std::fmt::Debug> + Unpin),
    cmd: &Command,
) -> Result<()> {
    let v = match cmd {
        Command::Mute   => json!({"cmd":"SET_VOICE_SETTINGS","args":{"mute":true},"nonce":"x"}),
        Command::Unmute => json!({"cmd":"SET_VOICE_SETTINGS","args":{"mute":false},"nonce":"x"}),
        Command::Deaf   => json!({"cmd":"SET_VOICE_SETTINGS","args":{"deaf":true},"nonce":"x"}),
        Command::Undeaf => json!({"cmd":"SET_VOICE_SETTINGS","args":{"deaf":false},"nonce":"x"}),
        Command::Leave  => json!({"cmd":"SELECT_VOICE_CHANNEL","args":{"channel_id":null},"nonce":"x"}),
        Command::MoveTo { channel_id } =>
            json!({"cmd":"SELECT_VOICE_CHANNEL","args":{"channel_id":channel_id,"force":true},"nonce":"x"}),
        _ => return Ok(()),
    };
    send(ws, v).await
}

// ── Helper WebSocket ──────────────────────────────────────────────────────────

async fn send(
    ws: &mut (impl SinkExt<Message, Error = impl std::fmt::Debug> + Unpin),
    v: Value,
) -> Result<()> {
    ws.send(Message::Text(v.to_string().into()))
        .await
        .map_err(|e| anyhow!("{e:?}"))
}

async fn recv_json(
    ws: &mut (impl StreamExt<Item = Result<Message, impl std::fmt::Debug>> + Unpin),
) -> Result<Value> {
    loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) =>
                return serde_json::from_str::<Value>(&t).map_err(|e| anyhow!("{e}")),
            Some(Ok(_)) => continue,
            _ => return Err(anyhow!("WebSocket chiuso")),
        }
    }
}

// ── Token cache ───────────────────────────────────────────────────────────────

fn token_path() -> std::path::PathBuf {
    crate::config::Config::config_dir().join("access_token")
}

fn load_cached_token() -> Option<String> {
    std::fs::read_to_string(token_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn save_cached_token(token: &str) {
    let p = token_path();
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    let _ = std::fs::write(p, token);
}
