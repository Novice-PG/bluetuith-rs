use bluetuith_rs::app::{App, MenuId, Overlay};
use bluetuith_rs::backend::demo::DemoBackend;
use bluetuith_rs::config::Config;
use bluetuith_rs::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn app() -> App {
    let cfg = Config {
        demo: true,
        language: "en".to_string(),
        ..Config::default()
    };
    let mut app = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        app.tick();
    }
    app
}

fn text_of(buf: &ratatui::buffer::Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        s.push('\n');
    }
    s
}

fn render(mut app: App, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| ui::render(f, &mut app)).unwrap();
    text_of(terminal.backend().buffer())
}

fn render_mut(app: &mut App, w: u16, h: u16) {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| ui::render(f, app)).unwrap();
}

#[test]
fn main_frame_has_the_brand_and_device_table() {
    let s = render(app(), 150, 44);
    assert!(s.contains("bluetuith-rs"), "brand missing");
    assert!(s.contains("Devices"), "device panel missing");
    assert!(s.contains("Device Details"), "details panel missing");
    assert!(s.contains("WH-1000XM5"), "demo device missing");
    assert!(s.contains("POWER"), "adapter chip missing");
}

#[test]
fn narrow_terminal_still_renders() {
    let s = render(app(), 60, 20);
    assert!(s.contains("bluetuith-rs"));
    assert!(s.contains("Devices"));
}

#[test]
fn tiny_terminal_shows_a_message() {
    let s = render(app(), 30, 8);
    assert!(s.contains("Terminal too small"));
}

#[test]
fn every_overlay_renders_without_panicking() {
    let overlays = [
        Overlay::Menu,
        Overlay::Help,
        Overlay::AdapterChange,
        Overlay::DeviceInfo,
        Overlay::FilePicker,
        Overlay::Progress,
        Overlay::Player,
        Overlay::Network,
        Overlay::Profiles,
        Overlay::ThemePicker,
        Overlay::Log,
        Overlay::CommandPalette,
        Overlay::About,
    ];
    for o in overlays {
        let mut a = app();
        a.push(o);
        let s = render(a, 120, 38);
        assert!(!s.trim().is_empty(), "overlay {o:?} rendered nothing");
    }
}

#[test]
fn keybindings_open_and_close_overlays() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert_eq!(a.top_overlay(), Some(Overlay::Help));
    a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(a.top_overlay().is_none());
}

#[test]
fn search_filters_devices() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    for c in "pixel".chars() {
        a.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(a.visible_devices().len(), 1);
    assert_eq!(a.visible_devices()[0].display_name(), "Pixel 8 Pro");
}

#[test]
fn navigation_wraps_around_the_list() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    let n = a.visible_devices().len();
    assert!(n > 1);
    a.selected = 0;
    // Up on the first row jumps to the last row.
    a.on_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(a.selected, n - 1);
    // Down on the last row jumps back to the first.
    a.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(a.selected, 0);
}

#[test]
fn command_palette_two_levels_and_wrap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert_eq!(a.top_overlay(), Some(Overlay::CommandPalette));
    assert_eq!(a.palette_stage, 0);
    // Level 1: categories wrap around.
    let n = a.palette_categories.len();
    assert!(n > 1);
    a.overlay_index = 0;
    a.on_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(a.overlay_index, n - 1);
    a.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(a.overlay_index, 0);
    // Enter opens the first category.
    a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(a.palette_stage, 1);
    let m = a.palette_items.len();
    assert!(!a.palette_items.is_empty());
    a.overlay_index = 0;
    a.on_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(a.overlay_index, m - 1);
    a.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(a.overlay_index, 0);
    // Esc steps back to level 1 without closing the palette.
    a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(a.top_overlay(), Some(Overlay::CommandPalette));
    assert_eq!(a.palette_stage, 0);
}

