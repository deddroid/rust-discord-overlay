//! PulseAudio/PipeWire integration via pactl.
//! Monitors default source (mic) and sink (speakers) for mute changes.
//! Syncs mute state with Discord overlay.

use crate::discord::{EventTx, RpcEvent};
use crate::state::SharedState;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

pub async fn run(_state: SharedState, tx: EventTx) {
    info!("Audio assist started (PulseAudio/PipeWire)");

    let mut child = match Command::new("pactl")
        .args(["subscribe"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => { warn!("pactl not found: {e}"); return; }
    };

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    let mut last_mute: Option<bool> = None;
    let mut last_deaf: Option<bool> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        // Only care about source/sink changes
        if !line.contains("source") && !line.contains("sink") {
            continue;
        }

        // Read current state immediately (no debounce — last_* checks prevent duplicates)
        let mute = get_source_muted().await;
        let deaf  = get_sink_muted().await;

        // Deafen check first
        if deaf != last_deaf {
            last_deaf = deaf;
            if let Some(d) = deaf {
                let cmd = if d { crate::cli::Command::Deaf } else { crate::cli::Command::Undeaf };
                let _ = tx.send(RpcEvent::Control(cmd));
                info!("Audio: {}", if d { "deafen" } else { "undeafen" });
            }
            last_mute = None; // state undefined while deafen changes
            continue;
        }

        // Mute check (only if not deafen)
        if deaf != Some(true) && mute != last_mute {
            last_mute = mute;
            if let Some(m) = mute {
                let cmd = if m { crate::cli::Command::Mute } else { crate::cli::Command::Unmute };
                let _ = tx.send(RpcEvent::Control(cmd));
                info!("Audio: {}", if m { "mute" } else { "unmute" });
            }
        }
    }

    warn!("pactl subscribe ended");
    let _ = child.wait().await;
}

async fn get_source_muted() -> Option<bool> {
    pactl_get_mute("get-source-mute", "@DEFAULT_SOURCE@").await
}

async fn get_sink_muted() -> Option<bool> {
    pactl_get_mute("get-sink-mute", "@DEFAULT_SINK@").await
}

async fn pactl_get_mute(cmd: &str, device: &str) -> Option<bool> {
    let out = Command::new("pactl")
        .args([cmd, device])
        .output().await.ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("yes") { Some(true) }
    else if s.contains("no") { Some(false) }
    else { None }
}
