//! Download e cache degli avatar Discord.
//! Scarica in background via reqwest e aggiorna lo stato quando pronto.

use crate::state::{Avatar, SharedState};
use gtk4::cairo;
use image::imageops::FilterType;
use std::sync::{Arc, OnceLock};
use tracing::{debug, warn};

/// Client HTTP condiviso: riusa connessioni TLS e pool invece di crearne uno per avatar
pub fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Avvia il download di un avatar in background.
/// Quando completo, aggiorna avatar_cache nello stato e manda un tick al broadcast.
/// Se un download per lo stesso utente è già in corso non ne parte un altro.
pub fn fetch_avatar(
    state: SharedState,
    tx: crate::discord::EventTx,
    user_id: String,
    url: String,
    size: u32,
) {
    if !state.lock().unwrap().avatar_pending.insert(user_id.clone()) {
        return;
    }
    tokio::spawn(async move {
        let result = download_avatar(&url, size).await;
        let mut s = state.lock().unwrap();
        s.avatar_pending.remove(&user_id);
        match result {
            Ok(avatar) => {
                if let Some(u) = s.voice_users.get_mut(&user_id) {
                    u.avatar_cache = Some(Arc::new(avatar));
                }
                drop(s);
                // Manda un evento vuoto per triggerare il ridisegno
                let _ = tx.send(crate::discord::RpcEvent::Connected);
            }
            Err(e) => warn!("Avatar download fallito per {user_id}: {e}"),
        }
    });
}

async fn download_avatar(url: &str, size: u32) -> anyhow::Result<Avatar> {
    debug!("Scarico avatar: {url}");
    let bytes = http_client().get(url).send().await?.error_for_status()?.bytes().await?;
    // Decodifica e resize sono CPU-bound: fuori dai worker async
    tokio::task::spawn_blocking(move || decode_avatar(&bytes, size)).await?
}

fn decode_avatar(bytes: &[u8], size: u32) -> anyhow::Result<Avatar> {
    let img = image::load_from_memory(bytes)?
        .resize(size, size, FilterType::Lanczos3)
        .to_rgba8();
    let (width, height) = img.dimensions();
    let stride = cairo::Format::ARgb32.stride_for_width(width)? as usize;

    // RGBA → ARGB32 premoltiplicato (ordine byte little-endian: B G R A), una sola volta
    let mut data = vec![0u8; height as usize * stride];
    for (row, src_row) in img.as_raw().chunks_exact(width as usize * 4).enumerate() {
        let dst_row = &mut data[row * stride..row * stride + width as usize * 4];
        for (src, dst) in src_row.as_chunks::<4>().0.iter().zip(dst_row.as_chunks_mut::<4>().0) {
            let a = src[3] as u32;
            let pm = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            dst[0] = pm(src[2]);
            dst[1] = pm(src[1]);
            dst[2] = pm(src[0]);
            dst[3] = src[3];
        }
    }
    Ok(Avatar { data, width, height, stride })
}
