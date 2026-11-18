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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
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
const OBEX_AGENT_PATH: &str = "/org/bluez/obex/agent/bluetuith_rs";

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

/// BlueZ, obexd and the various vendor forks disagree on the integer width
/// used for the same property (`RSSI` int16 vs int32, `Size` uint64 vs uint32,
/// `Volume` uint16 vs byte, …). Accept every D-Bus integer width and clamp to
/// the requested Rust type, instead of silently dropping the value.
macro_rules! num_getter {
    ($name:ident, $out:ty) => {
        fn $name(props: &HashMap<String, OwnedValue>, key: &str) -> Option<$out> {
            let v = get(props, key)?;
            let candidates = [
                v.downcast_ref::<u8>().ok().map(|x| x as i128),
                v.downcast_ref::<u16>().ok().map(|x| x as i128),
                v.downcast_ref::<u32>().ok().map(|x| x as i128),
                v.downcast_ref::<u64>().ok().map(|x| x as i128),
                v.downcast_ref::<i16>().ok().map(|x| x as i128),
                v.downcast_ref::<i32>().ok().map(|x| x as i128),
                v.downcast_ref::<i64>().ok().map(|x| x as i128),
            ];
            candidates
                .into_iter()
                .flatten()
                .find_map(|x| <$out>::try_from(x).ok())
        }
    };
}

num_getter!(as_i16, i16);
num_getter!(as_u8, u8);
num_getter!(as_u16, u16);
num_getter!(as_u32, u32);
num_getter!(as_u64, u64);

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
}

struct ThreadState {
    selected_adapter: String,
    pending_auth: HashMap<usize, async_channel::Sender<AuthResponse>>,
    obex_seen: bool,
    last_transfer_count: usize,
    /// Connection that owns `org.bluez.obex`. BlueZ's `obexd` is normally a
    /// *session*-bus service, but some setups run it on the system bus.
    obex_conn: Option<Connection>,
    /// OBEX transfer id -> object path, so Suspend/Resume/Cancel can reach it.
    transfer_paths: HashMap<usize, String>,
    /// Object path -> stable id. The old `sum(path bytes) % 100_000` scheme
    /// collided for distinct paths (e.g. `session0/transfer1` and
    /// `session1/transfer0` both hash to the same value), which made control
    /// commands hit the wrong transfer when two were active at once.
    transfer_ids: HashMap<String, usize>,
    /// Monotonic source for the stable transfer ids above.
    next_transfer_id: usize,
    /// Active network id -> (device address, role), so Disconnect can reach it.
    network_devices: HashMap<usize, (String, NetworkRole)>,
    /// "address|uuid" pairs the user disconnected this session. BlueZ does not
    /// expose BNEP connection objects, so the network list is synthesised from
    /// connected devices; without this set a disconnect would reappear on the
    /// very next poll.
    disconnected_networks: HashSet<String>,
    /// Whether the OBEX Object Push Agent has been registered on obexd.
    obex_agent_registered: bool,
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

        Ok(Self { cmd_tx, event_rx })
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

    // Probe for obexd on the session bus first (D-Bus activation starts it),
    // then the system bus. Report accurate OBEX capabilities to the UI so it
    // never offers "send file" when the service cannot work.
    let mut state = ThreadState {
        selected_adapter: String::new(),
        pending_auth: HashMap::new(),
        obex_seen: false,
        last_transfer_count: 0,
        obex_conn: None,
        transfer_paths: HashMap::new(),
        transfer_ids: HashMap::new(),
        next_transfer_id: 1,
        network_devices: HashMap::new(),
        disconnected_networks: HashSet::new(),
        obex_agent_registered: false,
    };
    if cfg.disable_obex_services {
        // Honour the flag in the backend too: do not probe obexd, do not
        // register a receive Agent, do not accept incoming pushes.
        let mut f = FeatureSet::all();
        f.obex = false;
        f.send_file = false;
        f.receive_file = false;
        let _ = event_tx.send(Event::Features(f));
    } else {
        let _ = event_tx.send(Event::Features(obex_features(&mut state)));
        if let Err(e) = register_obex_agent(&mut state, &agent_tx, &cfg) {
            let _ = event_tx.send(warn(format!("OBEX receive agent unavailable: {e}")));
        }
    }

