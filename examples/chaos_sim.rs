//! Model-driven chaos simulator — the most adversarial harness in the repo.
//!
//! Instead of hand-written scenarios or pure-function fuzzing, this drives the
//! WHOLE App state machine with a deterministic stream of random operations:
//! actions, keystrokes, mouse events, injected backend events (snapshot churn,
//! auth floods, device disappearance, extreme numeric values), real `tick()`
//! calls that drain the demo backend, and random extreme terminal resizes.
//!
//! After every single step it asserts a set of structural invariants and, on
//! a sample of steps, re-renders at the current size. Any panic or invariant
//! violation is caught with full context (step number, operation, state) so a
//! real bug can be reproduced from the fixed seed.
//!
//! Run: cargo run --release --example chaos_sim -- 300000

use bluetuith_rs::app::{App, Overlay};
use bluetuith_rs::backend::demo::DemoBackend;
use bluetuith_rs::backend::Event;
use bluetuith_rs::config::Config;
use bluetuith_rs::i18n::Lang;
use bluetuith_rs::keybind::Action;
use bluetuith_rs::model::*;
use bluetuith_rs::ui;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};

/// Print a failure with context and exit, flushing stdout first (it is block
/// buffered when piped, and `process::exit` would otherwise drop the message).
fn fail(step: u64, msg: &str, op: &str, a: &App) -> ! {
    println!("FAIL at step {step}: {msg}");
    println!("  op            = {op}");
    println!("  overlays      = {:?}", a.overlays);
    println!("  auth          = {:?}", a.auth);
    println!("  confirm       = {:?}", a.confirm);
    println!(
        "  selected      = {} (visible {})",
        a.selected,
        a.visible_devices().len()
    );
    println!(
        "  selected_ad   = {} (adapters {})",
        a.selected_adapter,
        a.adapters.len()
    );
    println!("  log len       = {}", a.log.len());
    std::io::stdout().flush().ok();
    std::process::exit(1);
}

// ---------------------------------------------------------------------------
// Deterministic RNG (splitmix64) — fixed seed => fully reproducible runs.
// ---------------------------------------------------------------------------
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        &s[(self.next() as usize) % s.len()]
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
    fn str(&mut self, max: usize) -> String {
        const CHARS: &[char] = &[
            'a', 'Z', '0', '9', ' ', '-', '_', '/', '.', '名', '👍', '\u{200d}', '1', '\u{fe0f}',
            '\u{20e3}', '\0', '\n', '\t',
        ];
        let len = (self.next() as usize) % (max + 1);
        (0..len).map(|_| *self.pick(CHARS)).collect()
    }
    fn addr(&mut self) -> String {
        let n = self.below(12);
        format!("AA:BB:CC:DD:EE:{n:02X}")
    }
}

const ACTIONS: &[Action] = &[
    Action::Menu,
    Action::MenuAdapter,
    Action::MenuDevice,
    Action::MenuView,
    Action::MenuHelp,
    Action::Select,
    Action::Cancel,
    Action::Suspend,
    Action::Quit,
    Action::Switch,
    Action::Close,
    Action::Help,
    Action::NavigateUp,
    Action::NavigateDown,
    Action::NavigateTop,
    Action::NavigateBottom,
    Action::Power,
    Action::Discoverable,
    Action::Pairable,
    Action::Scan,
    Action::ChangeAdapter,
    Action::ProgressView,
    Action::PlayerHide,
    Action::PlayerShow,
    Action::Connect,
    Action::Pair,
    Action::Trust,
    Action::Block,
    Action::SendFiles,
    Action::Network,
    Action::AudioProfiles,
    Action::Info,
    Action::Remove,
    Action::FileDirForward,
    Action::FileDirBack,
    Action::FileSelect,
    Action::FileInvertSelection,
    Action::FileSelectAll,
    Action::FileRefresh,
    Action::FileToggleHidden,
    Action::FileConfirmSelection,
    Action::TransferSuspend,
    Action::TransferResume,
    Action::TransferCancel,
    Action::PlayerTogglePlay,
    Action::PlayerNext,
    Action::PlayerPrevious,
    Action::PlayerSeekForward,
    Action::PlayerSeekBackward,
    Action::PlayerStop,
    Action::PlayerVolumeUp,
    Action::PlayerVolumeDown,
    Action::PlayerShuffle,
    Action::PlayerRepeat,
    Action::Search,
    Action::CycleSort,
    Action::ThemePicker,
    Action::ToggleDetails,
    Action::ShowLog,
    Action::Refresh,
    Action::CommandPalette,
    Action::About,
    Action::Language,
];

