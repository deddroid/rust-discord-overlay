use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Anchor { TopLeft, TopRight, BottomLeft, BottomRight }
impl Default for Anchor { fn default() -> Self { Self::BottomLeft } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AvatarOrder { Alphabetical, Id, LastSpoken }
impl Default for AvatarOrder { fn default() -> Self { Self::LastSpoken } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceConfig {
    pub enabled: bool,
    pub anchor: Anchor,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    // Avatar
    pub show_avatar: bool,
    pub icon_size: u32,
    pub square_avatar: bool,
    pub fancy_border: bool,
    pub icon_transparency: f64,
    pub avatar_bg_color: [f64; 4],
    // Text / names
    pub show_names: bool,
    pub nick_length: u32,
    pub font: String,
    pub text_padding: i32,
    pub text_baseline_adj: i32,
    // Layout
    pub horizontal: bool,
    pub icon_spacing: i32,
    pub vert_edge_padding: i32,
    pub horz_edge_padding: i32,
    pub overflow: u32,          // 0=none 1=wrap 2=shrink
    pub order: AvatarOrder,
    // Visibility
    pub only_speaking: bool,
    pub only_speaking_grace: u32,
    pub highlight_self: bool,
    pub show_title: bool,
    pub show_connection: bool,
    pub show_disconnected: bool,
    pub fade_time: f64,
    // Fade-out inactive
    pub fade_out_inactive: bool,
    pub fade_out_limit: f64,
    pub inactive_time: u32,
    pub inactive_fade_time: u32,
    // Border
    pub border_width: u32,
    // Colors
    pub talking_color: [f64; 4],
    pub talking_bg_color: [f64; 4],
    pub talking_border_color: [f64; 4],
    pub idle_color: [f64; 4],
    pub idle_bg_color: [f64; 4],
    pub idle_border_color: [f64; 4],
    pub mute_color: [f64; 4],
    pub mute_bg_color: [f64; 4],
    pub bg_color: [f64; 4],
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            anchor: Anchor::BottomLeft,
            x: 0, y: 0, width: 300, height: 600,
            show_avatar: true,
            icon_size: 48,
            square_avatar: false,
            fancy_border: true,
            icon_transparency: 1.0,
            avatar_bg_color: [0.0, 0.0, 0.0, 0.0],
            show_names: true,
            nick_length: 32,
            font: "Sans 12".into(),
            text_padding: 6,
            text_baseline_adj: 0,
            horizontal: false,
            icon_spacing: 8,
            vert_edge_padding: 0,
            horz_edge_padding: 0,
            overflow: 0,
            order: AvatarOrder::LastSpoken,
            only_speaking: false,
            only_speaking_grace: 0,
            highlight_self: false,
            show_title: false,
            show_connection: false,
            show_disconnected: true,
            fade_time: 5.0,
            fade_out_inactive: false,
            fade_out_limit: 0.3,
            inactive_time: 10,
            inactive_fade_time: 30,
            border_width: 2,
            talking_color:        [1.0, 1.0, 1.0, 1.0],
            talking_bg_color:     [0.0, 0.0, 0.0, 0.5],
            talking_border_color: [0.0, 0.7, 0.0, 1.0],
            idle_color:           [1.0, 1.0, 1.0, 1.0],
            idle_bg_color:        [0.0, 0.0, 0.0, 0.5],
            idle_border_color:    [0.0, 0.0, 0.0, 0.0],
            mute_color:           [0.6, 0.0, 0.0, 1.0],
            mute_bg_color:        [0.0, 0.0, 0.0, 0.5],
            bg_color:             [0.0, 0.0, 0.0, 0.4],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextConfig {
    pub enabled: bool,
    pub anchor: Anchor,
    pub x: i32, pub y: i32, pub width: i32, pub height: i32,
    pub channel_id: String,
    pub font: String,
    pub fg_color: [f64; 4],
    pub bg_color: [f64; 4],
    pub message_limit: usize,
    pub popup_style: bool,
    pub popup_time: u32,
    pub show_attachments: bool,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            enabled: false, anchor: Anchor::TopRight,
            x: 0, y: 0, width: 400, height: 300,
            channel_id: String::new(),
            font: "Sans 12".into(),
            fg_color: [1.0,1.0,1.0,1.0],
            bg_color: [0.0,0.0,0.0,0.4],
            message_limit: 20,
            popup_style: false,
            popup_time: 30,
            show_attachments: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub voice: VoiceConfig,
    pub text: TextConfig,
    pub audio_assist: bool,
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("rust-discord-overlay")
    }
    pub fn config_path() -> PathBuf { Self::config_dir().join("config.toml") }
    pub fn load() -> Self {
        match std::fs::read_to_string(Self::config_path()) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                warn!("Config parse error ({e}), using defaults"); Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(Self::config_dir())?;
        std::fs::write(Self::config_path(), toml::to_string_pretty(self).expect("serialize"))
    }
}
