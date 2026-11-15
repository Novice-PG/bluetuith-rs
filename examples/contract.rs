//! Low-level BlueZ contract test.
//!
//! Instead of asking the program "did you print success?", this runs the real
//! `BluezBackend` against a *fake* `org.bluez` service on a private D-Bus bus
//! and asserts the actual D-Bus traffic: method names, interfaces, argument
//! values and which bus they are sent on. It also feeds a hand-built
//! `GetManagedObjects` reply and checks that the backend parses it correctly.
//!
//! Run:  dbus-run-session -- cargo run --example contract
//! (exits non-zero if any contract check fails)

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use bluetuith_rs::app::App;
use bluetuith_rs::backend::bluez::BluezBackend;
use bluetuith_rs::backend::Command;
use bluetuith_rs::config::Config;
use bluetuith_rs::model::{AuthResponse, NetworkRole, PlayerCommand, TransferStatus};
use zbus::blocking::Connection;
use zbus::interface;
use zbus::names::OwnedInterfaceName;
use zbus::zvariant::{Array, OwnedObjectPath, OwnedValue, Str, Value};

type Objects = HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>>;

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Call {
    tag: String,
    iface: String,
    member: String,
    args: String,
}

#[derive(Default)]
struct Shared {
    calls: Mutex<Vec<Call>>,
    objects: Mutex<Objects>,
    player_status: Mutex<String>,
    slow_obex: AtomicBool,
}

type S = Arc<Shared>;

fn rec(s: &S, tag: &str, iface: &str, member: &str, args: impl std::fmt::Debug) {
    s.calls.lock().unwrap().push(Call {
        tag: tag.into(),
        iface: iface.into(),
        member: member.into(),
        args: format!("{args:?}"),
    });
}

fn iname(s: &str) -> OwnedInterfaceName {
    OwnedInterfaceName::try_from(s).unwrap()
}
fn opath(s: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(s).unwrap()
}
fn ov_str(s: &str) -> OwnedValue {
    OwnedValue::from(Str::from(s.to_string()))
}
fn ov_strings(xs: &[&str]) -> OwnedValue {
    let v: Vec<String> = xs.iter().map(|s| s.to_string()).collect();
    OwnedValue::try_from(Value::Array(Array::from(v))).unwrap()
}

fn props(pairs: &[(&str, OwnedValue)]) -> HashMap<String, OwnedValue> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

fn set(m: &mut HashMap<String, OwnedValue>, k: &str, v: OwnedValue) {
    m.insert(k.to_string(), v);
}

const DEV: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF";
const PLAYER: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF/player0";
const TRANSPORT: &str = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF/fd0";
const ADDR: &str = "AA:BB:CC:DD:EE:FF";

