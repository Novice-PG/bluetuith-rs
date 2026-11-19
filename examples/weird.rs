//! Weird/edge-case stress harness. Drives the real `App` against the demo
//! backend with unusual inputs and sequences, asserting no panics and
//! self-consistent state. Each probe is panic-caught and isolated (fresh App).
//!
//! Run: cargo run --example weird

use std::panic::{self, AssertUnwindSafe};

use bluetuith_rs::app::{App, Overlay};
use bluetuith_rs::backend::demo::DemoBackend;
use bluetuith_rs::backend::Event;
use bluetuith_rs::config::Config;
use bluetuith_rs::keybind::Action;
use bluetuith_rs::model::*;
use bluetuith_rs::ui;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
fn tick(a: &mut App, n: usize) {
    for _ in 0..n {
        a.tick();
    }
}

fn renders(a: &mut App, sizes: &[(u16, u16)]) -> bool {
    for &(w, h) in sizes {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        if t.draw(|f| ui::render(f, a)).is_err() {
            return false;
        }
    }
    true
}

fn auth(id: usize, kind: AuthKind) -> Event {
    Event::AuthRequest(AuthRequest {
        id,
        device: "AA:BB:CC:DD:01:01".into(),
        device_name: "WH-1000XM5".into(),
        kind,
    })
}

