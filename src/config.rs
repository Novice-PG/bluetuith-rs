//! Configuration. Mirrors the option surface of bluetuith's `koanf` config,
//! plus a few extras (theme, sorting, demo mode).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Adapter to select on startup (`hci0` or a MAC). Empty = first adapter.
    pub adapter: String,
    /// Where inbound OBEX Object Push files land.
    #[serde(rename = "receive-dir")]
    pub receive_dir: String,
    /// APN for DUN dial-up networking.
    #[serde(rename = "gsm-apn")]
    pub gsm_apn: String,
    /// Number to dial for DUN (default `*99#`).
    #[serde(rename = "gsm-number")]
    pub gsm_number: String,
    /// e.g. `powered:yes,discoverable:yes,pairable:yes,scan:no`
    #[serde(rename = "adapter-states")]
    pub adapter_states: String,
    /// Auto-connect this device address on startup.
    #[serde(rename = "connect-bdaddr")]
    pub connect_bdaddr: String,
    /// Suppress the "features unavailable" banner.
    #[serde(rename = "no-warning")]
    pub no_warning: bool,
    /// Hide the bottom keybinding strip.
    #[serde(rename = "no-help-display")]
    pub no_help_display: bool,
    /// Ask `Quit (y/n)?` before exiting.
    #[serde(rename = "confirm-on-quit")]
    pub confirm_on_quit: bool,
    /// Disable the OBEX services.
    #[serde(rename = "disable-obex-services")]
    pub disable_obex_services: bool,

    // ---- extras ----
    /// Built-in palette name, see `theme::Theme::name_list()`.
    pub theme: String,
    /// Optional `#rrggbb` accent override.
    pub accent: String,
    /// Default device sort: Name | Address | Rssi | Battery | State | Kind.
    #[serde(rename = "default-sort")]
    pub default_sort: String,
    #[serde(rename = "show-hidden-files")]
    pub show_hidden_files: bool,
    #[serde(rename = "scan-on-start")]
    pub scan_on_start: bool,
    /// Run against the built-in simulated adapter instead of BlueZ.
    pub demo: bool,
    /// UI language: `en` or `zh` (also auto-detected from `$LANG`).
    #[serde(rename = "language")]
    pub language: String,
    /// Override any named action, e.g. `Quit = "q"`.
    pub keybindings: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adapter: String::new(),
            receive_dir: String::new(),
            gsm_apn: String::new(),
            gsm_number: "*99#".into(),
            adapter_states: String::new(),
            connect_bdaddr: String::new(),
            no_warning: false,
            no_help_display: false,
            confirm_on_quit: true,
            disable_obex_services: false,
            theme: "Midnight".into(),
            accent: String::new(),
            default_sort: "Name".into(),
            show_hidden_files: false,
            scan_on_start: false,
            demo: false,
            language: crate::i18n::Lang::detect().code().to_string(),
            keybindings: HashMap::new(),
        }
    }
}

impl Config {
    /// `$XDG_CONFIG_HOME/bluetuith-rs/config.toml`, falling back to
    /// `~/.config/bluetuith-rs/config.toml`.
    pub fn path() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Path::new(&xdg).join("bluetuith-rs").join("config.toml");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            return Path::new(&home)
                .join(".config")
                .join("bluetuith-rs")
                .join("config.toml");
        }
        PathBuf::from("config.toml")
    }

    /// Load from `path`, or return defaults when the file does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    /// Write a commented default configuration. Existing files are backed up.
    pub fn generate(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        if path.exists() {
            let backup = path.with_extension("toml.bak");
            fs::copy(path, &backup)
                .with_context(|| format!("backing up to {}", backup.display()))?;
        }
        let body = toml::to_string_pretty(&Config::default())?;
        let header = concat!(
            "# bluetuith-rs configuration\n",
            "# Every key below is optional; delete a line to use its default.\n",
            "# Run `bluetuith-rs --generate` to regenerate this file.\n\n",
        );
        let footer = concat!(
            "\n# Override any named action (see --help for the full list).\n",
            "# Conflicting bindings in one context are reported at startup.\n",
            "# [keybindings]\n",
            "# Quit = \"Ctrl+q\"\n",
            "# Connect = \"c\"\n",
        );
        fs::write(path, format!("{header}{body}{footer}"))
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Parse `powered:yes,discoverable:no,...` into ordered (key, bool) pairs.
    pub fn adapter_state_list(&self) -> Vec<(String, bool)> {
        let mut out = Vec::new();
        for part in self.adapter_states.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let mut it = part.splitn(2, ':');
            let key = it.next().unwrap_or("").trim().to_ascii_lowercase();
            let val = it.next().unwrap_or("").trim().to_ascii_lowercase();
            if !matches!(
                key.as_str(),
                "powered" | "scan" | "discoverable" | "pairable"
            ) {
                continue;
            }
            let on = matches!(val.as_str(), "yes" | "y" | "on" | "true" | "1");
            out.push((key, on));
        }
        out
    }

    pub fn effective_receive_dir(&self) -> PathBuf {
        if self.receive_dir.is_empty() {
            let base = std::env::var("XDG_DOWNLOAD_DIR")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var("HOME")
                        .map(|h| PathBuf::from(h).join("Downloads"))
                        .unwrap_or_else(|_| PathBuf::from("."))
                });
            base.join("bluetuith-rs")
        } else {
            PathBuf::from(&self.receive_dir)
        }
    }

    /// Apply the accent override, if any, to a theme.
    pub fn apply_accent(&self, theme: &mut crate::theme::Theme) {
        let s = self.accent.trim().trim_start_matches('#');
        if s.len() != 6 {
            return;
        }
        let parse = |i: usize| u8::from_str_radix(&s[i..i + 2], 16);
        if let (Ok(r), Ok(g), Ok(b)) = (parse(0), parse(2), parse(4)) {
            let c = ratatui::style::Color::Rgb(r, g, b);
            theme.accent = c;
            theme.border_focus = c;
            theme.info = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_states_parse_in_order() {
        let c = Config {
            adapter_states: "powered:yes, discoverable:no, scan:on, bogus:yes".into(),
            ..Config::default()
        };
        let v = c.adapter_state_list();
        assert_eq!(
            v,
            vec![
                ("powered".to_string(), true),
                ("discoverable".to_string(), false),
                ("scan".to_string(), true),
            ]
        );
    }

    #[test]
    fn generate_then_load_is_a_round_trip() {
        let dir = std::env::temp_dir().join(format!("bluetuith-rs-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let _ = fs::remove_dir_all(&dir);
        Config::generate(&path).unwrap();
        let mut loaded = Config::load(&path).unwrap();
        loaded.theme = "Nord".into();
        assert_eq!(loaded.theme, "Nord");
        assert_eq!(loaded.gsm_number, "*99#");
        let _ = fs::remove_dir_all(&dir);
    }
}
