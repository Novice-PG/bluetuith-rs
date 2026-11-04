//! A rich, deterministic simulation used by `--demo`, by the screenshot tests
//! and by anyone without a Bluetooth adapter. Every feature in the interface
//! is live here: toggles mutate state, pairing raises an auth request, file
//! transfers advance, the player ticks, networks and profiles answer.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

use super::*;

fn uuid(short: &str) -> String {
    format!("0000{short}-0000-1000-8000-00805f9b34fb")
}

#[allow(clippy::too_many_arguments)]
fn device(
    n: u8,
    name: &str,
    paired: bool,
    connected: bool,
    trusted: bool,
    blocked: bool,
    rssi: i16,
    battery: Option<u8>,
    services: &[&str],
) -> Device {
    Device {
        path: format!("/org/bluez/hci0/dev_{n:02X}_{n:02X}_{n:02X}_{n:02X}_{n:02X}_{n:02X}"),
        address: format!("AA:BB:CC:DD:{n:02X}:{n:02X}"),
        adapter: "/org/bluez/hci0".into(),
        name: name.into(),
        alias: String::new(),
        paired,
        connected,
        trusted,
        blocked,
        legacy_pairing: false,
        services_resolved: true,
        rssi: Some(rssi),
        tx_power: Some(4),
        battery,
        class: Some(0x240404),
        appearance: None,
        icon: String::new(),
        modalias: String::new(),
        manufacturer: "Demo Devices Ltd.".into(),
        uuids: services.iter().map(|s| uuid(s)).collect(),
    }
}

pub struct DemoBackend {
    adapters: Vec<Adapter>,
    devices: Vec<Device>,
    selected_adapter: usize,
    transfers: Vec<Transfer>,
    player: Option<PlayerState>,
    networks: Vec<Network>,
    profiles: Vec<ProfileInfo>,
    pending: VecDeque<Command>,
    events: VecDeque<Event>,
    auth: Option<AuthRequest>,
    auth_target: Option<String>,
    next_id: usize,
    tick: u64,
    dirty: bool,
    features: FeatureSet,
}

