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
    /// Borrowed display name — avoids an allocation in sort comparators.
    pub fn display_name_ref(&self) -> &str {
        if !self.alias.is_empty() {
            &self.alias
        } else if !self.name.is_empty() {
            &self.name
        } else if !self.id.is_empty() {
            &self.id
        } else {
            &self.address
        }
    }

    pub fn display_name(&self) -> String {
        self.display_name_ref().to_string()
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
    /// Borrowed display name — avoids an allocation in sort comparators.
    pub fn display_name_ref(&self) -> &str {
        if !self.alias.is_empty() {
            &self.alias
        } else if !self.name.is_empty() {
            &self.name
        } else if !self.address.is_empty() {
            &self.address
        } else {
            "Unknown device"
        }
    }

    pub fn display_name(&self) -> String {
        self.display_name_ref().to_string()
    }

    pub fn kind(&self) -> DeviceKind {
        DeviceKind::from_device(self)
    }

    /// Coarse signal quality for the little bar chart.
    ///
    /// Returns 0..=4 so it matches the four glyphs of `util::signal_glyph`.
    /// RSSI is roughly -40 dBm (touching) down to -90 dBm (barely in range);
    /// the thresholds below spread the demo and real devices evenly across
    /// the four bars instead of pinning most of them at full strength.
    pub fn signal_bars(&self) -> u8 {
        match self.rssi {
            None => 0,
            Some(r) if r >= -55 => 4,
            Some(r) if r >= -67 => 3,
            Some(r) if r >= -78 => 2,
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

        // Product-name keywords are the most reliable signal, so they win
        // first. A generic Bluetooth Class of Device must never override an
        // obvious "keyboard"/"printer"/... name.
        //
        // Audio names MUST be checked before the generic "phone" keyword:
        // "headphone", "earphone", "speakerphone" and "microphone" all contain
        // the literal substring "phone", so a naive `contains("phone")` first
        // would mislabel every headset as a phone.
        if hay.contains("headset") {
            return Self::Headset;
        }
        if hay.contains("headphone")
            || hay.contains("earphone")
            || hay.contains("earbud")
            || hay.contains("airpod")
        {
            return Self::Headphones;
        }
        if hay.contains("speaker") {
            return Self::Speaker;
        }
        if hay.contains("mouse") || hay.contains("mice") {
            return Self::Mouse;
        }
        if hay.contains("keyboard") {
            return Self::Keyboard;
        }
        if hay.contains("gamepad") || hay.contains("controller") || hay.contains("joystick") {
            return Self::Gamepad;
        }
        if hay.contains("watch") || hay.contains("wearable") {
            return Self::Watch;
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
        if hay.contains("phone") || hay.contains("smartphone") || hay.contains("mobile") {
            return Self::Phone;
        }

        // With no name hint, fall back to service UUIDs. HSP/HFP (1108/
        // 111e/111f) are headsets; A2DP sink (110b) is headphones; A2DP
        // source (110a) with a generic "audio" name is an audio card.
        if has("1108") || has("111e") || has("111f") {
            return Self::Headset;
        }
        if has("110b") {
            return Self::Headphones;
        }
        if hay.contains("audio") && has("110a") {
            return Self::AudioCard;
        }

        if has("1115") || has("1116") || has("1103") || hay.contains("network") {
            return Self::Network;
        }
        // HID (0x1812) covers keyboards and mice. With no name hint, default
        // to keyboard — but do it *before* the class fallback, which is a much
        // weaker signal. Previously the class fallback ran first, so any HID
        // device carrying a generic audio class was mislabelled "Audio".
        if has("1812") || hay.contains("input") {
            return Self::Keyboard;
        }
        // Class of Device is a last resort. Major class 0x04 is the whole
        // "Audio/Video" family and far too broad to call AudioCard on its own,
        // so it is deliberately not mapped (audio devices are handled above).
        if let Some(class) = d.class {
            match (class >> 8) & 0x1f {
                0x01 => return Self::Computer,
                0x02 => return Self::Phone,
                0x05 => return Self::Keyboard, // peripheral
                0x06 => return Self::Camera,
                0x07 => return Self::Printer,
                _ => {}
            }
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

    pub fn label(self, lang: crate::i18n::Lang) -> &'static str {
        match (self, lang) {
            (Self::Headphones, crate::i18n::Lang::Zh) => "头戴耳机",
            (Self::Headset, crate::i18n::Lang::Zh) => "耳机",
            (Self::Speaker, crate::i18n::Lang::Zh) => "音箱",
            (Self::AudioCard, crate::i18n::Lang::Zh) => "声卡",
            (Self::Keyboard, crate::i18n::Lang::Zh) => "键盘",
            (Self::Mouse, crate::i18n::Lang::Zh) => "鼠标",
            (Self::Phone, crate::i18n::Lang::Zh) => "手机",
            (Self::Computer, crate::i18n::Lang::Zh) => "电脑",
            (Self::Watch, crate::i18n::Lang::Zh) => "手表",
            (Self::Gamepad, crate::i18n::Lang::Zh) => "手柄",
            (Self::Printer, crate::i18n::Lang::Zh) => "打印机",
            (Self::Camera, crate::i18n::Lang::Zh) => "相机",
            (Self::Network, crate::i18n::Lang::Zh) => "网络",
            (Self::Unknown, crate::i18n::Lang::Zh) => "设备",
            (Self::Headphones, _) => "Headphones",
            (Self::Headset, _) => "Headset",
            (Self::Speaker, _) => "Speaker",
            (Self::AudioCard, _) => "Audio",
            (Self::Keyboard, _) => "Keyboard",
            (Self::Mouse, _) => "Mouse",
            (Self::Phone, _) => "Phone",
            (Self::Computer, _) => "Computer",
            (Self::Watch, _) => "Watch",
            (Self::Gamepad, _) => "Gamepad",
            (Self::Printer, _) => "Printer",
            (Self::Camera, _) => "Camera",
            (Self::Network, _) => "Network",
            (Self::Unknown, _) => "Device",
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

impl PlayStatus {
    pub fn label(self, lang: crate::i18n::Lang) -> &'static str {
        let tr = lang.tr();
        match self {
            Self::Stopped => tr.p_stopped,
            Self::Playing => tr.p_playing,
            Self::Paused => tr.p_paused,
            Self::ForwardSeek => tr.p_seeking_fwd,
            Self::ReverseSeek => tr.p_seeking_rev,
            Self::Error => tr.p_error,
        }
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
    ToggleShuffle,
    CycleRepeat,
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
    pub fn label(self, lang: crate::i18n::Lang) -> &'static str {
        let tr = lang.tr();
        match self {
            Self::Active => tr.ts_active,
            Self::Suspended => tr.ts_suspended,
            Self::Complete => tr.ts_done,
            Self::Error => tr.ts_failed,
            Self::Cancelled => tr.ts_cancelled,
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
    /// Localised label for UI rendering (the plain `label()` stays English
    /// for backend status strings).
    pub fn label_for(self, lang: crate::i18n::Lang) -> &'static str {
        let tr = lang.tr();
        match self {
            Self::Panu => tr.nr_panu,
            Self::Nap => tr.nr_nap,
            Self::Dun => tr.nr_dun,
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
    /// An incoming OBEX Object Push; the payload is the suggested filename.
    FilePush(String),
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

impl SortKey {
    pub fn label(self, lang: crate::i18n::Lang) -> &'static str {
        let tr = lang.tr();
        match self {
            SortKey::Name => tr.sort_name,
            SortKey::Address => tr.sort_address,
            SortKey::Rssi => tr.sort_rssi,
            SortKey::Battery => tr.sort_battery,
            SortKey::State => tr.sort_state,
            SortKey::Kind => tr.sort_kind,
        }
    }
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
        assert_eq!(d.signal_bars(), 4);
        d.rssi = Some(-70);
        assert_eq!(d.signal_bars(), 2);
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

    #[test]
    fn hid_uuid_and_name_beat_a_generic_class_of_device() {
        // Regression: the class fallback used to run *before* the HID (1812)
        // fallback, so a keyboard carrying a generic Audio/Video class was
        // reported as an AudioCard. The `&& hay.contains("keyboard")` clause
        // was also dead code due to operator precedence.
        let keyboard = Device {
            name: "Keychron K8 Pro".into(),
            uuids: vec!["00001812-0000-1000-8000-00805f9b34fb".into()],
            class: Some(0x240404),
            ..Default::default()
        };
        assert_eq!(keyboard.kind(), DeviceKind::Keyboard);

        let named = Device {
            name: "Office Printer".into(),
            class: Some(0x240404),
            ..Default::default()
        };
        assert_eq!(named.kind(), DeviceKind::Printer);
    }

    #[test]
    fn a_broad_audio_class_alone_is_not_forced_to_audio() {
        let d = Device {
            name: "Generic gadget".into(),
            class: Some(0x240404),
            ..Default::default()
        };
        assert_eq!(d.kind(), DeviceKind::Unknown);
    }
}
