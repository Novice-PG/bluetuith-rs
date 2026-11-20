//! Exhaustive state-space explorer — complements the random chaos_sim.
//!
//! Where chaos_sim wanders randomly, this systematically enumerates:
//!   1. Every single overlay, every ordered pair and triple of overlays
//!      (15 + 15*14 + 15*14*13 ≈ 3300 stacks), and renders each at every
//!      extreme terminal size, with one of several adversarial App states.
//!   2. Every combination is rendered with a numerically/unicode-poisoned
//!      state (huge values, malformed UUIDs, emoji ZWJ, empty collections) so
//!      any overlay that indexes/slices/formats a field is stressed.
//!
//! Any panic is caught with full context (stack, size, state kind) so it can
//! be reproduced. This is designed to find render-path panics that random
//! sampling may rarely hit.
//!
//! Run: cargo run --release --example state_explorer

use bluetuith_rs::app::{App, ConfirmAction, ConfirmState, Overlay};
use bluetuith_rs::backend::demo::DemoBackend;
use bluetuith_rs::config::Config;
use bluetuith_rs::model::*;
use bluetuith_rs::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::panic::{self, AssertUnwindSafe};

const OVERLAYS: &[Overlay] = &[
    Overlay::Menu,
    Overlay::Help,
    Overlay::Confirm,
    Overlay::AdapterChange,
    Overlay::DeviceInfo,
    Overlay::FilePicker,
    Overlay::Progress,
    Overlay::Player,
    Overlay::Network,
    Overlay::Profiles,
    Overlay::Auth,
    Overlay::ThemePicker,
    Overlay::Log,
    Overlay::CommandPalette,
    Overlay::About,
];

// Extreme sizes: zero, one-cell, very narrow/tall, wide/short, normal, huge.
const SIZES: &[(u16, u16)] = &[
    (0, 0),
    (0, 10),
    (10, 0),
    (1, 1),
    (1, 40),
    (40, 1),
    (2, 2),
    (3, 50),
    (50, 3),
    (8, 8),
    (20, 4),
    (60, 10),
    (80, 24),
    (120, 40),
    (200, 80),
];

fn base_app() -> App {
    let cfg = Config {
        demo: true,
        language: "en".into(),
        confirm_on_quit: true,
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..3 {
        a.tick();
    }
    a
}

/// Fill every backing field with adversarial values so any overlay that
/// reads/formats/slices a field hits an edge case.
fn poison_state(a: &mut App, mode: u8) {
    // A device with a malformed (36-byte but multi-byte) UUID, emoji names,
    // extreme numerics and empty/overflowing collections.
    let bad_uuid = format!("{}x", "1️⃣".repeat(5)); // 36 bytes, slices inside a char
    let name = match mode {
        0 => "X".repeat(5000),
        1 => "👨‍👩‍👧名👍\u{200d}".repeat(40),
        _ => String::new(),
    };
    let mut d = Device {
        address: "AA:BB:CC:DD:EE:FF".into(),
        path: "/org/bluez/hci0/dev_X".into(),
        name: name.clone(),
        alias: name,
        paired: true,
        connected: true,
        rssi: Some(i16::MIN),
        tx_power: Some(i16::MAX),
        battery: Some(255),
        class: Some(u32::MAX),
        appearance: Some(u16::MAX),
        icon: "👾".into(),
        modalias: "m".repeat(2000),
        manufacturer: "名".repeat(100),
        ..Default::default()
    };
    d.uuids = match mode {
        0 => vec![bad_uuid, "0x1101".into()],
        1 => vec!["0000110b-0000-1000-8000-00805f9b34fb".into(), bad_uuid],
        _ => vec![],
    };
    a.devices = vec![d];
    a.adapters = vec![Adapter {
        path: "/org/bluez/hci0".into(),
        id: "hci0".into(),
        address: "AA:BB:CC:DD:EE:FF".into(),
        name: "🔥".repeat(100),
        alias: "名".repeat(100),
        powered: true,
        discovering: true,
        ..Default::default()
    }];
    a.selected = 0;
    a.selected_adapter = 0;
    a.filter = match mode {
        0 => "名".repeat(500),
        1 => "\0\t\n".repeat(100),
        _ => String::new(),
    };
    a.status.text = "🔥".repeat(300);
    a.log = (0..600)
        .map(|i| format!("log {i} {}", "名".repeat(20)))
        .collect();
    a.transfers = (0..10)
        .map(|i| Transfer {
            id: i,
            name: "📦".repeat(100),
            device: "AA:BB:CC:DD:EE:FF".into(),
            device_name: "名".repeat(50),
            size: if mode == 0 { 0 } else { u64::MAX },
            transferred: u64::MAX,
            direction: Direction::Incoming,
            status: TransferStatus::Error,
            error: "err".repeat(100),
            started_at: u64::MAX,
        })
        .collect();
    a.player = Some(PlayerState {
        device: "AA:BB:CC:DD:EE:FF".into(),
        device_name: "名".repeat(50),
        status: PlayStatus::Playing,
        title: "🎵".repeat(200),
        artist: "名".repeat(200),
        album: "🧊".repeat(200),
        genre: "x".repeat(200),
        track_number: u32::MAX,
        track_count: u32::MAX,
        duration_ms: if mode == 0 { 0 } else { u64::MAX },
        position_ms: u64::MAX,
        volume: 255,
        equalizer: "名".repeat(50),
        ..Default::default()
    });
    a.networks = (0..10)
        .map(|i| Network {
            id: i,
            device: "AA:BB:CC:DD:EE:FF".into(),
            device_name: "名".repeat(50),
            interface: "bnep0".into(),
            address: "1.2.3.4".into(),
            ..Default::default()
        })
        .collect();
    a.profiles = (0..10)
        .map(|_| ProfileInfo {
            name: "🎧".repeat(50),
            uuid: format!("{}y", "1️⃣".repeat(5)),
            description: "名".repeat(100),
            device: "AA:BB:CC:DD:EE:FF".into(),
            ..Default::default()
        })
        .collect();
    // Backing state for the stateful overlays.
    a.auth = Some(AuthRequest {
        id: usize::MAX,
        device: "AA:BB:CC:DD:EE:FF".into(),
        device_name: "名".repeat(50),
        kind: match mode {
            0 => AuthKind::PinCode,
            1 => AuthKind::FilePush("../../evil名👍".into()),
            _ => AuthKind::Passkey(u32::MAX),
        },
    });
    a.auth_input = "9".repeat(200);
    a.confirm = Some(ConfirmState {
        text: "名".repeat(300),
        action: ConfirmAction::Quit,
    });
    a.overlay_index = match mode {
        0 => usize::MAX,
        1 => 50,
        _ => 0,
    };
    a.overlay_scroll = match mode {
        0 => usize::MAX,
        _ => 25,
    };
}

/// Set up an overlay stack with backing state for every stateful overlay.
fn set_stack(a: &mut App, stack: &[Overlay]) {
    a.overlays.clear();
    for &o in stack {
        // push() calls on_open() which sets up FilePicker/Menu/Palette state.
        a.push(o);
        // Restore state that on_open() may have reset.
        match o {
            Overlay::Auth => {
                a.auth = Some(AuthRequest {
                    id: 7,
                    device: "AA:BB:CC:DD:EE:FF".into(),
                    device_name: "名".repeat(30),
                    kind: AuthKind::Confirmation(123456),
                });
            }
            Overlay::Confirm => {
                a.confirm = Some(ConfirmState {
                    text: "名".repeat(80),
                    action: ConfirmAction::Quit,
                });
            }
            _ => {}
        }
    }
}

fn render_all_sizes(a: &mut App, stack: &[Overlay], label: &str) -> usize {
    let mut ok = 0;
    for &(w, h) in SIZES {
        let r = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| ui::render(f, a)).is_ok()
        }));
        match r {
            Ok(true) => ok += 1,
            Ok(false) => {
                panic::set_hook(Box::new(|_| {}));
                panic!("render Err | {label} | stack={stack:?} | {w}x{h}");
            }
            Err(_) => {
                panic!("render PANIC | {label} | stack={stack:?} | {w}x{h}");
            }
        }
    }
    ok
}

