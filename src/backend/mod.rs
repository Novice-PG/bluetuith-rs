//! The backend boundary.
//!
//! The UI never talks to BlueZ (or to the simulator) directly: it sends
//! [`Command`]s and reacts to [`Event`]s. That keeps the whole interface
//! exercisable without Bluetooth hardware and makes the BlueZ implementation
//! replaceable.

pub mod bluez;
pub mod demo;

use std::path::PathBuf;

use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterProperty {
    Powered,
    Discoverable,
    Pairable,
}

#[derive(Clone, Debug)]
pub enum Command {
    SelectAdapter(String),
    SetAdapterProperty {
        property: AdapterProperty,
        value: bool,
    },
    SetScan(bool),
    Refresh,
    ConnectDevice(String),
    DisconnectDevice(String),
    PairDevice(String),
    CancelPairing(String),
    TrustDevice(String, bool),
    BlockDevice(String, bool),
    RemoveDevice(String),
    SendFiles {
        device: String,
        files: Vec<PathBuf>,
    },
    SuspendTransfer(usize),
    ResumeTransfer(usize),
    CancelTransfer(usize),
    PlayerCommand(PlayerCommand),
    ShowPlayer(String),
    HidePlayer,
    ConnectNetwork {
        device: String,
        role: NetworkRole,
    },
    DisconnectNetwork(usize),
    SetAudioProfile {
        device: String,
        profile: String,
    },
    RespondAuth {
        id: usize,
        response: AuthResponse,
    },
    RefreshNetworks(String),
    RefreshProfiles(String),
}

#[derive(Clone, Debug)]
pub enum Event {
    /// Full replacement of adapters + devices (the BlueZ backend polls).
    Snapshot {
        adapters: Vec<Adapter>,
        devices: Vec<Device>,
    },
    Status(StatusMessage),
    Features(FeatureSet),
    Transfers(Vec<Transfer>),
    Player(Option<PlayerState>),
    Networks(Vec<Network>),
    Profiles(Vec<ProfileInfo>),
    AuthRequest(AuthRequest),
    Log(String),
}

pub fn info(text: impl Into<String>) -> Event {
    Event::Status(StatusMessage {
        level: Level::Info,
        text: text.into(),
        busy: false,
    })
}
pub fn success(text: impl Into<String>) -> Event {
    Event::Status(StatusMessage {
        level: Level::Success,
        text: text.into(),
        busy: false,
    })
}
pub fn warn(text: impl Into<String>) -> Event {
    Event::Status(StatusMessage {
        level: Level::Warning,
        text: text.into(),
        busy: false,
    })
}
pub fn error(text: impl Into<String>) -> Event {
    Event::Status(StatusMessage {
        level: Level::Error,
        text: text.into(),
        busy: false,
    })
}
pub fn busy(text: impl Into<String>) -> Event {
    Event::Status(StatusMessage {
        level: Level::Busy,
        text: text.into(),
        busy: true,
    })
}

pub trait Backend: Send {
    fn dispatch(&mut self, cmd: Command);
    /// Called once per UI tick; returns everything that happened since.
    fn drain(&mut self) -> Vec<Event>;
    fn features(&self) -> FeatureSet;
    fn label(&self) -> &'static str;
    fn is_demo(&self) -> bool;
}

/// A short human-readable UUID -> service name map for the UI.
pub fn service_name(uuid: &str) -> &'static str {
    let u = uuid.to_ascii_lowercase();
    let short = if u.len() == 36 {
        &u[4..8]
    } else {
        u.trim_start_matches("0x")
    };
    match short {
        "1101" => "Serial Port",
        "1103" => "Dial-up Networking",
        "1105" => "OBEX Object Push",
        "1106" => "OBEX File Transfer",
        "1108" => "Headset",
        "110a" => "Audio Source",
        "110b" => "Audio Sink",
        "110c" => "A/V Remote Target",
        "110d" => "Advanced Audio",
        "110e" => "A/V Remote Control",
        "1112" => "Headset Audio Gateway",
        "1115" => "PANU",
        "1116" => "NAP",
        "1117" => "GN",
        "111e" => "Handsfree",
        "111f" => "Handsfree Audio Gateway",
        "1124" => "Human Interface Device",
        "1200" => "PnP Information",
        "1800" => "Generic Access",
        "1801" => "Generic Attribute",
        "180a" => "Device Information",
        "180f" => "Battery Service",
        "1812" => "Human Interface Device",
        _ => "Unknown service",
    }
}