#[test]
fn function_keys_open_menu_tabs() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.on_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    assert_eq!(a.top_overlay(), Some(Overlay::Menu));
    assert_eq!(a.menu_id, MenuId::Adapter);
    a.on_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE));
    assert_eq!(a.menu_id, MenuId::Device);
    a.on_key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE));
    assert_eq!(a.menu_id, MenuId::View);
    a.on_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE));
    assert_eq!(a.menu_id, MenuId::Help);
}

#[test]
fn connect_action_reaches_the_backend() {
    use bluetuith_rs::keybind::Action;
    let mut a = app();
    let addr = a.selected_device().unwrap().address.clone();
    let before = a
        .devices
        .iter()
        .find(|d| d.address == addr)
        .unwrap()
        .connected;
    a.do_action(Action::Connect);
    a.tick();
    let after = a
        .devices
        .iter()
        .find(|d| d.address == addr)
        .unwrap()
        .connected;
    assert_eq!(after, !before, "connect should toggle the device state");
}

#[test]
fn trust_action_reaches_the_backend() {
    use bluetuith_rs::keybind::Action;
    let mut a = app();
    let addr = a.selected_device().unwrap().address.clone();
    let before = a
        .devices
        .iter()
        .find(|d| d.address == addr)
        .unwrap()
        .trusted;
    a.do_action(Action::Trust);
    a.tick();
    let after = a
        .devices
        .iter()
        .find(|d| d.address == addr)
        .unwrap()
        .trusted;
    assert_ne!(after, before, "trust should toggle the device state");
}

#[test]
fn scan_action_toggles_adapter() {
    use bluetuith_rs::keybind::Action;
    let mut a = app();
    let before = a.current_adapter().unwrap().discovering;
    a.do_action(Action::Scan);
    a.tick();
    let after = a.current_adapter().unwrap().discovering;
    assert_ne!(after, before, "scan should toggle discovery");
}

#[test]
fn pairing_opens_an_auth_prompt() {
    use bluetuith_rs::keybind::Action;
    let mut a = app();
    let idx = { a.visible_devices().iter().position(|d| !d.paired) };
    let Some(idx) = idx else {
        return;
    };
    a.selected = idx;
    a.do_action(Action::Pair);
    a.tick();
    assert!(a.auth.is_some(), "pairing should raise an auth request");
}

#[test]
fn send_files_creates_and_advances_a_transfer() {
    use bluetuith_rs::backend::Command;
    let mut a = app();
    let addr = a.selected_device().unwrap().address.clone();
    let path = std::env::temp_dir().join("bluetuith-rs-test-send.bin");
    std::fs::write(&path, vec![0u8; 4096]).unwrap();
    a.dispatch(Command::SendFiles {
        device: addr,
        files: vec![path.clone()],
    });
    a.tick();
    assert!(!a.transfers.is_empty(), "a transfer should be created");
    let start = a.transfers[0].transferred;
    for _ in 0..3 {
        a.tick();
    }
    assert!(
        a.transfers[0].transferred > start,
        "transfer progress should advance"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn player_toggle_reaches_the_backend() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    let Some(before) = a.player.as_ref().map(|p| p.status) else {
        return;
    };
    a.push(Overlay::Player);
    a.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    a.tick();
    let after = a.player.as_ref().map(|p| p.status);
    assert_ne!(
        before,
        after.unwrap_or(before),
        "play/pause should change state"
    );
}

#[test]
fn overlay_keys_do_not_leak_to_the_list() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    let addr = a.selected_device().unwrap().address.clone();
    let before = a
        .devices
        .iter()
        .find(|d| d.address == addr)
        .unwrap()
        .connected;
    a.push(Overlay::About);
    // 'c' is the connect shortcut, but the About window must swallow it.
    a.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
    a.tick();
    let after = a
        .devices
        .iter()
        .find(|d| d.address == addr)
        .unwrap()
        .connected;
    assert_eq!(after, before, "main shortcut leaked through the overlay");
    // Esc closes the overlay.
    a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(a.top_overlay().is_none());
}