const KEYCODES: &[KeyCode] = &[
    KeyCode::Char('a'),
    KeyCode::Char('y'),
    KeyCode::Char('n'),
    KeyCode::Char('1'),
    KeyCode::Char('9'),
    KeyCode::Char('/'),
    KeyCode::Char('👍'),
    KeyCode::Enter,
    KeyCode::Esc,
    KeyCode::Backspace,
    KeyCode::Tab,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Delete,
    KeyCode::Null,
    KeyCode::F(1),
    KeyCode::F(12),
];

const MODS: &[KeyModifiers] = &[
    KeyModifiers::NONE,
    KeyModifiers::SHIFT,
    KeyModifiers::CONTROL,
    KeyModifiers::ALT,
    KeyModifiers::SUPER,
];

const SIZES: &[(u16, u16)] = &[
    (0, 0),
    (1, 1),
    (2, 1),
    (1, 2),
    (3, 3),
    (5, 2),
    (2, 5),
    (10, 3),
    (20, 6),
    (40, 12),
    (80, 24),
    (120, 40),
    (200, 60),
    (300, 100),
];

fn gen_device(r: &mut Rng) -> Device {
    Device {
        path: format!("/org/bluez/hci0/dev_{}", r.addr().replace(':', "_")),
        address: r.addr(),
        adapter: "/org/bluez/hci0".into(),
        name: r.str(20),
        alias: r.str(12),
        paired: r.chance(50),
        connected: r.chance(30),
        trusted: r.chance(20),
        blocked: r.chance(10),
        legacy_pairing: r.chance(10),
        services_resolved: r.chance(50),
        rssi: if r.chance(80) {
            Some((r.below(200) as i16) - 100)
        } else {
            None
        },
        tx_power: if r.chance(50) {
            Some((r.below(40) as i16) - 20)
        } else {
            None
        },
        battery: if r.chance(50) {
            Some(r.below(101) as u8)
        } else {
            None
        },
        class: if r.chance(50) {
            Some(r.below(u32::MAX as u64) as u32)
        } else {
            None
        },
        appearance: if r.chance(50) {
            Some(r.below(u16::MAX as u64) as u16)
        } else {
            None
        },
        icon: r.str(10),
        modalias: r.str(10),
        manufacturer: r.str(10),
        uuids: (0..r.below(6)).map(|_| r.str(36)).collect(),
    }
}

fn gen_adapter(r: &mut Rng) -> Adapter {
    Adapter {
        path: format!("/org/bluez/hci{}", r.below(3)),
        id: format!("hci{}", r.below(3)),
        address: r.addr(),
        name: r.str(12),
        alias: r.str(12),
        powered: r.chance(80),
        discoverable: r.chance(30),
        pairable: r.chance(50),
        discovering: r.chance(30),
    }
}

fn gen_snapshot(r: &mut Rng) -> Event {
    let ndev = r.below(30) as usize;
    let nad = r.below(3) as usize + 1;
    Event::Snapshot {
        adapters: (0..nad).map(|_| gen_adapter(r)).collect(),
        devices: (0..ndev).map(|_| gen_device(r)).collect(),
    }
}