fn build_objects() -> Objects {
    let mut o = Objects::new();

    let mut ai = HashMap::new();
    ai.insert(
        iname("org.bluez.Adapter1"),
        props(&[
            ("Address", ov_str("00:11:22:33:44:55")),
            ("Name", ov_str("Mock Adapter")),
            ("Alias", ov_str("Mock")),
            ("Powered", OwnedValue::from(true)),
            ("Discoverable", OwnedValue::from(false)),
            ("Pairable", OwnedValue::from(true)),
            ("Discovering", OwnedValue::from(false)),
        ]),
    );
    o.insert(opath("/org/bluez/hci0"), ai);

    let mut di = HashMap::new();
    di.insert(
        iname("org.bluez.Device1"),
        props(&[
            ("Address", ov_str(ADDR)),
            ("Adapter", ov_str("/org/bluez/hci0")),
            ("Name", ov_str("Mock Phone")),
            ("Alias", ov_str("")),
            ("Paired", OwnedValue::from(true)),
            ("Connected", OwnedValue::from(true)),
            ("Trusted", OwnedValue::from(false)),
            ("Blocked", OwnedValue::from(false)),
            ("LegacyPairing", OwnedValue::from(false)),
            ("ServicesResolved", OwnedValue::from(true)),
            ("RSSI", OwnedValue::from(-55i16)),
            ("TxPower", OwnedValue::from(4i16)),
            ("Class", OwnedValue::from(0x240404u32)),
            ("Appearance", OwnedValue::from(0u16)),
            ("Icon", ov_str("phone")),
            ("Modalias", ov_str("")),
            (
                "UUIDs",
                ov_strings(&[
                    "00001105-0000-1000-8000-00805f9b34fb", // OPP
                    "00001106-0000-1000-8000-00805f9b34fb", // FTP
                    "00001115-0000-1000-8000-00805f9b34fb", // PANU
                    "00001116-0000-1000-8000-00805f9b34fb", // NAP
                    "0000110b-0000-1000-8000-00805f9b34fb", // A2DP sink
                    "0000110c-0000-1000-8000-00805f9b34fb", // AVRCP
                    "0000180f-0000-1000-8000-00805f9b34fb", // Battery
                ]),
            ),
        ]),
    );
    di.insert(iname("org.bluez.Network1"), props(&[]));
    di.insert(
        iname("org.bluez.Battery1"),
        props(&[("Percentage", OwnedValue::from(77u8))]),
    );
    o.insert(opath(DEV), di);

    let mut pi = HashMap::new();
    pi.insert(
        iname("org.bluez.MediaPlayer1"),
        props(&[
            ("Status", ov_str("playing")),
            ("Position", OwnedValue::from(61_000u32)),
            ("Shuffle", OwnedValue::from(false)),
            ("Repeat", ov_str("alltracks")),
            ("Volume", OwnedValue::from(60u8)),
        ]),
    );
    o.insert(opath(PLAYER), pi);

    let mut ti = HashMap::new();
    ti.insert(
        iname("org.bluez.MediaTransport1"),
        props(&[("Volume", OwnedValue::from(64u16))]),
    );
    o.insert(opath(TRANSPORT), ti);

    let mut obex = HashMap::new();
    obex.insert(
        iname("org.bluez.obex.Transfer1"),
        props(&[
            ("Status", ov_str("active")),
            ("Filename", ov_str("photo.jpg")),
            ("Session", ov_str("/org/bluez/obex/session0")),
            ("Size", OwnedValue::from(2048u64)),
            ("Transferred", OwnedValue::from(512u64)),
        ]),
    );
    o.insert(opath("/org/bluez/obex/transfer0"), obex);

    o
}

// ---------------------------------------------------------------------------
// Fake BlueZ interfaces. Every mutating call is recorded with its argument.

struct ObjectManager {
    s: S,
}
#[interface(name = "org.freedesktop.DBus.ObjectManager")]
impl ObjectManager {
    async fn get_managed_objects(&self) -> zbus::fdo::Result<Objects> {
        rec(&self.s, "/", "ObjectManager", "GetManagedObjects", "");
        Ok(self.s.objects.lock().unwrap().clone())
    }
}

struct Adapter1 {
    s: S,
}
#[interface(name = "org.bluez.Adapter1")]
impl Adapter1 {
    #[zbus(property)]
    async fn powered(&self) -> bool {
        true
    }
    #[zbus(property)]
    async fn discoverable(&self) -> bool {
        false
    }
    #[zbus(property)]
    async fn pairable(&self) -> bool {
        true
    }
    #[zbus(property)]
    async fn discovering(&self) -> bool {
        false
    }
    #[zbus(property)]
    async fn address(&self) -> String {
        "00:11:22:33:44:55".into()
    }
    #[zbus(property)]
    async fn set_powered(&self, v: bool) {
        rec(&self.s, "hci0", "Adapter1", "Set", format!("Powered={v}"));
    }
    #[zbus(property)]
    async fn set_discoverable(&self, v: bool) {
        rec(
            &self.s,
            "hci0",
            "Adapter1",
            "Set",
            format!("Discoverable={v}"),
        );
    }
    #[zbus(property)]
    async fn set_pairable(&self, v: bool) {
        rec(&self.s, "hci0", "Adapter1", "Set", format!("Pairable={v}"));
    }
    async fn start_discovery(&self) {
        rec(&self.s, "hci0", "Adapter1", "StartDiscovery", "");
    }
    async fn stop_discovery(&self) {
        rec(&self.s, "hci0", "Adapter1", "StopDiscovery", "");
    }
    async fn set_discovery_filter(&self, _f: HashMap<String, OwnedValue>) {
        rec(&self.s, "hci0", "Adapter1", "SetDiscoveryFilter", "");
    }
    async fn remove_device(&self, p: OwnedObjectPath) {
        rec(&self.s, "hci0", "Adapter1", "RemoveDevice", p.as_str());
    }
}

