use crate::config::Config;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A Discord user currently visible in a voice channel
#[derive(Debug, Clone)]
pub struct VoiceUser {
    pub user_id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    /// Is the microphone active right now?
    pub speaking: bool,
    pub muted: bool,
    pub deafened: bool,
    /// When they last spoke (for fade-out timer)
    pub last_spoke: Option<Instant>,
    /// Decoded avatar, shared so cloning a user never copies pixels
    pub avatar_cache: Option<Arc<Avatar>>,
}

/// Avatar pixels already in cairo's premultiplied ARGB32 layout
#[derive(Debug)]
pub struct Avatar {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: usize,
}

impl VoiceUser {
    pub fn new(user_id: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            username: username.into(),
            avatar_url: None,
            speaking: false,
            muted: false,
            deafened: false,
            last_spoke: None,
            avatar_cache: None,
        }
    }
}

/// A Discord text message
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TextMessage {
    pub author: String,
    pub content: String,
    pub timestamp: std::time::SystemTime,
    pub avatar_url: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Inner {
    pub config: Config,
    pub voice_users: HashMap<String, VoiceUser>,
    pub channel_name: Option<String>,
    pub guild_name: Option<String>,
    pub text_messages: VecDeque<TextMessage>,
    /// Overlay visibility toggle
    pub visible: bool,
    /// Connected to Discord RPC?
    pub connected: bool,
    /// Our own user id (so we can highlight ourselves)
    pub self_user_id: Option<String>,
    /// User ids whose avatar download is in flight (avoids duplicate fetches)
    pub avatar_pending: HashSet<String>,
}

impl Inner {
    fn new(config: Config) -> Self {
        Self {
            config,
            voice_users: HashMap::new(),
            channel_name: None,
            guild_name: None,
            text_messages: VecDeque::new(),
            visible: true,
            connected: false,
            self_user_id: None,
            avatar_pending: HashSet::new(),
        }
    }
}

/// Cheaply-cloneable shared state used across all threads
pub type SharedState = Arc<Mutex<Inner>>;

pub struct AppState;

impl AppState {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(config: Config) -> SharedState {
        Arc::new(Mutex::new(Inner::new(config)))
    }
}