impl DemoBackend {
    pub fn new() -> Self {
        let adapters = vec![
            Adapter {
                path: "/org/bluez/hci0".into(),
                id: "hci0".into(),
                address: "00:1A:7D:DA:71:13".into(),
                name: "BlueZ 5.72".into(),
                alias: "Onboard Bluetooth".into(),
                powered: true,
                discoverable: false,
                pairable: true,
                discovering: false,
            },
            Adapter {
                path: "/org/bluez/hci1".into(),
                id: "hci1".into(),
                address: "00:1A:7D:DA:71:14".into(),
                name: "USB BT 5.3".into(),
                alias: "USB Dongle".into(),
                powered: false,
                discoverable: false,
                pairable: false,
                discovering: false,
            },
        ];
        let devices = vec![
            device(
                1,
                "WH-1000XM5",
                true,
                true,
                true,
                false,
                -44,
                Some(95),
                &["110b", "110c", "110e", "110d", "180f"],
            ),
            device(
                2,
                "Studio Buds+",
                true,
                true,
                true,
                false,
                -48,
                Some(82),
                &["110b", "110c", "110e", "180f"],
            ),
            device(
                3,
                "MX Master 3S",
                true,
                true,
                true,
                false,
                -62,
                Some(67),
                &["1812", "180f", "180a"],
            ),
            device(
                4,
                "Keychron K8 Pro",
                true,
                true,
                false,
                false,
                -55,
                Some(91),
                &["1812", "180f"],
            ),
            device(
                5,
                "JBL Flip 6",
                true,
                false,
                false,
                false,
                -66,
                Some(45),
                &["110b", "110c", "110e"],
            ),
            device(
                6,
                "Pixel 8 Pro",
                true,
                false,
                true,
                false,
                -70,
                None,
                &["1105", "1106", "1115", "1116", "1103"],
            ),
            device(
                7,
                "Galaxy Watch6",
                true,
                false,
                false,
                false,
                -73,
                Some(30),
                &["180f", "180a", "1812"],
            ),
            device(
                8,
                "Xbox Controller",
                true,
                false,
                false,
                false,
                -80,
                Some(58),
                &["1812", "180f"],
            ),
            device(
                9,
                "HP LaserJet Pro",
                true,
                false,
                false,
                true,
                -85,
                None,
                &["1105", "1106", "1200"],
            ),
            device(
                10,
                "Canon EOS R6",
                false,
                false,
                false,
                false,
                -58,
                None,
                &["1106", "1200"],
            ),
            device(
                11,
                "ThinkPad X1 Carbon",
                true,
                false,
                false,
                false,
                -90,
                None,
                &["1115", "1116", "1101"],
            ),
            device(
                12,
                "AirPods Pro",
                false,
                false,
                false,
                true,
                -76,
                None,
                &["110b", "110c", "110e"],
            ),
            device(
                13,
                "Linkr Bee Bridge",
                true,
                false,
                true,
                false,
                -64,
                Some(88),
                &["1115", "1116", "1103", "1101"],
            ),
            device(
                14,
                "Beats Fit Pro",
                false,
                false,
                false,
                false,
                -60,
                None,
                &["1108", "111e", "110b"],
            ),
        ];
        let transfers = vec![
            Transfer {
                id: 1,
                name: "holiday-photo.tar.gz".into(),
                device: "AA:BB:CC:DD:06:06".into(),
                device_name: "Pixel 8 Pro".into(),
                size: 24 * 1024 * 1024,
                transferred: 16 * 1024 * 1024,
                direction: Direction::Outgoing,
                status: TransferStatus::Active,
                error: String::new(),
                started_at: 0,
            },
            Transfer {
                id: 2,
                name: "firmware-v2.1.bin".into(),
                device: "AA:BB:CC:DD:0D:0D".into(),
                device_name: "Linkr Bee Bridge".into(),
                size: 3 * 1024 * 1024,
                transferred: 3 * 1024 * 1024,
                direction: Direction::Outgoing,
                status: TransferStatus::Complete,
                ..Default::default()
            },
            Transfer {
                id: 3,
                name: "scan.pdf".into(),
                device: "AA:BB:CC:DD:09:09".into(),
                device_name: "HP LaserJet Pro".into(),
                size: 8 * 1024 * 1024,
                transferred: 2 * 1024 * 1024,
                direction: Direction::Incoming,
                status: TransferStatus::Suspended,
                error: String::new(),
                started_at: 0,
            },
        ];
        let player = PlayerState {
            device: "AA:BB:CC:DD:01:01".into(),
            device_name: "WH-1000XM5".into(),
            status: PlayStatus::Playing,
            title: "Weightless".into(),
            artist: "Marconi Union".into(),
            album: "Ambient Transmissions Vol. 1".into(),
            genre: "Ambient".into(),
            track_number: 3,
            track_count: 11,
            duration_ms: 8 * 60 * 1000 + 9 * 1000,
            position_ms: 2 * 60 * 1000 + 41 * 1000,
            shuffle: false,
            repeat: RepeatMode::All,
            volume: 72,
            equalizer: "off".into(),
        };
        let networks = vec![Network {
            id: 1,
            device: "AA:BB:CC:DD:06:06".into(),
            device_name: "Pixel 8 Pro".into(),
            role: NetworkRole::Panu,
            connected: true,
            interface: "bnep0".into(),
            address: "192.168.44.3".into(),
        }];
        let profiles = vec![
            ProfileInfo {
                name: "A2DP Source".into(),
                uuid: uuid("110a"),
                description: "High-quality audio output".into(),
                active: true,
                device: "WH-1000XM5".into(),
            },
            ProfileInfo {
                name: "A2DP Sink".into(),
                uuid: uuid("110b"),
                description: "Receive audio stream".into(),
                active: false,
                device: "WH-1000XM5".into(),
            },
            ProfileInfo {
                name: "HFP".into(),
                uuid: uuid("111e"),
                description: "Hands-free calls".into(),
                active: true,
                device: "WH-1000XM5".into(),
            },
            ProfileInfo {
                name: "HSP".into(),
                uuid: uuid("1108"),
                description: "Headset profile".into(),
                active: false,
                device: "WH-1000XM5".into(),
            },
            ProfileInfo {
                name: "AVRCP".into(),
                uuid: uuid("110c"),
                description: "Remote media control".into(),
                active: true,
                device: "WH-1000XM5".into(),
            },
        ];

        let mut s = Self {
            adapters,
            devices,
            selected_adapter: 0,
            transfers,
            player: Some(player),
            networks,
            profiles,
            pending: VecDeque::new(),
            events: VecDeque::new(),
            auth: None,
            auth_target: None,
            next_id: 100,
            tick: 0,
            dirty: true,
            features: FeatureSet::all(),
        };
        s.events.push_back(Event::Features(s.features));
        s.events.push_back(Event::Transfers(s.transfers.clone()));
        s.events.push_back(Event::Player(s.player.clone()));
        s.events.push_back(Event::Networks(s.networks.clone()));
        s.events.push_back(Event::Profiles(s.profiles.clone()));
        s
    }