struct Device1 {
    s: S,
}
#[interface(name = "org.bluez.Device1")]
impl Device1 {
    #[zbus(property)]
    async fn trusted(&self) -> bool {
        false
    }
    #[zbus(property)]
    async fn blocked(&self) -> bool {
        false
    }
    #[zbus(property)]
    async fn paired(&self) -> bool {
        true
    }
    #[zbus(property)]
    async fn connected(&self) -> bool {
        true
    }
    #[zbus(property)]
    async fn alias(&self) -> String {
        "Mock Phone".into()
    }
    #[zbus(property)]
    async fn name(&self) -> String {
        "Mock Phone".into()
    }
    #[zbus(property)]
    async fn address(&self) -> String {
        ADDR.into()
    }
    #[zbus(property)]
    async fn set_trusted(&self, v: bool) {
        rec(&self.s, "dev", "Device1", "Set", format!("Trusted={v}"));
    }
    #[zbus(property)]
    async fn set_blocked(&self, v: bool) {
        rec(&self.s, "dev", "Device1", "Set", format!("Blocked={v}"));
    }
    async fn connect(&self) {
        rec(&self.s, "dev", "Device1", "Connect", "");
    }
    async fn disconnect(&self) {
        rec(&self.s, "dev", "Device1", "Disconnect", "");
    }
    async fn pair(&self) {
        rec(&self.s, "dev", "Device1", "Pair", "");
    }
    async fn cancel_pairing(&self) {
        rec(&self.s, "dev", "Device1", "CancelPairing", "");
    }
}

struct Network1 {
    s: S,
}
#[interface(name = "org.bluez.Network1")]
impl Network1 {
    async fn connect(&self, uuid: String) {
        rec(&self.s, "dev", "Network1", "Connect", &uuid);
    }
    async fn disconnect(&self) {
        rec(&self.s, "dev", "Network1", "Disconnect", "");
    }
}

struct ObexAgentManager {
    s: S,
}
#[interface(name = "org.bluez.obex.AgentManager1")]
impl ObexAgentManager {
    async fn register_agent(&self, p: OwnedObjectPath) {
        rec(
            &self.s,
            "obex-manager",
            "AgentManager1",
            "RegisterAgent",
            p.as_str(),
        );
    }
    async fn unregister_agent(&self, p: OwnedObjectPath) {
        rec(
            &self.s,
            "obex-manager",
            "AgentManager1",
            "UnregisterAgent",
            p.as_str(),
        );
    }
}

struct MediaPlayer1 {
    s: S,
}
#[interface(name = "org.bluez.MediaPlayer1")]
impl MediaPlayer1 {
    #[zbus(property)]
    async fn status(&self) -> String {
        self.s.player_status.lock().unwrap().clone()
    }
    #[zbus(property)]
    async fn position(&self) -> u32 {
        61_000
    }
    #[zbus(property)]
    async fn shuffle(&self) -> String {
        "off".into()
    }
    #[zbus(property)]
    async fn set_shuffle(&self, v: String) {
        rec(
            &self.s,
            "player",
            "MediaPlayer1",
            "Set",
            format!("Shuffle={v}"),
        );
    }
    #[zbus(property)]
    async fn repeat(&self) -> String {
        "off".into()
    }
    #[zbus(property)]
    async fn set_repeat(&self, v: String) {
        rec(
            &self.s,
            "player",
            "MediaPlayer1",
            "Set",
            format!("Repeat={v}"),
        );
    }
    async fn play(&self) {
        rec(&self.s, "player", "MediaPlayer1", "Play", "");
    }
    async fn pause(&self) {
        rec(&self.s, "player", "MediaPlayer1", "Pause", "");
    }
    async fn stop(&self) {
        rec(&self.s, "player", "MediaPlayer1", "Stop", "");
    }
    async fn next(&self) {
        rec(&self.s, "player", "MediaPlayer1", "Next", "");
    }
    async fn previous(&self) {
        rec(&self.s, "player", "MediaPlayer1", "Previous", "");
    }
    async fn fast_forward(&self) {
        rec(&self.s, "player", "MediaPlayer1", "FastForward", "");
    }
    async fn rewind(&self) {
        rec(&self.s, "player", "MediaPlayer1", "Rewind", "");
    }
}