    let mut ticks: u64 = 0;
    // Publish the first snapshot right away. The loop below waits up to 1.2s
    // in recv_timeout before its first refresh, which would otherwise leave
    // the UI showing "0 devices" for over a second on every startup.
    refresh(&conn, &event_tx, &mut state);
    loop {
        // Pairing prompts (device agent) and incoming-file prompts (OBEX
        // agent) share the same channel and the same UI dialog.
        while let Ok((mut req, responder)) = agent_rx.try_recv() {
            if !matches!(req.kind, AuthKind::FilePush(_)) {
                req.device_name = name_of(&conn, &req.device);
            }
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

        // If obexd is started later, pick it up without restarting the app.
        ticks += 1;
        if ticks % 20 == 0 && !cfg.disable_obex_services {
            if state.obex_conn.is_none() {
                let _ = event_tx.send(Event::Features(obex_features(&mut state)));
            }
            if !state.obex_agent_registered {
                let _ = register_obex_agent(&mut state, &agent_tx, &cfg);
            }
        }
        refresh(&conn, &event_tx, &mut state);
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

// ---------------------------------------------------------------------------
// OBEX Object Push receiving agent.
//
// obexd owns incoming transfers but delegates the accept/reject decision to a
// registered `org.bluez.obex.Agent1`. `AuthorizePush` returns the full path the
// received file should be written to, which is exactly where `receive-dir`
// belongs. The prompt reuses the same AuthRequest/pending_auth dialog as
// device pairing, so no new UI state machine is needed.

/// Read the suggested filename from an OBEX Transfer1 object.
fn obex_filename(conn: &Connection, transfer: &str) -> String {
    conn.call_method(
        Some("org.bluez.obex"),
        transfer,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.bluez.obex.Transfer1", "Filename"),
    )
    .and_then(|r| r.body().deserialize::<(OwnedValue,)>())
    .ok()
    .and_then(|(v,)| v.downcast_ref::<String>().ok())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "incoming.bin".into())
}

/// Strip any directory component from a peer-supplied filename so an incoming
/// push can never write outside `receive-dir` (e.g. `../../etc/passwd`).
fn safe_filename(raw: &str) -> String {
    Path::new(raw)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "incoming.bin".into())
}

struct ObexAgent {
    conn: Connection,
    requests: AsyncSender<AgentRequest>,
    next_id: AtomicUsize,
    receive_dir: PathBuf,
}

#[interface(name = "org.bluez.obex.Agent1")]
impl ObexAgent {
    async fn release(&self) {}

    async fn cancel(&self) {}

