//! Real BlueZ backend.
//!
//! One thread owns a blocking D-Bus connection. It polls
//! `org.freedesktop.DBus.ObjectManager.GetManagedObjects` on `org.bluez`
//! (and on `org.bluez.obex` when present), turns the reply into our plain
//! model types, and executes commands through `org.bluez.*` methods. A
//! registered `org.bluez.Agent1` forwards pairing prompts to the UI and
//! blocks until the user answers, so the auth dialog is real, not decorative.
//!
//! Polling rather than signal subscriptions is deliberate: the TUI repaints
//! a few times a second anyway, and a full snapshot is both simpler and more
//! robust against BlueZ's many optional interfaces.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use async_channel::Sender as AsyncSender;
use zbus::blocking::Connection;
use zbus::fdo;
use zbus::interface;
use zbus::names::OwnedInterfaceName;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use super::*;
use crate::config::Config;
use crate::model::PlayerCommand as PlayerCmd;
use crate::model::*;

type ManagedObjects =
    HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>;

const AGENT_PATH: &str = "/org/bluez/agent/bluetuith_rs";

// ---------------------------------------------------------------------------
// Value helpers. BlueZ is inconsistent about integer widths across versions,
// so every numeric getter tries a few representations.

fn get<'a>(props: &'a HashMap<String, OwnedValue>, key: &str) -> Option<&'a OwnedValue> {
    props.get(key)
}

fn as_string(props: &HashMap<String, OwnedValue>, key: &str) -> String {
    get(props, key)
        .and_then(|v| v.downcast_ref::<String>().ok())
        .unwrap_or_default()
}

fn as_bool(props: &HashMap<String, OwnedValue>, key: &str) -> bool {
    get(props, key)
        .and_then(|v| v.downcast_ref::<bool>().ok())
        .unwrap_or(false)
}

fn as_i16(props: &HashMap<String, OwnedValue>, key: &str) -> Option<i16> {
    let v = get(props, key)?;
    if let Ok(n) = v.downcast_ref::<i16>() {
        return Some(n);
    }
    if let Ok(n) = v.downcast_ref::<i32>() {
        return Some(n as i16);
    }
    if let Ok(n) = v.downcast_ref::<u8>() {
        return Some(n as i16);
    }
    None
}

fn as_u8(props: &HashMap<String, OwnedValue>, key: &str) -> Option<u8> {
    let v = get(props, key)?;
    if let Ok(n) = v.downcast_ref::<u8>() {
        return Some(n);
    }
    if let Ok(n) = v.downcast_ref::<i32>() {
        return Some(n as u8);
    }
    None
}

fn as_u16(props: &HashMap<String, OwnedValue>, key: &str) -> Option<u16> {
    let v = get(props, key)?;
    if let Ok(n) = v.downcast_ref::<u16>() {
        return Some(n);
    }
    if let Ok(n) = v.downcast_ref::<u32>() {
        return Some(n as u16);
    }
    None
}

fn as_u32(props: &HashMap<String, OwnedValue>, key: &str) -> Option<u32> {
    let v = get(props, key)?;
    if let Ok(n) = v.downcast_ref::<u32>() {
        return Some(n);
    }
    if let Ok(n) = v.downcast_ref::<u64>() {
        return Some(n as u32);
    }
    None
}

fn as_u64(props: &HashMap<String, OwnedValue>, key: &str) -> Option<u64> {
    let v = get(props, key)?;
    if let Ok(n) = v.downcast_ref::<u64>() {
        return Some(n);
    }
    if let Ok(n) = v.downcast_ref::<u32>() {
        return Some(n as u64);
    }
    None
}

fn as_strings(props: &HashMap<String, OwnedValue>, key: &str) -> Vec<String> {
    let Some(v) = get(props, key) else {
        return Vec::new();
    };
    match &**v {
        Value::Array(a) => a
            .iter()
            .filter_map(|item| item.downcast_ref::<String>().ok())
            .collect(),
        _ => Vec::new(),
    }
}

fn parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(0) => "/".into(),
        Some(i) => path[..i].into(),
        None => path.into(),
    }
}

// ---------------------------------------------------------------------------
// Pairing agent

type AgentRequest = (AuthRequest, async_channel::Sender<AuthResponse>);

