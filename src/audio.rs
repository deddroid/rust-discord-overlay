//! Integrazione PulseAudio/PipeWire via pactl.
//!
//! Funzionamento (come audio_assist.py originale):
//! - Monitora lo stato del microfono (source) e delle cuffie (sink) tramite pactl
//! - Quando Discord muta il mic → muta anche il source PulseAudio
//! - Quando PulseAudio muta il mic → invia mute a Discord via RPC
//!
//! Usa `pactl subscribe` per ascoltare gli eventi in tempo reale.

use crate::discord::{EventTx, RpcEvent};
use crate::state::SharedState;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Avvia il watcher audio in background.
/// Chiama questa funzione solo se `config.audio_assist == true`.
pub async fn run(state: SharedState, tx: EventTx) {
    info!("Audio assist avviato (PulseAudio/PipeWire via pactl)");

    // Ascolta eventi pactl subscribe
    let mut child = match Command::new("pactl")
        .args(["subscribe"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("pactl non trovato, audio assist disabilitato: {e}");
            return;
        }
    };

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    // Stato locale per evitare loop infiniti
    let mut last_mute: Option<bool> = None;
    let mut last_deaf: Option<bool> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        debug!("pactl: {line}");

        // pactl subscribe emette righe come:
        // "Event 'change' on source #0"
        // "Event 'change' on sink #0"
        if line.contains("source") || line.contains("sink") {
            let (mute, deaf) = get_pulse_state().await;

            // Deaf (cuffie mute) → invia DEAF a Discord
            if deaf != last_deaf {
                last_deaf = deaf;
                if let Some(d) = deaf {
                    let cmd = if d { crate::cli::Command::Deaf } else { crate::cli::Command::Undeaf };
                    let _ = tx.send(RpcEvent::Control(cmd));
                    info!("Audio assist → {}", if d {"Deaf"} else {"Undeaf"});
                }
                // Quando stiamo deafen, lo stato mute è indefinito
                last_mute = None;
                continue;
            }

            // Mute microfono → invia MUTE a Discord (solo se non deafen)
            if deaf != Some(true) {
                if mute != last_mute {
                    last_mute = mute;
                    if let Some(m) = mute {
                        let cmd = if m { crate::cli::Command::Mute } else { crate::cli::Command::Unmute };
                        let _ = tx.send(RpcEvent::Control(cmd));
                        info!("Audio assist → {}", if m {"Mute"} else {"Unmute"});
                    }
                }
            }
        }
    }
    warn!("pactl subscribe terminato");
}

/// Legge lo stato corrente di mute/deaf da pactl
async fn get_pulse_state() -> (Option<bool>, Option<bool>) {
    let mute = get_default_source_mute().await;
    let deaf  = get_default_sink_mute().await;
    (mute, deaf)
}

async fn get_default_source_mute() -> Option<bool> {
    let out = Command::new("pactl")
        .args(["get-source-mute", "@DEFAULT_SOURCE@"])
        .output().await.ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    // Output: "Mute: yes" o "Mute: no"
    if s.contains("yes") { Some(true) }
    else if s.contains("no") { Some(false) }
    else { None }
}

async fn get_default_sink_mute() -> Option<bool> {
    let out = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output().await.ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("yes") { Some(true) }
    else if s.contains("no") { Some(false) }
    else { None }
}

/// Muta/smuta il microfono di sistema (chiamato quando Discord manda MUTE)
pub async fn set_source_mute(muted: bool) {
    let val = if muted { "1" } else { "0" };
    let _ = Command::new("pactl")
        .args(["set-source-mute", "@DEFAULT_SOURCE@", val])
        .status().await;
}

/// Muta/smuta le cuffie di sistema (chiamato quando Discord manda DEAF)
pub async fn set_sink_mute(muted: bool) {
    let val = if muted { "1" } else { "0" };
    let _ = Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", val])
        .status().await;
}
