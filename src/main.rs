mod audio;
mod avatar;
mod cli;
mod config;
mod discord;
mod ipc;
mod overlay;
mod settings;
mod state;
mod tray;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Configure) => {
            settings::open_settings();
            return;
        }
        Some(cmd) => {
            if let Err(e) = ipc::send_command(cmd).await {
                error!("Cannot reach daemon: {e}");
                std::process::exit(1);
            }
            return;
        }
        None => {}
    }

    let _ = ipc::send_command(Command::Close).await;
    info!("Starting Rust Discord Overlay");

    let cfg = config::Config::load();
    let audio_assist = cfg.audio_assist;
    let state = state::AppState::new(cfg);

    if let Err(e) = run(state, audio_assist).await {
        error!("Fatal: {e}");
        std::process::exit(1);
    }
}

async fn run(state: state::SharedState, audio_assist: bool) -> Result<()> {
    let (tx, rx) = tokio::sync::broadcast::channel(64);

    tokio::spawn(discord::run(state.clone(), tx.clone()));
    tokio::spawn(ipc::serve(state.clone(), tx.clone()));

    if audio_assist {
        info!("PulseAudio/PipeWire audio assist enabled");
        tokio::spawn(audio::run(state.clone(), tx.clone()));
    }

    // Spawn system tray icon (background thread, non-blocking)
    tray::spawn();

    overlay::run(state, rx).await;
    Ok(())
}