#[test]
fn wheel_scrolls_the_overlay_not_the_list() {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let mut a = app();
    a.push(Overlay::About);
    let before = a.selected;
    a.on_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(a.selected, before, "wheel must not move the list selection");
    assert!(a.overlay_scroll > 0, "wheel should scroll the open window");
}

#[test]
fn clicking_a_hint_runs_its_action() {
    use bluetuith_rs::keybind::Action;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut a = app();
    render_mut(&mut a, 150, 44);
    let target = a
        .layout
        .help_rects
        .iter()
        .find(|(_, act)| *act == Action::Info)
        .cloned();
    let Some((r, _)) = target else {
        panic!("info hint not recorded");
    };
    a.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(a.top_overlay(), Some(Overlay::DeviceInfo));
}

#[test]
fn clicking_a_menu_item_runs_it() {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    let mut a = app();
    a.on_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE)); // Help menu
    assert_eq!(a.menu_id, MenuId::Help);
    render_mut(&mut a, 120, 40);
    // Help menu item 1 is "About".
    let item = a
        .layout
        .overlay_items
        .iter()
        .find(|(_, i)| *i == 1)
        .cloned();
    let Some((r, _)) = item else {
        panic!("menu items not recorded");
    };
    a.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x + 1,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(a.top_overlay(), Some(Overlay::About));
}

#[test]
fn clicking_outside_closes_the_overlay() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut a = app();
    a.push(Overlay::About);
    render_mut(&mut a, 120, 40);
    a.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(a.top_overlay().is_none(), "outside click should dismiss");
}

#[test]
fn clicking_a_palette_category_opens_actions() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut a = app();
    a.push(Overlay::CommandPalette);
    render_mut(&mut a, 120, 40);
    assert_eq!(a.palette_stage, 0);
    let item = a
        .layout
        .overlay_items
        .iter()
        .find(|(_, i)| *i == 0)
        .cloned();
    let Some((r, _)) = item else {
        panic!("palette categories not recorded");
    };
    a.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: r.x + 1,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(a.palette_stage, 1);
}

#[test]
fn function_keys_work_from_the_main_view() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    // F9 = About, no menu/help window needed first.
    a.on_key(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE));
    assert_eq!(a.top_overlay(), Some(Overlay::About));
    a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(a.top_overlay().is_none());
    // F7 = theme picker.
    a.on_key(KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE));
    assert_eq!(a.top_overlay(), Some(Overlay::ThemePicker));
    a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    // F6 = toggle details (no overlay).
    let before = a.show_details;
    a.on_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
    assert_ne!(a.show_details, before);
}

#[test]
fn chinese_ui_renders() {
    let cfg = Config {
        demo: true,
        language: "zh".to_string(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        a.tick();
    }
    let s = render(a, 150, 44);
    // Wide glyphs leave a continuation cell; drop whitespace before matching.
    let flat: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(flat.contains("设备详情"), "chinese details title missing");
    assert!(flat.contains("就绪"), "chinese status missing");
}

#[test]
fn command_palette_has_no_dead_entries() {
    use bluetuith_rs::keybind::{Action, Context};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.push(Overlay::CommandPalette);
    let cats = a.palette_categories.clone();
    assert!(!cats.is_empty());
    // Walk every category through the real Enter interaction.
    for i in 0..cats.len() {
        a.palette_go_back();
        a.overlay_index = i;
        a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(a.palette_stage, 1);
        assert!(!a.palette_items.is_empty());
        for nav in [
            Action::Select,
            Action::NavigateUp,
            Action::NavigateDown,
            Action::NavigateTop,
            Action::NavigateBottom,
        ] {
            assert!(
                !a.palette_items.contains(&nav),
                "navigation key leaked into the palette"
            );
        }
    }
    // A window-scoped action must open that window rather than do nothing.
    for (action, expected) in [
        (Action::FileSelect, Overlay::FilePicker),
        (Action::TransferSuspend, Overlay::Progress),
        (Action::PlayerNext, Overlay::Player),
    ] {
        let mut b = app();
        assert_eq!(
            action.context(),
            match action {
                Action::FileSelect => Context::Files,
                Action::TransferSuspend => Context::Progress,
                _ => Context::Player,
            }
        );
        b.do_action(action);
        assert_eq!(b.top_overlay(), Some(expected), "{action:?} opened nothing");
    }
}

#[test]
fn disable_obex_services_really_disables_file_transfer() {
    let cfg = Config {
        demo: true,
        language: "en".to_string(),
        disable_obex_services: true,
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        a.tick();
    }
    assert!(!a.features.send_file, "CLI flag had no effect");
    assert!(!a.features.obex);
    assert!(!a.features.receive_file);
}

#[test]
fn right_click_behind_an_overlay_does_not_replace_it() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let mut a = app();
    a.push(Overlay::About);
    render_mut(&mut a, 120, 40);
    let r = a.layout.device_rows;
    a.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: r.x + 1,
        row: r.y,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(a.top_overlay(), Some(Overlay::About));
}

