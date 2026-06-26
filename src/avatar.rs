//! Download e cache degli avatar Discord.
//! Scarica in background via reqwest e aggiorna lo stato quando pronto.

use crate::state::SharedState;
use image::imageops::FilterType;
use tracing::{debug, warn};

/// Avvia il download di un avatar in background.
/// Quando completo, aggiorna avatar_cache nello stato e manda un tick al broadcast.
pub fn fetch_avatar(
    state: SharedState,
    tx: crate::discord::EventTx,
    user_id: String,
    url: String,
    size: u32,
) {
    tokio::spawn(async move {
        match download_avatar(&url, size).await {
            Ok((pixels, w, h)) => {
                let mut s = state.lock().unwrap();
                if let Some(u) = s.voice_users.get_mut(&user_id) {
                    u.avatar_cache = Some(pixels);
                    u.avatar_size  = (w, h);
                }
                // Manda un evento vuoto per triggerare il ridisegno
                let _ = tx.send(crate::discord::RpcEvent::Connected);
            }
            Err(e) => warn!("Avatar download fallito per {user_id}: {e}"),
        }
    });
}

async fn download_avatar(url: &str, size: u32) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    debug!("Scarico avatar: {url}");
    let bytes = reqwest::get(url).await?.bytes().await?;
    let img = image::load_from_memory(&bytes)?
        .resize(size, size, FilterType::Lanczos3)
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok((img.into_raw(), w, h))
}