fn main() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut stacks = 0u64;
    let mut renders = 0u64;

    // Mode 1: every single overlay.
    for &o in OVERLAYS {
        for mode in 0..3u8 {
            let mut a = base_app();
            poison_state(&mut a, mode);
            let stack = [o];
            set_stack(&mut a, &stack);
            renders += render_all_sizes(&mut a, &stack, &format!("single m{mode}")) as u64;
            stacks += 1;
        }
    }

    // Mode 2: every ordered pair.
    for &a0 in OVERLAYS {
        for &a1 in OVERLAYS {
            if a0 == a1 {
                continue;
            }
            let mut a = base_app();
            poison_state(&mut a, (stacks % 3) as u8);
            let stack = [a0, a1];
            set_stack(&mut a, &stack);
            renders += render_all_sizes(&mut a, &stack, "pair") as u64;
            stacks += 1;
        }
    }

    // Mode 3: FULLY exhaustive ordered triples (15×14×13 = 2730).
    for &a0 in OVERLAYS {
        for &a1 in OVERLAYS {
            if a1 == a0 {
                continue;
            }
            for &a2 in OVERLAYS {
                if a2 == a0 || a2 == a1 {
                    continue;
                }
                let mut a = base_app();
                poison_state(&mut a, (stacks % 3) as u8);
                let stack = [a0, a1, a2];
                set_stack(&mut a, &stack);
                renders += render_all_sizes(&mut a, &stack, "triple") as u64;
                stacks += 1;
            }
        }
    }

    // Mode 4: sampled ordered quadruples (rotating offsets), to exercise
    // deeper stacks without the O(15^4) explosion.
    for (i, &a0) in OVERLAYS.iter().enumerate() {
        for j in 0..8u8 {
            let a1 = OVERLAYS[(i + j as usize + 1) % OVERLAYS.len()];
            let a2 = OVERLAYS[(i + j as usize * 3 + 2) % OVERLAYS.len()];
            let a3 = OVERLAYS[(i + j as usize * 5 + 7) % OVERLAYS.len()];
            if a1 == a0 || a2 == a0 || a2 == a1 || a3 == a0 || a3 == a1 || a3 == a2 {
                continue;
            }
            let mut a = base_app();
            poison_state(&mut a, j % 3);
            let stack = [a0, a1, a2, a3];
            set_stack(&mut a, &stack);
            renders += render_all_sizes(&mut a, &stack, "quad") as u64;
            stacks += 1;
        }
    }

    panic::set_hook(hook);
    println!(
        "PASS: {stacks} overlay stacks × {} sizes = {renders} renders, no panics",
        SIZES.len()
    );
}
