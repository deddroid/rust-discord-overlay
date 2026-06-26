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

pub fn pid_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("rust-discord-overlay.pid")
}

pub fn write_pid() {
    let _ = std::fs::write(pid_path(), std::process::id().to_string());
}

pub fn read_pid() -> Option<u32> {
    std::fs::read_to_string(pid_path())
        .ok()?
        .trim()
        .parse()
        .ok()
}

pub fn remove_pid() {
    let _ = std::fs::remove_file(pid_path());
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
    info!("IPC socket at {path:?}");

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
                info!("Close received — exiting");
                remove_pid();
                // Exit the entire process cleanly
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
                let new_cfg = crate::config::Config::load();
                state.lock().unwrap().config = new_cfg;
                let _ = tx.send(RpcEvent::Control(cmd));
            }
            other => { let _ = tx.send(RpcEvent::Control(other.clone())); }
        }
    }
}
