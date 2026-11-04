//! Command line. Hand-rolled rather than pulling in clap, so the release
//! binary stays small and statically linkable for every target.

use std::path::PathBuf;

use crate::config::Config;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub list_adapters: bool,
    pub generate: bool,
    pub help: bool,
    pub version: bool,
    pub demo: bool,
    pub config_path: Option<PathBuf>,
    pub overrides: Overrides,
}

#[derive(Clone, Debug, Default)]
pub struct Overrides {
    pub adapter: Option<String>,
    pub receive_dir: Option<String>,
    pub gsm_apn: Option<String>,
    pub gsm_number: Option<String>,
    pub adapter_states: Option<String>,
    pub connect_bdaddr: Option<String>,
    pub no_warning: Option<bool>,
    pub no_help_display: Option<bool>,
    pub confirm_on_quit: Option<bool>,
    pub disable_obex_services: Option<bool>,
    pub theme: Option<String>,
    pub accent: Option<String>,
    pub default_sort: Option<String>,
    pub show_hidden_files: Option<bool>,
    pub scan_on_start: Option<bool>,
}

impl Overrides {
    pub fn apply(&self, cfg: &mut Config) {
        macro_rules! ov {
            ($field:ident) => {
                if let Some(v) = &self.$field {
                    cfg.$field = v.clone();
                }
            };
        }
        ov!(adapter);
        ov!(receive_dir);
        ov!(gsm_apn);
        ov!(gsm_number);
        ov!(adapter_states);
        ov!(connect_bdaddr);
        ov!(theme);
        ov!(accent);
        ov!(default_sort);
        if let Some(v) = self.no_warning {
            cfg.no_warning = v;
        }
        if let Some(v) = self.no_help_display {
            cfg.no_help_display = v;
        }
        if let Some(v) = self.confirm_on_quit {
            cfg.confirm_on_quit = v;
        }
        if let Some(v) = self.disable_obex_services {
            cfg.disable_obex_services = v;
        }
        if let Some(v) = self.show_hidden_files {
            cfg.show_hidden_files = v;
        }
        if let Some(v) = self.scan_on_start {
            cfg.scan_on_start = v;
        }
    }
}

pub fn parse() -> Result<Options, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Options::default();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        // Support --opt=value.
        let (name, inline) = match a.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (a.clone(), None),
        };
        let needs_value = |args: &[String],
                           i: &mut usize,
                           inline: Option<String>,
                           field: &str|
         -> Result<String, String> {
            if let Some(v) = inline {
                return Ok(v);
            }
            *i += 1;
            args.get(*i)
                .cloned()
                .ok_or_else(|| format!("{field} requires a value"))
        };
        match name.as_str() {
            "-h" | "--help" => o.help = true,
            "-V" | "--version" => o.version = true,
            "-l" | "--list-adapters" => o.list_adapters = true,
            "-g" | "--generate" => o.generate = true,
            "--demo" => {
                o.demo = true;
                o.overrides.scan_on_start = Some(true);
            }
            "-w" | "--no-warning" => o.overrides.no_warning = Some(true),
            "-i" | "--no-help-display" => o.overrides.no_help_display = Some(true),
            "-c" | "--confirm-on-quit" => o.overrides.confirm_on_quit = Some(true),
            "--no-confirm-on-quit" => o.overrides.confirm_on_quit = Some(false),
            "-o" | "--disable-obex-services" => o.overrides.disable_obex_services = Some(true),
            "--show-hidden-files" => o.overrides.show_hidden_files = Some(true),
            "--scan-on-start" => o.overrides.scan_on_start = Some(true),
            "-a" | "--adapter" => {
                o.overrides.adapter = Some(needs_value(&args, &mut i, inline, "--adapter")?)
            }
            "-r" | "--receive-dir" => {
                o.overrides.receive_dir = Some(needs_value(&args, &mut i, inline, "--receive-dir")?)
            }
            "-m" | "--gsm-apn" => {
                o.overrides.gsm_apn = Some(needs_value(&args, &mut i, inline, "--gsm-apn")?)
            }
            "-b" | "--gsm-number" => {
                o.overrides.gsm_number = Some(needs_value(&args, &mut i, inline, "--gsm-number")?)
            }
            "-s" | "--adapter-states" => {
                o.overrides.adapter_states =
                    Some(needs_value(&args, &mut i, inline, "--adapter-states")?)
            }
            "-t" | "--connect-bdaddr" => {
                o.overrides.connect_bdaddr =
                    Some(needs_value(&args, &mut i, inline, "--connect-bdaddr")?)
            }
            "--theme" => o.overrides.theme = Some(needs_value(&args, &mut i, inline, "--theme")?),
            "--accent" => {
                o.overrides.accent = Some(needs_value(&args, &mut i, inline, "--accent")?)
            }
            "--default-sort" => {
                o.overrides.default_sort =
                    Some(needs_value(&args, &mut i, inline, "--default-sort")?)
            }
            "--config" => {
                o.config_path = Some(PathBuf::from(needs_value(
                    &args, &mut i, inline, "--config",
                )?))
            }
            _ => return Err(format!("unknown argument: {a}")),
        }
        i += 1;
    }
    Ok(o)
}