#[test]
fn help_scroll_is_clamped_to_the_content() {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let mut a = app();
    a.push(Overlay::Help);
    for _ in 0..120 {
        a.on_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        render_mut(&mut a, 100, 30);
    }
    let stable = a.overlay_scroll;
    for _ in 0..5 {
        a.on_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        render_mut(&mut a, 100, 30);
    }
    assert_eq!(
        a.overlay_scroll, stable,
        "scroll must not run past the text"
    );
}

#[test]
fn backend_status_is_localized_in_chinese_ui() {
    use bluetuith_rs::backend::Event;
    use bluetuith_rs::model::{Level, StatusMessage};
    let cfg = Config {
        demo: true,
        language: "zh".to_string(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    a.apply_event(Event::Status(StatusMessage {
        level: Level::Success,
        text: "Connected".into(),
        busy: false,
    }));
    assert_eq!(a.status.text, "已连接");
}

#[test]
fn cli_help_lists_the_current_bindings() {
    let h = bluetuith_rs::cli::help_text();
    assert!(h.contains("Alt+s"), "stale binding in --help");
    assert!(h.contains("Alt+a"));
    assert!(h.contains("F9"));
    assert!(h.contains("q quit"));
    assert!(
        !h.contains("F3 sort"),
        "old F3/F4 bindings still documented"
    );
    assert!(!h.contains("F4 theme"));
}

// ---------------------------------------------------------------------------
// Regression tests for the from-scratch review + full functional simulation.

#[test]
fn demo_send_reports_the_files_it_actually_queued() {
    use bluetuith_rs::keybind::Action;
    use crossterm::event::KeyEvent;
    use std::path::PathBuf;

    let path = PathBuf::from("/tmp/bluetuith-test-send.txt");
    std::fs::write(&path, b"x").unwrap();
    let mut a = app();
    a.push(Overlay::FilePicker);
    {
        let fp = a.file_picker.as_mut().unwrap();
        fp.cwd = PathBuf::from("/tmp");
        fp.refresh();
        fp.index = fp.entries.iter().position(|p| p == &path).unwrap();
    }
    let s = a.keys.get(Action::FileSelect);
    a.on_key(KeyEvent::new(s.code, s.mods));
    let s = a.keys.get(Action::FileConfirmSelection);
    a.on_key(KeyEvent::new(s.code, s.mods));
    for _ in 0..3 {
        a.tick();
    }
    let _ = std::fs::remove_file(&path);
    // The demo used to report the *total* transfer count (3 existing + 1 new).
    assert!(
        a.status.text.contains("1 file(s)"),
        "status was {:?}",
        a.status.text
    );
}

#[test]
fn palette_excludes_local_and_dangerous_actions() {
    use bluetuith_rs::keybind::Action;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut offered = Vec::new();
    for cat in 0..6 {
        let mut a = app();
        a.push(Overlay::CommandPalette);
        for _ in 0..cat {
            a.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        offered.extend(a.palette_items.iter().copied());
    }
    for bad in [
        Action::Suspend, // would raise SIGTSTP and freeze the TUI
        Action::Close,
        Action::Cancel,
        Action::Switch,
        Action::Select,
        Action::NavigateUp,
        Action::NavigateDown,
        Action::NavigateTop,
        Action::NavigateBottom,
    ] {
        assert!(!offered.contains(&bad), "{bad:?} leaked into the palette");
    }
    assert!(offered.contains(&Action::Connect));
}

#[test]
fn palette_confirm_honours_a_custom_select_binding() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut cfg = Config {
        demo: true,
        language: "en".to_string(),
        ..Config::default()
    };
    cfg.keybindings.insert("Select".into(), "F12".into());
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        a.tick();
    }
    a.push(Overlay::CommandPalette);
    a.on_key(KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE));
    assert_eq!(a.palette_stage, 1, "palette ignored the rebound Select key");
}

