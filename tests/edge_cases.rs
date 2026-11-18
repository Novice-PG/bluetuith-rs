use std::collections::HashMap;
use std::io::Write;

use bluetuith_rs::config::Config;
use bluetuith_rs::keybind::{parse_key, KeyMap};
use bluetuith_rs::model::{PlayerState, Transfer};

#[test]
fn parse_key_never_panics_on_weird_input() {
    let cases = [
        "",
        " ",
        "   ",
        "+",
        "ctrl",
        "Ctrl",
        "SHIFT",
        "ctrl+",
        "+ctrl",
        "ctrl++alt+x",
        "F",
        "F0",
        "F00",
        "F99",
        "Fabc",
        "F0000000001",
        "f13",
        "F12",
        "f-1",
        "中",
        "é",
        "ctrl+alt+shift+x",
        "a+b+c+d+e",
        "\t",
        "\n",
        "Space",
        "space",
        "spc",
        "  PgUp  ",
        &"x".repeat(10000),
        &"+".repeat(1000),
    ];
    for c in cases {
        let _ = parse_key(c); // must not panic
    }
}

#[test]
fn keymap_from_config_never_panics() {
    let mut kv = HashMap::new();
    kv.insert("".to_string(), "".to_string());
    kv.insert("Quit".to_string(), "".to_string());
    kv.insert("quit".to_string(), "ctrl+q".to_string());
    kv.insert("bogus action".to_string(), "x".to_string());
    kv.insert("Connect".to_string(), "p".to_string()); // conflict with Pair
    let map = KeyMap::from_config(&kv);
    let _ = map.conflicts;
    // get() must not panic for any action.
    for a in bluetuith_rs::keybind::Action::all() {
        let _ = map.label(a);
    }
}

#[test]
fn config_load_never_panics_on_malformed_toml() {
    let inputs = [
        "not toml at all {{{",
        "[keybindings]\nQuit = 12345\n",
        "accent = 999\n",
        "adapter_states = [1,2,3]\n",
        "\0\0\0",
        "theme = \"\u{1F600}\"\n",
    ];
    for text in inputs {
        let dir = std::env::temp_dir().join(format!("fuzzcfg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("c.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(text.as_bytes()).unwrap();
        drop(f);
        let _ = Config::load(&path); // must not panic
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn model_progress_handles_extremes() {
    let p = PlayerState {
        duration_ms: 0,
        position_ms: u64::MAX,
        ..Default::default()
    };
    assert_eq!(p.progress(), 0.0);
    let p = PlayerState {
        duration_ms: u64::MAX,
        position_ms: u64::MAX,
        ..Default::default()
    };
    let v = p.progress();
    assert!(v.is_finite() && (0.0..=1.0).contains(&v));

    let t = Transfer {
        size: 0,
        transferred: u64::MAX,
        ..Default::default()
    };
    assert_eq!(t.progress(), 0.0);
    let t = Transfer {
        size: u64::MAX,
        transferred: u64::MAX,
        ..Default::default()
    };
    let v = t.progress();
    assert!(v.is_finite() && (0.0..=1.0).contains(&v));
}