struct Agent {
    requests: AsyncSender<AgentRequest>,
    next_id: AtomicUsize,
}

impl Agent {
    fn request(&self, device: &str, kind: AuthKind) -> async_channel::Receiver<AuthResponse> {
        let (tx, rx) = async_channel::bounded(1);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let req = AuthRequest {
            id,
            device: device.into(),
            device_name: String::new(),
            kind,
        };
        let _ = self.requests.try_send((req, tx));
        rx
    }
}

#[interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn release(&self) {}

    async fn request_pin_code(&self, device: OwnedObjectPath) -> fdo::Result<String> {
        let rx = self.request(device.as_str(), AuthKind::PinCode);
        match rx.recv().await {
            Ok(AuthResponse::PinCode(pin)) => Ok(pin),
            _ => Err(fdo::Error::Failed("Pairing rejected by user".into())),
        }
    }

    async fn display_pin_code(&self, device: OwnedObjectPath, pincode: String) -> fdo::Result<()> {
        let _ = self.request(device.as_str(), AuthKind::DisplayPinCode(pincode));
        Ok(())
    }

    async fn request_passkey(&self, device: OwnedObjectPath) -> fdo::Result<u32> {
        let rx = self.request(device.as_str(), AuthKind::Passkey(0));
        match rx.recv().await {
            Ok(AuthResponse::Passkey(k)) => Ok(k),
            _ => Err(fdo::Error::Failed("Pairing rejected by user".into())),
        }
    }

    async fn display_passkey(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        _entered: u16,
    ) -> fdo::Result<()> {
        let _ = self.request(device.as_str(), AuthKind::DisplayPasskey(passkey));
        Ok(())
    }

    async fn request_confirmation(&self, device: OwnedObjectPath, passkey: u32) -> fdo::Result<()> {
        let rx = self.request(device.as_str(), AuthKind::Confirmation(passkey));
        match rx.recv().await {
            Ok(AuthResponse::Accept) => Ok(()),
            _ => Err(fdo::Error::Failed("Pairing rejected by user".into())),
        }
    }

    async fn request_authorization(&self, device: OwnedObjectPath) -> fdo::Result<()> {
        let rx = self.request(device.as_str(), AuthKind::Authorization);
        match rx.recv().await {
            Ok(AuthResponse::Accept) => Ok(()),
            _ => Err(fdo::Error::Failed("Pairing rejected by user".into())),
        }
    }

    async fn cancel(&self) {}
}

// ---------------------------------------------------------------------------
// Public handle

pub struct BluezBackend {
    cmd_tx: Sender<Command>,
    event_rx: Receiver<Event>,
    features: FeatureSet,
}

struct ThreadState {
    selected_adapter: String,
    pending_auth: HashMap<usize, async_channel::Sender<AuthResponse>>,
    obex_seen: bool,
    last_transfer_count: usize,
}

impl BluezBackend {
    pub fn new(cfg: &Config) -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (agent_tx, agent_rx) = async_channel::unbounded::<AgentRequest>();
        let cfg = cfg.clone();
        thread::Builder::new()
            .name("bluez".into())
            .spawn(move || run(cmd_rx, event_tx, agent_rx, agent_tx, cfg))?;

        Ok(Self {
            cmd_tx,
            event_rx,
            features: FeatureSet::all(),
        })
    }
}