#[test]
fn network_overlay_hides_disconnected_connections() {
    use bluetuith_rs::model::Network;
    let mut a = app();
    a.networks.push(Network {
        id: 99,
        device_name: "GONE-DEVICE".into(),
        connected: false,
        ..Default::default()
    });
    a.push(Overlay::Network);
    let s = render(a, 100, 30);
    assert!(!s.contains("GONE-DEVICE"), "stale connection still listed");
}

#[test]
fn tall_menu_scrolls_its_last_item_into_view() {
    for id in [MenuId::Adapter, MenuId::Device, MenuId::View, MenuId::Help] {
        let mut a = app();
        a.open_menu_at(id);
        let last = a.menu_items.len() - 1;
        a.menu_index = last;
        render_mut(&mut a, 40, 12);
        assert!(
            a.layout.overlay_items.iter().any(|(_, i)| *i == last),
            "{id:?} last item is unreachable on a small terminal"
        );
    }
}

#[test]
fn chinese_overlays_have_no_stray_english() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let cfg = Config {
        demo: true,
        language: "zh".to_string(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        a.tick();
    }
    a.push(Overlay::Player);
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| ui::render(f, &mut a)).unwrap();
    assert!(!text_of(t.backend().buffer()).contains("track"));

    a.close_overlay();
    a.push(Overlay::CommandPalette);
    a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| ui::render(f, &mut a)).unwrap();
    assert!(!text_of(t.backend().buffer()).contains("type"));
}

#[test]
fn player_show_and_hide_are_dispatched_to_the_backend() {
    use bluetuith_rs::keybind::Action;
    let mut a = app();
    let name = a.visible_devices()[3].display_name().clone();
    a.selected = 3;
    a.do_action(Action::PlayerShow);
    for _ in 0..3 {
        a.tick();
    }
    assert_eq!(a.player.as_ref().unwrap().device_name, name);
    a.do_action(Action::PlayerHide);
    for _ in 0..3 {
        a.tick();
    }
    assert_eq!(a.status.text, "Media player hidden");
    assert!(!a.show_player_bar);
}

#[test]
fn new_backend_statuses_are_localized() {
    use bluetuith_rs::i18n::{translate_status, Lang};
    assert_eq!(
        translate_status("Media player shown", Lang::Zh),
        "已显示媒体播放器"
    );
    assert_eq!(
        translate_status("Network list refreshed", Lang::Zh),
        "网络列表已刷新"
    );
    assert_eq!(
        translate_status("Audio profiles refreshed", Lang::Zh),
        "音频配置已刷新"
    );
    assert_eq!(
        translate_status(
            "DUN dial-up is not provided by BlueZ Network1 (reserved)",
            Lang::Zh
        ),
        "DUN 拨号不由 BlueZ Network1 提供（预留功能）"
    );
}

