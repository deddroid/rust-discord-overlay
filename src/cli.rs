use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "rusto-discord-overlay", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(short, long)]
    pub debug: bool,
}

#[derive(Subcommand, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Ferma il daemon
    Close,
    /// Nascondi overlay (il processo rimane attivo)
    Hide,
    /// Mostra overlay
    Show,
    /// Apri finestra impostazioni
    Configure,
    /// Muta microfono
    Mute,
    /// Riattiva microfono
    Unmute,
    /// Disattiva audio
    Deaf,
    /// Riattiva audio
    Undeaf,
    /// Lascia il canale vocale
    Leave,
    /// Spostati in un canale per ID
    MoveTo { channel_id: String },
    /// Ricarica la configurazione dal file
    Reload,
    /// Scrivi la lista guild su channels.rpc
    RefreshGuilds,
    /// Scrivi la lista canali di una guild su channels.rpc
    GuildRequest { guild_id: String },
}
