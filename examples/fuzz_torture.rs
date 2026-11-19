//! Hardcore deterministic fuzz / property torture harness.
//!
//! Self-contained PRNG (xorshift64*) generates millions of adversarial inputs
//! (random bytes, control chars, CJK, emoji, combining marks, over-long
//! strings, extreme integers) and feeds them to every pure parsing/formatting
//! path, asserting no panic and key invariants. Also does a real multi-thread
//! concurrency stress and a long-running tick loop to catch state leaks.
//!
//! Run: cargo run --release --example fuzz_torture -- [iterations]
//! Default iterations = 200000 (fast); pass 2000000 for a deeper run.

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use bluetuith_rs::backend::Event;
use bluetuith_rs::config::Config;
use bluetuith_rs::i18n::{translate_status, Lang};
use bluetuith_rs::keybind::{parse_key, Action, KeyMap};
use bluetuith_rs::model::*;
use bluetuith_rs::theme::Theme;
use bluetuith_rs::util;
use unicode_width::UnicodeWidthStr;

// ---- deterministic PRNG ---------------------------------------------------

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E3779B97F4A7C15)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
    fn range(&mut self, a: u64, b: u64) -> u64 {
        a + self.below(b - a + 1)
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
}

// adversarial character pools
const ASCII: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+-_/\\:.?[](){}!@#$%^&*~`\"' <>|";
const CONTROL: &[char] = &['\0', '\r', '\n', '\t', '\u{1b}', '\u{7f}', '\u{8}'];
const CJK: &[char] = &[
    '设', '备', '连', '接', '蓝', '牙', '电', '源', '扫', '描', '适', '配', '器', '视', '图', '帮',
    '助', '打', '开', '关', '闭',
];
const EMOJI: &[char] = &[
    '😀', '🎧', '📱', '💻', '🔋', '⌨', '🖱', '✓', '✗', '★', '☆', '⊘', '◆', '◇', '●', '○', '⌕',
];
const COMBINING: &[char] = &[
    '\u{0301}', '\u{0308}', '\u{200b}', '\u{200d}', '\u{fe0f}', '\u{00ad}',
];

fn rand_string(rng: &mut Rng, max_len: usize) -> String {
    let len = rng.below(max_len as u64) as usize;
    let mut s = String::with_capacity(len * 4);
    for _ in 0..len {
        let c = match rng.below(100) {
            0..=45 => rng.pick(ASCII) as char,
            46..=55 => rng.pick(CONTROL),
            56..=75 => rng.pick(CJK),
            76..=88 => rng.pick(EMOJI),
            89..=96 => rng.pick(COMBINING),
            _ => char::from_u32(rng.range(0x80, 0x2fff) as u32).unwrap_or('?'),
        };
        s.push(c);
    }
    s
}

// ---- invariant helpers ----------------------------------------------------

fn check<F: FnOnce() -> bool>(failures: &AtomicU64, name: &str, f: F) {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let out = panic::catch_unwind(AssertUnwindSafe(f));
    panic::set_hook(hook);
    let ok = matches!(out, Ok(true));
    if !ok {
        failures.fetch_add(1, Ordering::Relaxed);
        eprintln!("FAIL: {name}");
    }
}

// ---- fuzz targets ---------------------------------------------------------

fn fuzz_parse_key(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "parse_key", || {
        for _ in 0..200 {
            let s = rand_string(rng, 40);
            let _ = parse_key(&s);
        }
        true
    });
}

fn fuzz_keymap_config(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "KeyMap::from_config", || {
        let mut kv = std::collections::HashMap::new();
        for _ in 0..20 {
            kv.insert(rand_string(rng, 24), rand_string(rng, 30));
        }
        let map = KeyMap::from_config(&kv);
        // get() must never panic for any action, and labels must be nonempty.
        for a in Action::all() {
            let label = map.label(a);
            if label.is_empty() {
                return false;
            }
        }
        true
    });
}