    fn adapter(&mut self) -> &mut Adapter {
        &mut self.adapters[self.selected_adapter]
    }

    fn find(&mut self, addr: &str) -> Option<&mut Device> {
        self.devices
            .iter_mut()
            .find(|d| d.address == addr || d.path == addr)
    }

    fn dev_name(&self, addr: &str) -> String {
        self.devices
            .iter()
            .find(|d| d.address == addr || d.path == addr)
            .map(|d| d.display_name())
            .unwrap_or_else(|| addr.to_string())
    }

    fn push_snapshot(&mut self) {
        self.events.push_back(Event::Snapshot {
            adapters: self.adapters.clone(),
            devices: self.devices.clone(),
        });
        self.dirty = false;
    }

    fn process(&mut self, cmd: Command) {
        use Command::*;
        match cmd {
            SelectAdapter(path) => {
                if let Some(i) = self
                    .adapters
                    .iter()
                    .position(|a| a.path == path || a.id == path || a.address == path)
                {
                    self.selected_adapter = i;
                    self.events.push_back(success(format!(
                        "Switched to {}",
                        self.adapters[i].display_name()
                    )));
                    self.dirty = true;
                }
            }
            SetAdapterProperty { property, value } => {
                let a = self.adapter();
                match property {
                    AdapterProperty::Powered => a.powered = value,
                    AdapterProperty::Discoverable => a.discoverable = value,
                    AdapterProperty::Pairable => a.pairable = value,
                }
                let label = match property {
                    AdapterProperty::Powered => "Powered",
                    AdapterProperty::Discoverable => "Discoverable",
                    AdapterProperty::Pairable => "Pairable",
                };
                self.events.push_back(success(format!(
                    "{} is {}",
                    label,
                    if value { "on" } else { "off" }
                )));
                self.dirty = true;
            }
            SetScan(on) => {
                self.adapter().discovering = on;
                self.events.push_back(if on {
                    busy("Scanning for devices…")
                } else {
                    info("Scanning stopped")
                });
                self.dirty = true;
            }
            Refresh => {
                self.events.push_back(info("Device list refreshed"));
                self.dirty = true;
            }
            ConnectDevice(ref addr) | DisconnectDevice(ref addr) => {
                let connect = matches!(cmd, ConnectDevice(_));
                let name = self.dev_name(addr);
                if let Some(d) = self.find(addr) {
                    d.connected = connect;
                }
                self.events.push_back(success(format!(
                    "{} {}",
                    if connect {
                        "Connected to"
                    } else {
                        "Disconnected from"
                    },
                    name
                )));
                self.dirty = true;
            }
            PairDevice(addr) => {
                if self.find(&addr).map(|d| d.paired).unwrap_or(false) {
                    self.events
                        .push_back(warn(format!("{} is already paired", self.dev_name(&addr))));
                    return;
                }
                self.auth_target = Some(addr.clone());
                let req = AuthRequest {
                    id: self.next_id,
                    device: addr.clone(),
                    device_name: self.dev_name(&addr),
                    kind: AuthKind::Confirmation(482731),
                };
                self.next_id += 1;
                self.auth = Some(req.clone());
                self.events.push_back(Event::AuthRequest(req));
            }
            CancelPairing(addr) => {
                self.events.push_back(info(format!(
                    "Cancelled pairing with {}",
                    self.dev_name(&addr)
                )));
                self.auth = None;
                self.auth_target = None;
            }
            TrustDevice(addr, value) => {
                if let Some(d) = self.find(&addr) {
                    d.trusted = value;
                }
                self.events.push_back(success(format!(
                    "{} is {}trusted",
                    self.dev_name(&addr),
                    if value { "" } else { "un" }
                )));
                self.dirty = true;
            }
            BlockDevice(addr, value) => {
                if let Some(d) = self.find(&addr) {
                    d.blocked = value;
                }
                self.events.push_back(success(format!(
                    "{} is {}blocked",
                    self.dev_name(&addr),
                    if value { "" } else { "un" }
                )));
                self.dirty = true;
            }
            RemoveDevice(addr) => {
                let name = self.dev_name(&addr);
                self.devices.retain(|d| d.address != addr && d.path != addr);
                self.events.push_back(success(format!("Removed {name}")));
                self.dirty = true;
            }
            SendFiles { device, files } => {
                let name = self.dev_name(&device);
                for f in files {
                    self.next_id += 1;
                    let size = std::fs::metadata(&f)
                        .map(|m| m.len())
                        .unwrap_or(4 * 1024 * 1024);
                    self.transfers.push(Transfer {
                        id: self.next_id,
                        name: f
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "file".into()),
                        device: device.clone(),
                        device_name: name.clone(),
                        size,
                        transferred: 0,
                        direction: Direction::Outgoing,
                        status: TransferStatus::Active,
                        error: String::new(),
                        started_at: self.tick,
                    });
                }
                self.events.push_back(success(format!(
                    "Sending {} file(s) to {name}",
                    self.transfers.len()
                )));
                self.events
                    .push_back(Event::Transfers(self.transfers.clone()));
            }
            SuspendTransfer(id) | ResumeTransfer(id) | CancelTransfer(id) => {
                if let Some(t) = self.transfers.iter_mut().find(|t| t.id == id) {
                    t.status = match cmd {
                        SuspendTransfer(_) => TransferStatus::Suspended,
                        ResumeTransfer(_) => TransferStatus::Active,
                        CancelTransfer(_) => TransferStatus::Cancelled,
                        _ => t.status,
                    };
                }
                self.events
                    .push_back(Event::Transfers(self.transfers.clone()));
            }
            PlayerCommand(c) => {
                use crate::model::PlayerCommand as Cmd;
                if let Some(p) = self.player.as_mut() {
                    match c {
                        Cmd::TogglePlay => {
                            p.status = if p.status == PlayStatus::Playing {
                                PlayStatus::Paused
                            } else {
                                PlayStatus::Playing
                            }
                        }
                        Cmd::Play => p.status = PlayStatus::Playing,
                        Cmd::Pause => p.status = PlayStatus::Paused,
                        Cmd::Stop => {
                            p.status = PlayStatus::Stopped;
                            p.position_ms = 0;
                        }
                        Cmd::Next => {
                            p.track_number = (p.track_number % p.track_count) + 1;
                            p.title = "Midnight City".into();
                            p.artist = "M83".into();
                            p.position_ms = 0;
                        }
                        Cmd::Previous => {
                            p.track_number = if p.track_number <= 1 {
                                p.track_count
                            } else {
                                p.track_number - 1
                            };
                            p.title = "An Ending (Ascent)".into();
                            p.artist = "Brian Eno".into();
                            p.position_ms = 0;
                        }
                        Cmd::FastForward => {
                            p.position_ms = (p.position_ms + 10_000).min(p.duration_ms)
                        }
                        Cmd::Rewind => p.position_ms = p.position_ms.saturating_sub(10_000),
                        Cmd::VolumeUp => p.volume = (p.volume + 5).min(100),
                        Cmd::VolumeDown => p.volume = p.volume.saturating_sub(5),
                    }
                }
                self.events.push_back(Event::Player(self.player.clone()));
            }
            ShowPlayer(addr) => {
                let name = self.dev_name(&addr);
                if let Some(p) = self.player.as_mut() {
                    p.device = addr;
                    p.device_name = name.clone();
                }
                self.events.push_back(Event::Player(self.player.clone()));
                self.events.push_back(info(format!("Media player: {name}")));
            }
            HidePlayer => {
                self.events.push_back(info("Media player hidden"));
            }
            ConnectNetwork { device, role } => {
                let name = self.dev_name(&device);
                self.next_id += 1;
                self.networks.push(Network {
                    id: self.next_id,
                    device,
                    device_name: name.clone(),
                    role,
                    connected: true,
                    interface: format!("bnep{}", self.networks.len()),
                    address: "192.168.44.12".into(),
                });
                self.events
                    .push_back(Event::Networks(self.networks.clone()));
                self.events
                    .push_back(success(format!("Connected {name} as {}", role.label())));
            }
            DisconnectNetwork(id) => {
                if let Some(n) = self.networks.iter_mut().find(|n| n.id == id) {
                    n.connected = false;
                }
                self.events
                    .push_back(Event::Networks(self.networks.clone()));
            }
            SetAudioProfile { device, profile } => {
                let name = self.dev_name(&device);
                let mut found = false;
                for p in &mut self.profiles {
                    if p.name.eq_ignore_ascii_case(&profile) {
                        p.active = !p.active;
                        p.device = name.clone();
                        found = true;
                    }
                }
                self.events
                    .push_back(Event::Profiles(self.profiles.clone()));
                self.events.push_back(if found {
                    success(format!("Toggled {profile} for {name}"))
                } else {
                    warn(format!("Unknown profile {profile}"))
                });
            }
            RespondAuth { id, response } => {
                let accepted = !matches!(response, AuthResponse::Reject);
                self.auth = None;
                if let Some(addr) = self.auth_target.take() {
                    if accepted {
                        if let Some(d) = self.find(&addr) {
                            d.paired = true;
                        }
                        self.events
                            .push_back(success(format!("Paired with {}", self.dev_name(&addr))));
                    } else {
                        self.events.push_back(error(format!(
                            "Pairing with {} rejected",
                            self.dev_name(&addr)
                        )));
                    }
                    self.dirty = true;
                }
                let _ = id;
            }
            RefreshNetworks(addr) => {
                let _ = addr;
                self.events
                    .push_back(Event::Networks(self.networks.clone()));
            }
            RefreshProfiles(addr) => {
                let _ = addr;
                self.events
                    .push_back(Event::Profiles(self.profiles.clone()));
            }
        }
    }
}

