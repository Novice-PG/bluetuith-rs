//! Temporary real-hardware causality check: make the *program* write an
//! adapter property through BluezBackend, then independently read it back
//! with a separate D-Bus connection, and restore it afterwards.

use std::thread::sleep;
use std::time::{Duration, Instant};

use bluetuith_rs::app::App;
use bluetuith_rs::backend::bluez::BluezBackend;
use bluetuith_rs::backend::{AdapterProperty, Command};
use bluetuith_rs::config::Config;
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

fn pump(app: &mut App, ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        app.tick();
        sleep(Duration::from_millis(25));
    }
}

fn read_discoverable(c: &Connection) -> anyhow::Result<bool> {
    let r = c.call_method(
        Some("org.bluez"),
        "/org/bluez/hci0",
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.bluez.Adapter1", "Discoverable"),
    )?;
    let (v,): (OwnedValue,) = r.body().deserialize()?;
    Ok(v.downcast_ref::<bool>().unwrap_or(false))
}

fn main() -> anyhow::Result<()> {
    let cfg = Config {
        demo: false,
        language: "en".into(),
        ..Config::default()
    };
    let mut app = App::new(cfg.clone(), Box::new(BluezBackend::new(&cfg)?));
    let probe = Connection::system()?;
    pump(&mut app, 700);

    // The real obexd (activated on the session bus) must accept our
    // org.bluez.obex.Agent1 registration, otherwise receiving is impossible.
    let agent_failed = app
        .log
        .iter()
        .any(|l| l.contains("OBEX receive agent unavailable"));
    println!(
        "obex receive agent registered = {}",
        !agent_failed && app.features.obex
    );
    assert!(!agent_failed, "real obexd rejected the receive Agent");

    let before = read_discoverable(&probe)?;
    println!("before            = {before}");

    app.dispatch(Command::SetAdapterProperty {
        property: AdapterProperty::Discoverable,
        value: true,
    });
    pump(&mut app, 600);
    let after_true = read_discoverable(&probe)?;
    println!(
        "after Set(true)   = {after_true}   status={:?}",
        app.status.text
    );

    app.dispatch(Command::SetAdapterProperty {
        property: AdapterProperty::Discoverable,
        value: false,
    });
    pump(&mut app, 600);
    let restored = read_discoverable(&probe)?;
    println!(
        "after Set(false)  = {restored}   status={:?}",
        app.status.text
    );

    assert!(after_true, "the program's Set(true) did not change BlueZ");
    assert_eq!(restored, before, "adapter state was not restored");
    println!("REAL PROPERTY ROUND-TRIP OK");
    Ok(())
}