#[test]
fn chinese_help_can_scroll_to_the_final_hints() {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let cfg = Config {
        demo: true,
        language: "zh".to_string(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        a.tick();
    }
    a.push(Overlay::Help);
    for _ in 0..200 {
        a.on_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        render_mut(&mut a, 80, 24);
    }
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| ui::render(f, &mut a)).unwrap();
    let s = text_of(t.backend().buffer());
    assert!(
        s.contains("鼠标") || s.contains("config.toml"),
        "the bottom of the help text is unreachable after wrapping"
    );
}

// ---------------------------------------------------------------------------
// OBEX receiving, player shuffle/repeat, reserved options.

#[test]
fn incoming_file_push_uses_the_auth_dialog() {
    use bluetuith_rs::backend::Event;
    use bluetuith_rs::model::{AuthKind, AuthRequest};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let cfg = Config {
        demo: true,
        language: "zh".to_string(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    for _ in 0..4 {
        a.tick();
    }
    a.apply_event(Event::AuthRequest(AuthRequest {
        id: 42,
        device: "photo.jpg".into(),
        device_name: "photo.jpg".into(),
        kind: AuthKind::FilePush("photo.jpg".into()),
    }));
    assert_eq!(a.top_overlay(), Some(Overlay::Auth));
    let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
    t.draw(|f| ui::render(f, &mut a)).unwrap();
    // TestBackend pads wide CJK glyphs across two cells.
    let compact: String = text_of(t.backend().buffer())
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(compact.contains("收到文件"));
    a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(a.auth.is_none());
    assert!(a.top_overlay().is_none());
}

#[test]
fn player_shuffle_and_repeat_commands_reach_the_backend() {
    use bluetuith_rs::keybind::Action;
    use crossterm::event::KeyEvent;
    let mut a = app();
    a.push(Overlay::Player);
    let s0 = a.player.as_ref().unwrap().shuffle;
    let r0 = a.player.as_ref().unwrap().repeat;
    let s = a.keys.get(Action::PlayerShuffle);
    a.on_key(KeyEvent::new(s.code, s.mods));
    for _ in 0..2 {
        a.tick();
    }
    assert_ne!(a.player.as_ref().unwrap().shuffle, s0);
    let s = a.keys.get(Action::PlayerRepeat);
    a.on_key(KeyEvent::new(s.code, s.mods));
    for _ in 0..2 {
        a.tick();
    }
    assert_ne!(a.player.as_ref().unwrap().repeat, r0);
}

#[test]
fn reserved_gsm_options_warn_at_startup() {
    let cfg = Config {
        demo: true,
        gsm_apn: "internet".into(),
        ..Config::default()
    };
    let a = App::new(cfg, Box::new(DemoBackend::new()));
    assert!(a
        .startup_warning
        .as_deref()
        .unwrap_or("")
        .contains("gsm-apn"));
}

#[test]
fn cli_help_documents_the_player_controls() {
    let h = bluetuith_rs::cli::help_text();
    assert!(h.contains("shuffle"));
    assert!(h.contains("repeat"));
    assert!(h.contains("Directory for received OBEX files"));
}

#[test]
fn arrow_keys_move_the_selection_while_searching() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert!(a.search_active);
    let before = a.selected;
    a.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_ne!(a.selected, before, "Down was swallowed in search mode");
    assert!(a.search_active, "search must stay active while navigating");
}

#[test]
fn header_shows_the_filter_once_while_searching() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    // A token no demo device contains, so its only occurrence is the header.
    for c in "ZZQQ".chars() {
        a.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert!(a.search_active);
    assert_eq!(a.filter, "ZZQQ");
    let buf = render(a, 150, 44);
    let count = buf.matches("ZZQQ").count();
    assert_eq!(
        count, 1,
        "filter text must appear exactly once in the header while typing, not duplicated"
    );
}

#[test]
fn switching_language_refreshes_an_open_menu() {
    use bluetuith_rs::keybind::Action;
    let mut a = app();
    a.open_menu_at(MenuId::Adapter);
    let before = a.menu_items[0].label.clone();
    a.do_action(Action::Language);
    assert_ne!(
        a.menu_items[0].label, before,
        "an open menu keeps stale labels after a language switch"
    );
}

#[test]
fn player_show_warns_when_no_player_exists() {
    let mut a = app();
    a.player = None;
    a.do_action(bluetuith_rs::keybind::Action::PlayerShow);
    assert!(
        a.top_overlay().is_none(),
        "must not open an empty player window"
    );
    assert!(a.status.text.contains("No media player"));
}

#[test]
fn an_auth_prompt_is_answerable_while_a_filter_is_being_typed() {
    use bluetuith_rs::backend::Event;
    use bluetuith_rs::model::{AuthKind, AuthRequest};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    a.search_active = true;
    a.filter = "WH".into();
    // A pairing / file-push prompt can arrive asynchronously, mid-filter.
    a.apply_event(Event::AuthRequest(AuthRequest {
        id: 7,
        device: "AA:BB:CC:DD:EE:FF".into(),
        device_name: "Phone".into(),
        kind: AuthKind::Confirmation(123456),
    }));
    assert_eq!(a.top_overlay(), Some(Overlay::Auth));
    // Enter must ACCEPT the request, not be swallowed by the search filter.
    a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(a.auth.is_none(), "auth request was never answered");
    assert!(a.top_overlay().is_none(), "auth overlay did not close");
    assert_eq!(a.filter, "WH", "the dialog must not consume filter text");
}

#[test]
fn bracketed_paste_reaches_the_active_text_input() {
    use bluetuith_rs::backend::Event;
    use bluetuith_rs::model::{AuthKind, AuthRequest};
    let mut a = app();

    a.search_active = true;
    a.on_paste("WH-1000\nXM5");
    assert_eq!(a.filter, "WH-1000XM5", "newlines must be stripped");
    a.search_active = false;
    a.filter.clear();

    a.push(Overlay::CommandPalette);
    a.on_paste("Quit");
    assert_eq!(a.palette_query, "Quit");
    a.close_overlay();

    a.apply_event(Event::AuthRequest(AuthRequest {
        id: 9,
        device: "AA:BB:CC:DD:EE:FF".into(),
        device_name: "Phone".into(),
        kind: AuthKind::PinCode,
    }));
    a.on_paste("12 34\t56");
    assert_eq!(a.auth_input, "12 3456", "control chars must be stripped");
    a.close_overlay();

    a.apply_event(Event::AuthRequest(AuthRequest {
        id: 10,
        device: "AA:BB:CC:DD:EE:FF".into(),
        device_name: "Phone".into(),
        kind: AuthKind::Passkey(0),
    }));
    a.on_paste("ab12cd");
    assert_eq!(a.auth_input, "12", "passkey entry accepts digits only");
}

#[test]
fn a_stale_auth_overlay_can_always_be_dismissed() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut a = app();
    // Simulate a desync: an Auth overlay is present with no live request.
    a.push(Overlay::Auth);
    a.auth = None;
    a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        !a.overlays.contains(&Overlay::Auth),
        "Esc must dismiss a stale Auth overlay instead of trapping the user"
    );
}

