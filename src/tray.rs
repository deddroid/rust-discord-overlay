use tracing::info;

#[derive(Debug)]
struct RdoTray;

impl ksni::Tray for RdoTray {
    fn icon_name(&self) -> String { "rust-discord-overlay".into() }
    fn title(&self) -> String { "Rust Discord Overlay".into() }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Settings…".into(),
                activate: Box::new(|_: &mut Self| {
                    let exe = std::env::current_exe()
                        .unwrap_or_else(|_| "rust-discord-overlay".into());
                    let _ = std::process::Command::new(exe)
                        .arg("configure")
                        .spawn();
                }),
                ..Default::default()
            }),
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Hide Overlay".into(),
                activate: Box::new(|_: &mut Self| send_ipc(crate::cli::Command::Hide)),
                ..Default::default()
            }),
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Show Overlay".into(),
                activate: Box::new(|_: &mut Self| send_ipc(crate::cli::Command::Show)),
                ..Default::default()
            }),
            ksni::MenuItem::Separator,
            ksni::MenuItem::Standard(ksni::menu::StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_: &mut Self| {
                    // Kill the entire process group (daemon + pactl + any subprocesses)
                    // read_pid() gives us the daemon PID which is the group leader
                    if let Some(pid) = crate::ipc::read_pid() {
                        crate::ipc::remove_pid();
                        // Negative PID = kill entire process group
                        unsafe { libc::kill(-(pid as i32), libc::SIGKILL); }
                    }
                    // Also clean up settings lock if present
                    let _ = std::fs::remove_file(
                        dirs::runtime_dir()
                            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                            .join("rust-discord-overlay-settings.lock")
                    );
                    std::process::exit(0);
                }),
                ..Default::default()
            }),
        ]
    }
}

fn send_ipc(cmd: crate::cli::Command) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all().build().unwrap();
        let _ = rt.block_on(crate::ipc::send_command(cmd));
    });
}

pub fn spawn() {
    info!("Starting system tray");
    std::thread::spawn(|| {
        let service = ksni::TrayService::new(RdoTray);
        service.spawn();
        loop { std::thread::sleep(std::time::Duration::from_secs(60)); }
    });
}