    async fn authorize_push(&self, transfer: OwnedObjectPath) -> fdo::Result<String> {
        let raw = obex_filename(&self.conn, transfer.as_str());
        // A remote peer controls this name; keep it inside receive_dir.
        let name = safe_filename(&raw);

        // IDs start well above the device-agent range (100+) so both agents
        // can share `pending_auth` without colliding.
        let (tx, rx) = async_channel::bounded(1);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1_000_001;
        let req = AuthRequest {
            id,
            device: name.clone(),
            device_name: name.clone(),
            kind: AuthKind::FilePush(name.clone()),
        };
        let _ = self.requests.try_send((req, tx));
        match rx.recv().await {
            Ok(AuthResponse::Accept) => {
                if let Err(e) = std::fs::create_dir_all(&self.receive_dir) {
                    return Err(fdo::Error::Failed(format!(
                        "Cannot create receive dir: {e}"
                    )));
                }
                Ok(self.receive_dir.join(&name).to_string_lossy().to_string())
            }
            _ => Err(fdo::Error::Failed("File push rejected by user".into())),
        }
    }
}

fn register_obex_agent(
    state: &mut ThreadState,
    requests: &AsyncSender<AgentRequest>,
    cfg: &Config,
) -> Result<()> {
    let Some(conn) = state.obex_conn.clone() else {
        return Ok(());
    };
    if state.obex_agent_registered {
        return Ok(());
    }
    let agent = ObexAgent {
        conn: conn.clone(),
        requests: requests.clone(),
        next_id: AtomicUsize::new(1_000_000),
        receive_dir: cfg.effective_receive_dir(),
    };
    conn.object_server().at(OBEX_AGENT_PATH, agent)?;
    let path = OwnedObjectPath::try_from(OBEX_AGENT_PATH)?;
    conn.call_method(
        Some("org.bluez.obex"),
        "/org/bluez/obex",
        Some("org.bluez.obex.AgentManager1"),
        "RegisterAgent",
        &(path,),
    )?;
    state.obex_agent_registered = true;
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
        Refresh => {
            let _ = tx.send(status_of("Device list refreshed", Level::Info));
            return;
        }
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
        SendFiles { .. } if cfg.disable_obex_services => {
            let _ = tx.send(status_of("OBEX services are disabled", Level::Warning));
        }
        SendFiles { device, files } => {
            let dst = as_address(&device);
            // The receive directory belongs to obexd on the *receiving* side;
            // creating it locally before a send has no effect.
            let Some(obex) = obex_connection(state) else {
                let _ = tx.send(status_of(
                    "Cannot send: OBEX service (obexd) is unavailable. Install bluez-obexd and/or start it with `systemctl --user start obex.service`.",
                    Level::Error,
                ));
                return;
            };
            // CreateSession blocks for several seconds while BlueZ tries to
            // reach an unreachable/unpaired device (measured ~5s here). Doing
            // it on the polling thread would freeze snapshots and status for
            // that whole time, so hand the batch to a side thread: the UI
            // keeps refreshing and the result arrives whenever OBEX answers.
            let tx = tx.clone();
            thread::spawn(move || {
                let mut ok = 0;
                for f in &files {
                    match obex_send_file(&obex, &dst, f) {
                        Ok(()) => ok += 1,
                        Err(e) => {
                            let _ =
                                tx.send(status_of(format!("{}: {e}", f.display()), Level::Error));
                        }
                    }
                }
                if ok > 0 {
                    let _ = tx.send(status_of(
                        format!("Queued {ok} file(s) via OBEX Object Push"),
                        Level::Success,
                    ));
                }
            });
        }
        PlayerCommand(c) => {
            if matches!(c, PlayerCmd::ToggleShuffle | PlayerCmd::CycleRepeat) {
                let ok = if matches!(c, PlayerCmd::ToggleShuffle) {
                    "Shuffle toggled"
                } else {
                    "Repeat cycled"
                };
                report(tx, player_property_cycle(conn, c), ok);
                return;
            }
            if matches!(c, PlayerCmd::VolumeUp | PlayerCmd::VolumeDown) {
                let delta = if matches!(c, PlayerCmd::VolumeUp) {
                    8
                } else {
                    -8
                };
                match adjust_volume(conn, delta) {
                    Ok(()) => {
                        let _ = tx.send(status_of("Volume adjusted", Level::Success));
                    }
                    // Not an error: many adapters simply have no transport
                    // volume property; the system mixer owns volume there.
                    Err(e) => {
                        let _ = tx.send(status_of(format!("{e}"), Level::Warning));
                    }
                }
                return;
            }
            if let Some(p) = find_player_path(conn) {
                let name = match c {
                    // MediaPlayer1 has no TogglePlay: read the current Status
                    // and issue the opposite command, otherwise the "play/pause"
                    // key could only ever start playback.
                    PlayerCmd::TogglePlay => {
                        let playing = get_properties(conn, &p, "org.bluez.MediaPlayer1")
                            .map(|props| {
                                as_string(&props, "Status").eq_ignore_ascii_case("playing")
                            })
                            .unwrap_or(false);
                        Some(if playing { "Pause" } else { "Play" })
                    }
                    PlayerCmd::Play => Some("Play"),
                    PlayerCmd::Pause => Some("Pause"),
                    PlayerCmd::Stop => Some("Stop"),
                    PlayerCmd::Next => Some("Next"),
                    PlayerCmd::Previous => Some("Previous"),
                    PlayerCmd::FastForward => Some("FastForward"),
                    PlayerCmd::Rewind => Some("Rewind"),
                    PlayerCmd::VolumeUp
                    | PlayerCmd::VolumeDown
                    | PlayerCmd::ToggleShuffle
                    | PlayerCmd::CycleRepeat => None,
                };
                if let Some(name) = name {
                    let r = method(conn, &p, "org.bluez.MediaPlayer1", name);
                    report(tx, r, name);
                }
            } else {
                let _ = tx.send(status_of("No media player is available", Level::Warning));
            }
        }
        // UI-only commands on real hardware; acknowledge them with a neutral
        // status so the bar never shows a stale, unrelated error.
        ShowPlayer(_) => {
            let _ = tx.send(info("Media player shown"));
        }
        HidePlayer => {
            let _ = tx.send(info("Media player hidden"));
        }
        ConnectNetwork { device, role } => {
            // Network1.Connect accepts "panu"/"nap"/"gn" or a full 128-bit
            // UUID; a bare short form such as "1115" is rejected. DUN is not
            // a BNEP role at all, so report that instead of firing a call that
            // can never succeed.
            if role == NetworkRole::Dun {
                let _ = tx.send(status_of(
                    "DUN dial-up is not provided by BlueZ Network1 (reserved)",
                    Level::Warning,
                ));
                return;
            }
            let p = device_path(conn, &device);
            let uuid = format!("0000{}-0000-1000-8000-00805f9b34fb", role.uuid());
            let key = network_key(&device, role);
            let r = conn
                .call_method(
                    Some("org.bluez"),
                    p.as_str(),
                    Some("org.bluez.Network1"),
                    "Connect",
                    &(uuid,),
                )
                .map(|_| ());
            if r.is_ok() {
                state.disconnected_networks.remove(&key);
            }
            report(tx, r, &format!("Connected {} network", role.label()));
        }
        DisconnectNetwork(id) => {
            let Some((addr, role)) = state.network_devices.get(&id).cloned() else {
                let _ = tx.send(status_of("No such network connection", Level::Warning));
                return;
            };
            let p = device_path(conn, &addr);
            let r = method(conn, &p, "org.bluez.Network1", "Disconnect");
            if r.is_ok() {
                // Keep it hidden on subsequent polls (the list is synthesised
                // from connected devices and would otherwise come straight back).
                state.disconnected_networks.insert(network_key(&addr, role));
            }
            report(tx, r, "Disconnected network");
        }
        SetAudioProfile { profile, .. } => {
            // BlueZ itself does not switch A2DP/HFP profiles; that belongs to
            // bluez-alsa / PipeWire / PulseAudio. Say so plainly instead of
            // pretending the switch succeeded.
            let _ = tx.send(status_of(
                format!("Switching {profile} needs bluez-alsa/PulseAudio; use your audio server"),
                Level::Warning,
            ));
        }
        SuspendTransfer(id) => {
            control_transfer(state, tx, id, "Suspend", "Transfer suspended");
        }
        ResumeTransfer(id) => {
            control_transfer(state, tx, id, "Resume", "Transfer resumed");
        }
        CancelTransfer(id) => {
            control_transfer(state, tx, id, "Cancel", "Transfer cancelled");
        }
        RefreshNetworks(_) => {
            let _ = tx.send(info("Network list refreshed"));
        }
        RefreshProfiles(_) => {
            let _ = tx.send(info("Audio profiles refreshed"));
        }
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

fn network_key(addr: &str, role: NetworkRole) -> String {
    format!("{addr}|{}", role.uuid())
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

/// Try the session bus, then the system bus, for `org.bluez.obex`.
///
/// Sending a call to a well-known name triggers D-Bus activation, so this
/// also starts `obexd` on demand when the service file is installed
/// (`bluez-obexd` provides `/usr/share/dbus-1/services/org.bluez.obex.service`).
fn probe_obex() -> Option<Connection> {
    for c in [Connection::session(), Connection::system()] {
        let Ok(conn) = c else { continue };
        // Peer.Ping on the well-known name forces the bus to activate obexd.
        if conn
            .call_method(
                Some("org.bluez.obex"),
                "/org/bluez/obex",
                Some("org.freedesktop.DBus.Peer"),
                "Ping",
                &(),
            )
            .is_ok()
        {
            return Some(conn);
        }
    }
    None
}

/// Return (and cache) the OBEX connection, re-probing if it went away.
fn obex_connection(state: &mut ThreadState) -> Option<Connection> {
    if state.obex_conn.is_none() {
        state.obex_conn = probe_obex();
    }
    state.obex_conn.clone()
}

/// Feature flags that reflect whether OBEX can actually be used.
fn obex_features(state: &mut ThreadState) -> FeatureSet {
    let has_obex = obex_connection(state).is_some();
    let mut f = FeatureSet::all();
    f.obex = has_obex;
    f.send_file = has_obex;
    f.receive_file = has_obex;
    f
}

fn obex_send_file(conn: &Connection, destination: &str, file: &std::path::Path) -> Result<()> {
    let mut args = HashMap::<&str, Value<'_>>::new();
    args.insert("Target", Value::from("OPP"));
    let reply = conn
        .call_method(
            Some("org.bluez.obex"),
            "/org/bluez/obex",
            Some("org.bluez.obex.Client1"),
            "CreateSession",
            &(destination, args),
        )
        .map_err(|e| {
            anyhow!("OBEX CreateSession failed ({e}); is the device paired and in range?")
        })?;
    let (session,): (OwnedObjectPath,) = reply.body().deserialize()?;
    let name = file.to_string_lossy().to_string();
    conn.call_method(
        Some("org.bluez.obex"),
        session.as_str(),
        Some("org.bluez.obex.ObjectPush1"),
        "SendFile",
        &(name,),
    )
    .map_err(|e| anyhow!("OBEX SendFile failed ({e})"))?;
    Ok(())
}

/// Control an OBEX transfer through its D-Bus object on obexd.
fn control_transfer(
    state: &mut ThreadState,
    tx: &Sender<Event>,
    id: usize,
    method_name: &str,
    ok: &str,
) {
    let Some(obex) = obex_connection(state) else {
        let _ = tx.send(status_of("OBEX service unavailable", Level::Warning));
        return;
    };
    let Some(path) = state.transfer_paths.get(&id).cloned() else {
        let _ = tx.send(status_of("No such transfer", Level::Warning));
        return;
    };
    let r = obex
        .call_method(
            Some("org.bluez.obex"),
            path.as_str(),
            Some("org.bluez.obex.Transfer1"),
            method_name,
            &(),
        )
        .map(|_| ());
    report(tx, r, ok);
}

/// MediaPlayer1 exposes writable `Shuffle` / `Repeat` string properties.
fn player_property_cycle(conn: &Connection, c: PlayerCmd) -> Result<()> {
    let Some(path) = find_player_path(conn) else {
        return Err(anyhow!("No media player is available"));
    };
    let props = get_properties(conn, &path, "org.bluez.MediaPlayer1")?;
    match c {
        PlayerCmd::ToggleShuffle => {
            let cur = as_string(&props, "Shuffle").to_ascii_lowercase();
            let next = if cur.is_empty() || cur == "off" {
                "alltracks"
            } else {
                "off"
            };
            set_property(
                conn,
                &path,
                "org.bluez.MediaPlayer1",
                "Shuffle",
                Value::from(next),
            )?;
        }
        PlayerCmd::CycleRepeat => {
            let cur = as_string(&props, "Repeat").to_ascii_lowercase();
            let next = match cur.as_str() {
                "" | "off" => "singletrack",
                "singletrack" => "alltracks",
                "alltracks" => "group",
                _ => "off",
            };
            set_property(
                conn,
                &path,
                "org.bluez.MediaPlayer1",
                "Repeat",
                Value::from(next),
            )?;
        }
        _ => {}
    }
    Ok(())
}

/// BlueZ exposes a writable `Volume` (u16, 0..127) on MediaTransport1.
fn adjust_volume(conn: &Connection, delta: i16) -> Result<()> {
    let objects = managed_objects(conn, "org.bluez")?;
    let iface = OwnedInterfaceName::try_from("org.bluez.MediaTransport1").unwrap();
    for (path, ifaces) in &objects {
        let Some(props) = ifaces.get(&iface) else {
            continue;
        };
        let cur = as_u16(props, "Volume").unwrap_or(0) as i16;
        let next = (cur + delta).clamp(0, 127) as u16;
        set_property(
            conn,
            path.as_str(),
            "org.bluez.MediaTransport1",
            "Volume",
            Value::U16(next),
        )?;
        return Ok(());
    }
    Err(anyhow!(
        "no MediaTransport1 with a writable Volume property"
    ))
}

/// Return a stable, collision-free id for an OBEX transfer object path.
/// Ids are allocated once per path and reused on every subsequent poll, so a
/// Suspend/Resume/Cancel issued later still reaches the same transfer even if
/// other transfers have appeared in the meantime.
fn transfer_id(state: &mut ThreadState, path: &str) -> usize {
    if let Some(id) = state.transfer_ids.get(path) {
        return *id;
    }
    let id = state.next_transfer_id;
    state.next_transfer_id += 1;
    state.transfer_ids.insert(path.to_string(), id);
    id
}

fn refresh(conn: &Connection, tx: &Sender<Event>, state: &mut ThreadState) {
    let objects = match managed_objects(conn, "org.bluez") {
        Ok(o) => o,
        Err(e) => {
            let _ = tx.send(status_of(format!("BlueZ: {e}"), Level::Error));
            return;
        }
    };

    let mut adapters = Vec::new();
    let mut devices = Vec::new();
    // Resolve the player only after `devices` is complete. The object map is
    // a HashMap, so a MediaPlayer1 object may be visited *before* its Device1
    // object; parsing it inside the loop used to leave the player with an
    // empty device/device_name depending on iteration order.
    let mut player_source: Option<(&str, &HashMap<String, OwnedValue>)> = None;

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
            player_source = Some((path.as_str(), p));
        }
    }
    let mut player = player_source.map(|(p, props)| parse_player(props, p, &devices));
    // BlueZ keeps the AVRCP absolute volume (0..127) on MediaTransport1, not
    // on MediaPlayer1 (which has no Volume property at all). Override the
    // fallback from `parse_player` so the UI tracks what `adjust_volume` sets.
    if let (Some(pl), Some(v)) = (player.as_mut(), first_transport_volume(&objects)) {
        pl.volume = v.min(u8::MAX as u16) as u8;
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
            if d.has_service(n)
                && d.connected
                && !state
                    .disconnected_networks
                    .contains(&network_key(&d.address, role))
            {
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
    state.network_devices.clear();
    for n in &networks {
        state
            .network_devices
            .insert(n.id, (n.device.clone(), n.role));
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
                    uuid: super::service_name(short, crate::i18n::Lang::En).to_string(),
                    description: desc.into(),
                    active: d.connected,
                    device: d.display_name(),
                });
            }
        }
    }
    let _ = tx.send(Event::Profiles(profiles));

    // OBEX transfers live on the connection that owns `org.bluez.obex`
    // (usually the session bus), not the BlueZ system bus.
    let obex_objects = state
        .obex_conn
        .as_ref()
        .and_then(|c| managed_objects(c, "org.bluez.obex").ok());
    if let Some(obex) = obex_objects {
        state.obex_seen = true;
        state.transfer_paths.clear();
        let mut transfers = Vec::new();
        for (path, ifaces) in &obex {
            let Some(p) =
                ifaces.get(&OwnedInterfaceName::try_from("org.bluez.obex.Transfer1").unwrap())
            else {
                continue;
            };
            let status = as_string(p, "Status").to_ascii_lowercase();
            let id = transfer_id(state, path.as_str());
            state.transfer_paths.insert(id, path.as_str().to_string());
            transfers.push(Transfer {
                id,
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
        state.transfer_paths.clear();
        let _ = tx.send(Event::Transfers(Vec::new()));
    }
}

/// First MediaTransport1 `Volume` found in the object map, if any.
fn first_transport_volume(objects: &ManagedObjects) -> Option<u16> {
    let iface = OwnedInterfaceName::try_from("org.bluez.MediaTransport1").ok()?;
    objects
        .values()
        .filter_map(|ifaces| ifaces.get(&iface))
        .find_map(|p| as_u16(p, "Volume"))
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
    // BlueZ exposes Shuffle (and Repeat) as *strings* ("off"/"alltracks"/
    // "group"), not booleans — `player_property_cycle` writes them back as
    // strings too. Reading it with `as_bool` always yielded false, so a real
    // shuffle toggle was never reflected in the UI.
    let shuffle = {
        let s = as_string(p, "Shuffle").to_ascii_lowercase();
        !s.is_empty() && s != "off"
    };
    let mut ps = PlayerState {
        status,
        position_ms: as_u32(p, "Position").unwrap_or(0) as u64,
        shuffle,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_state() -> ThreadState {
        ThreadState {
            selected_adapter: String::new(),
            pending_auth: HashMap::new(),
            obex_seen: false,
            last_transfer_count: 0,
            obex_conn: None,
            transfer_paths: HashMap::new(),
            transfer_ids: HashMap::new(),
            next_transfer_id: 1,
            network_devices: HashMap::new(),
            disconnected_networks: HashSet::new(),
            obex_agent_registered: false,
        }
    }

    #[test]
    fn transfer_ids_are_stable_and_collision_free() {
        let mut state = empty_state();
        // These two distinct object paths collided under the old
        // `sum(bytes) % 100_000` scheme (both hashed to 3963).
        let a = "/org/bluez/obex/client/session0/transfer1";
        let b = "/org/bluez/obex/client/session1/transfer0";
        let id_a = transfer_id(&mut state, a);
        let id_b = transfer_id(&mut state, b);
        assert_ne!(id_a, id_b, "distinct transfer paths must get distinct ids");
        // Repeated lookups across polls must return the same id.
        assert_eq!(transfer_id(&mut state, a), id_a);
        assert_eq!(transfer_id(&mut state, b), id_b);
        // And a third, unrelated path gets yet another fresh id.
        assert_ne!(transfer_id(&mut state, "/org/bluez/obex/transfer0"), id_a);
    }

    /// OBEX capability flags must be driven by the live probe, never hardcoded
    /// to true (that is what made "Send file" fail with a raw ServiceUnknown).
    #[test]
    fn obex_features_track_the_live_probe() {
        let mut state = empty_state();
        let f = obex_features(&mut state);
        assert_eq!(f.obex, f.send_file);
        assert_eq!(f.obex, f.receive_file);
        assert_eq!(f.obex, state.obex_conn.is_some());
        // Non-OBEX features always remain available.
        assert!(f.adapter && f.device && f.network);
    }

    #[test]
    fn integer_getters_accept_every_dbus_integer_width() {
        use zbus::zvariant::OwnedValue;
        let mut p = HashMap::new();
        p.insert("a".to_string(), OwnedValue::from(-55i32));
        assert_eq!(as_i16(&p, "a"), Some(-55));
        p.insert("a".to_string(), OwnedValue::from(77u32));
        assert_eq!(as_u8(&p, "a"), Some(77));
        p.insert("a".to_string(), OwnedValue::from(64i64));
        assert_eq!(as_u16(&p, "a"), Some(64));
        p.insert("a".to_string(), OwnedValue::from(4096u64));
        assert_eq!(as_u32(&p, "a"), Some(4096));
        assert_eq!(as_u64(&p, "a"), Some(4096));
        // Out-of-range values must be dropped, never wrapped.
        p.insert("a".to_string(), OwnedValue::from(999u32));
        assert_eq!(as_u8(&p, "a"), None);
        assert_eq!(as_u16(&p, "missing"), None);
    }

    #[test]
    fn player_device_name_requires_the_complete_device_list() {
        // refresh() defers parse_player until `devices` is fully built: a
        // MediaPlayer1 object can be visited before its Device1 object because
        // GetManagedObjects is a HashMap. This pins that contract down.
        use zbus::zvariant::{OwnedValue, Str};
        let mut props = HashMap::new();
        props.insert(
            "Status".to_string(),
            OwnedValue::from(Str::from("playing".to_string())),
        );
        props.insert("Position".to_string(), OwnedValue::from(1000u32));
        let path = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF/player0";
        let bare = parse_player(&props, path, &[]);
        assert_eq!(bare.device_name, "");
        let full = vec![Device {
            path: "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF".into(),
            address: "AA:BB:CC:DD:EE:FF".into(),
            name: "Mock Phone".into(),
            ..Default::default()
        }];
        let ok = parse_player(&props, path, &full);
        assert_eq!(ok.device_name, "Mock Phone");
        assert_eq!(ok.device, "AA:BB:CC:DD:EE:FF");
        assert_eq!(ok.status, PlayStatus::Playing);
        assert_eq!(ok.position_ms, 1000);
    }

    #[test]
    fn obex_filename_cannot_escape_the_receive_dir() {
        assert_eq!(safe_filename("photo.jpg"), "photo.jpg");
        assert_eq!(safe_filename("../../etc/passwd"), "passwd");
        assert_eq!(safe_filename("/tmp/a/b.txt"), "b.txt");
        assert_eq!(safe_filename(""), "incoming.bin");
    }

    #[test]
    fn parse_player_reads_shuffle_from_a_string() {
        use zbus::zvariant::{OwnedValue, Str};
        let mut props = HashMap::new();
        props.insert(
            "Shuffle".to_string(),
            OwnedValue::from(Str::from("alltracks".to_string())),
        );
        assert!(parse_player(&props, "/org/bluez/hci0/dev_A/player0", &[]).shuffle);
        props.insert(
            "Shuffle".to_string(),
            OwnedValue::from(Str::from("off".to_string())),
        );
        assert!(!parse_player(&props, "/org/bluez/hci0/dev_A/player0", &[]).shuffle);
        props.remove("Shuffle");
        assert!(!parse_player(&props, "/org/bluez/hci0/dev_A/player0", &[]).shuffle);
    }

    #[test]
    fn transport_volume_is_read_from_media_transport() {
        use zbus::zvariant::OwnedValue;
        let mut objects = ManagedObjects::new();
        let mut ifaces = HashMap::new();
        let mut props = HashMap::new();
        props.insert("Volume".to_string(), OwnedValue::from(72u16));
        ifaces.insert(
            OwnedInterfaceName::try_from("org.bluez.MediaTransport1").unwrap(),
            props,
        );
        objects.insert(
            OwnedObjectPath::try_from("/org/bluez/hci0/dev_A/fd0").unwrap(),
            ifaces,
        );
        assert_eq!(first_transport_volume(&objects), Some(72));
        assert_eq!(first_transport_volume(&ManagedObjects::new()), None);
    }

    #[test]
    fn network_key_is_distinct_per_device_and_role() {
        assert_eq!(network_key("AA:BB", NetworkRole::Panu), "AA:BB|1115");
        assert_ne!(
            network_key("AA:BB", NetworkRole::Panu),
            network_key("AA:BB", NetworkRole::Nap)
        );
        assert_ne!(
            network_key("AA:BB", NetworkRole::Panu),
            network_key("CC:DD", NetworkRole::Panu)
        );
    }
}
