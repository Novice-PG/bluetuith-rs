//! Render a demo frame into a TestBackend and dump every cell (symbol + fg/bg
//! colours) as JSON. Used by `packaging/render-snapshot.py` to produce the
//! screenshots in the README, and handy for checking layout without a TTY.
//!
//! Usage:
//!   cargo run --example snapshot -- 150 44 main /tmp/frame.json

use std::env;
use std::fs;

use bluetuith_rs::app::{App, Overlay};
use bluetuith_rs::backend::demo::DemoBackend;
use bluetuith_rs::config::Config;
use bluetuith_rs::ui;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;

#[allow(unreachable_patterns)]
fn rgb(c: Color) -> (u8, u8, u8) {
    use Color::*;
    match c {
        Rgb(r, g, b) => (r, g, b),
        Reset => (13, 17, 27),
        Black => (0, 0, 0),
        Red => (205, 49, 49),
        Green => (13, 188, 121),
        Yellow => (229, 229, 16),
        Blue => (36, 114, 200),
        Magenta => (188, 63, 188),
        Cyan => (17, 168, 205),
        Gray => (128, 128, 128),
        DarkGray => (64, 64, 64),
        LightRed => (255, 100, 100),
        LightGreen => (100, 255, 100),
        LightYellow => (255, 255, 100),
        LightBlue => (100, 150, 255),
        LightMagenta => (255, 100, 255),
        LightCyan => (100, 255, 255),
        White => (220, 220, 220),
        Indexed(_) => (160, 160, 160),
        _ => (160, 160, 160),
    }
}

#[allow(clippy::collapsible_str_replace)]
fn main() {
    let args: Vec<String> = env::args().collect();
    let width: u16 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(150);
    let height: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(44);
    let which = args.get(3).map(String::as_str).unwrap_or("main");
    let out = args
        .get(4)
        .cloned()
        .unwrap_or_else(|| "/tmp/bluetuith-frame.json".into());

    let cfg = Config {
        demo: true,
        ..Config::default()
    };
    let backend = Box::new(DemoBackend::new());
    let mut app = App::new(cfg, backend);
    for _ in 0..4 {
        app.tick();
    }
    // A friendlier selection than row 0.
    app.selected = 1;

    match which {
        "menu" => app.open_menu(),
        "device-menu" => {
            app.menu_id = bluetuith_rs::app::MenuId::Device;
            app.rebuild_menu();
            app.push(Overlay::Menu);
        }
        "help" => app.push(Overlay::Help),
        "info" => app.push(Overlay::DeviceInfo),
        "progress" | "transfers" => app.push(Overlay::Progress),
        "player" => app.push(Overlay::Player),
        "network" => app.push(Overlay::Network),
        "profiles" => app.push(Overlay::Profiles),
        "themes" | "theme" => app.push(Overlay::ThemePicker),
        "adapters" => app.push(Overlay::AdapterChange),
        "files" | "filepicker" => app.push(Overlay::FilePicker),
        "log" => app.push(Overlay::Log),
        "palette" => app.push(Overlay::CommandPalette),
        "about" => app.push(Overlay::About),
        "confirm" => {
            app.confirm = Some(bluetuith_rs::app::ConfirmState {
                text: "Quit bluetuith-rs?".into(),
                action: bluetuith_rs::app::ConfirmAction::Quit,
            });
            app.push(Overlay::Confirm);
        }
        "auth" => {
            app.auth = Some(bluetuith_rs::model::AuthRequest {
                id: 1,
                device: "AA:BB:CC:DD:0A:0A".into(),
                device_name: "Canon EOS R6".into(),
                kind: bluetuith_rs::model::AuthKind::Confirmation(482731),
            });
            app.push(Overlay::Auth);
        }
        _ => {}
    }
    app.tick();

    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::render(f, &mut app)).unwrap();
    let buf = terminal.backend().buffer();
    let area = buf.area;

    let mut json = format!(
        "{{\"width\":{},\"height\":{},\"cells\":[",
        area.width, area.height
    );
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            let sym = cell.symbol();
            let (fr, fg, fb) = rgb(cell.fg);
            let (br, bg, bb) = rgb(cell.bg);
            let reversed = cell.modifier.contains(Modifier::REVERSED);
            let bold = cell.modifier.contains(Modifier::BOLD);
            let (fr, fg, fb, br, bg, bb) = if reversed {
                (br, bg, bb, fr, fg, fb)
            } else {
                (fr, fg, fb, br, bg, bb)
            };
            let escaped = sym
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', " ")
                .replace('\r', " ");
            json.push_str(&format!(
                "[\"{}\",[{fr},{fg},{fb}],[{br},{bg},{bb}],{}],",
                escaped,
                if bold { 1 } else { 0 }
            ));
        }
    }
    json.pop(); // trailing comma
    json.push_str("]}");
    fs::write(&out, json).unwrap();
    println!("wrote {out} ({}x{})", area.width, area.height);
}
