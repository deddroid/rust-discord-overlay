//! PulseAudio/PipeWire integration via pactl.
//! Single pactl subscribe process, no respawn loop.

use crate::discord::{EventTx, RpcEvent};
use crate::state::SharedState;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info, warn};

pub async fn run(_state: SharedState, tx: EventTx) {
    info!("Audio assist started");

    let mut child = match Command::new("pactl")
        .args(["subscribe"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)  // Kill pactl when this task is dropped
        .spawn()
    {
        Ok(c) => c,
        Err(e) => { warn!("pactl not found: {e}"); return; }
    };

    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    let mut last_mute: Option<bool> = None;
    let mut last_deaf: Option<bool> = None;

    // Debounce: only check state at most every 300ms
    let mut last_check = tokio::time::Instant::now();

    while let Ok(Some(line)) = lines.next_line().await {
        debug!("pactl: {line}");

        if !line.contains("source") && !line.contains("sink") {
            continue;
        }

        // Debounce rapid events
        if last_check.elapsed().as_millis() < 300 {
            continue;
        }
        last_check = tokio::time::Instant::now();

        let mute = get_mute("@DEFAULT_SOURCE@").await;
        let deaf = get_mute("@DEFAULT_SINK@").await;

        if deaf != last_deaf {
            last_deaf = deaf;
            if let Some(d) = deaf {
                let cmd = if d { crate::cli::Command::Deaf } else { crate::cli::Command::Undeaf };
                let _ = tx.send(RpcEvent::Control(cmd));
            }
            last_mute = None;
            continue;
        }

        if deaf != Some(true) && mute != last_mute {
            last_mute = mute;
            if let Some(m) = mute {
                let cmd = if m { crate::cli::Command::Mute } else { crate::cli::Command::Unmute };
                let _ = tx.send(RpcEvent::Control(cmd));
            }
        }
    }

    warn!("pactl subscribe ended");
    let _ = child.wait().await;
}

async fn get_mute(device: &str) -> Option<bool> {
    let out = Command::new("pactl")
        .args(["get-source-mute", device])
        .output().await.ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("yes") { Some(true) } else if s.contains("no") { Some(false) } else { None }
}
