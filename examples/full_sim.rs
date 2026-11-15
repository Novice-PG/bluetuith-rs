//! Full functional simulation: drives the real `App` state machine against the
//! demo backend and asserts that every visible feature actually does something.
//!
//! Run with `cargo run --example full_sim`. Each probe is isolated (fresh App)
//! and panic-caught so one broken feature does not hide the rest. A non-zero
//! exit code means at least one check failed.

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use bluetuith_rs::app::{App, ConfirmAction, ConfirmState, MenuId, Overlay};
use bluetuith_rs::backend::demo::DemoBackend;
use bluetuith_rs::config::Config;
use bluetuith_rs::i18n::Lang;
use bluetuith_rs::keybind::Action;
use bluetuith_rs::model::*;
use bluetuith_rs::ui;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

struct Results {
    pass: u32,
    fail: u32,
}

fn probe<F: FnOnce() -> bool>(r: &mut Results, name: &str, f: F) {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let out = panic::catch_unwind(AssertUnwindSafe(f));
    panic::set_hook(hook);
    match out {
        Ok(true) => {
            r.pass += 1;
            println!("  ok   {name}");
        }
        Ok(false) => {
            r.fail += 1;
            println!("  FAIL {name}");
        }
        Err(_) => {
            r.fail += 1;
            println!("  PANIC {name}");
        }
    }
}

fn fresh(lang: &str) -> App {
    let cfg = Config {
        demo: true,
        language: lang.into(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        a.tick();
    }
    a
}

fn press(a: &mut App, action: Action) {
    let s = a.keys.get(action);
    a.on_key(KeyEvent::new(s.code, s.mods));
}

fn raw(a: &mut App, code: KeyCode, mods: KeyModifiers) {
    a.on_key(KeyEvent::new(code, mods));
}

fn tick(a: &mut App) {
    for _ in 0..2 {
        a.tick();
    }
}

fn render(a: &mut App, w: u16, h: u16) -> bool {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| ui::render(f, a)).is_ok()
}

fn render_text(a: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| ui::render(f, a)).unwrap();
    let b = t.backend().buffer();
    let mut s = String::new();
    for y in 0..b.area.height {
        for x in 0..b.area.width {
            s.push_str(b.cell((x, y)).unwrap().symbol());
        }
        s.push('\n');
    }
    s
}

fn mouse(a: &mut App, kind: MouseEventKind, _button: MouseButton, x: u16, y: u16) {
    a.on_mouse(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    });
}