#[test]
fn a_second_auth_request_supersedes_the_first() {
    use bluetuith_rs::backend::Event;
    use bluetuith_rs::model::{AuthKind, AuthRequest};
    let mut a = app();
    let first = AuthRequest {
        id: 41,
        device: "AA:BB:CC:DD:EE:FF".into(),
        device_name: "Phone".into(),
        kind: AuthKind::Confirmation(1111),
    };
    let second = AuthRequest {
        id: 42,
        device: "AA:BB:CC:DD:01:01".into(),
        device_name: "Earbuds".into(),
        kind: AuthKind::FilePush("photo.jpg".into()),
    };
    a.apply_event(Event::AuthRequest(first));
    a.apply_event(Event::AuthRequest(second));
    // Exactly one Auth overlay, showing the newest request.
    assert_eq!(
        a.overlays.iter().filter(|&&o| o == Overlay::Auth).count(),
        1
    );
    assert_eq!(a.auth.as_ref().map(|r| r.id), Some(42));
}

#[test]
fn repeated_quit_keeps_a_single_confirm_dialog() {
    use bluetuith_rs::keybind::Action;
    let cfg = Config {
        demo: true,
        confirm_on_quit: true,
        language: "en".into(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    // A stale Confirm overlay left deeper in the stack by an earlier dialog.
    a.push(Overlay::Confirm);
    a.do_action(Action::Quit);
    a.do_action(Action::Quit);
    assert_eq!(
        a.overlays
            .iter()
            .filter(|&&o| o == Overlay::Confirm)
            .count(),
        1,
        "repeated opens must not stack multiple Confirm overlays"
    );
    assert!(a.confirm.is_some());
}

#[test]
fn closing_an_overlay_above_confirm_keeps_confirm_state() {
    let cfg = Config {
        demo: true,
        confirm_on_quit: true,
        language: "en".into(),
        ..Config::default()
    };
    let mut a = App::new(cfg, Box::new(DemoBackend::new()));
    a.do_action(bluetuith_rs::keybind::Action::Quit);
    a.push(Overlay::Help); // open another overlay on top of the Confirm dialog
    a.close_overlay(); // closes Help, not Confirm
    assert!(
        a.confirm.is_some(),
        "closing an unrelated overlay must not drop the live confirm state"
    );
    assert!(a.overlays.contains(&Overlay::Confirm));
}

#[test]
fn non_ascii_uuid_never_panics_when_rendering() {
    use bluetuith_rs::backend::Event;
    use bluetuith_rs::model::Device;
    // 36 bytes long, but byte 4 sits inside a multi-byte char (the keycap
    // sequence), which used to make `&u[4..8]` panic on a char boundary.
    let bad_uuid = format!("{}x", "1️⃣".repeat(5));
    assert_eq!(bad_uuid.len(), 36);
    let d = Device {
        address: "AA:BB:CC:DD:EE:FF".into(),
        name: "Phone".into(),
        uuids: vec![bad_uuid],
        ..Default::default()
    };
    let mut a = app();
    a.apply_event(Event::Snapshot {
        adapters: vec![],
        devices: vec![d],
    });
    render_mut(&mut a, 120, 40); // device details pane
    a.push(Overlay::DeviceInfo);
    render_mut(&mut a, 120, 40); // device info overlay
}

#[test]
fn stale_mouse_click_index_cannot_corrupt_menu_selection() {
    use bluetuith_rs::app::MenuId;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;
    let mut a = app();
    a.open_menu_at(MenuId::View);
    let n = a.menu_items.len();
    // A click region cached from an earlier, larger layout (stale index).
    a.layout.overlay_rect = Some(Rect::new(0, 0, 120, 40));
    a.layout.overlay_items = vec![(Rect::new(0, 0, 40, 1), n + 50)];
    a.layout.help_rects.clear();
    a.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        a.menu_index < a.menu_items.len(),
        "a stale mouse index must not leave menu_index out of range"
    );
}

#[test]
fn language_switch_reclamps_device_selection() {
    use bluetuith_rs::keybind::Action;
    let mut a = app();
    let n = a.visible_devices().len();
    assert!(n > 0, "demo backend should populate devices");
    a.selected = n + 100; // deliberately stale index
    a.do_action(Action::Language);
    assert!(
        a.selected < a.visible_devices().len(),
        "switching language must re-clamp the selection to the visible list"
    );
}