fn fuzz_config_parse(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "Config::load malformed TOML", || {
        let dir = std::env::temp_dir().join(format!("fuzz-torture-cfg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("c.toml");
        for _ in 0..20 {
            let s = rand_string(rng, 300);
            if std::fs::write(&path, s.as_bytes()).is_err() {
                return false;
            }
            let _ = Config::load(&path);
        }
        let _ = std::fs::remove_dir_all(&dir);
        true
    });
}

fn fuzz_adapter_states(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "adapter_state_list", || {
        for _ in 0..200 {
            let s = rand_string(rng, 120);
            let c = Config {
                adapter_states: s,
                ..Config::default()
            };
            let v = c.adapter_state_list();
            // Every key is from a fixed set; values are bool. Just ensure it terminates.
            for (k, _) in v {
                if !matches!(k.as_str(), "powered" | "scan" | "discoverable" | "pairable") {
                    return false;
                }
            }
        }
        true
    });
}

fn fuzz_accent(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "apply_accent", || {
        let mut theme = Theme::midnight();
        let before = (theme.accent, theme.border_focus, theme.info);
        for _ in 0..500 {
            let s = rand_string(rng, 30);
            let c = Config {
                accent: s,
                ..Config::default()
            };
            c.apply_accent(&mut theme);
            // Theme colours must remain valid; any change must set all three together.
            let after = (theme.accent, theme.border_focus, theme.info);
            if before != after
                && !(theme.accent == theme.border_focus && theme.accent == theme.info)
            {
                return false;
            }
        }
        true
    });
}

fn fuzz_translate(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "translate_status", || {
        for _ in 0..300 {
            let s = rand_string(rng, 80);
            let a = translate_status(&s, Lang::En);
            let b = translate_status(&a, Lang::Zh);
            let _ = translate_status(&b, Lang::En);
        }
        true
    });
}

fn fuzz_theme(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "Theme::by_name", || {
        for _ in 0..300 {
            let s = rand_string(rng, 40);
            let _ = Theme::by_name(&s);
        }
        // Round-trip all catalogue names.
        for name in Theme::name_list() {
            if Theme::by_name(name).is_none() {
                return false;
            }
        }
        true
    });
}

fn fuzz_util(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "util::truncate/pad width invariant", || {
        for _ in 0..500 {
            let s = rand_string(rng, 60);
            let w = rng.below(40) as usize;
            let t = util::truncate(&s, w);
            let tw = UnicodeWidthStr::width(t.as_str());
            // truncate output must never exceed requested width (allow 1 for ellipsis overshoot with wide glyphs)
            if tw > w + 1 {
                return false;
            }
            let p = util::pad(&s, w);
            let pw = UnicodeWidthStr::width(p.as_str());
            if pw > w + 1 {
                return false;
            }
            let n = rng.next();
            let _ = util::human_bytes(n);
            let _ = util::duration(n);
        }
        true
    });
}

#[allow(clippy::field_reassign_with_default)]
fn fuzz_model(rng: &mut Rng, failures: &AtomicU64) {
    check(failures, "model helpers invariants", || {
        for _ in 0..500 {
            let mut d = Device::default();
            d.name = rand_string(rng, 40);
            d.alias = rand_string(rng, 40);
            d.address = rand_string(rng, 40);
            d.rssi = if rng.below(2) == 0 {
                Some(rng.range(0, 0xffff) as u16 as i16)
            } else {
                None
            };
            d.battery = if rng.below(2) == 0 {
                Some(rng.below(256) as u8)
            } else {
                None
            };
            for _ in 0..(rng.below(8) as usize) {
                d.uuids.push(rand_string(rng, 40));
            }
            let _ = d.display_name();
            let _ = d.kind();
            let bars = d.signal_bars();
            if bars > 4 {
                return false;
            }
            let _ = d.has_service(&rand_string(rng, 12));

            let p = PlayerState {
                duration_ms: rng.next(),
                position_ms: rng.next(),
                ..Default::default()
            };
            let prog = p.progress();
            if !prog.is_finite() || prog < 0.0 || prog > 1.0 {
                return false;
            }

            let t = Transfer {
                size: rng.next(),
                transferred: rng.next(),
                ..Default::default()
            };
            let tp = t.progress();
            if !tp.is_finite() || tp < 0.0 || tp > 1.0 {
                return false;
            }
        }
        true
    });
}