impl Backend for BluezBackend {
    fn dispatch(&mut self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    fn drain(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(ev) = self.event_rx.try_recv() {
            out.push(ev);
        }
        out
    }

    fn features(&self) -> FeatureSet {
        self.features
    }
    fn label(&self) -> &'static str {
        "BlueZ"
    }
    fn is_demo(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// The worker

fn run(
    cmd_rx: Receiver<Command>,
    event_tx: Sender<Event>,
    agent_rx: async_channel::Receiver<AgentRequest>,
    agent_tx: AsyncSender<AgentRequest>,
    cfg: Config,
) {
    let conn = match Connection::system() {
        Ok(c) => c,
        Err(e) => {
            let _ = event_tx.send(error(format!("Cannot open system D-Bus: {e}")));
            let _ = event_tx.send(Event::Snapshot {
                adapters: vec![],
                devices: vec![],
            });
            let _ = event_tx.send(Event::Features(FeatureSet::default()));
            while cmd_rx.recv().is_ok() {
                let _ = event_tx.send(error("BlueZ is unavailable on this system"));
            }
            return;
        }
    };

    // Register the pairing agent (best effort: BlueZ may be old or absent).
    match register_agent(&conn, &agent_tx) {
        Ok(()) => {
            let _ = event_tx.send(super::info("Pairing agent registered"));
        }
        Err(e) => {
            let _ = event_tx.send(warn(format!("Pairing agent unavailable: {e}")));
        }
    }

    let _ = event_tx.send(Event::Features(FeatureSet::all()));
    let mut state = ThreadState {
        selected_adapter: String::new(),
        pending_auth: HashMap::new(),
        obex_seen: false,
        last_transfer_count: 0,
    };

    loop {
        // Pairing prompts raised by the agent.
        while let Ok((mut req, responder)) = agent_rx.try_recv() {
            req.device_name = name_of(&conn, &req.device);
            state.pending_auth.insert(req.id, responder);
            let _ = event_tx.send(Event::AuthRequest(req));
        }

        match cmd_rx.recv_timeout(Duration::from_millis(1200)) {
            Ok(cmd) => {
                handle_command(&conn, cmd, &event_tx, &mut state, &cfg);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        refresh(&conn, &event_tx, &mut state, &cfg);
    }
}

fn register_agent(conn: &Connection, agent_tx: &AsyncSender<AgentRequest>) -> Result<()> {
    let agent = Agent {
        requests: agent_tx.clone(),
        next_id: AtomicUsize::new(100),
    };
    conn.object_server().at(AGENT_PATH, agent)?;
    let path = OwnedObjectPath::try_from(AGENT_PATH)?;
    conn.call_method(
        Some("org.bluez"),
        "/org/bluez",
        Some("org.bluez.AgentManager1"),
        "RegisterAgent",
        &(path.clone(), "KeyboardDisplay"),
    )?;
    conn.call_method(
        Some("org.bluez"),
        "/org/bluez",
        Some("org.bluez.AgentManager1"),
        "RequestDefaultAgent",
        &(path,),
    )?;
    Ok(())
}

fn name_of(conn: &Connection, device_path: &str) -> String {
    let props = match get_properties(conn, device_path, "org.bluez.Device1") {
        Ok(p) => p,
        Err(_) => return device_path.rsplit('/').next().unwrap_or(device_path).into(),
    };
    let n = as_string(&props, "Alias");
    if !n.is_empty() {
        return n;
    }
    let n = as_string(&props, "Name");
    if !n.is_empty() {
        return n;
    }
    as_string(&props, "Address")
}

fn get_properties(
    conn: &Connection,
    path: &str,
    iface: &str,
) -> Result<HashMap<String, OwnedValue>> {
    let reply = conn.call_method(
        Some("org.bluez"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "GetAll",
        &(iface,),
    )?;
    Ok(reply
        .body()
        .deserialize::<(HashMap<String, OwnedValue>,)>()?
        .0)
}

fn managed_objects(conn: &Connection, service: &str) -> Result<ManagedObjects> {
    let reply = conn.call_method(
        Some(service),
        "/",
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    )?;
    Ok(reply.body().deserialize::<(ManagedObjects,)>()?.0)
}

fn set_property(
    conn: &Connection,
    path: &str,
    iface: &str,
    key: &str,
    value: Value<'_>,
) -> Result<()> {
    conn.call_method(
        Some("org.bluez"),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Set",
        &(iface, key, value),
    )?;
    Ok(())
}

fn method(conn: &Connection, path: &str, iface: &str, name: &str) -> Result<()> {
    conn.call_method(Some("org.bluez"), path, Some(iface), name, &())?;
    Ok(())
}

fn status_of(text: impl Into<String>, level: Level) -> Event {
    Event::Status(StatusMessage {
        level,
        text: text.into(),
        busy: level == Level::Busy,
    })
}

fn report<E: std::fmt::Display>(tx: &Sender<Event>, r: std::result::Result<(), E>, ok: &str) {
    let _ = tx.send(match r {
        Ok(()) => status_of(ok, Level::Success),
        Err(e) => status_of(format!("{e}"), Level::Error),
    });
}

fn handle_command(
    conn: &Connection,
    cmd: Command,
    tx: &Sender<Event>,
    state: &mut ThreadState,
    cfg: &Config,
) {
    use Command::*;

    match cmd {
        RespondAuth { id, response } => {
            if let Some(responder) = state.pending_auth.remove(&id) {
                let _ = responder.try_send(response);
            }
            return;
        }
        SelectAdapter(path) => {
            state.selected_adapter = path;
            let _ = tx.send(status_of("Adapter selected", Level::Info));
            return;
        }
        Refresh => return,
        _ => {}
    }

    let adapter = current_adapter(conn, state, cfg);
    match cmd {
        SetAdapterProperty { property, value } => {
            let key = match property {
                AdapterProperty::Powered => "Powered",
                AdapterProperty::Discoverable => "Discoverable",
                AdapterProperty::Pairable => "Pairable",
            };
            let r = set_property(
                conn,
                &adapter,
                "org.bluez.Adapter1",
                key,
                Value::Bool(value),
            );
            report(tx, r, &format!("{key} set to {value}"));
        }
        SetScan(on) => {
            let r = method(
                conn,
                &adapter,
                "org.bluez.Adapter1",
                if on {
                    "StartDiscovery"
                } else {
                    "StopDiscovery"
                },
            );
            report(
                tx,
                r,
                if on {
                    "Scanning for devices…"
                } else {
                    "Scanning stopped"
                },
            );
        }
        ConnectDevice(addr) => {
            let p = device_path(conn, &addr);
            let r = method(conn, &p, "org.bluez.Device1", "Connect");
            report(tx, r, "Connected");
        }
        DisconnectDevice(addr) => {
            let p = device_path(conn, &addr);
            let r = method(conn, &p, "org.bluez.Device1", "Disconnect");
            report(tx, r, "Disconnected");
        }
        PairDevice(addr) => {
            let p = device_path(conn, &addr);
            let r = method(conn, &p, "org.bluez.Device1", "Pair");
            report(tx, r, "Pairing complete");
        }
        CancelPairing(addr) => {
            let p = device_path(conn, &addr);
            let r = method(conn, &p, "org.bluez.Device1", "CancelPairing");
            report(tx, r, "Pairing cancelled");
        }
        TrustDevice(addr, value) => {
            let p = device_path(conn, &addr);
            let r = set_property(conn, &p, "org.bluez.Device1", "Trusted", Value::Bool(value));
            report(tx, r, &format!("Trusted = {value}"));
        }
        BlockDevice(addr, value) => {
            let p = device_path(conn, &addr);
            let r = set_property(conn, &p, "org.bluez.Device1", "Blocked", Value::Bool(value));
            report(tx, r, &format!("Blocked = {value}"));
        }
        RemoveDevice(addr) => {
            let p = device_path(conn, &addr);
            let op = OwnedObjectPath::try_from(p.as_str())
                .unwrap_or_else(|_| OwnedObjectPath::try_from("/").unwrap());
            let r = conn
                .call_method(
                    Some("org.bluez"),
                    adapter.as_str(),
                    Some("org.bluez.Adapter1"),
                    "RemoveDevice",
                    &(op,),
                )
                .map(|_| ());
            report(tx, r, "Device removed");
        }
        SendFiles { device, files } => {
            let dst = as_address(&device);
            let root = cfg.effective_receive_dir();
            let _ = std::fs::create_dir_all(&root);
            let mut ok = 0;
            for f in files {
                match obex_send_file(conn, &dst, &f) {
                    Ok(()) => ok += 1,
                    Err(e) => {
                        let _ = tx.send(status_of(format!("{}: {e}", f.display()), Level::Error));
                    }
                }
            }
            if ok > 0 {
                let _ = tx.send(status_of(
                    format!("Queued {ok} file(s) via OBEX Object Push"),
                    Level::Success,
                ));
            }
        }
        PlayerCommand(c) => {
            if let Some(p) = find_player_path(conn) {
                let name = match c {
                    PlayerCmd::TogglePlay | PlayerCmd::Play => Some("Play"),
                    PlayerCmd::Pause => Some("Pause"),
                    PlayerCmd::Stop => Some("Stop"),
                    PlayerCmd::Next => Some("Next"),
                    PlayerCmd::Previous => Some("Previous"),
                    PlayerCmd::FastForward => Some("FastForward"),
                    PlayerCmd::Rewind => Some("Rewind"),
                    PlayerCmd::VolumeUp | PlayerCmd::VolumeDown => None,
                };
                if let Some(name) = name {
                    let r = method(conn, &p, "org.bluez.MediaPlayer1", name);
                    report(tx, r, name);
                }
            } else {
                let _ = tx.send(status_of("No media player is available", Level::Warning));
            }
        }
        ShowPlayer(_) | HidePlayer => {}
        ConnectNetwork { device, role } => {
            let p = device_path(conn, &device);
            let r = conn
                .call_method(
                    Some("org.bluez"),
                    p.as_str(),
                    Some("org.bluez.Network1"),
                    "Connect",
                    &(role.uuid(),),
                )
                .map(|_| ());
            report(tx, r, &format!("Connected {} network", role.label()));
        }
        DisconnectNetwork(_id) => {
            let _ = tx.send(status_of(
                "Disconnect with `org.bluez.Network1.Disconnect` on the device",
                Level::Info,
            ));
        }
        SetAudioProfile { profile, .. } => {
            let _ = tx.send(status_of(
                format!("Switching {profile} needs bluez-alsa/PulseAudio; use your audio server"),
                Level::Info,
            ));
        }
        SuspendTransfer(id) | ResumeTransfer(id) | CancelTransfer(id) => {
            let _ = id;
            let _ = tx.send(status_of(
                "Transfer control is provided by obexd",
                Level::Info,
            ));
        }
        RefreshNetworks(_) | RefreshProfiles(_) => {}
        _ => {}
    }
}

fn current_adapter(conn: &Connection, state: &mut ThreadState, cfg: &Config) -> String {
    if !state.selected_adapter.is_empty() {
        return state.selected_adapter.clone();
    }
    let wanted = cfg.adapter.trim();
    if let Ok(objects) = managed_objects(conn, "org.bluez") {
        for (path, ifaces) in &objects {
            if !ifaces.keys().any(|i| i.as_str() == "org.bluez.Adapter1") {
                continue;
            }
            let props = &ifaces[&OwnedInterfaceName::try_from("org.bluez.Adapter1").unwrap()];
            let id = path.as_str().rsplit('/').next().unwrap_or("");
            if wanted.is_empty()
                || wanted == id
                || wanted.eq_ignore_ascii_case(&as_string(props, "Address"))
            {
                state.selected_adapter = path.to_string();
                return path.to_string();
            }
        }
    }
    "/org/bluez/hci0".into()
}

fn as_address(addr_or_path: &str) -> String {
    if addr_or_path.contains(':') {
        return addr_or_path.to_string();
    }
    // /org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF
    let tail = addr_or_path.rsplit('/').next().unwrap_or(addr_or_path);
    tail.trim_start_matches("dev_").replace('_', ":")
}

fn device_path(conn: &Connection, addr: &str) -> String {
    if addr.starts_with('/') {
        return addr.to_string();
    }
    let suffix = addr.replace(':', "_");
    if let Ok(objects) = managed_objects(conn, "org.bluez") {
        for (path, ifaces) in &objects {
            if ifaces.keys().any(|i| i.as_str() == "org.bluez.Device1")
                && path.as_str().ends_with(&format!("dev_{suffix}"))
            {
                return path.to_string();
            }
        }
    }
    format!("/org/bluez/hci0/dev_{suffix}")
}

fn find_player_path(conn: &Connection) -> Option<String> {
    let objects = managed_objects(conn, "org.bluez").ok()?;
    for (path, ifaces) in &objects {
        if ifaces
            .keys()
            .any(|i| i.as_str() == "org.bluez.MediaPlayer1")
        {
            return Some(path.to_string());
        }
    }
    None
}

fn obex_send_file(conn: &Connection, destination: &str, file: &std::path::Path) -> Result<()> {
    let mut args = HashMap::<&str, Value<'_>>::new();
    args.insert("Target", Value::from("OPP"));
    let reply = conn.call_method(
        Some("org.bluez.obex"),
        "/org/bluez/obex",
        Some("org.bluez.obex.Client1"),
        "CreateSession",
        &(destination, args),
    )?;
    let (session,): (OwnedObjectPath,) = reply.body().deserialize()?;
    let name = file.to_string_lossy().to_string();
    conn.call_method(
        Some("org.bluez.obex"),
        session.as_str(),
        Some("org.bluez.obex.ObjectPush1"),
        "SendFile",
        &(name,),
    )?;
    Ok(())
}

fn refresh(conn: &Connection, tx: &Sender<Event>, state: &mut ThreadState, cfg: &Config) {
    let objects = match managed_objects(conn, "org.bluez") {
        Ok(o) => o,
        Err(e) => {
            let _ = tx.send(status_of(format!("BlueZ: {e}"), Level::Error));
            return;
        }
    };

    let mut adapters = Vec::new();
    let mut devices = Vec::new();
    let mut player: Option<PlayerState> = None;

    for (path, ifaces) in &objects {
        let adapter_props =
            ifaces.get(&OwnedInterfaceName::try_from("org.bluez.Adapter1").unwrap());
        let device_props = ifaces.get(&OwnedInterfaceName::try_from("org.bluez.Device1").unwrap());
        let battery_props =
            ifaces.get(&OwnedInterfaceName::try_from("org.bluez.Battery1").unwrap());
        let player_props =
            ifaces.get(&OwnedInterfaceName::try_from("org.bluez.MediaPlayer1").unwrap());

        if let Some(p) = adapter_props {
            adapters.push(Adapter {
                path: path.to_string(),
                id: path.as_str().rsplit('/').next().unwrap_or("").into(),
                address: as_string(p, "Address"),
                name: as_string(p, "Name"),
                alias: as_string(p, "Alias"),
                powered: as_bool(p, "Powered"),
                discoverable: as_bool(p, "Discoverable"),
                pairable: as_bool(p, "Pairable"),
                discovering: as_bool(p, "Discovering"),
            });
        }

        if let Some(p) = device_props {
            devices.push(Device {
                path: path.to_string(),
                address: as_string(p, "Address"),
                adapter: as_string(p, "Adapter"),
                name: as_string(p, "Name"),
                alias: as_string(p, "Alias"),
                paired: as_bool(p, "Paired"),
                connected: as_bool(p, "Connected"),
                trusted: as_bool(p, "Trusted"),
                blocked: as_bool(p, "Blocked"),
                legacy_pairing: as_bool(p, "LegacyPairing"),
                services_resolved: as_bool(p, "ServicesResolved"),
                rssi: as_i16(p, "RSSI"),
                tx_power: as_i16(p, "TxPower"),
                battery: battery_props.and_then(|b| as_u8(b, "Percentage")),
                class: as_u32(p, "Class"),
                appearance: as_u16(p, "Appearance"),
                icon: as_string(p, "Icon"),
                modalias: as_string(p, "Modalias"),
                manufacturer: String::new(),
                uuids: as_strings(p, "UUIDs"),
            });
        }

        if let Some(p) = player_props {
            player = Some(parse_player(p, path.as_str(), &devices));
        }
    }

    let _ = tx.send(Event::Snapshot {
        adapters,
        devices: devices.clone(),
    });
    let _ = tx.send(Event::Player(player));

    // Networks: synthesise from connected devices that advertise PAN roles.
    let mut networks = Vec::new();
    for (i, d) in devices.iter().enumerate() {
        for (n, role) in [
            ("1115", NetworkRole::Panu),
            ("1116", NetworkRole::Nap),
            ("1103", NetworkRole::Dun),
        ] {
            if d.has_service(n) && d.connected {
                networks.push(Network {
                    id: i * 10 + networks.len(),
                    device: d.address.clone(),
                    device_name: d.display_name(),
                    role,
                    connected: true,
                    interface: format!("bnep{networks_len}", networks_len = networks.len()),
                    address: String::new(),
                });
            }
        }
    }
    let _ = tx.send(Event::Networks(networks));

    // Audio profiles from the selected device's advertised services.
    let selected = devices
        .iter()
        .find(|d| d.connected)
        .or_else(|| devices.first());
    let mut profiles = Vec::new();
    if let Some(d) = selected {
        for (short, name, desc) in [
            ("110a", "A2DP Source", "High-quality audio output"),
            ("110b", "A2DP Sink", "Receive audio stream"),
            ("1108", "HSP", "Headset profile"),
            ("111e", "HFP", "Hands-free calls"),
            ("110c", "AVRCP", "Remote media control"),
        ] {
            if d.has_service(short) {
                profiles.push(ProfileInfo {
                    name: name.into(),
                    uuid: super::service_name(short).to_string(),
                    description: desc.into(),
                    active: d.connected,
                    device: d.display_name(),
                });
            }
        }
    }
    let _ = tx.send(Event::Profiles(profiles));

    // OBEX transfers, when obexd exports an ObjectManager.
    if let Ok(obex) = managed_objects(conn, "org.bluez.obex") {
        state.obex_seen = true;
        let mut transfers = Vec::new();
        for (path, ifaces) in &obex {
            let Some(p) =
                ifaces.get(&OwnedInterfaceName::try_from("org.bluez.obex.Transfer1").unwrap())
            else {
                continue;
            };
            let status = as_string(p, "Status").to_ascii_lowercase();
            transfers.push(Transfer {
                id: path
                    .as_str()
                    .as_bytes()
                    .iter()
                    .map(|b| *b as usize)
                    .sum::<usize>()
                    % 100_000,
                name: as_string(p, "Filename"),
                device: as_string(p, "Session"),
                device_name: String::new(),
                size: as_u64(p, "Size").unwrap_or(0),
                transferred: as_u64(p, "Transferred").unwrap_or(0),
                direction: Direction::Outgoing,
                status: match status.as_str() {
                    "complete" => TransferStatus::Complete,
                    "suspended" => TransferStatus::Suspended,
                    "error" => TransferStatus::Error,
                    "cancelled" => TransferStatus::Cancelled,
                    _ => TransferStatus::Active,
                },
                error: String::new(),
                started_at: 0,
            });
        }
        state.last_transfer_count = transfers.len();
        let _ = tx.send(Event::Transfers(transfers));
    } else if !state.obex_seen && state.last_transfer_count == 0 {
        let _ = tx.send(Event::Transfers(Vec::new()));
    }

    let _ = cfg;
    let _ = parent_path;
}

fn parse_player(p: &HashMap<String, OwnedValue>, path: &str, devices: &[Device]) -> PlayerState {
    let status = match as_string(p, "Status").to_ascii_lowercase().as_str() {
        "playing" => PlayStatus::Playing,
        "paused" => PlayStatus::Paused,
        "forward-seek" => PlayStatus::ForwardSeek,
        "reverse-seek" => PlayStatus::ReverseSeek,
        "error" => PlayStatus::Error,
        _ => PlayStatus::Stopped,
    };
    let mut ps = PlayerState {
        status,
        position_ms: as_u32(p, "Position").unwrap_or(0) as u64,
        shuffle: as_bool(p, "Shuffle"),
        repeat: match as_string(p, "Repeat").to_ascii_lowercase().as_str() {
            "singletrack" => RepeatMode::Single,
            "alltracks" => RepeatMode::All,
            "group" => RepeatMode::Group,
            _ => RepeatMode::Off,
        },
        volume: as_u8(p, "Volume").unwrap_or(100),
        ..Default::default()
    };
    if let Some(track) = get(p, "Track") {
        if let Value::Dict(d) = &**track {
            for (k, v) in d.iter() {
                let key = match k {
                    Value::Str(s) => s.as_str(),
                    _ => continue,
                };
                let val = v.downcast_ref::<String>().unwrap_or_default();
                match key {
                    "Title" => ps.title = val,
                    "Artist" => ps.artist = val,
                    "Album" => ps.album = val,
                    "Genre" => ps.genre = val,
                    "TrackNumber" => ps.track_number = v.downcast_ref::<u32>().unwrap_or(0),
                    "NumberOfTracks" => ps.track_count = v.downcast_ref::<u32>().unwrap_or(0),
                    "Duration" => ps.duration_ms = v.downcast_ref::<u32>().unwrap_or(0) as u64,
                    "ItemNumber" => {}
                    _ => {}
                }
            }
        }
    }
    let dev_path = parent_path(path);
    if let Some(d) = devices.iter().find(|d| dev_path.starts_with(&d.path)) {
        ps.device = d.address.clone();
        ps.device_name = d.display_name();
    }
    ps
}