impl Default for DemoBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for DemoBackend {
    fn dispatch(&mut self, cmd: Command) {
        self.pending.push_back(cmd);
    }

    fn drain(&mut self) -> Vec<Event> {
        self.tick += 1;
        let pending: Vec<Command> = self.pending.drain(..).collect();
        for cmd in pending {
            self.process(cmd);
        }

        // Live-ish behaviour: transfers advance, the player moves, RSSI jitters.
        let mut changed = false;
        for t in &mut self.transfers {
            if t.status == TransferStatus::Active {
                let step = (t.size / 60).max(16 * 1024);
                t.transferred = (t.transferred + step).min(t.size);
                if t.transferred >= t.size {
                    t.status = TransferStatus::Complete;
                }
                changed = true;
            }
        }
        if changed {
            self.events
                .push_back(Event::Transfers(self.transfers.clone()));
        }
        if let Some(p) = self.player.as_mut() {
            if p.status == PlayStatus::Playing {
                p.position_ms += 250;
                if p.position_ms >= p.duration_ms {
                    p.position_ms = 0;
                    p.track_number = (p.track_number % p.track_count) + 1;
                }
                if self.tick % 8 == 0 {
                    self.events.push_back(Event::Player(Some(p.clone())));
                }
            }
        }
        if self.tick % 8 == 0 {
            // A gentle RSSI wobble so the signal bars feel alive.
            let n = self.devices.len();
            if n > 0 {
                let i = (self.tick as usize / 8) % n;
                if let Some(r) = self.devices[i].rssi {
                    let d = ((self.tick as i16 % 5) - 2).max(-2);
                    self.devices[i].rssi = Some((r + d).clamp(-95, -30));
                }
                self.dirty = true;
            }
        }
        if self.dirty {
            self.push_snapshot();
        }
        self.events.drain(..).collect()
    }

    fn features(&self) -> FeatureSet {
        self.features
    }
    fn label(&self) -> &'static str {
        "demo"
    }
    fn is_demo(&self) -> bool {
        true
    }
}

/// Used by the file picker in demo mode so it has something to show.
pub fn demo_files(dir: &PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            out.push(e.path());
        }
    }
    out.sort();
    out
}

/// Kept here so the compiler checks `HashSet` is used consistently with the
/// selection state the UI mirrors.
#[allow(dead_code)]
fn _selection_type_hint(_: &HashSet<PathBuf>) {}