/// Environment variables are applied before CLI flags so flags win.
pub fn apply_env(cfg: &mut Config) {
    let env = |a: &str, b: &str| std::env::var(a).ok().or_else(|| std::env::var(b).ok());
    macro_rules! str_env {
        ($field:ident, $a:expr, $b:expr) => {
            if let Some(v) = env($a, $b) {
                cfg.$field = v;
            }
        };
    }
    str_env!(adapter, "BLUETUITH_RS_ADAPTER", "BLUETUITH_ADAPTER");
    str_env!(
        receive_dir,
        "BLUETUITH_RS_RECEIVE_DIR",
        "BLUETUITH_RECEIVE_DIR"
    );
    str_env!(gsm_apn, "BLUETUITH_RS_GSM_APN", "BLUETUITH_GSM_APN");
    str_env!(
        gsm_number,
        "BLUETUITH_RS_GSM_NUMBER",
        "BLUETUITH_GSM_NUMBER"
    );
    str_env!(
        adapter_states,
        "BLUETUITH_RS_ADAPTER_STATES",
        "BLUETUITH_ADAPTER_STATES"
    );
    str_env!(
        connect_bdaddr,
        "BLUETUITH_RS_CONNECT_BDADDR",
        "BLUETUITH_CONNECT_BDADDR"
    );
    str_env!(theme, "BLUETUITH_RS_THEME", "BLUETUITH_THEME");
    str_env!(accent, "BLUETUITH_RS_ACCENT", "");
    str_env!(default_sort, "BLUETUITH_RS_DEFAULT_SORT", "");
    if env("BLUETUITH_RS_NO_WARNING", "BLUETUITH_NO_WARNING").is_some() {
        cfg.no_warning = true;
    }
    if env("BLUETUITH_RS_NO_HELP_DISPLAY", "BLUETUITH_NO_HELP_DISPLAY").is_some() {
        cfg.no_help_display = true;
    }
    if env("BLUETUITH_RS_CONFIRM_ON_QUIT", "BLUETUITH_CONFIRM_ON_QUIT").is_some() {
        cfg.confirm_on_quit = true;
    }
    if env("BLUETUITH_RS_DEMO", "").is_some() {
        cfg.demo = true;
    }
}

pub const HELP: &str = "\
\x1b[1mbluetuith-rs\x1b[0m — a full-featured Bluetooth manager for the terminal

\x1b[1mUSAGE\x1b[0m
    bluetuith-rs [OPTIONS]

\x1b[1mOPTIONS\x1b[0m
    -l, --list-adapters            List available adapters and exit
    -g, --generate                 Write a default config file and exit
    -a, --adapter <NAME>           Adapter to use (e.g. hci0 or a MAC)
    -r, --receive-dir <DIR>        Directory for received OBEX files
    -m, --gsm-apn <APN>            APN for DUN dial-up networking
    -b, --gsm-number <NUMBER>      Number to dial (default *99#)
    -s, --adapter-states <LIST>    Startup states, e.g.
                                   'powered:yes,discoverable:yes,scan:no'
    -t, --connect-bdaddr <MAC>     Auto-connect this device on startup
    -w, --no-warning               Suppress the unavailable-features banner
    -i, --no-help-display          Hide the bottom keybinding strip
    -c, --confirm-on-quit          Ask for confirmation before quitting
        --no-confirm-on-quit       Quit immediately
    -o, --disable-obex-services    Disable OBEX services
        --show-hidden-files        Show hidden files in the picker
        --scan-on-start            Start discovery at launch
        --theme <NAME>             Palette: Midnight, Nord, Catppuccin,
                                   Dracula, Gruvbox, Tokyo Night,
                                   Rosé Pine, Solarized, Daylight
        --accent <#RRGGBB>         Override the accent colour
        --default-sort <KEY>       Name|Address|Rssi|Battery|State|Kind
        --config <PATH>            Use an alternate config file
        --demo                     Run against a simulated adapter (no BlueZ)
    -h, --help                     Print this help
    -V, --version                  Print version

\x1b[1mKEYBINDINGS (main screen)\x1b[0m
    ↑/↓ navigate · c connect · p pair · o power · S discoverable
    P pairable · s scan · a adapter · t trust · b block · f send files
    n network · A audio profiles · i info · d remove · v transfers
    m media player · M hide player · / search · F3 sort · F4 theme
    F2 details · F6 log · Ctrl+p palette · ? help · Q quit · Alt+m menu

\x1b[1mFILES\x1b[0m
    ~/.config/bluetuith-rs/config.toml   configuration
";