fn main() {
    let mut r = Results { pass: 0, fail: 0 };
    println!("=== weird scenarios ===");

    probe(&mut r, "render at tiny/zero sizes", || {
        let mut a = fresh("en");
        renders(
            &mut a,
            &[(1, 1), (2, 2), (5, 3), (20, 1), (0, 0), (150, 44)],
        )
    });

    probe(&mut r, "render every overlay at tiny size", || {
        let mut a = fresh("zh");
        for o in [
            Overlay::Help,
            Overlay::About,
            Overlay::Player,
            Overlay::Network,
            Overlay::Profiles,
            Overlay::FilePicker,
            Overlay::Progress,
            Overlay::CommandPalette,
            Overlay::Log,
            Overlay::ThemePicker,
            Overlay::AdapterChange,
            Overlay::Menu,
            Overlay::DeviceInfo,
        ] {
            a.overlays.clear();
            a.push(o);
            if !renders(&mut a, &[(40, 12), (150, 44)]) {
                return false;
            }
        }
        true
    });

    probe(&mut r, "rapid connect/disconnect/pair x200", || {
        let mut a = fresh("en");
        for _ in 0..200 {
            press(&mut a, Action::Connect); // toggle: connect then disconnect
            press(&mut a, Action::Connect);
            press(&mut a, Action::Pair);
            tick(&mut a, 1);
        }
        tick(&mut a, 3);
        renders(&mut a, &[(150, 44)])
    });

    probe(&mut r, "actions on empty device list", || {
        let mut a = fresh("en");
        a.apply_event(Event::Snapshot {
            adapters: vec![],
            devices: vec![],
        });
        a.selected = 999;
        for act in [
            Action::Connect,
            Action::Pair,
            Action::Trust,
            Action::Block,
            Action::Remove,
            Action::Info,
            Action::SendFiles,
            Action::Network,
            Action::NavigateUp,
            Action::NavigateDown,
            Action::NavigateTop,
            Action::NavigateBottom,
            Action::Switch,
            Action::CycleSort,
        ] {
            press(&mut a, act);
            tick(&mut a, 1);
        }
        renders(&mut a, &[(150, 44)])
    });

    probe(&mut r, "filter to nothing then all actions", || {
        let mut a = fresh("en");
        a.filter = "ZZQQQQNOTHING".into();
        a.search_active = true;
        for act in [
            Action::Connect,
            Action::Pair,
            Action::NavigateDown,
            Action::Select,
        ] {
            press(&mut a, act);
        }
        tick(&mut a, 2);
        renders(&mut a, &[(150, 44)])
    });

    probe(&mut r, "auth dialog takes keyboard during search", || {
        let mut a = fresh("en");
        a.filter = "abc".into();
        a.search_active = true;
        a.apply_event(auth(1, AuthKind::Confirmation(1234)));
        // Typing a search char must NOT go into the filter while auth is up.
        raw(&mut a, KeyCode::Char('z'), KeyModifiers::NONE);
        let frozen = a.filter.clone();
        // Esc on auth must close it and clear self.auth.
        raw(&mut a, KeyCode::Esc, KeyModifiers::NONE);
        tick(&mut a, 2);
        a.filter == frozen && a.auth.is_none() && a.top_overlay() != Some(Overlay::Auth)
    });

    probe(&mut r, "accept confirmation auth", || {
        let mut a = fresh("en");
        a.apply_event(auth(7, AuthKind::Confirmation(1234)));
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 2);
        a.auth.is_none() && a.top_overlay() != Some(Overlay::Auth)
    });

    probe(&mut r, "pincode auth with empty + long input", || {
        let mut a = fresh("en");
        a.apply_event(auth(8, AuthKind::PinCode));
        // empty accept should not panic
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 1);
        a.apply_event(auth(9, AuthKind::PinCode));
        for _ in 0..200 {
            raw(&mut a, KeyCode::Char('9'), KeyModifiers::NONE);
        }
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 2);
        a.auth.is_none()
    });

    probe(&mut r, "passkey auth with non-numeric input", || {
        let mut a = fresh("en");
        a.apply_event(auth(10, AuthKind::Passkey(0)));
        for c in ['a', 'x', ' ', '-'] {
            raw(&mut a, KeyCode::Char(c), KeyModifiers::NONE);
        }
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 2);
        a.auth.is_none()
    });

    probe(&mut r, "file-push auth accept/reject", || {
        let mut a = fresh("zh");
        a.apply_event(auth(11, AuthKind::FilePush("../../evil.png".into())));
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 1);
        a.apply_event(auth(12, AuthKind::FilePush("ok.txt".into())));
        raw(&mut a, KeyCode::Esc, KeyModifiers::NONE);
        tick(&mut a, 2);
        a.auth.is_none()
    });

    probe(&mut r, "two auth requests back-to-back", || {
        let mut a = fresh("en");
        a.apply_event(auth(20, AuthKind::Confirmation(1111)));
        a.apply_event(auth(21, AuthKind::Confirmation(2222)));
        // Which auth is shown, and how many overlays are stacked?
        let shown = a.auth.as_ref().map(|x| x.id);
        let overlays = a.overlays.iter().filter(|&&o| o == Overlay::Auth).count();
        // Respond twice (the second id is what is stored).
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 1);
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 2);
        // Invariant: no stale auth overlay should remain after responding.
        a.auth.is_none() && a.top_overlay() != Some(Overlay::Auth) && {
            println!("    [info] two-auth: shown={shown:?} auth_overlays={overlays}");
            true
        }
    });

    probe(&mut r, "respond to stale auth id", || {
        let mut a = fresh("en");
        a.apply_event(auth(30, AuthKind::Confirmation(1)));
        raw(&mut a, KeyCode::Esc, KeyModifiers::NONE);
        tick(&mut a, 1);
        // A late backend response event with no matching dialog must not panic.
        a.apply_event(Event::Log("late".into()));
        renders(&mut a, &[(150, 44)])
    });

    probe(&mut r, "overlay stack chaos", || {
        let mut a = fresh("en");
        for o in [
            Overlay::Help,
            Overlay::About,
            Overlay::Player,
            Overlay::Network,
            Overlay::Confirm,
            Overlay::Auth,
            Overlay::Menu,
            Overlay::Log,
        ] {
            a.push(o);
        }
        for _ in 0..50 {
            raw(&mut a, KeyCode::Esc, KeyModifiers::NONE);
        }
        tick(&mut a, 2);
        a.overlays.is_empty()
    });

    probe(&mut r, "menu navigation and activation chaos", || {
        let mut a = fresh("zh");
        for _ in 0..100 {
            press(&mut a, Action::Menu);
            press(&mut a, Action::NavigateDown);
            press(&mut a, Action::Select);
            raw(&mut a, KeyCode::Esc, KeyModifiers::NONE);
        }
        tick(&mut a, 2);
        renders(&mut a, &[(120, 40)])
    });

    probe(&mut r, "repeat mode and sort cycling x500", || {
        let mut a = fresh("en");
        for _ in 0..500 {
            press(&mut a, Action::CycleSort);
            press(&mut a, Action::PlayerRepeat);
            press(&mut a, Action::ToggleDetails);
        }
        tick(&mut a, 2);
        renders(&mut a, &[(150, 44)])
    });

    probe(&mut r, "snapshot churn with changing device set", || {
        let mut a = fresh("en");
        for n in 0..50 {
            let devices = if n % 7 == 0 {
                vec![]
            } else {
                a.devices
                    .iter()
                    .take(n % (a.devices.len().max(1)))
                    .cloned()
                    .collect()
            };
            a.apply_event(Event::Snapshot {
                adapters: a.adapters.clone(),
                devices,
            });
            a.selected = n;
            press(&mut a, Action::Connect);
            tick(&mut a, 1);
            if !renders(&mut a, &[(100, 30)]) {
                return false;
            }
        }
        true
    });

    probe(
        &mut r,
        "transfer/network/profile empty and populated churn",
        || {
            let mut a = fresh("en");
            for _ in 0..100 {
                a.apply_event(Event::Transfers(vec![]));
                a.apply_event(Event::Transfers(vec![Transfer {
                    id: 1,
                    name: "x".into(),
                    size: 0,
                    transferred: u64::MAX,
                    ..Default::default()
                }]));
                a.apply_event(Event::Networks(vec![]));
                a.apply_event(Event::Profiles(vec![]));
                a.apply_event(Event::Player(None));
                a.apply_event(Event::Player(Some(PlayerState {
                    duration_ms: 0,
                    position_ms: u64::MAX,
                    ..Default::default()
                })));
                tick(&mut a, 1);
            }
            renders(&mut a, &[(150, 44)])
        },
    );

    probe(&mut r, "language switch with every overlay open", || {
        let mut a = fresh("en");
        for o in [
            Overlay::Help,
            Overlay::Player,
            Overlay::Network,
            Overlay::About,
        ] {
            a.overlays.clear();
            a.push(o);
            press(&mut a, Action::Language);
            tick(&mut a, 1);
            if !renders(&mut a, &[(120, 36)]) {
                return false;
            }
        }
        true
    });

    probe(&mut r, "confirm quit accept and cancel", || {
        let mut a = fresh("en");
        press(&mut a, Action::Quit);
        raw(&mut a, KeyCode::Esc, KeyModifiers::NONE);
        tick(&mut a, 1);
        press(&mut a, Action::Quit);
        raw(&mut a, KeyCode::Enter, KeyModifiers::NONE);
        tick(&mut a, 2);
        true
    });

    probe(&mut r, "mouse clicks at corners and edges", || {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let mut a = fresh("en");
        let pts = [(0u16, 0u16), (149, 43), (75, 22), (1, 1), (148, 42)];
        for &(x, y) in &pts {
            for kind in [
                MouseEventKind::Down(MouseButton::Left),
                MouseEventKind::Up(MouseButton::Left),
                MouseEventKind::ScrollUp,
                MouseEventKind::ScrollDown,
                MouseEventKind::Down(MouseButton::Right),
            ] {
                a.on_mouse(MouseEvent {
                    kind,
                    column: x,
                    row: y,
                    modifiers: KeyModifiers::NONE,
                });
                tick(&mut a, 1);
            }
        }
        renders(&mut a, &[(150, 44)])
    });

    println!("\n=== result: {} passed, {} failed ===", r.pass, r.fail);
    if r.fail > 0 {
        std::process::exit(1);
    }
}