struct MediaTransport1 {
    s: S,
}
#[interface(name = "org.bluez.MediaTransport1")]
impl MediaTransport1 {
    #[zbus(property)]
    async fn volume(&self) -> u16 {
        64
    }
    #[zbus(property)]
    async fn set_volume(&self, v: u16) {
        rec(
            &self.s,
            "transport",
            "MediaTransport1",
            "Set",
            format!("Volume={v}"),
        );
    }
}

struct AgentManager1 {
    s: S,
}
#[interface(name = "org.bluez.AgentManager1")]
impl AgentManager1 {
    async fn register_agent(&self, p: OwnedObjectPath, cap: String) {
        rec(
            &self.s,
            "/org/bluez",
            "AgentManager1",
            "RegisterAgent",
            format!("{}/{}", p.as_str(), cap),
        );
    }
    async fn request_default_agent(&self, p: OwnedObjectPath) {
        rec(
            &self.s,
            "/org/bluez",
            "AgentManager1",
            "RequestDefaultAgent",
            p.as_str(),
        );
    }
}

struct ObexClient {
    s: S,
}
#[interface(name = "org.bluez.obex.Client1")]
impl ObexClient {
    async fn create_session(
        &self,
        dest: String,
        args: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        // Simulate BlueZ taking a long time to reach an unreachable device.
        if self.s.slow_obex.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(1200));
        }
        rec(
            &self.s,
            "obex-client",
            "Client1",
            "CreateSession",
            format!("{dest}/{:?}", args.keys().collect::<Vec<_>>()),
        );
        Ok(opath("/org/bluez/obex/session0"))
    }
}

struct ObexPush {
    s: S,
}
#[interface(name = "org.bluez.obex.ObjectPush1")]
impl ObexPush {
    async fn send_file(&self, name: String) {
        rec(&self.s, "obex-push", "ObjectPush1", "SendFile", &name);
    }
}

struct ObexTransfer {
    s: S,
}
#[interface(name = "org.bluez.obex.Transfer1")]
impl ObexTransfer {
    async fn suspend(&self) {
        rec(&self.s, "obex-transfer", "Transfer1", "Suspend", "");
    }
    async fn resume(&self) {
        rec(&self.s, "obex-transfer", "Transfer1", "Resume", "");
    }
    async fn cancel(&self) {
        rec(&self.s, "obex-transfer", "Transfer1", "Cancel", "");
    }
}

// ---------------------------------------------------------------------------

struct Results {
    pass: u32,
    fail: u32,
}

fn check(r: &mut Results, name: &str, ok: bool) {
    if ok {
        r.pass += 1;
        println!("  ok   {name}");
    } else {
        r.fail += 1;
        println!("  FAIL {name}");
    }
}

fn has(calls: &[Call], iface: &str, member: &str, needle: &str) -> bool {
    calls.iter().any(|c| {
        c.iface == iface && c.member == member && (needle.is_empty() || c.args.contains(needle))
    })
}

fn pump(app: &mut App, ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        app.tick();
        sleep(Duration::from_millis(25));
    }
}

fn run_cmd(app: &mut App, s: &S, cmd: Command, ms: u64) -> Vec<Call> {
    s.calls.lock().unwrap().clear();
    app.dispatch(cmd);
    pump(app, ms);
    s.calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| !(c.iface == "ObjectManager" && c.member == "GetManagedObjects"))
        .cloned()
        .collect()
}

