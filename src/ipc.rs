//! IPC via Unix socket at $XDG_RUNTIME_DIR/rust-discord-overlay.sock
//!
//! The daemon listens; other invocations with a sub-command connect,
//! send one JSON line, then exit.

use crate::{cli::Command, discord::RpcEvent, state::SharedState};
use anyhow::Result;
use std::path::PathBuf;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::{error, info};

fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("rust-discord-overlay.sock")
}

pub async fn send_command(cmd: Command) -> Result<()> {
    let mut stream = UnixStream::connect(socket_path()).await?;
    let line = serde_json::to_string(&cmd)? + "\n";
    stream.write_all(line.as_bytes()).await?;
    Ok(())
}

pub async fn serve(state: SharedState, tx: crate::discord::EventTx) {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => { error!("Cannot bind IPC socket: {e}"); return; }
    };
    info!("Rusto Discord Overlay — IPC socket at {path:?}");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle_client(stream, state.clone(), tx.clone()));
            }
            Err(e) => error!("IPC accept error: {e}"),
        }
    }
}

async fn handle_client(stream: UnixStream, state: SharedState, tx: crate::discord::EventTx) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let cmd: Command = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(_) => continue,
        };
        match &cmd {
            Command::Close => {
                info!("Close command received — shutting down");
                // Give a moment for the IPC response to be sent
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                std::process::exit(0);
            }
            Command::Hide => {
                state.lock().unwrap().visible = false;
                let _ = tx.send(RpcEvent::Control(cmd));
            }
            Command::Show => {
                state.lock().unwrap().visible = true;
                let _ = tx.send(RpcEvent::Control(cmd));
            }
            Command::Reload => {
                // Ricarica config dal file e aggiorna lo stato condiviso
                let new_cfg = crate::config::Config::load();
                state.lock().unwrap().config = new_cfg;
                let _ = tx.send(RpcEvent::Control(cmd));
            }
            other => { let _ = tx.send(RpcEvent::Control(other.clone())); }
        }
    }
}