fn gen_event(r: &mut Rng, step: u64) -> Event {
    // Bias toward snapshot churn and auth floods — these are the two paths
    // that mutate the most state and are most likely to expose a desync.
    match r.below(100) {
        0..=29 => gen_snapshot(r),
        30..=49 => Event::AuthRequest(AuthRequest {
            id: (step % 97) as usize, // deliberately reused ids / out-of-order
            device: r.addr(),
            device_name: r.str(20),
            kind: match r.below(7) {
                0 => AuthKind::PinCode,
                1 => AuthKind::Passkey(r.below(1_000_000) as u32),
                2 => AuthKind::Confirmation(r.below(1_000_000) as u32),
                3 => AuthKind::Authorization,
                4 => AuthKind::DisplayPinCode(r.str(8)),
                5 => AuthKind::DisplayPasskey(r.below(1_000_000) as u32),
                _ => AuthKind::FilePush(r.str(30)),
            },
        }),
        50..=59 => Event::Status(StatusMessage {
            level: match r.below(5) {
                0 => Level::Info,
                1 => Level::Success,
                2 => Level::Warning,
                3 => Level::Error,
                _ => Level::Busy,
            },
            text: r.str(60),
            busy: r.chance(30),
        }),
        60..=66 => Event::Log(r.str(80)),
        67..=73 => Event::Transfers(
            (0..r.below(8))
                .map(|i| Transfer {
                    id: (step + i) as usize,
                    name: r.str(30),
                    device: r.addr(),
                    device_name: r.str(20),
                    size: if r.chance(20) { 0 } else { r.next() },
                    transferred: r.next(),
                    direction: if r.chance(50) {
                        Direction::Incoming
                    } else {
                        Direction::Outgoing
                    },
                    status: match r.below(5) {
                        0 => TransferStatus::Active,
                        1 => TransferStatus::Suspended,
                        2 => TransferStatus::Complete,
                        3 => TransferStatus::Error,
                        _ => TransferStatus::Cancelled,
                    },
                    error: r.str(20),
                    started_at: r.next(),
                })
                .collect(),
        ),
        74..=80 => Event::Player(if r.chance(20) {
            None
        } else {
            Some(PlayerState {
                device: r.addr(),
                device_name: r.str(20),
                status: match r.below(6) {
                    0 => PlayStatus::Stopped,
                    1 => PlayStatus::Playing,
                    2 => PlayStatus::Paused,
                    3 => PlayStatus::ForwardSeek,
                    4 => PlayStatus::ReverseSeek,
                    _ => PlayStatus::Error,
                },
                title: r.str(40),
                artist: r.str(30),
                album: r.str(30),
                genre: r.str(20),
                track_number: r.below(100) as u32,
                track_count: r.below(100) as u32,
                duration_ms: if r.chance(20) { 0 } else { r.next() },
                position_ms: r.next(),
                shuffle: r.chance(50),
                repeat: match r.below(4) {
                    0 => RepeatMode::Off,
                    1 => RepeatMode::Single,
                    2 => RepeatMode::All,
                    _ => RepeatMode::Group,
                },
                volume: r.below(101) as u8,
                equalizer: r.str(20),
            })
        }),
        81..=86 => Event::Networks(
            (0..r.below(6))
                .map(|i| Network {
                    id: i as usize,
                    device: r.addr(),
                    device_name: r.str(20),
                    role: match r.below(3) {
                        0 => NetworkRole::Panu,
                        1 => NetworkRole::Nap,
                        _ => NetworkRole::Dun,
                    },
                    connected: r.chance(50),
                    interface: r.str(12),
                    address: r.str(20),
                })
                .collect(),
        ),
        87..=91 => Event::Profiles(
            (0..r.below(6))
                .map(|_| ProfileInfo {
                    name: r.str(20),
                    uuid: r.str(36),
                    description: r.str(30),
                    active: r.chance(50),
                    device: r.addr(),
                })
                .collect(),
        ),
        92..=95 => Event::Features(FeatureSet {
            adapter: r.chance(80),
            device: r.chance(80),
            send_file: r.chance(50),
            receive_file: r.chance(50),
            network: r.chance(50),
            media_player: r.chance(50),
            audio_profiles: r.chance(50),
            obex: r.chance(50),
        }),
        _ => Event::Snapshot {
            adapters: vec![gen_adapter(r)],
            devices: vec![],
        },
    }
}

fn gen_key(r: &mut Rng) -> KeyEvent {
    KeyEvent::new(*r.pick(KEYCODES), *r.pick(MODS))
}

fn gen_mouse(r: &mut Rng) -> MouseEvent {
    let btn = *r.pick(&[MouseButton::Left, MouseButton::Right, MouseButton::Middle]);
    let kind = match r.below(7) {
        0 => MouseEventKind::Down(btn),
        1 => MouseEventKind::Up(btn),
        2 => MouseEventKind::Drag(btn),
        3 => MouseEventKind::Moved,
        4 => MouseEventKind::ScrollUp,
        5 => MouseEventKind::ScrollDown,
        _ => MouseEventKind::ScrollLeft,
    };
    MouseEvent {
        kind,
        column: r.below(320) as u16,
        row: r.below(110) as u16,
        modifiers: *r.pick(MODS),
    }
}

// ---------------------------------------------------------------------------
// Structural invariants — checked after every step.
// ---------------------------------------------------------------------------
/// Hard upper bounds for every collection in the App. These are tripwires for
/// slow leaks: any field that grows past its bound is being appended to without
/// ever being cleared/replaced, which would eventually exhaust memory.
const COLLECTION_BOUNDS: &[(&str, usize)] = &[
    ("log", 500),
    ("overlays", 256),
    ("devices", 64),
    ("adapters", 16),
    ("transfers", 64),
    ("networks", 64),
    ("profiles", 64),
    ("menu_items", 128),
    ("palette_items", 256),
    ("palette_categories", 64),
];

