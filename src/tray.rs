//! System tray via D-Bus StatusNotifierItem (KDE, GNOME+AppIndicator, XFCE).
//! Uses zbus for D-Bus communication — zero external tray dependencies.
//! Falls back gracefully if D-Bus is unavailable.

use std::sync::{Arc, Mutex};
use tracing::{info, warn};

pub fn spawn() {
    info!("Starting system tray icon");
    std::thread::spawn(|| {
        // Try to register via ksni
        match run_tray() {
            Ok(_) => {}
            Err(e) => warn!("Tray icon unavailable: {e}"),
        }
    });
}

fn run_tray() -> Result<(), Box<dyn std::error::Error>> {
    let visible = Arc::new(Mutex::new(true));

    struct RdoTray {
        visible: Arc<Mutex<bool>>,
    }

    impl ksni::Tray for RdoTray {
        fn icon_name(&self) -> String {
        // Uses icon installed to /usr/share/icons or ~/.local/share/icons
        // Falls back to audio-headset if not installed
        "rust-discord-overlay".into()
    }
        fn title(&self) -> String { "Rust Discord Overlay".into() }
        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            let vis = *self.visible.lock().unwrap();
            vec![
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: "Settings…".into(),
                    activate: Box::new(|_: &mut Self| {
                        // Launch settings as a separate process — GTK can only run
                        // on the main thread, and the overlay already owns it.
                        let exe = std::env::current_exe()
                            .unwrap_or_else(|_| std::path::PathBuf::from("rust-discord-overlay"));
                        let _ = std::process::Command::new(exe)
                            .arg("configure")
                            .spawn();
                    }),
                    ..Default::default()
                }),
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: if vis { "Hide Overlay" } else { "Show Overlay" }.into(),
                    activate: Box::new(move |this: &mut Self| {
                        let mut v = this.visible.lock().unwrap();
                        *v = !*v;
                        let show = *v;
                        drop(v);
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all().build().unwrap();
                            let cmd = if show {
                                crate::cli::Command::Show
                            } else {
                                crate::cli::Command::Hide
                            };
                            let _ = rt.block_on(crate::ipc::send_command(cmd));
                        });
                    }),
                    ..Default::default()
                }),
                ksni::MenuItem::Separator,
                ksni::MenuItem::Standard(ksni::menu::StandardItem {
                    label: "Quit".into(),
                    activate: Box::new(|_: &mut Self| {
                        std::process::exit(0);
                    }),
                    ..Default::default()
                }),
            ]
        }
    }

    let tray = RdoTray { visible };
    let service = ksni::TrayService::new(tray);
    service.spawn();
    info!("Tray icon registered (StatusNotifierItem)");

    // Keep thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
