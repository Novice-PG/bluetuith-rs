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
    fn label(&self) -> &'static str;
    fn is_demo(&self) -> bool;
}

/// A short human-readable UUID -> service name map for the UI.
pub fn service_name(uuid: &str, lang: crate::i18n::Lang) -> &'static str {
    use crate::i18n::Lang;
    let u = uuid.to_ascii_lowercase();
    let short = if u.len() == 36 {
        &u[4..8]
    } else {
        u.trim_start_matches("0x")
    };
    let zh = lang == Lang::Zh;
    match short {
        "1101" => {
            if zh {
                "串口"
            } else {
                "Serial Port"
            }
        }
        "1103" => {
            if zh {
                "拨号网络"
            } else {
                "Dial-up Networking"
            }
        }
        "1105" => {
            if zh {
                "OBEX 对象推送"
            } else {
                "OBEX Object Push"
            }
        }
        "1106" => {
            if zh {
                "OBEX 文件传输"
            } else {
                "OBEX File Transfer"
            }
        }
        "1108" => {
            if zh {
                "耳机"
            } else {
                "Headset"
            }
        }
        "110a" => {
            if zh {
                "音频源"
            } else {
                "Audio Source"
            }
        }
        "110b" => {
            if zh {
                "音频接收"
            } else {
                "Audio Sink"
            }
        }
        "110c" => {
            if zh {
                "A/V 远程目标"
            } else {
                "A/V Remote Target"
            }
        }
        "110d" => {
            if zh {
                "高级音频"
            } else {
                "Advanced Audio"
            }
        }
        "110e" => {
            if zh {
                "A/V 遥控"
            } else {
                "A/V Remote Control"
            }
        }
        "1112" => {
            if zh {
                "耳机音频网关"
            } else {
                "Headset Audio Gateway"
            }
        }
        "1115" => "PANU",
        "1116" => "NAP",
        "1117" => "GN",
        "111e" => {
            if zh {
                "免提"
            } else {
                "Handsfree"
            }
        }
        "111f" => {
            if zh {
                "免提音频网关"
            } else {
                "Handsfree Audio Gateway"
            }
        }
        "1124" | "1812" => {
            if zh {
                "人机接口设备"
            } else {
                "Human Interface Device"
            }
        }
        "1200" => {
            if zh {
                "即插即用信息"
            } else {
                "PnP Information"
            }
        }
        "1800" => {
            if zh {
                "通用访问"
            } else {
                "Generic Access"
            }
        }
        "1801" => {
            if zh {
                "通用属性"
            } else {
                "Generic Attribute"
            }
        }
        "180a" => {
            if zh {
                "设备信息"
            } else {
                "Device Information"
            }
        }
        "180f" => {
            if zh {
                "电池服务"
            } else {
                "Battery Service"
            }
        }
        _ => {
            if zh {
                "未知服务"
            } else {
                "Unknown service"
            }
        }
    }
}
