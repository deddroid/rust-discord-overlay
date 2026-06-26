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

    // Kill existing instance
    kill_existing_instance().await;

    info!("Starting Rust Discord Overlay (pid={})", std::process::id());

    // Become process group leader so children can be killed together
    unsafe {
        libc::setpgid(0, 0);
        // Auto-reap children so they don't become zombies
        // (settings subprocesses would otherwise stay as <defunct>)
        libc::signal(libc::SIGCHLD, libc::SIG_IGN);
    }

    ipc::write_pid();

    let cfg = config::Config::load();
    let audio_assist = cfg.audio_assist;
    let state = state::AppState::new(cfg);

    if let Err(e) = run(state, audio_assist).await {
        error!("Fatal: {e}");
        std::process::exit(1);
    }
}

async fn kill_existing_instance() {
    // Try IPC first
    let _ = ipc::send_command(Command::Close).await;
    // Kill entire process group of old instance
    if let Some(pid) = ipc::read_pid() {
        if pid != std::process::id() {
            unsafe { libc::kill(-(pid as i32), libc::SIGKILL); }
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
        }
    }
    ipc::remove_pid();
    // Also remove stale settings lock
    let _ = std::fs::remove_file(
        dirs::runtime_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("rust-discord-overlay-settings.lock")
    );
}

async fn run(state: state::SharedState, audio_assist: bool) -> Result<()> {
    let (tx, rx) = tokio::sync::broadcast::channel(64);

    tokio::spawn(discord::run(state.clone(), tx.clone()));
    tokio::spawn(ipc::serve(state.clone(), tx.clone()));

    if audio_assist {
        info!("PulseAudio/PipeWire audio assist enabled");
        tokio::spawn(audio::run(state.clone(), tx.clone()));
    }

    tray::spawn();
    overlay::run(state, rx).await;

    ipc::remove_pid();
    Ok(())
}