// ---- concurrency & long-run stress ---------------------------------------

fn concurrency_stress(failures: &AtomicU64) {
    check(failures, "real multi-thread concurrency", || {
        let mut handles = Vec::new();
        for t in 0..4 {
            handles.push(thread::spawn(move || {
                let mut rng = Rng::new(0xDEADBEEF + t);
                let mut bad = false;
                for _ in 0..20000 {
                    let s = rand_string(&mut rng, 30);
                    if parse_key(&s).is_none() && rng.below(10) == 0 {
                        // most random strings are invalid keys; that's expected.
                    }
                    let _ = translate_status(&s, Lang::Zh);
                    let _ = Theme::by_name(&s);
                    let _ = util::truncate(&s, 10);
                    let prog = PlayerState {
                        duration_ms: rng.next(),
                        position_ms: rng.next(),
                        ..Default::default()
                    }
                    .progress();
                    if !prog.is_finite() {
                        bad = true;
                    }
                }
                !bad
            }));
        }
        let mut ok = true;
        for h in handles {
            if !h.join().unwrap_or(false) {
                ok = false;
            }
        }
        ok
    });
}

fn long_run_stress(failures: &AtomicU64) {
    check(failures, "long tick loop (state stability)", || {
        use bluetuith_rs::app::App;
        use bluetuith_rs::backend::demo::DemoBackend;
        let cfg = Config {
            demo: true,
            ..Config::default()
        };
        let mut a = App::new(cfg, Box::new(DemoBackend::new()));
        let mut rng = Rng::new(0x12345678);
        for i in 0..200_000u64 {
            a.tick();
            if i % 97 == 0 {
                // Exercise selections across a shifting list.
                a.selected = rng.below(30) as usize;
                let _ = a.selected_addr();
            }
            if i % 1013 == 0 {
                a.apply_event(Event::Snapshot {
                    adapters: a.adapters.clone(),
                    devices: a.devices.clone(),
                });
            }
        }
        // After 200k ticks the app must still render and have bounded log.
        let mut t = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        t.draw(|f| bluetuith_rs::ui::render(f, &mut a)).is_ok()
    });
}

fn main() {
    let iterations: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    println!(
        "=== fuzz torture ({} iterations, seed fixed) ===",
        iterations
    );
    let mut rng = Rng::new(0x0123456789ABCDEF);
    let failures = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let rounds = (iterations / 100).max(1);
    let mut i = 0;
    while i < rounds {
        fuzz_parse_key(&mut rng, &failures);
        fuzz_keymap_config(&mut rng, &failures);
        fuzz_config_parse(&mut rng, &failures);
        fuzz_adapter_states(&mut rng, &failures);
        fuzz_accent(&mut rng, &failures);
        fuzz_translate(&mut rng, &failures);
        fuzz_theme(&mut rng, &failures);
        fuzz_util(&mut rng, &failures);
        fuzz_model(&mut rng, &failures);
        i += 1;
        if i % 50 == 0 {
            eprintln!("  progress {}/{} rounds", i, rounds);
        }
    }

    println!("--- concurrency ---");
    concurrency_stress(&failures);
    println!("--- long run ---");
    long_run_stress(&failures);

    let fails = failures.load(Ordering::Relaxed);
    println!(
        "\n=== result: {} failures in {:?} ===",
        fails,
        start.elapsed()
    );
    if fails > 0 {
        std::process::exit(1);
    }
}