/// Collect every action the palette can offer, by walking all six categories
/// through the public keyboard path (no private helpers).
fn palette_actions() -> Vec<Action> {
    let mut out = Vec::new();
    for cat_idx in 0..6 {
        let mut a = fresh("en");
        a.push(Overlay::CommandPalette);
        for _ in 0..cat_idx {
            press(&mut a, Action::NavigateDown);
        }
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        if a.palette_stage == 1 {
            out.extend(a.palette_items.iter().copied());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn select_addr(a: &mut App, addr: &str) {
    let i = a.visible_devices().iter().position(|d| d.address == addr);
    if let Some(i) = i {
        a.selected = i;
    }
}

/// A coarse fingerprint of everything the user can observe.
fn snapshot(a: &App) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "ov={:?} idx={} scroll={}",
        a.overlays, a.overlay_index, a.overlay_scroll
    );
    let _ = writeln!(
        s,
        "sel={} sort={:?} filter={:?} details={} playerbar={}",
        a.selected, a.sort, a.filter, a.show_details, a.show_player_bar
    );
    let _ = writeln!(
        s,
        "lang={:?} theme={} running={} suspend={}",
        a.lang, a.config.theme, a.running, a.suspend_requested
    );
    let _ = writeln!(s, "status={:?}", a.status.text);
    for d in &a.devices {
        let _ = writeln!(
            s,
            "dev {}: p{} c{} t{} b{}",
            d.address, d.paired, d.connected, d.trusted, d.blocked
        );
    }
    for ad in &a.adapters {
        let _ = writeln!(
            s,
            "ad {}: p{} d{} pa{} s{}",
            ad.id, ad.powered, ad.discoverable, ad.pairable, ad.discovering
        );
    }
    for t in &a.transfers {
        let _ = writeln!(s, "tr {}: {:?}", t.id, t.status);
    }
    for n in &a.networks {
        let _ = writeln!(s, "nw {}: {:?} {}", n.id, n.role, n.connected);
    }
    for p in &a.profiles {
        let _ = writeln!(s, "pf {}: {}", p.name, p.active);
    }
    if let Some(p) = &a.player {
        let _ = writeln!(
            s,
            "pl {:?} tr{} vol{} pos{}",
            p.status, p.track_number, p.volume, p.position_ms
        );
    }
    s
}

fn main() {
    let mut r = Results { pass: 0, fail: 0 };
    println!("=== boot / snapshot ===");
    probe(&mut r, "fresh app has adapters + devices", || {
        let a = fresh("en");
        !a.adapters.is_empty() && !a.devices.is_empty() && a.features.send_file
    });
    probe(&mut r, "every overlay opens", || {
        let mut a = fresh("en");
        let all = [
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
        for o in all {
            a.confirm = Some(ConfirmState {
                text: "t".into(),
                action: ConfirmAction::Quit,
            });
            a.auth = Some(AuthRequest {
                id: 1,
                device: "x".into(),
                device_name: "x".into(),
                kind: AuthKind::Confirmation(1),
            });
            a.push(o);
            if a.top_overlay() != Some(o) {
                return false;
            }
            a.close_overlay();
        }
        true
    });

    println!("=== every overlay renders at 5 sizes ===");
    let all_overlays = [
        ("menu", Overlay::Menu),
        ("help", Overlay::Help),
        ("confirm", Overlay::Confirm),
        ("adapters", Overlay::AdapterChange),
        ("info", Overlay::DeviceInfo),
        ("files", Overlay::FilePicker),
        ("progress", Overlay::Progress),
        ("player", Overlay::Player),
        ("network", Overlay::Network),
        ("profiles", Overlay::Profiles),
        ("auth", Overlay::Auth),
        ("themes", Overlay::ThemePicker),
        ("log", Overlay::Log),
        ("palette", Overlay::CommandPalette),
        ("about", Overlay::About),
    ];
    for (name, o) in all_overlays {
        let nm = name.to_string();
        probe(
            &mut r,
            &format!("render {nm} (40x12 .. 150x44)"),
            move || {
                let mut a = fresh("en");
                a.confirm = Some(ConfirmState {
                    text: "Quit?".into(),
                    action: ConfirmAction::Quit,
                });
                a.auth = Some(AuthRequest {
                    id: 1,
                    device: "x".into(),
                    device_name: "x".into(),
                    kind: AuthKind::Confirmation(123456),
                });
                a.push(o);
                for (w, h) in [(40, 12), (60, 20), (80, 24), (120, 40), (150, 44)] {
                    if !render(&mut a, w, h) {
                        return false;
                    }
                    // Hit-test regions must lie inside the drawn overlay.
                    if let Some(outer) = a.layout.overlay_rect {
                        for (rr, _) in &a.layout.overlay_items {
                            if rr.x < outer.x
                                || rr.y < outer.y
                                || rr.x + rr.width > outer.x + outer.width
                                || rr.y + rr.height > outer.y + outer.height
                            {
                                return false;
                            }
                        }
                    }
                }
                true
            },
        );
    }

    println!("=== global actions are non-panicking and observable ===");
    probe(&mut r, "all actions: press + tick + render", || {
        for a in Action::all() {
            for lang in ["en", "zh"] {
                let mut app = fresh(lang);
                press(&mut app, a);
                tick(&mut app);
                if !render(&mut app, 100, 30) {
                    return false;
                }
            }
        }
        true
    });

    println!("=== adapter / device actions really mutate state ===");
    probe(&mut r, "Power toggles adapter.powered", || {
        let mut a = fresh("en");
        let before = a.adapters[0].powered;
        press(&mut a, Action::Power);
        tick(&mut a);
        a.adapters[0].powered != before
    });
    probe(&mut r, "Discoverable toggles adapter.discoverable", || {
        let mut a = fresh("en");
        let before = a.adapters[0].discoverable;
        press(&mut a, Action::Discoverable);
        tick(&mut a);
        a.adapters[0].discoverable != before
    });
    probe(&mut r, "Pairable toggles adapter.pairable", || {
        let mut a = fresh("en");
        let before = a.adapters[0].pairable;
        press(&mut a, Action::Pairable);
        tick(&mut a);
        a.adapters[0].pairable != before
    });
    probe(&mut r, "Scan toggles adapter.discovering", || {
        let mut a = fresh("en");
        let before = a.adapters[0].discovering;
        press(&mut a, Action::Scan);
        tick(&mut a);
        a.adapters[0].discovering != before
    });
    probe(&mut r, "Connect disconnects a connected device", || {
        let mut a = fresh("en");
        select_addr(&mut a, "AA:BB:CC:DD:01:01");
        press(&mut a, Action::Connect);
        tick(&mut a);
        !a.devices
            .iter()
            .find(|d| d.address == "AA:BB:CC:DD:01:01")
            .unwrap()
            .connected
    });
    probe(&mut r, "Trust toggles device.trusted", || {
        let mut a = fresh("en");
        select_addr(&mut a, "AA:BB:CC:DD:01:01");
        let before = a
            .devices
            .iter()
            .find(|d| d.address == "AA:BB:CC:DD:01:01")
            .unwrap()
            .trusted;
        press(&mut a, Action::Trust);
        tick(&mut a);
        a.devices
            .iter()
            .find(|d| d.address == "AA:BB:CC:DD:01:01")
            .unwrap()
            .trusted
            != before
    });
    probe(&mut r, "Block toggles device.blocked", || {
        let mut a = fresh("en");
        select_addr(&mut a, "AA:BB:CC:DD:01:01");
        let before = a
            .devices
            .iter()
            .find(|d| d.address == "AA:BB:CC:DD:01:01")
            .unwrap()
            .blocked;
        press(&mut a, Action::Block);
        tick(&mut a);
        a.devices
            .iter()
            .find(|d| d.address == "AA:BB:CC:DD:01:01")
            .unwrap()
            .blocked
            != before
    });
    probe(&mut r, "Remove + confirm deletes the device", || {
        let mut a = fresh("en");
        select_addr(&mut a, "AA:BB:CC:DD:01:01");
        press(&mut a, Action::Remove);
        tick(&mut a);
        if a.top_overlay() != Some(Overlay::Confirm) {
            return false;
        }
        raw(&mut a, KeyCode::Char('y'), KeyModifiers::NONE);
        tick(&mut a);
        !a.devices.iter().any(|d| d.address == "AA:BB:CC:DD:01:01")
    });
    probe(
        &mut r,
        "Pair raises auth dialog, accept pairs device",
        || {
            let mut a = fresh("en");
            select_addr(&mut a, "AA:BB:CC:DD:0A:0A");
            press(&mut a, Action::Pair);
            tick(&mut a);
            if a.top_overlay() != Some(Overlay::Auth) {
                return false;
            }
            press(&mut a, Action::Select);
            tick(&mut a);
            a.devices
                .iter()
                .find(|d| d.address == "AA:BB:CC:DD:0A:0A")
                .unwrap()
                .paired
        },
    );
    probe(&mut r, "Pair + reject leaves device unpaired", || {
        let mut a = fresh("en");
        select_addr(&mut a, "AA:BB:CC:DD:0A:0A");
        press(&mut a, Action::Pair);
        tick(&mut a);
        press(&mut a, Action::Close);
        tick(&mut a);
        !a.devices
            .iter()
            .find(|d| d.address == "AA:BB:CC:DD:0A:0A")
            .unwrap()
            .paired
    });

    println!("=== file sending ===");
    probe(
        &mut r,
        "SendFiles -> picker -> select -> confirm queues exactly 1 file",
        || {
            let path = PathBuf::from("/tmp/bluetuith-sim-send.txt");
            std::fs::write(&path, b"sim").unwrap();
            let mut a = fresh("en");
            select_addr(&mut a, "AA:BB:CC:DD:06:06");
            press(&mut a, Action::SendFiles);
            tick(&mut a);
            if a.top_overlay() != Some(Overlay::FilePicker) {
                return false;
            }
            {
                let fp = a.file_picker.as_mut().unwrap();
                fp.cwd = PathBuf::from("/tmp");
                fp.refresh();
                let i = fp.entries.iter().position(|p| p == &path).unwrap();
                fp.index = i;
            }
            press(&mut a, Action::FileSelect); // check the box
            let n_before = a.transfers.len();
            press(&mut a, Action::FileConfirmSelection);
            tick(&mut a);
            let added = a.transfers.len() - n_before;
            let _ = std::fs::remove_file(&path);
            added == 1 && a.status.text.contains("1 file(s)")
        },
    );
    probe(
        &mut r,
        "FilePicker keyboard: nav, hidden, invert, all, dir",
        || {
            let mut a = fresh("en");
            a.push(Overlay::FilePicker);
            for act in [
                Action::NavigateUp,
                Action::NavigateDown,
                Action::FileInvertSelection,
                Action::FileSelectAll,
                Action::FileToggleHidden,
                Action::FileRefresh,
                Action::FileDirBack,
                Action::FileDirForward,
            ] {
                press(&mut a, act);
                if !render(&mut a, 100, 30) {
                    return false;
                }
            }
            true
        },
    );

    println!("=== transfers ===");
    probe(
        &mut r,
        "Progress: suspend / resume / cancel change status",
        || {
            let mut a = fresh("en");
            a.push(Overlay::Progress);
            a.overlay_index = 0;
            press(&mut a, Action::TransferSuspend);
            tick(&mut a);
            if a.transfers[0].status != TransferStatus::Suspended {
                return false;
            }
            press(&mut a, Action::TransferResume);
            tick(&mut a);
            if a.transfers[0].status != TransferStatus::Active {
                return false;
            }
            press(&mut a, Action::TransferCancel);
            tick(&mut a);
            a.transfers[0].status == TransferStatus::Cancelled
        },
    );
    probe(&mut r, "Progress closes with v even when empty", || {
        let mut a = fresh("en");
        a.transfers.clear();
        a.push(Overlay::Progress);
        press(&mut a, Action::ProgressView);
        a.top_overlay().is_none()
    });

    println!("=== network ===");
    probe(&mut r, "Network: connect adds a connection", || {
        let mut a = fresh("en");
        select_addr(&mut a, "AA:BB:CC:DD:06:06");
        press(&mut a, Action::Network);
        tick(&mut a);
        let before = a.networks.len();
        a.overlay_index = 1; // NAP
        press(&mut a, Action::Connect);
        tick(&mut a);
        a.networks.len() == before + 1
    });
    probe(
        &mut r,
        "Network: disconnect marks connection inactive",
        || {
            let mut a = fresh("en");
            select_addr(&mut a, "AA:BB:CC:DD:06:06");
            press(&mut a, Action::Network);
            tick(&mut a);
            a.overlay_index = 0; // PANU (the existing active one)
            press(&mut a, Action::Remove);
            tick(&mut a);
            !a.networks
                .iter()
                .any(|n| n.role == NetworkRole::Panu && n.connected)
        },
    );
    probe(
        &mut r,
        "Network overlay hides disconnected connections",
        || {
            let mut a = fresh("en");
            a.push(Overlay::Network);
            // Force one disconnected row into the list.
            a.networks.push(Network {
                id: 99,
                device_name: "GONE".into(),
                connected: false,
                ..Default::default()
            });
            let txt = render_text(&mut a, 100, 30);
            !txt.contains("GONE")
        },
    );

    println!("=== audio profiles ===");
    probe(&mut r, "Profiles: demo toggles active flag", || {
        let mut a = fresh("en");
        press(&mut a, Action::AudioProfiles);
        tick(&mut a);
        if a.top_overlay() != Some(Overlay::Profiles) {
            return false;
        }
        let before = a.profiles[0].active;
        press(&mut a, Action::Select);
        tick(&mut a);
        a.profiles[0].active != before
    });

    println!("=== media player ===");
    probe(
        &mut r,
        "Player: next / volume / toggle mutate player state",
        || {
            let mut a = fresh("en");
            press(&mut a, Action::PlayerShow);
            tick(&mut a);
            if a.top_overlay() != Some(Overlay::Player) {
                return false;
            }
            let t0 = a.player.as_ref().unwrap().track_number;
            let v0 = a.player.as_ref().unwrap().volume;
            let s0 = a.player.as_ref().unwrap().status;
            press(&mut a, Action::PlayerNext);
            tick(&mut a);
            if a.player.as_ref().unwrap().track_number == t0 {
                return false;
            }
            press(&mut a, Action::PlayerVolumeUp);
            tick(&mut a);
            if a.player.as_ref().unwrap().volume <= v0 {
                return false;
            }
            press(&mut a, Action::PlayerTogglePlay);
            tick(&mut a);
            a.player.as_ref().unwrap().status != s0
        },
    );
    probe(&mut r, "Player: all controls are non-panicking", || {
        let mut a = fresh("en");
        a.push(Overlay::Player);
        for act in [
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
            Action::PlayerHide,
        ] {
            press(&mut a, act);
            tick(&mut a);
        }
        true
    });
    probe(
        &mut r,
        "Player shuffle / repeat actually mutate state",
        || {
            let mut a = fresh("en");
            a.push(Overlay::Player);
            let s0 = a.player.as_ref().unwrap().shuffle;
            let r0 = a.player.as_ref().unwrap().repeat;
            press(&mut a, Action::PlayerShuffle);
            tick(&mut a);
            if a.player.as_ref().unwrap().shuffle == s0 {
                return false;
            }
            press(&mut a, Action::PlayerRepeat);
            tick(&mut a);
            a.player.as_ref().unwrap().repeat != r0
        },
    );
    probe(&mut r, "PlayerHide hides the bar", || {
        let mut a = fresh("en");
        a.push(Overlay::Player);
        press(&mut a, Action::PlayerHide);
        !a.show_player_bar && a.top_overlay().is_none()
    });

    println!("=== view / misc actions ===");
    probe(&mut r, "CycleSort changes the sort key", || {
        let mut a = fresh("en");
        let before = a.sort;
        press(&mut a, Action::CycleSort);
        a.sort != before
    });
    probe(&mut r, "ToggleDetails / Switch toggle details", || {
        let mut a = fresh("en");
        let b = a.show_details;
        press(&mut a, Action::ToggleDetails);
        if a.show_details == b {
            return false;
        }
        press(&mut a, Action::Switch);
        a.show_details == b
    });
    probe(&mut r, "ThemePicker applies a theme", || {
        let mut a = fresh("en");
        press(&mut a, Action::ThemePicker);
        let before = a.config.theme.clone();
        press(&mut a, Action::NavigateDown);
        press(&mut a, Action::Select);
        a.config.theme != before
    });
    probe(&mut r, "AdapterChange selects hci1", || {
        let mut a = fresh("en");
        press(&mut a, Action::ChangeAdapter);
        press(&mut a, Action::NavigateDown);
        press(&mut a, Action::Select);
        tick(&mut a);
        a.selected_adapter == 1
    });
    probe(&mut r, "Search filters the device list", || {
        let mut a = fresh("en");
        press(&mut a, Action::Search);
        for c in "pixel".chars() {
            raw(&mut a, KeyCode::Char(c), KeyModifiers::NONE);
        }
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        a.visible_devices().len() < a.devices.len() && !a.filter.is_empty()
    });
    probe(
        &mut r,
        "Language switches to Chinese and status is localized",
        || {
            let mut a = fresh("en");
            press(&mut a, Action::Language);
            if a.lang != Lang::Zh {
                return false;
            }
            a.apply_event(bluetuith_rs::backend::Event::Status(StatusMessage {
                level: Level::Success,
                text: "Connected".into(),
                busy: false,
            }));
            a.status.text == "已连接"
        },
    );
    probe(&mut r, "Quit + confirm stops the loop", || {
        let mut a = fresh("en");
        press(&mut a, Action::Quit);
        if a.top_overlay() != Some(Overlay::Confirm) {
            return false;
        }
        raw(&mut a, KeyCode::Char('y'), KeyModifiers::NONE);
        !a.running
    });
    probe(&mut r, "Close / Esc dismisses info overlays", || {
        for o in [
            Overlay::Help,
            Overlay::Log,
            Overlay::About,
            Overlay::DeviceInfo,
        ] {
            let mut a = fresh("en");
            a.push(o);
            press(&mut a, Action::Close);
            if a.top_overlay().is_some() {
                return false;
            }
        }
        true
    });

    println!("=== command palette ===");
    probe(&mut r, "palette has no navigation / dead entries", || {
        let items = palette_actions();
        for nav in [
            Action::Select,
            Action::NavigateUp,
            Action::NavigateDown,
            Action::NavigateTop,
            Action::NavigateBottom,
        ] {
            if items.contains(&nav) {
                return false;
            }
        }
        !items.is_empty()
    });
    probe(
        &mut r,
        "palette does not offer Suspend (would SIGTSTP)",
        || !palette_actions().contains(&Action::Suspend),
    );
    probe(
        &mut r,
        "every palette action produces an observable change",
        || {
            let items = palette_actions();
            // These are legitimate no-ops when nothing is active.
            let allowed = [
                Action::Close,
                Action::Cancel,
                Action::Switch,
                Action::Suspend,
            ];
            for act in items {
                let mut a = fresh("en");
                let before = snapshot(&a);
                a.close_overlay();
                a.do_action(act);
                tick(&mut a);
                if !render(&mut a, 100, 30) {
                    return false;
                }
                let after = snapshot(&a);
                if before == after && !allowed.contains(&act) {
                    println!("      dead palette entry: {act:?}");
                    return false;
                }
            }
            true
        },
    );
    probe(&mut r, "palette level 1 Enter enters a category", || {
        let mut a = fresh("en");
        a.push(Overlay::CommandPalette);
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        a.palette_stage == 1
    });
    probe(&mut r, "palette level 2 Enter runs the action", || {
        let mut a = fresh("en");
        a.push(Overlay::CommandPalette);
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE); // -> stage 1
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE); // run first action
        !a.overlays.contains(&Overlay::CommandPalette)
            || a.top_overlay() != Some(Overlay::CommandPalette)
    });
    probe(&mut r, "palette Backspace returns to categories", || {
        let mut a = fresh("en");
        a.push(Overlay::CommandPalette);
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        raw(&mut a, KeyCode::Backspace, KeyModifiers::NONE);
        a.palette_stage == 0
    });
    probe(&mut r, "palette respects a rebound Select key", || {
        let mut cfg = Config {
            demo: true,
            language: "en".into(),
            ..Config::default()
        };
        cfg.keybindings.insert("Select".into(), "F12".into());
        let mut a = App::new(cfg, Box::new(DemoBackend::new()));
        for _ in 0..4 {
            a.tick();
        }
        a.push(Overlay::CommandPalette);
        press(&mut a, Action::Select); // F12
        a.palette_stage == 1
    });

    println!("=== mouse ===");
    probe(&mut r, "click menu tab opens that menu", || {
        let mut a = fresh("en");
        render(&mut a, 120, 40);
        let r = a.layout.menu_rects[2];
        mouse(
            &mut a,
            MouseEventKind::Down(MouseButton::Left),
            MouseButton::Left,
            r.x + 1,
            r.y,
        );
        a.top_overlay() == Some(Overlay::Menu) && a.menu_id == MenuId::View
    });
    probe(&mut r, "click outside closes overlay", || {
        let mut a = fresh("en");
        a.push(Overlay::About);
        render(&mut a, 120, 40);
        mouse(
            &mut a,
            MouseEventKind::Down(MouseButton::Left),
            MouseButton::Left,
            0,
            0,
        );
        a.top_overlay().is_none()
    });
    probe(&mut r, "right-click device row opens device menu", || {
        let mut a = fresh("en");
        render(&mut a, 120, 40);
        let r = a.layout.device_rows;
        mouse(
            &mut a,
            MouseEventKind::Down(MouseButton::Right),
            MouseButton::Right,
            r.x + 1,
            r.y,
        );
        a.menu_id == MenuId::Device && a.top_overlay() == Some(Overlay::Menu)
    });
    probe(&mut r, "click outside confirm cancels it", || {
        let mut a = fresh("en");
        a.confirm = Some(ConfirmState {
            text: "Quit?".into(),
            action: ConfirmAction::Quit,
        });
        a.push(Overlay::Confirm);
        render(&mut a, 120, 40);
        mouse(
            &mut a,
            MouseEventKind::Down(MouseButton::Left),
            MouseButton::Left,
            0,
            0,
        );
        a.running && a.top_overlay().is_none()
    });
    probe(
        &mut r,
        "scroll wheel moves overlay selection not device list",
        || {
            let mut a = fresh("en");
            a.push(Overlay::Progress);
            let d0 = a.selected;
            let i0 = a.overlay_index;
            mouse(
                &mut a,
                MouseEventKind::ScrollDown,
                MouseButton::Left,
                50,
                15,
            );
            a.selected == d0 && a.overlay_index != i0
        },
    );
    probe(&mut r, "helpbar hint click runs its action", || {
        let mut a = fresh("en");
        render(&mut a, 150, 44);
        if a.layout.help_rects.is_empty() {
            return false;
        }
        let (r, _) = a.layout.help_rects[1];
        let before = snapshot(&a);
        mouse(
            &mut a,
            MouseEventKind::Down(MouseButton::Left),
            MouseButton::Left,
            r.x + 1,
            r.y,
        );
        tick(&mut a);
        snapshot(&a) != before
    });
    probe(&mut r, "overlay item click activates a menu item", || {
        let mut a = fresh("en");
        a.open_menu_at(MenuId::Help);
        render(&mut a, 120, 40);
        if a.layout.overlay_items.is_empty() {
            return false;
        }
        let (r, i) = a.layout.overlay_items[1];
        mouse(
            &mut a,
            MouseEventKind::Down(MouseButton::Left),
            MouseButton::Left,
            r.x + 1,
            r.y,
        );
        tick(&mut a);
        a.top_overlay() != Some(Overlay::Menu) || a.menu_index == i
    });

    println!("=== custom keybindings across overlays ===");
    probe(
        &mut r,
        "rebound Select works in ThemePicker / Network / Confirm",
        || {
            let mut cfg = Config {
                demo: true,
                language: "en".into(),
                ..Config::default()
            };
            cfg.keybindings.insert("Select".into(), "F12".into());
            let mk = || App::new(cfg.clone(), Box::new(DemoBackend::new()));
            let mut a = mk();
            for _ in 0..4 {
                a.tick();
            }
            press(&mut a, Action::ThemePicker);
            press(&mut a, Action::NavigateDown); // pick a theme other than the default
            press(&mut a, Action::Select);
            if a.config.theme == "Midnight" {
                return false;
            }
            let mut b = mk();
            for _ in 0..4 {
                b.tick();
            }
            b.confirm = Some(ConfirmState {
                text: "Quit?".into(),
                action: ConfirmAction::Quit,
            });
            b.push(Overlay::Confirm);
            press(&mut b, Action::Select);
            if b.running {
                return false;
            }
            let mut c = mk();
            for _ in 0..4 {
                c.tick();
            }
            select_addr(&mut c, "AA:BB:CC:DD:06:06");
            press(&mut c, Action::Network);
            tick(&mut c);
            let n0 = c.networks.len();
            press(&mut c, Action::Select);
            tick(&mut c);
            c.networks.len() > n0
        },
    );
    probe(
        &mut r,
        "rebound Select works for palette mouse activation",
        || {
            let mut cfg = Config {
                demo: true,
                language: "en".into(),
                ..Config::default()
            };
            cfg.keybindings.insert("Select".into(), "F12".into());
            let mut a = App::new(cfg, Box::new(DemoBackend::new()));
            for _ in 0..4 {
                a.tick();
            }
            a.push(Overlay::CommandPalette);
            render(&mut a, 120, 40);
            if a.layout.overlay_items.is_empty() {
                return false;
            }
            let (r, _) = a.layout.overlay_items[0];
            mouse(
                &mut a,
                MouseEventKind::Down(MouseButton::Left),
                MouseButton::Left,
                r.x + 1,
                r.y,
            );
            a.palette_stage == 1
        },
    );

    println!("=== OBEX receiving / reserved options ===");
    probe(
        &mut r,
        "incoming OBEX file shows an authorization dialog",
        || {
            let mut a = fresh("zh");
            a.apply_event(bluetuith_rs::backend::Event::AuthRequest(AuthRequest {
                id: 7,
                device: "photo.jpg".into(),
                device_name: "photo.jpg".into(),
                kind: AuthKind::FilePush("photo.jpg".into()),
            }));
            if a.top_overlay() != Some(Overlay::Auth) {
                return false;
            }
            let txt = render_text(&mut a, 100, 30);
            // TestBackend pads wide CJK glyphs across two cells, so collapse the
            // inter-cell spaces before matching a Chinese phrase.
            let compact: String = txt.chars().filter(|c| !c.is_whitespace()).collect();
            if !compact.contains("收到文件") {
                return false;
            }
            press(&mut a, Action::Select);
            tick(&mut a);
            a.auth.is_none() && a.top_overlay().is_none()
        },
    );
    probe(
        &mut r,
        "reserved gsm options produce a startup warning",
        || {
            let cfg = Config {
                demo: true,
                gsm_apn: "internet".into(),
                ..Config::default()
            };
            let a = App::new(cfg, Box::new(DemoBackend::new()));
            a.startup_warning
                .as_deref()
                .unwrap_or("")
                .contains("gsm-apn")
        },
    );

    println!("=== i18n / text ===");
    probe(
        &mut r,
        "zh player overlay has no stray English 'track'",
        || {
            let mut a = fresh("zh");
            a.push(Overlay::Player);
            !render_text(&mut a, 100, 30).contains("track")
        },
    );
    probe(
        &mut r,
        "zh palette footer has no stray English 'type'",
        || {
            let mut a = fresh("zh");
            a.push(Overlay::CommandPalette);
            raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
            !render_text(&mut a, 100, 30).contains("type")
        },
    );
    probe(&mut r, "zh help scroll is clamped (no runaway)", || {
        let mut a = fresh("zh");
        a.push(Overlay::Help);
        for _ in 0..200 {
            mouse(&mut a, MouseEventKind::ScrollDown, MouseButton::Left, 1, 1);
            render(&mut a, 80, 24);
        }
        let s = a.overlay_scroll;
        mouse(&mut a, MouseEventKind::ScrollDown, MouseButton::Left, 1, 1);
        render(&mut a, 80, 24);
        a.overlay_scroll == s
    });
    probe(
        &mut r,
        "zh help can actually scroll to the last line",
        || {
            let mut a = fresh("zh");
            a.push(Overlay::Help);
            for _ in 0..200 {
                mouse(&mut a, MouseEventKind::ScrollDown, MouseButton::Left, 1, 1);
            }
            let txt = render_text(&mut a, 80, 24);
            // The final two help lines are the config + mouse hints.
            txt.contains("bluetuith-rs/config.toml") || txt.contains("鼠标")
        },
    );
    probe(&mut r, "en UI has no placeholder braces", || {
        let mut a = fresh("en");
        a.push(Overlay::DeviceInfo);
        !render_text(&mut a, 120, 40).contains("{}")
    });

    println!("=== tiny / boundary terminals ===");
    probe(
        &mut r,
        "tall menus scroll so the last item stays visible",
        || {
            for id in [MenuId::Adapter, MenuId::Device, MenuId::View, MenuId::Help] {
                let mut a = fresh("en");
                a.open_menu_at(id);
                let last = a.menu_items.len().saturating_sub(1);
                a.menu_index = last;
                if !render(&mut a, 40, 12) {
                    return false;
                }
                if !a.layout.overlay_items.iter().any(|(_, i)| *i == last) {
                    return false;
                }
            }
            true
        },
    );
    probe(&mut r, "render at 40x12 and 41x13 without panic", || {
        for lang in ["en", "zh"] {
            let mut a = fresh(lang);
            for (w, h) in [(40, 12), (41, 13), (39, 20), (100, 11)] {
                if !render(&mut a, w, h) {
                    return false;
                }
            }
        }
        true
    });

    println!("\n=== result: {} passed, {} failed ===", r.pass, r.fail);
    if r.fail > 0 {
        std::process::exit(1);
    }
}