fn collection_lengths(a: &App) -> [usize; COLLECTION_BOUNDS.len()] {
    [
        a.log.len(),
        a.overlays.len(),
        a.devices.len(),
        a.adapters.len(),
        a.transfers.len(),
        a.networks.len(),
        a.profiles.len(),
        a.menu_items.len(),
        a.palette_items.len(),
        a.palette_categories.len(),
    ]
}

fn check_invariants(a: &App) -> Result<(), String> {
    if a.log.len() > 500 {
        return Err(format!("log grew to {} (>500)", a.log.len()));
    }
    let lens = collection_lengths(a);
    for ((name, bound), len) in COLLECTION_BOUNDS.iter().zip(lens.iter()) {
        if *len > *bound {
            return Err(format!(
                "collection '{name}' leaked to {len} (bound {bound})"
            ));
        }
    }
    if let Some(fp) = &a.file_picker {
        if fp.entries.len() > 100_000 {
            return Err(format!(
                "file_picker entries leaked to {}",
                fp.entries.len()
            ));
        }
    }
    let auth_overlays = a.overlays.iter().filter(|&&o| o == Overlay::Auth).count();
    let confirm_overlays = a
        .overlays
        .iter()
        .filter(|&&o| o == Overlay::Confirm)
        .count();
    if auth_overlays > 1 {
        return Err(format!("{auth_overlays} duplicate Auth overlays"));
    }
    if confirm_overlays > 1 {
        return Err(format!("{confirm_overlays} duplicate Confirm overlays"));
    }
    if a.auth.is_some() != (auth_overlays == 1) {
        return Err(format!(
            "auth/overlay desync: auth={} auth_overlays={} overlays={:?}",
            a.auth.is_some(),
            auth_overlays,
            a.overlays
        ));
    }
    if a.confirm.is_some() != (confirm_overlays == 1) {
        return Err(format!(
            "confirm/overlay desync: confirm={} confirm_overlays={} overlays={:?}",
            a.confirm.is_some(),
            confirm_overlays,
            a.overlays
        ));
    }
    let vis = a.visible_devices().len();
    if vis == 0 {
        if a.selected != 0 {
            return Err(format!("selected={} with 0 visible devices", a.selected));
        }
    } else if a.selected >= vis {
        return Err(format!(
            "selected={} out of range for {vis} visible devices",
            a.selected
        ));
    }
    if a.adapters.is_empty() {
        if a.selected_adapter != 0 {
            return Err(format!(
                "selected_adapter={} with 0 adapters",
                a.selected_adapter
            ));
        }
    } else if a.selected_adapter >= a.adapters.len() {
        return Err(format!(
            "selected_adapter={} out of range for {} adapters",
            a.selected_adapter,
            a.adapters.len()
        ));
    }
    if a.menu_index >= a.menu_items.len().saturating_add(1) {
        return Err(format!(
            "menu_index={} with {} menu items",
            a.menu_index,
            a.menu_items.len()
        ));
    }
    if a.overlay_index > 1_000_000 || a.overlay_scroll > 1_000_000 {
        return Err(format!(
            "overlay index/scroll runaway: {} / {}",
            a.overlay_index, a.overlay_scroll
        ));
    }
    if let Some(p) = &a.player {
        let prog = p.progress();
        if !prog.is_finite() {
            return Err(format!("player progress not finite: {prog}"));
        }
    }
    for t in &a.transfers {
        let prog = t.progress();
        if !prog.is_finite() {
            return Err(format!("transfer {} progress not finite: {prog}", t.id));
        }
    }
    Ok(())
}

fn fresh(r: &mut Rng) -> (App, Terminal<TestBackend>) {
    let cfg = Config {
        demo: true,
        confirm_on_quit: r.chance(50),
        ..Config::default()
    };
    let app = App::new(cfg, Box::new(DemoBackend::new()));
    let term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    (app, term)
}

