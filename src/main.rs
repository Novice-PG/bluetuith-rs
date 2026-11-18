//! Entry point: parse options, set up the terminal, run the event loop.

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use bluetuith_rs::app::App;
use bluetuith_rs::backend::{AdapterProperty, Backend, Command, Event};
use bluetuith_rs::cli;
use bluetuith_rs::config::Config;
use bluetuith_rs::ui;

fn main() {
    if let Err(e) = run() {
        eprintln!("bluetuith-rs: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let opts = cli::parse().map_err(anyhow::Error::msg)?;
    if opts.help {
        print!("{}", cli::help_text());
        return Ok(());
    }
    if opts.version {
        println!(
            "bluetuith-rs {} ({})",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_NAME")
        );
        return Ok(());
    }

    let config_path = opts.config_path.clone().unwrap_or_else(Config::path);
    if opts.generate {
        Config::generate(&config_path)?;
        println!("Wrote {}", config_path.display());
        return Ok(());
    }

    let mut config = Config::load(&config_path)?;
    cli::apply_env(&mut config);
    opts.overrides.apply(&mut config);
    if opts.demo {
        config.demo = true;
    }

    if opts.list_adapters {
        return list_adapters(&config);
    }

    let backend: Box<dyn Backend> = if config.demo {
        Box::new(bluetuith_rs::backend::demo::DemoBackend::new())
    } else {
        Box::new(bluetuith_rs::backend::bluez::BluezBackend::new(&config)?)
    };

    let mut app = App::new(config.clone(), backend);
    apply_startup(&mut app, &config);

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original_hook(info);
    }));

    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let _ = execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        )
    );
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.hide_cursor()?;

    let result = event_loop(&mut terminal, &mut app);

    restore_terminal();
    let _ = terminal.show_cursor();
    if let Some(w) = app.startup_warning.take() {
        eprintln!("warning: {w}");
    }
    result
}

fn apply_startup(app: &mut App, cfg: &Config) {
    for (key, value) in cfg.adapter_state_list() {
        let property = match key.as_str() {
            "powered" => AdapterProperty::Powered,
            "discoverable" => AdapterProperty::Discoverable,
            "pairable" => AdapterProperty::Pairable,
            "scan" => {
                app.dispatch(Command::SetScan(value));
                continue;
            }
            _ => continue,
        };
        app.dispatch(Command::SetAdapterProperty { property, value });
    }
    if cfg.scan_on_start {
        app.dispatch(Command::SetScan(true));
    }
    if !cfg.connect_bdaddr.is_empty() {
        app.dispatch(Command::ConnectDevice(cfg.connect_bdaddr.clone()));
    }
    if !cfg.adapter.is_empty() {
        app.dispatch(Command::SelectAdapter(cfg.adapter.clone()));
    }
}

fn list_adapters(cfg: &Config) -> Result<()> {
    let mut backend: Box<dyn Backend> = if cfg.demo {
        Box::new(bluetuith_rs::backend::demo::DemoBackend::new())
    } else {
        Box::new(bluetuith_rs::backend::bluez::BluezBackend::new(cfg)?)
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut adapters = Vec::new();
    while Instant::now() < deadline {
        for ev in backend.drain() {
            if let Event::Snapshot { adapters: a, .. } = ev {
                adapters = a;
            }
        }
        if !adapters.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    if adapters.is_empty() {
        println!("No Bluetooth adapters found.");
    } else {
        println!("List of adapters:");
        for a in adapters {
            println!(
                "- {} ({}) {}  [{}]",
                a.display_name(),
                a.id,
                a.address,
                if a.powered { "powered" } else { "off" }
            );
        }
    }
    Ok(())
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    while app.running {
        app.tick();
        terminal.draw(|f| ui::render(f, app))?;

        if app.suspend_requested {
            app.suspend_requested = false;
            restore_terminal();
            #[cfg(unix)]
            unsafe {
                libc::raise(libc::SIGTSTP);
            }
            enable_raw_mode()?;
            let mut stdout = io::stdout();
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste
            )?;
            // Keyboard enhancement flags do not survive the terminal reset.
            let _ = execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                )
            );
            terminal.clear()?;
        }

        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                event::Event::Key(k) => {
                    if k.kind != KeyEventKind::Release {
                        app.on_key(k);
                    }
                }
                event::Event::Mouse(m) => app.on_mouse(m),
                // Bracketed paste is enabled above; without this arm a paste
                // is delivered as one event and would otherwise be dropped.
                event::Event::Paste(text) => app.on_paste(&text),
                event::Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    Ok(())
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste,
        PopKeyboardEnhancementFlags
    );
}
