//! Plain data. Nothing in here knows about a terminal, a widget, or D-Bus.
//!
//! The BlueZ backend fills these from `GetManagedObjects`, the demo backend
//! fills them with a fixed cast, and the UI only ever reads them.

use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Adapter {
    /// D-Bus object path, e.g. `/org/bluez/hci0`.
    pub path: String,
    /// Kernel name, e.g. `hci0`.
    pub id: String,
    pub address: String,
    pub name: String,
    pub alias: String,
    pub powered: bool,
    pub discoverable: bool,
    pub pairable: bool,
    pub discovering: bool,
}

impl Adapter {
    pub fn display_name(&self) -> String {
        if !self.alias.is_empty() {
            return self.alias.clone();
        }
        if !self.name.is_empty() {
            return self.name.clone();
        }
        if !self.id.is_empty() {
            return self.id.clone();
        }
        self.address.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Device {
    pub path: String,
    pub address: String,
    pub adapter: String,
    pub name: String,
    pub alias: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub blocked: bool,
    pub legacy_pairing: bool,
    pub services_resolved: bool,
    pub rssi: Option<i16>,
    pub tx_power: Option<i16>,
    pub battery: Option<u8>,
    pub class: Option<u32>,
    pub appearance: Option<u16>,
    pub icon: String,
    pub modalias: String,
    pub manufacturer: String,
    pub uuids: Vec<String>,
}

impl Device {
    pub fn display_name(&self) -> String {
        if !self.alias.is_empty() {
            return self.alias.clone();
        }
        if !self.name.is_empty() {
            return self.name.clone();
        }
        if !self.address.is_empty() {
            return self.address.clone();
        }
        "Unknown device".into()
    }

    pub fn kind(&self) -> DeviceKind {
        DeviceKind::from_device(self)
    }

    /// Coarse signal quality for the little bar chart.
    pub fn signal_bars(&self) -> u8 {
        match self.rssi {
            None => 0,
            Some(r) if r >= -50 => 5,
            Some(r) if r >= -60 => 4,
            Some(r) if r >= -70 => 3,
            Some(r) if r >= -80 => 2,
            Some(_) => 1,
        }
    }

    pub fn has_service(&self, short: &str) -> bool {
        // Service UUIDs are hexadecimal short forms (`110b`), not decimal.
        let n = u16::from_str_radix(short, 16).unwrap_or(0);
        let needle = format!("0000{n:04x}");
        self.uuids
            .iter()
            .any(|u| u.to_ascii_lowercase().contains(&needle))
            || self.uuids.iter().any(|u| u.eq_ignore_ascii_case(short))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeviceKind {
    Headphones,
    Speaker,
    Headset,
    Keyboard,
    Mouse,
    Phone,
    Computer,
    Watch,
    Gamepad,
    Printer,
    Camera,
    AudioCard,
    Network,
    #[default]
    Unknown,
}

impl DeviceKind {
    pub fn from_device(d: &Device) -> Self {
        let hay = format!("{} {} {} {}", d.name, d.alias, d.icon, d.modalias).to_ascii_lowercase();
        let uuids = d.uuids.join(" ").to_ascii_lowercase();

        let has = |short: &str| {
            let full = format!("0000{}-0000-1000-8000-00805f9b34fb", short);
            uuids.contains(&full) || uuids.contains(short)
        };

        if hay.contains("headphone") || hay.contains("headset") || has("1108") || has("110b") {
            if hay.contains("headset") || has("1108") || has("111e") {
                return Self::Headset;
            }
            return Self::Headphones;
        }
        if hay.contains("speaker") || has("110b") || has("110a") {
            return Self::Speaker;
        }
        if hay.contains("audio") && (has("110a") || has("110b")) {
            return Self::AudioCard;
        }
        if hay.contains("keyboard") || has("1812") && hay.contains("keyboard") {
            return Self::Keyboard;
        }
        if hay.contains("mouse") || hay.contains("mice") {
            return Self::Mouse;
        }
        if hay.contains("phone") || hay.contains("smartphone") || hay.contains("mobile") {
            return Self::Phone;
        }
        if hay.contains("watch") || hay.contains("wearable") {
            return Self::Watch;
        }
        if hay.contains("gamepad") || hay.contains("controller") || hay.contains("joystick") {
            return Self::Gamepad;
        }
        if hay.contains("printer") {
            return Self::Printer;
        }
        if hay.contains("camera") || hay.contains("imaging") {
            return Self::Camera;
        }
        if hay.contains("computer") || hay.contains("laptop") || hay.contains("desktop") {
            return Self::Computer;
        }
        if has("1115") || has("1116") || hay.contains("network") {
            return Self::Network;
        }
        // Fall back to the Bluetooth major device class when present.
        if let Some(class) = d.class {
            match (class >> 8) & 0x1f {
                0x01 => return Self::Computer,
                0x02 => return Self::Phone,
                0x04 => return Self::AudioCard,
                0x05 => return Self::Keyboard, // peripheral
                0x06 => return Self::Camera,
                0x07 => return Self::Printer,
                _ => {}
            }
        }
        if hay.contains("input") || has("1812") {
            return Self::Keyboard;
        }
        Self::Unknown
    }

    /// Only glyphs from the basic geometric / musical ranges: monospace fonts
    /// on real terminals rarely carry emoji or the media-control pictographs.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Headphones | Self::Headset => "♫",
            Self::Speaker => "◖",
            Self::AudioCard => "♪",
            Self::Keyboard => "▤",
            Self::Mouse => "◉",
            Self::Phone => "▯",
            Self::Computer => "▣",
            Self::Watch => "◷",
            Self::Gamepad => "◈",
            Self::Printer => "■",
            Self::Camera => "●",
            Self::Network => "↔",
            Self::Unknown => "◇",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Headphones => "Headphones",
            Self::Headset => "Headset",
            Self::Speaker => "Speaker",
            Self::AudioCard => "Audio",
            Self::Keyboard => "Keyboard",
            Self::Mouse => "Mouse",
            Self::Phone => "Phone",
            Self::Computer => "Computer",
            Self::Watch => "Watch",
            Self::Gamepad => "Gamepad",
            Self::Printer => "Printer",
            Self::Camera => "Camera",
            Self::Network => "Network",
            Self::Unknown => "Device",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
    ForwardSeek,
    ReverseSeek,
    Error,
}

impl fmt::Display for PlayStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Stopped => "Stopped",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::ForwardSeek => "Seeking ▸▸",
            Self::ReverseSeek => "Seeking ◂◂",
            Self::Error => "Error",
        };
        f.write_str(s)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    #[default]
    Off,
    Single,
    All,
    Group,
}

impl RepeatMode {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Off => "·",
            Self::Single => "1",
            Self::All => "∞",
            Self::Group => "G",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub device: String,
    pub device_name: String,
    pub status: PlayStatus,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub track_number: u32,
    pub track_count: u32,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub volume: u8,
    pub equalizer: String,
}

impl PlayerState {
    pub fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            0.0
        } else {
            (self.position_ms as f64 / self.duration_ms as f64).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerCommand {
    #[default]
    TogglePlay,
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    FastForward,
    Rewind,
    VolumeUp,
    VolumeDown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Outgoing,
    Incoming,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransferStatus {
    #[default]
    Active,
    Suspended,
    Complete,
    Error,
    Cancelled,
}

impl TransferStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Suspended => "Suspended",
            Self::Complete => "Done",
            Self::Error => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Transfer {
    pub id: usize,
    pub name: String,
    pub device: String,
    pub device_name: String,
    pub size: u64,
    pub transferred: u64,
    pub direction: Direction,
    pub status: TransferStatus,
    pub error: String,
    pub started_at: u64,
}

impl Transfer {
    pub fn progress(&self) -> f64 {
        if self.size == 0 {
            0.0
        } else {
            (self.transferred as f64 / self.size as f64).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkRole {
    #[default]
    Panu,
    Nap,
    Dun,
}

impl NetworkRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Panu => "PANU (client)",
            Self::Nap => "NAP (access point)",
            Self::Dun => "DUN (dial-up)",
        }
    }
    pub fn uuid(self) -> &'static str {
        match self {
            Self::Panu => "1115",
            Self::Nap => "1116",
            Self::Dun => "1103",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Network {
    pub id: usize,
    pub device: String,
    pub device_name: String,
    pub role: NetworkRole,
    pub connected: bool,
    pub interface: String,
    pub address: String,
}

#[derive(Clone, Debug, Default)]
pub struct ProfileInfo {
    pub name: String,
    pub uuid: String,
    pub description: String,
    pub active: bool,
    pub device: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AuthKind {
    #[default]
    PinCode,
    Passkey(u32),
    Confirmation(u32),
    Authorization,
    DisplayPinCode(String),
    DisplayPasskey(u32),
}

#[derive(Clone, Debug, Default)]
pub struct AuthRequest {
    pub id: usize,
    pub device: String,
    pub device_name: String,
    pub kind: AuthKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AuthResponse {
    #[default]
    Accept,
    Reject,
    PinCode(String),
    Passkey(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level {
    #[default]
    Info,
    Success,
    Warning,
    Error,
    Busy,
}

#[derive(Clone, Debug, Default)]
pub struct StatusMessage {
    pub level: Level,
    pub text: String,
    pub busy: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FeatureSet {
    pub adapter: bool,
    pub device: bool,
    pub send_file: bool,
    pub receive_file: bool,
    pub network: bool,
    pub media_player: bool,
    pub audio_profiles: bool,
    pub obex: bool,
}

impl FeatureSet {
    pub fn all() -> Self {
        Self {
            adapter: true,
            device: true,
            send_file: true,
            receive_file: true,
            network: true,
            media_player: true,
            audio_profiles: true,
            obex: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    Name,
    Address,
    Rssi,
    Battery,
    State,
    Kind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_kind_from_name_and_uuid() {
        let mut d = Device {
            name: "Studio Headphones".into(),
            ..Default::default()
        };
        assert_eq!(d.kind(), DeviceKind::Headphones);
        d = Device {
            uuids: vec!["00001812-0000-1000-8000-00805f9b34fb".into()],
            ..Default::default()
        };
        assert_eq!(d.kind(), DeviceKind::Keyboard);
    }

    #[test]
    fn signal_bars_track_rssi() {
        let mut d = Device {
            rssi: Some(-44),
            ..Default::default()
        };
        assert_eq!(d.signal_bars(), 5);
        d.rssi = Some(-85);
        assert_eq!(d.signal_bars(), 1);
        d.rssi = None;
        assert_eq!(d.signal_bars(), 0);
    }

    #[test]
    fn service_lookup_matches_short_uuid() {
        let d = Device {
            uuids: vec!["0000110b-0000-1000-8000-00805f9b34fb".into()],
            ..Default::default()
        };
        assert!(d.has_service("110b"));
        assert!(!d.has_service("1103"));
    }
}