fn main() {
    let steps: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(300_000);
    let seed: u64 = std::env::args()
        .nth(2)
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(&s);
            u64::from_str_radix(s, 16).ok()
        })
        .unwrap_or(0xC0FFEE_DEADBEEF);

    println!("=== chaos sim: {steps} steps, seed {seed:#x} ===");
    let mut r = Rng::new(seed);
    let (mut app, mut term) = fresh(&mut r);
    // Seed a realistic initial world.
    app.apply_event(gen_snapshot(&mut r));

    let mut max_overlays = 0usize;
    let mut max_lens = [0usize; COLLECTION_BOUNDS.len()];
    let mut resizes = 0u64;
    let mut renders = 0u64;

    for step in 0..steps {
        // Periodically unwind the whole overlay stack. Random action storms
        // would otherwise stack thousands of dialogs — something no real user
        // does, and which makes each render O(depth) and the run needlessly
        // slow. Bounding depth keeps the test realistic while still stressing
        // moderate nesting, and exercises the close path (auth reject/confirm
        // clearing) on whatever happens to be on the stack.
        let op: String;
        if r.chance(2) {
            op = "unwind::all".into();
            while !app.overlays.is_empty() {
                app.close_overlay();
            }
        } else if r.chance(2) {
            // A short auth burst every so often: floods the one-prompt invariant.
            let burst = 2 + r.below(5);
            op = format!("AUTH-BURST x{burst}");
            for _ in 0..burst {
                app.apply_event(gen_event(&mut r, step));
            }
        } else {
            let k = r.below(100);
            match k {
                0..=24 => {
                    let act = *r.pick(ACTIONS);
                    op = format!("action::{act:?}");
                    app.do_action(act);
                }
                25..=49 => {
                    let key = gen_key(&mut r);
                    op = format!("key::{key:?}");
                    app.on_key(key);
                }
                50..=59 => {
                    let m = gen_mouse(&mut r);
                    op = format!("mouse::{:?}@({},{} )", m.kind, m.column, m.row);
                    app.on_mouse(m);
                }
                60..=79 => {
                    let ev = gen_event(&mut r, step);
                    op = format!("event::{:?}", std::mem::discriminant(&ev));
                    app.apply_event(ev);
                }
                80..=94 => {
                    op = "tick".into();
                    app.tick();
                }
                95..=97 => {
                    let (w, h) = *r.pick(SIZES);
                    op = format!("resize::{w}x{h}");
                    term = Terminal::new(TestBackend::new(w, h)).unwrap();
                    resizes += 1;
                }
                _ => {
                    // Benign direct state pokes that the UI itself performs.
                    match r.below(5) {
                        0 => {
                            // Mirror the real search_key(): every filter edit
                            // resets the selection to the first row.
                            app.filter = r.str(30);
                            app.selected = 0;
                            op = "poke::filter".into();
                        }
                        1 => {
                            app.search_active = !app.search_active;
                            op = "poke::search".into();
                        }
                        2 => {
                            app.show_details = !app.show_details;
                            op = "poke::details".into();
                        }
                        3 => {
                            app.lang = if r.chance(50) { Lang::Zh } else { Lang::En };
                            op = "poke::lang".into();
                        }
                        _ => {
                            let vis = app.visible_devices().len();
                            if vis > 0 {
                                app.selected = (r.next() as usize) % vis;
                            }
                            op = "poke::selected".into();
                        }
                    }
                }
            }
        }

        // Catch panics with full context.
        let result = panic::catch_unwind(AssertUnwindSafe(|| check_invariants(&app)));
        let inv = match result {
            Ok(v) => v,
            Err(_) => Err("PANIC in invariant check".into()),
        };
        max_overlays = max_overlays.max(app.overlays.len());
        let cur_lens = collection_lengths(&app);
        for (m, &c) in max_lens.iter_mut().zip(cur_lens.iter()) {
            *m = (*m).max(c);
        }

        // Periodically re-render at the current (possibly extreme) size.
        if step % 16 == 0 {
            let rr = panic::catch_unwind(AssertUnwindSafe(|| {
                term.draw(|f| ui::render(f, &mut app)).is_ok()
            }));
            match rr {
                Ok(true) => renders += 1,
                Ok(false) => fail(step, "render returned Err", &op, &app),
                Err(_) => fail(step, "render PANICKED", &op, &app),
            }
        }

        if let Err(msg) = inv {
            fail(step, &msg, &op, &app);
        }

        if step > 0 && step % 50_000 == 0 {
            println!("  ... {step} steps (max overlays {max_overlays})");
        }
    }

    println!(
        "\n=== PASS: {steps} steps, seed {seed:#x} | resizes {resizes} | renders {renders} | max overlay depth {max_overlays} ==="
    );
    println!("  peak collection lengths:");
    for ((name, bound), m) in COLLECTION_BOUNDS.iter().zip(max_lens.iter()) {
        println!("    {name:<20} peak {m:>6} / bound {bound}");
    }
}
