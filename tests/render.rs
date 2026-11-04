use bluetuith_rs::app::{App, Overlay};
use bluetuith_rs::backend::demo::DemoBackend;
use bluetuith_rs::config::Config;
use bluetuith_rs::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn app() -> App {
    let cfg = Config {
        demo: true,
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