fn main() -> anyhow::Result<()> {
    let addr = std::env::var("DBUS_SESSION_BUS_ADDRESS").expect("run under dbus-run-session");
    std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", &addr);

    let s: S = Arc::new(Shared {
        calls: Mutex::new(Vec::new()),
        objects: Mutex::new(build_objects()),
        player_status: Mutex::new("playing".into()),
        slow_obex: AtomicBool::new(false),
    });

    let server = Connection::system()?;
    server.request_name("org.bluez")?;
    server.request_name("org.bluez.obex")?;
    let os = server.object_server();
    os.at("/", ObjectManager { s: s.clone() })?;
    os.at("/org/bluez", AgentManager1 { s: s.clone() })?;
    os.at("/org/bluez/hci0", Adapter1 { s: s.clone() })?;
    os.at(DEV, Device1 { s: s.clone() })?;
    os.at(DEV, Network1 { s: s.clone() })?;
    os.at(PLAYER, MediaPlayer1 { s: s.clone() })?;
    os.at(TRANSPORT, MediaTransport1 { s: s.clone() })?;
    os.at("/org/bluez/obex", ObexClient { s: s.clone() })?;
    os.at("/org/bluez/obex", ObexAgentManager { s: s.clone() })?;
    os.at("/org/bluez/obex/session0", ObexPush { s: s.clone() })?;
    os.at("/org/bluez/obex/transfer0", ObexTransfer { s: s.clone() })?;

    let cfg = Config {
        demo: false,
        language: "en".into(),
        ..Config::default()
    };
    let mut app = App::new(cfg.clone(), Box::new(BluezBackend::new(&cfg)?));
    pump(&mut app, 700);

    let mut r = Results { pass: 0, fail: 0 };

    println!("=== startup / agent / feature probe ===");
    check(&mut r, "pairing agent registered on the bus", {
        let c = s.calls.lock().unwrap().clone();
        has(
            &c,
            "AgentManager1",
            "RegisterAgent",
            "/org/bluez/agent/bluetuith_rs",
        ) && has(&c, "AgentManager1", "RequestDefaultAgent", "")
    });
    check(
        &mut r,
        "OBEX discovered on the session bus -> features enabled",
        app.features.obex && app.features.send_file && app.features.receive_file,
    );
    check(&mut r, "OBEX receive Agent registered on obexd", {
        let c = s.calls.lock().unwrap().clone();
        has(
            &c,
            "AgentManager1",
            "RegisterAgent",
            "/org/bluez/obex/agent/bluetuith_rs",
        )
    });

    println!("=== GetManagedObjects parsing ===");
    check(&mut r, "adapter parsed", {
        app.adapters.len() == 1
            && app.adapters[0].id == "hci0"
            && app.adapters[0].address == "00:11:22:33:44:55"
            && app.adapters[0].powered
    });
    check(&mut r, "device parsed (battery / uuids / state)", {
        app.devices.len() == 1
            && app.devices[0].address == ADDR
            && app.devices[0].paired
            && app.devices[0].connected
            && app.devices[0].battery == Some(77)
            && app.devices[0].uuids.len() == 7
    });
    check(&mut r, "media player parsed", {
        app.player
            .as_ref()
            .map(|p| {
                p.status == bluetuith_rs::model::PlayStatus::Playing
                    && p.position_ms == 61_000
                    && p.device_name == "Mock Phone"
            })
            .unwrap_or(false)
    });
    check(&mut r, "networks / profiles / transfers synthesised", {
        !app.networks.is_empty() && !app.profiles.is_empty() && !app.transfers.is_empty()
    });

    println!("=== D-Bus call contract (adapter / device) ===");
    let c = run_cmd(
        &mut app,
        &s,
        Command::SetAdapterProperty {
            property: bluetuith_rs::backend::AdapterProperty::Powered,
            value: true,
        },
        250,
    );
    check(
        &mut r,
        "SetAdapterProperty -> Properties.Set(Powered,true)",
        { has(&c, "Adapter1", "Set", "Powered=true") },
    );
    let c = run_cmd(&mut app, &s, Command::SetScan(true), 250);
    check(
        &mut r,
        "SetScan(true) -> Adapter1.StartDiscovery",
        has(&c, "Adapter1", "StartDiscovery", ""),
    );
    let c = run_cmd(&mut app, &s, Command::SetScan(false), 250);
    check(
        &mut r,
        "SetScan(false) -> Adapter1.StopDiscovery",
        has(&c, "Adapter1", "StopDiscovery", ""),
    );
    let c = run_cmd(&mut app, &s, Command::ConnectDevice(ADDR.into()), 250);
    check(
        &mut r,
        "ConnectDevice -> Device1.Connect",
        has(&c, "Device1", "Connect", ""),
    );
    let c = run_cmd(&mut app, &s, Command::DisconnectDevice(ADDR.into()), 250);
    check(
        &mut r,
        "DisconnectDevice -> Device1.Disconnect",
        has(&c, "Device1", "Disconnect", ""),
    );
    let c = run_cmd(&mut app, &s, Command::PairDevice(ADDR.into()), 250);
    check(
        &mut r,
        "PairDevice -> Device1.Pair",
        has(&c, "Device1", "Pair", ""),
    );
    let c = run_cmd(&mut app, &s, Command::TrustDevice(ADDR.into(), true), 250);
    check(
        &mut r,
        "TrustDevice -> Properties.Set(Trusted,true)",
        has(&c, "Device1", "Set", "Trusted=true"),
    );
    let c = run_cmd(&mut app, &s, Command::BlockDevice(ADDR.into(), true), 250);
    check(
        &mut r,
        "BlockDevice -> Properties.Set(Blocked,true)",
        has(&c, "Device1", "Set", "Blocked=true"),
    );
    let c = run_cmd(&mut app, &s, Command::RemoveDevice(ADDR.into()), 250);
    check(
        &mut r,
        "RemoveDevice -> Adapter1.RemoveDevice(correct path)",
        { has(&c, "Adapter1", "RemoveDevice", DEV) },
    );

    println!("=== D-Bus call contract (network) ===");
    let c = run_cmd(
        &mut app,
        &s,
        Command::ConnectNetwork {
            device: ADDR.into(),
            role: NetworkRole::Panu,
        },
        250,
    );
    check(&mut r, "ConnectNetwork(PANU) uses a full 128-bit UUID", {
        has(
            &c,
            "Network1",
            "Connect",
            "00001115-0000-1000-8000-00805f9b34fb",
        )
    });
    let c = run_cmd(
        &mut app,
        &s,
        Command::ConnectNetwork {
            device: ADDR.into(),
            role: NetworkRole::Nap,
        },
        250,
    );
    check(&mut r, "ConnectNetwork(NAP) uses a full 128-bit UUID", {
        has(
            &c,
            "Network1",
            "Connect",
            "00001116-0000-1000-8000-00805f9b34fb",
        )
    });
    let c = run_cmd(
        &mut app,
        &s,
        Command::ConnectNetwork {
            device: ADDR.into(),
            role: NetworkRole::Dun,
        },
        250,
    );
    check(
        &mut r,
        "ConnectNetwork(DUN) fires no call and reports reserved",
        { !has(&c, "Network1", "Connect", "") && app.status.text.contains("DUN") },
    );
    let id = app.networks.first().map(|n| n.id).unwrap_or(999_999);
    let c = run_cmd(&mut app, &s, Command::DisconnectNetwork(id), 250);
    check(
        &mut r,
        "DisconnectNetwork -> Network1.Disconnect",
        has(&c, "Network1", "Disconnect", ""),
    );

    println!("=== D-Bus call contract (media player) ===");
    *s.player_status.lock().unwrap() = "playing".into();
    let c = run_cmd(
        &mut app,
        &s,
        Command::PlayerCommand(PlayerCommand::TogglePlay),
        250,
    );
    check(
        &mut r,
        "TogglePlay while playing -> MediaPlayer1.Pause",
        has(&c, "MediaPlayer1", "Pause", ""),
    );
    *s.player_status.lock().unwrap() = "paused".into();
    let c = run_cmd(
        &mut app,
        &s,
        Command::PlayerCommand(PlayerCommand::TogglePlay),
        250,
    );
    check(
        &mut r,
        "TogglePlay while paused -> MediaPlayer1.Play",
        has(&c, "MediaPlayer1", "Play", ""),
    );
    let c = run_cmd(
        &mut app,
        &s,
        Command::PlayerCommand(PlayerCommand::Next),
        250,
    );
    check(
        &mut r,
        "Player Next -> MediaPlayer1.Next",
        has(&c, "MediaPlayer1", "Next", ""),
    );
    let c = run_cmd(
        &mut app,
        &s,
        Command::PlayerCommand(PlayerCommand::VolumeUp),
        250,
    );
    check(
        &mut r,
        "VolumeUp -> MediaTransport1.Set(Volume=72)",
        has(&c, "MediaTransport1", "Set", "Volume=72"),
    );
    let c = run_cmd(
        &mut app,
        &s,
        Command::PlayerCommand(PlayerCommand::ToggleShuffle),
        250,
    );
    check(&mut r, "ToggleShuffle -> MediaPlayer1.Set(Shuffle)", {
        has(&c, "MediaPlayer1", "Set", "Shuffle=alltracks")
    });
    let c = run_cmd(
        &mut app,
        &s,
        Command::PlayerCommand(PlayerCommand::CycleRepeat),
        250,
    );
    check(&mut r, "CycleRepeat -> MediaPlayer1.Set(Repeat)", {
        has(&c, "MediaPlayer1", "Set", "Repeat=singletrack")
    });

    println!("=== D-Bus call contract (OBEX) ===");
    let c = run_cmd(
        &mut app,
        &s,
        Command::SendFiles {
            device: ADDR.into(),
            files: vec![std::path::PathBuf::from("/tmp/contract-photo.jpg")],
        },
        900,
    );
    check(&mut r, "SendFiles -> Client1.CreateSession(OPP target)", {
        has(&c, "Client1", "CreateSession", ADDR) && has(&c, "Client1", "CreateSession", "Target")
    });
    check(&mut r, "SendFiles -> ObjectPush1.SendFile(path)", {
        has(&c, "ObjectPush1", "SendFile", "/tmp/contract-photo.jpg")
    });
    let tid = app.transfers.first().map(|t| t.id).unwrap_or(999_999);
    let c = run_cmd(&mut app, &s, Command::SuspendTransfer(tid), 300);
    check(
        &mut r,
        "SuspendTransfer -> Transfer1.Suspend",
        has(&c, "Transfer1", "Suspend", ""),
    );
    let c = run_cmd(&mut app, &s, Command::ResumeTransfer(tid), 300);
    check(
        &mut r,
        "ResumeTransfer -> Transfer1.Resume",
        has(&c, "Transfer1", "Resume", ""),
    );
    let c = run_cmd(&mut app, &s, Command::CancelTransfer(tid), 300);
    check(
        &mut r,
        "CancelTransfer -> Transfer1.Cancel",
        has(&c, "Transfer1", "Cancel", ""),
    );

    println!("=== no-op / informational commands must not touch BlueZ ===");
    let c = run_cmd(
        &mut app,
        &s,
        Command::SetAudioProfile {
            device: ADDR.into(),
            profile: "HFP".into(),
        },
        250,
    );
    check(
        &mut r,
        "SetAudioProfile sends no BlueZ method",
        c.is_empty(),
    );
    let c = run_cmd(
        &mut app,
        &s,
        Command::RespondAuth {
            id: 999_999,
            response: AuthResponse::Reject,
        },
        250,
    );
    check(
        &mut r,
        "RespondAuth with no pending request sends no method",
        c.is_empty(),
    );
    let c = run_cmd(&mut app, &s, Command::RefreshNetworks(ADDR.into()), 250);
    check(
        &mut r,
        "RefreshNetworks sends no BlueZ method",
        c.is_empty(),
    );
    let c = run_cmd(&mut app, &s, Command::RefreshProfiles(ADDR.into()), 250);
    check(
        &mut r,
        "RefreshProfiles sends no BlueZ method",
        c.is_empty(),
    );

    println!("=== variant-typed / incomplete GetManagedObjects ===");
    {
        let mut o = build_objects();
        if let Some(ifaces) = o.get_mut(&opath(DEV)) {
            if let Some(d) = ifaces.get_mut(&iname("org.bluez.Device1")) {
                set(d, "RSSI", OwnedValue::from(-61i32)); // int32, not int16
                set(d, "TxPower", OwnedValue::from(7i64)); // int64
                set(d, "Class", OwnedValue::from(0x240404u64)); // uint64
                set(d, "Appearance", OwnedValue::from(0u8)); // byte
                d.remove("Modalias"); // missing property
                set(d, "UUIDs", ov_strings(&[])); // empty service list
            }
            if let Some(b) = ifaces.get_mut(&iname("org.bluez.Battery1")) {
                set(b, "Percentage", OwnedValue::from(42i32));
            }
        }
        if let Some(t) = o.get_mut(&opath("/org/bluez/obex/transfer0")) {
            if let Some(p) = t.get_mut(&iname("org.bluez.obex.Transfer1")) {
                set(p, "Size", OwnedValue::from(4096u32)); // uint32, not uint64
                set(p, "Transferred", OwnedValue::from(1024u32));
                set(p, "Status", ov_str("COMPLETE")); // mixed case
            }
        }
        *s.objects.lock().unwrap() = o;
    }
    let _ = run_cmd(&mut app, &s, Command::Refresh, 350);
    check(&mut r, "variant integer widths are parsed, not dropped", {
        app.devices.len() == 1
            && app.devices[0].rssi == Some(-61)
            && app.devices[0].tx_power == Some(7)
            && app.devices[0].class == Some(0x240404)
            && app.devices[0].appearance == Some(0)
            && app.devices[0].battery == Some(42)
            && app.devices[0].modalias.is_empty()
            && app.devices[0].uuids.is_empty()
    });
    check(&mut r, "OBEX uint32 sizes + mixed-case status", {
        app.transfers
            .first()
            .map(|t| {
                t.size == 4096 && t.transferred == 1024 && t.status == TransferStatus::Complete
            })
            .unwrap_or(false)
    });

    println!("=== a slow OBEX call must not stall the polling thread ===");
    s.slow_obex.store(true, Ordering::Relaxed);
    s.calls.lock().unwrap().clear();
    let t0 = Instant::now();
    app.dispatch(Command::SendFiles {
        device: ADDR.into(),
        files: vec![std::path::PathBuf::from("/tmp/contract-slow.jpg")],
    });
    app.dispatch(Command::Refresh);
    pump(&mut app, 350);
    check(
        &mut r,
        "an unrelated command is answered while OBEX is still pending",
        app.status.text.contains("Device list refreshed")
            && t0.elapsed() < Duration::from_millis(900),
    );
    pump(&mut app, 1400); // let the slow send finish before tearing down
    s.slow_obex.store(false, Ordering::Relaxed);

    println!("=== --disable-obex-services is honoured by the backend ===");
    s.calls.lock().unwrap().clear();
    let cfg2 = Config {
        demo: false,
        language: "en".into(),
        disable_obex_services: true,
        ..Config::default()
    };
    let mut app2 = App::new(cfg2.clone(), Box::new(BluezBackend::new(&cfg2)?));
    pump(&mut app2, 700);
    check(&mut r, "disabled backend does not probe/register OBEX", {
        let c = s.calls.lock().unwrap().clone();
        !has(
            &c,
            "AgentManager1",
            "RegisterAgent",
            "/org/bluez/obex/agent/bluetuith_rs",
        ) && !app2.features.obex
            && !app2.features.send_file
            && !app2.features.receive_file
    });
    let c = run_cmd(
        &mut app2,
        &s,
        Command::SendFiles {
            device: ADDR.into(),
            files: vec![std::path::PathBuf::from("/tmp/disabled.jpg")],
        },
        300,
    );
    check(
        &mut r,
        "disabled backend refuses SendFiles without a call",
        { c.is_empty() && app2.status.text.contains("OBEX services are disabled") },
    );

    println!("\n=== result: {} passed, {} failed ===", r.pass, r.fail);
    if r.fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}
