//! Application state and input handling. No rendering happens here: `ui`
//! reads `&App`, and every mutation goes through a method on `App`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::backend::{AdapterProperty, Backend, Command, Event};
use crate::config::Config;
use crate::keybind::{Action, Context, KeyMap};
use crate::model::*;
use crate::theme::Theme;
use crate::util;

// ---------------------------------------------------------------------------
// Overlays

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Menu,
    Help,
    Confirm,
    AdapterChange,
    DeviceInfo,
    FilePicker,
    Progress,
    Player,
    Network,
    Profiles,
    Auth,
    ThemePicker,
    Log,
    CommandPalette,
    About,
}

impl Overlay {
    pub fn title(self) -> &'static str {
        match self {
            Overlay::Menu => "Menu",
            Overlay::Help => "Keybindings & Help",
            Overlay::Confirm => "Confirm",
            Overlay::AdapterChange => "Select Adapter",
            Overlay::DeviceInfo => "Device Information",
            Overlay::FilePicker => "Send Files",
            Overlay::Progress => "Transfers",
            Overlay::Player => "Media Player",
            Overlay::Network => "Network Options",
            Overlay::Profiles => "Audio Profiles",
            Overlay::Auth => "Pairing Request",
            Overlay::ThemePicker => "Theme",
            Overlay::Log => "Event Log",
            Overlay::CommandPalette => "Command Palette",
            Overlay::About => "About",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuId {
    Adapter,
    Device,
    View,
    Help,
}

impl MenuId {
    pub fn all() -> [MenuId; 4] {
        [MenuId::Adapter, MenuId::Device, MenuId::View, MenuId::Help]
    }
    pub fn label(self) -> &'static str {
        match self {
            MenuId::Adapter => "Adapter",
            MenuId::Device => "Device",
            MenuId::View => "View",
            MenuId::Help => "Help",
        }
    }
    pub fn index(self) -> usize {
        match self {
            MenuId::Adapter => 0,
            MenuId::Device => 1,
            MenuId::View => 2,
            MenuId::Help => 3,
        }
    }
    pub fn from_index(i: usize) -> MenuId {
        MenuId::all()[i.min(3)]
    }
}

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub action: Action,
    pub enabled: bool,
    pub hint: String,
}

#[derive(Clone, Debug)]
pub enum ConfirmAction {
    Quit,
    RemoveDevice(String),
}

#[derive(Clone, Debug)]
pub struct ConfirmState {
    pub text: String,
    pub action: ConfirmAction,
}

#[derive(Clone, Debug)]
pub struct FilePickerState {
    pub cwd: PathBuf,
    pub entries: Vec<PathBuf>,
    pub selected: HashSet<PathBuf>,
    pub index: usize,
    pub show_hidden: bool,
    pub device: String,
    pub device_name: String,
}

impl FilePickerState {
    pub fn refresh(&mut self) {
        self.entries = crate::backend::demo::demo_files(&self.cwd);
        if !self.show_hidden {
            self.entries.retain(|p| {
                !p.file_name()
                    .map(|n| n.to_string_lossy().starts_with('.'))
                    .unwrap_or(false)
            });
        }
        self.entries.sort_by_key(|p| {
            (
                !p.is_dir(),
                p.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        });
        self.index = self.index.min(self.entries.len().saturating_sub(1));
    }
}

#[derive(Clone, Debug, Default)]
pub struct LayoutInfo {
    pub header: Rect,
    pub menubar: Rect,
    pub devices: Rect,
    pub details: Rect,
    pub status: Rect,
    pub helpbar: Rect,
    pub menu_rects: [Rect; 4],
    pub device_rows: Rect,
    pub device_scroll: usize,
}

// ---------------------------------------------------------------------------
// App

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub keys: KeyMap,
    pub backend: Box<dyn Backend>,

    pub adapters: Vec<Adapter>,
    pub selected_adapter: usize,
    pub devices: Vec<Device>,
    pub selected: usize,
    pub sort: SortKey,
    pub filter: String,
    pub search_active: bool,
    pub show_details: bool,
    pub show_player_bar: bool,

    pub transfers: Vec<Transfer>,
    pub player: Option<PlayerState>,
    pub networks: Vec<Network>,
    pub profiles: Vec<ProfileInfo>,
    pub features: FeatureSet,

    pub status: StatusMessage,
    pub status_at: Instant,
    pub log: Vec<String>,
    pub auth: Option<AuthRequest>,
    pub auth_input: String,

    pub overlays: Vec<Overlay>,
    pub menu_id: MenuId,
    pub menu_items: Vec<MenuItem>,
    pub menu_index: usize,
    pub confirm: Option<ConfirmState>,
    pub file_picker: Option<FilePickerState>,
    pub overlay_index: usize,
    pub overlay_scroll: usize,
    pub palette_query: String,
    pub palette_items: Vec<Action>,

    pub layout: LayoutInfo,
    pub tick: u64,
    pub running: bool,
    pub suspend_requested: bool,
    pub mouse_enabled: bool,
    pub startup_warning: Option<String>,
    pub last_click: Option<(Instant, usize)>,
}

impl App {
    pub fn new(config: Config, backend: Box<dyn Backend>) -> Self {
        let keys = KeyMap::from_config(&config.keybindings);
        let mut theme = Theme::by_name(&config.theme).unwrap_or_else(Theme::midnight);
        config.apply_accent(&mut theme);
        let show_details = true;
        let show_player_bar = true;
        let sort = match config.default_sort.to_ascii_lowercase().as_str() {
            "address" => SortKey::Address,
            "rssi" => SortKey::Rssi,
            "battery" => SortKey::Battery,
            "state" => SortKey::State,
            "kind" => SortKey::Kind,
            _ => SortKey::Name,
        };

        let mut app = Self {
            config,
            theme,
            keys,
            backend,
            adapters: Vec::new(),
            selected_adapter: 0,
            devices: Vec::new(),
            selected: 0,
            sort,
            filter: String::new(),
            search_active: false,
            show_details,
            show_player_bar,
            transfers: Vec::new(),
            player: None,
            networks: Vec::new(),
            profiles: Vec::new(),
            features: FeatureSet::all(),
            status: StatusMessage {
                level: Level::Info,
                text: "Ready".into(),
                busy: false,
            },
            status_at: Instant::now(),
            log: Vec::new(),
            auth: None,
            auth_input: String::new(),
            overlays: Vec::new(),
            menu_id: MenuId::Adapter,
            menu_items: Vec::new(),
            menu_index: 0,
            confirm: None,
            file_picker: None,
            overlay_index: 0,
            overlay_scroll: 0,
            palette_query: String::new(),
            palette_items: Vec::new(),
            layout: LayoutInfo::default(),
            tick: 0,
            running: true,
            suspend_requested: false,
            mouse_enabled: true,
            startup_warning: None,
            last_click: None,
        };

        if !app.keys.conflicts.is_empty() && !app.config.no_warning {
            app.startup_warning = Some(format!(
                "Keybinding conflicts: {}",
                app.keys.conflicts.join("; ")
            ));
        }
        app.log.push(format!(
            "bluetuith-rs {} started ({} backend)",
            env!("CARGO_PKG_VERSION"),
            app.backend.label()
        ));
        app
    }

    // ---- small helpers ----------------------------------------------------

    pub fn current_adapter(&self) -> Option<&Adapter> {
        self.adapters.get(self.selected_adapter)
    }

    pub fn current_adapter_path(&self) -> String {
        self.current_adapter()
            .map(|a| a.path.clone())
            .unwrap_or_default()
    }

    /// Devices on the selected adapter, filtered and sorted.
    pub fn visible_devices(&self) -> Vec<&Device> {
        let ap = self.current_adapter_path();
        let needle = self.filter.trim().to_ascii_lowercase();
        let mut out: Vec<&Device> = self
            .devices
            .iter()
            .filter(|d| ap.is_empty() || d.adapter.is_empty() || d.adapter == ap)
            .filter(|d| {
                needle.is_empty()
                    || d.display_name().to_ascii_lowercase().contains(&needle)
                    || d.address.to_ascii_lowercase().contains(&needle)
                    || d.kind().label().to_ascii_lowercase().contains(&needle)
            })
            .collect();
        match self.sort {
            SortKey::Name => out.sort_by(|a, b| {
                a.display_name()
                    .to_ascii_lowercase()
                    .cmp(&b.display_name().to_ascii_lowercase())
            }),
            SortKey::Address => out.sort_by(|a, b| a.address.cmp(&b.address)),
            SortKey::Rssi => out.sort_by_key(|d| std::cmp::Reverse(d.rssi.unwrap_or(-999))),
            SortKey::Battery => out.sort_by_key(|d| std::cmp::Reverse(d.battery.unwrap_or(0))),
            SortKey::State => out.sort_by(|a, b| {
                b.connected
                    .cmp(&a.connected)
                    .then(b.paired.cmp(&a.paired))
                    .then(a.display_name().cmp(&b.display_name()))
            }),
            SortKey::Kind => out.sort_by(|a, b| {
                a.kind()
                    .label()
                    .cmp(b.kind().label())
                    .then(a.display_name().cmp(&b.display_name()))
            }),
        }
        out
    }

    pub fn selected_device(&self) -> Option<&Device> {
        self.visible_devices().get(self.selected).copied()
    }

    pub fn selected_addr(&self) -> Option<String> {
        self.selected_device().map(|d| d.address.clone())
    }

    pub fn push(&mut self, o: Overlay) {
        if self.overlays.last() == Some(&o) {
            return;
        }
        self.overlays.push(o);
        self.overlay_index = 0;
        self.overlay_scroll = 0;
        self.on_open(o);
    }

    pub fn top_overlay(&self) -> Option<Overlay> {
        self.overlays.last().copied()
    }

    pub fn close_overlay(&mut self) {
        self.overlays.pop();
        self.confirm = None;
    }

    pub fn set_status(&mut self, level: Level, text: impl Into<String>, busy: bool) {
        self.status = StatusMessage {
            level,
            text: text.into(),
            busy,
        };
        self.status_at = Instant::now();
    }

    pub fn dispatch(&mut self, cmd: Command) {
        self.backend.dispatch(cmd);
    }

    pub fn log_line(&mut self, s: impl Into<String>) {
        self.log.push(s.into());
        if self.log.len() > 500 {
            self.log.remove(0);
        }
    }

    fn on_open(&mut self, o: Overlay) {
        match o {
            Overlay::Menu => self.rebuild_menu(),
            Overlay::ThemePicker => {
                self.overlay_index = Theme::name_list()
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(&self.config.theme))
                    .unwrap_or(0);
            }
            Overlay::AdapterChange => {
                self.overlay_index = self.selected_adapter;
            }
            Overlay::CommandPalette => {
                self.palette_query.clear();
                self.rebuild_palette();
            }
            Overlay::FilePicker => {
                let (addr, name) = self
                    .selected_addr()
                    .map(|a| {
                        (
                            a.clone(),
                            self.selected_device()
                                .map(|d| d.display_name())
                                .unwrap_or(a),
                        )
                    })
                    .unwrap_or_else(|| (String::new(), "device".into()));
                let mut fp = FilePickerState {
                    cwd: std::env::var("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from(".")),
                    entries: Vec::new(),
                    selected: HashSet::new(),
                    index: 0,
                    show_hidden: self.config.show_hidden_files,
                    device: addr,
                    device_name: name,
                };
                fp.refresh();
                self.file_picker = Some(fp);
            }
            Overlay::Auth => self.auth_input.clear(),
            _ => {}
        }
    }

    fn rebuild_palette(&mut self) {
        let q = self.palette_query.to_ascii_lowercase();
        self.palette_items = Action::all()
            .into_iter()
            .filter(|a| q.is_empty() || a.name().to_ascii_lowercase().contains(&q))
            .collect();
        self.overlay_index = self
            .overlay_index
            .min(self.palette_items.len().saturating_sub(1));
    }

    // ---- backend events ---------------------------------------------------

    pub fn apply_event(&mut self, ev: Event) {
        match ev {
            Event::Snapshot { adapters, devices } => {
                let prev_sel = self.selected_addr();
                let prev_path = self.current_adapter_path();
                self.adapters = adapters;
                if let Some(i) = self.adapters.iter().position(|a| a.path == prev_path) {
                    self.selected_adapter = i;
                } else {
                    self.selected_adapter = self
                        .selected_adapter
                        .min(self.adapters.len().saturating_sub(1));
                }
                self.devices = devices;
                if let Some(addr) = prev_sel {
                    if let Some(i) = self
                        .visible_devices()
                        .iter()
                        .position(|d| d.address == addr)
                    {
                        self.selected = i;
                    }
                }
                self.selected = self
                    .selected
                    .min(self.visible_devices().len().saturating_sub(1));
            }
            Event::Status(s) => {
                self.log_line(format!("[{:?}] {}", s.level, s.text));
                self.status = s;
                self.status_at = Instant::now();
            }
            Event::Features(f) => self.features = f,
            Event::Transfers(t) => self.transfers = t,
            Event::Player(p) => self.player = p,
            Event::Networks(n) => self.networks = n,
            Event::Profiles(p) => self.profiles = p,
            Event::AuthRequest(r) => {
                self.auth = Some(r);
                self.auth_input.clear();
                self.push(Overlay::Auth);
            }
            Event::Log(s) => self.log_line(s),
        }
    }

    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        let events = self.backend.drain();
        for ev in events {
            self.apply_event(ev);
        }
        if self.status.busy && self.status_at.elapsed().as_secs() > 25 {
            self.status.busy = false;
            self.status.level = Level::Info;
        }
    }

    // ---- actions ----------------------------------------------------------

    pub fn do_action(&mut self, a: Action) {
        use Action::*;
        match a {
            Quit => {
                if self.config.confirm_on_quit {
                    self.confirm = Some(ConfirmState {
                        text: "Quit bluetuith-rs?".into(),
                        action: ConfirmAction::Quit,
                    });
                    self.push(Overlay::Confirm);
                } else {
                    self.running = false;
                }
            }
            Menu => self.open_menu(),
            Help => self.push(Overlay::Help),
            Suspend => self.suspend_requested = true,
            Close => {
                if !self.overlays.is_empty() {
                    self.close_overlay();
                } else if !self.filter.is_empty() {
                    self.filter.clear();
                } else {
                    self.set_status(Level::Info, "Ready", false);
                }
            }
            Cancel => {
                if let Some(addr) = self.selected_addr() {
                    self.dispatch(Command::CancelPairing(addr));
                }
                self.set_status(Level::Info, "Cancelled", false);
            }
            Switch => {
                self.show_details = !self.show_details;
            }
            Power | Discoverable | Pairable => {
                if let Some(ad) = self.current_adapter() {
                    let (property, value, label) = match a {
                        Power => (AdapterProperty::Powered, !ad.powered, "Power"),
                        Discoverable => (
                            AdapterProperty::Discoverable,
                            !ad.discoverable,
                            "Discoverable",
                        ),
                        _ => (AdapterProperty::Pairable, !ad.pairable, "Pairable"),
                    };
                    self.set_status(Level::Busy, format!("Setting {label}…"), true);
                    self.dispatch(Command::SetAdapterProperty { property, value });
                }
            }
            Scan => {
                let on = !self
                    .current_adapter()
                    .map(|a| a.discovering)
                    .unwrap_or(false);
                self.dispatch(Command::SetScan(on));
            }
            ChangeAdapter => self.push(Overlay::AdapterChange),
            ProgressView => self.push(Overlay::Progress),
            PlayerHide => {
                if self.top_overlay() == Some(Overlay::Player) {
                    self.close_overlay();
                }
                self.show_player_bar = false;
            }
            PlayerShow => {
                self.show_player_bar = true;
                self.push(Overlay::Player);
            }
            Connect => {
                if let Some(d) = self.selected_device() {
                    let (addr, connected, name) =
                        (d.address.clone(), d.connected, d.display_name());
                    self.set_status(
                        Level::Busy,
                        format!(
                            "{} {name}…",
                            if connected {
                                "Disconnecting from"
                            } else {
                                "Connecting to"
                            }
                        ),
                        true,
                    );
                    self.dispatch(if connected {
                        Command::DisconnectDevice(addr)
                    } else {
                        Command::ConnectDevice(addr)
                    });
                }
            }
            Pair => {
                if let Some(addr) = self.selected_addr() {
                    self.set_status(Level::Busy, "Pairing…", true);
                    self.dispatch(Command::PairDevice(addr));
                }
            }
            Trust | Block => {
                if let Some(d) = self.selected_device() {
                    let (addr, value) = if a == Trust {
                        (d.address.clone(), !d.trusted)
                    } else {
                        (d.address.clone(), !d.blocked)
                    };
                    self.dispatch(if a == Trust {
                        Command::TrustDevice(addr, value)
                    } else {
                        Command::BlockDevice(addr, value)
                    });
                }
            }
            SendFiles => {
                if !self.features.send_file {
                    self.set_status(
                        Level::Warning,
                        "File transfer is unavailable on this session",
                        false,
                    );
                    return;
                }
                if self.selected_device().is_none() {
                    self.set_status(Level::Warning, "Select a device first", false);
                    return;
                }
                self.push(Overlay::FilePicker);
            }
            Network => self.push(Overlay::Network),
            AudioProfiles => self.push(Overlay::Profiles),
            Info => self.push(Overlay::DeviceInfo),
            Remove => {
                if let Some(d) = self.selected_device() {
                    let (addr, name) = (d.address.clone(), d.display_name());
                    self.confirm = Some(ConfirmState {
                        text: format!("Remove {name}?"),
                        action: ConfirmAction::RemoveDevice(addr),
                    });
                    self.push(Overlay::Confirm);
                }
            }
            Search => {
                self.search_active = true;
                self.set_status(
                    Level::Info,
                    "Search: type to filter, Enter to keep, Esc to clear",
                    false,
                );
            }
            CycleSort => {
                let all = [
                    SortKey::Name,
                    SortKey::Address,
                    SortKey::Rssi,
                    SortKey::Battery,
                    SortKey::State,
                    SortKey::Kind,
                ];
                let i = all.iter().position(|s| *s == self.sort).unwrap_or(0);
                self.sort = all[(i + 1) % all.len()];
                self.selected = 0;
                self.set_status(Level::Info, format!("Sort by {:?}", self.sort), false);
            }
            ThemePicker => self.push(Overlay::ThemePicker),
            ToggleDetails => self.show_details = !self.show_details,
            ShowLog => self.push(Overlay::Log),
            Refresh => {
                self.dispatch(Command::Refresh);
                self.set_status(Level::Info, "Refreshing…", false);
            }
            CommandPalette => self.push(Overlay::CommandPalette),
            About => self.push(Overlay::About),
            _ => {}
        }
    }

    // ---- menus ------------------------------------------------------------

    pub fn open_menu(&mut self) {
        self.menu_id = MenuId::Adapter;
        self.menu_index = 0;
        self.rebuild_menu();
        self.push(Overlay::Menu);
    }

    pub fn set_menu(&mut self, id: MenuId) {
        self.menu_id = id;
        self.menu_index = 0;
        self.rebuild_menu();
    }

    fn item(&self, label: impl Into<String>, action: Action, enabled: bool) -> MenuItem {
        MenuItem {
            label: label.into(),
            action,
            enabled,
            hint: self.keys.label(action),
        }
    }

    pub fn rebuild_menu(&mut self) {
        let d = self.selected_device();
        let ad = self.current_adapter();
        let mut items = Vec::new();
        match self.menu_id {
            MenuId::Adapter => {
                let p = ad.map(|a| a.powered).unwrap_or(false);
                let disc = ad.map(|a| a.discoverable).unwrap_or(false);
                let pair = ad.map(|a| a.pairable).unwrap_or(false);
                let scan = ad.map(|a| a.discovering).unwrap_or(false);
                items.push(self.item(
                    format!("Power: {}", if p { "On" } else { "Off" }),
                    Action::Power,
                    true,
                ));
                items.push(self.item(
                    format!("Discoverable: {}", if disc { "On" } else { "Off" }),
                    Action::Discoverable,
                    p,
                ));
                items.push(self.item(
                    format!("Pairable: {}", if pair { "On" } else { "Off" }),
                    Action::Pairable,
                    p,
                ));
                items.push(self.item(
                    if scan { "Stop Scan" } else { "Start Scan" },
                    Action::Scan,
                    p,
                ));
                items.push(self.item("Change Adapter…", Action::ChangeAdapter, true));
                items.push(self.item("View Downloads…", Action::ProgressView, true));
                items.push(self.item(
                    "Hide Media Player",
                    Action::PlayerHide,
                    self.show_player_bar,
                ));
                items.push(self.item("Quit", Action::Quit, true));
            }
            MenuId::Device => {
                let connected = d.map(|x| x.connected).unwrap_or(false);
                let paired = d.map(|x| x.paired).unwrap_or(false);
                let trusted = d.map(|x| x.trusted).unwrap_or(false);
                let blocked = d.map(|x| x.blocked).unwrap_or(false);
                let has_obex = d
                    .map(|x| x.has_service("1105") || x.has_service("1106"))
                    .unwrap_or(false);
                let has_net = d
                    .map(|x| {
                        x.has_service("1116") || x.has_service("1115") || x.has_service("1103")
                    })
                    .unwrap_or(false);
                let has_audio = d
                    .map(|x| x.has_service("110a") || x.has_service("110b"))
                    .unwrap_or(false);
                let has_avrcp = d
                    .map(|x| x.has_service("110c") || x.has_service("110e"))
                    .unwrap_or(false);
                let any = d.is_some();
                items.push(self.item(
                    if connected { "Disconnect" } else { "Connect" },
                    Action::Connect,
                    any,
                ));
                items.push(self.item("Pair", Action::Pair, any && !paired));
                items.push(self.item(
                    if trusted { "Untrust" } else { "Trust" },
                    Action::Trust,
                    any,
                ));
                items.push(self.item(
                    if blocked { "Unblock" } else { "Block" },
                    Action::Block,
                    any,
                ));
                items.push(self.item(
                    "Send Files…",
                    Action::SendFiles,
                    any && self.features.send_file && has_obex,
                ));
                items.push(self.item(
                    "Network Options…",
                    Action::Network,
                    any && self.features.network && has_net,
                ));
                items.push(self.item("Audio Profiles…", Action::AudioProfiles, any && has_audio));
                items.push(self.item("Show Media Player…", Action::PlayerShow, any && has_avrcp));
                items.push(self.item("Device Info…", Action::Info, any));
                items.push(self.item("Remove", Action::Remove, any));
            }
            MenuId::View => {
                items.push(self.item("Search / Filter", Action::Search, true));
                items.push(self.item("Cycle Sort", Action::CycleSort, true));
                items.push(self.item(
                    if self.show_details {
                        "Hide Details"
                    } else {
                        "Show Details"
                    },
                    Action::ToggleDetails,
                    true,
                ));
                items.push(self.item("Theme Picker…", Action::ThemePicker, true));
                items.push(self.item("Event Log…", Action::ShowLog, true));
                items.push(self.item("Media Player…", Action::PlayerShow, true));
                items.push(self.item("Transfers…", Action::ProgressView, true));
                items.push(self.item("Refresh", Action::Refresh, true));
                items.push(self.item("Command Palette…", Action::CommandPalette, true));
            }
            MenuId::Help => {
                items.push(self.item("Keybindings & Help", Action::Help, true));
                items.push(self.item("About", Action::About, true));
            }
        }
        self.menu_items = items;
        self.menu_index = self.menu_index.min(self.menu_items.len().saturating_sub(1));
    }

    pub fn menu_select(&mut self) {
        let Some(item) = self.menu_items.get(self.menu_index).cloned() else {
            return;
        };
        if !item.enabled {
            self.set_status(Level::Warning, "That action is unavailable here", false);
            return;
        }
        self.close_overlay();
        self.do_action(item.action);
    }

    // ---- key handling -----------------------------------------------------

    pub fn on_key(&mut self, ev: KeyEvent) {
        if self.search_active {
            self.search_key(ev);
            return;
        }
        if self.top_overlay() == Some(Overlay::Auth) {
            self.auth_key(ev);
            return;
        }
        if self.top_overlay() == Some(Overlay::Confirm) {
            self.confirm_key(ev);
            return;
        }
        // Quit is global, except in text-entry modes handled above.
        if self.keys.is(&ev, Action::Quit) {
            self.do_action(Action::Quit);
            return;
        }
        if let Some(o) = self.top_overlay() {
            if self.overlay_key(o, ev) {
                return;
            }
        }
        // Global keys.
        if self.keys.is(&ev, Action::Help) {
            self.push(Overlay::Help);
            return;
        }
        if self.keys.is(&ev, Action::Menu) {
            self.open_menu();
            return;
        }
        if self.keys.is(&ev, Action::Close) {
            self.do_action(Action::Close);
            return;
        }
        if self.keys.is(&ev, Action::Suspend) {
            self.do_action(Action::Suspend);
            return;
        }
        if self.keys.is(&ev, Action::Cancel) {
            self.do_action(Action::Cancel);
            return;
        }
        if self.keys.is(&ev, Action::Switch) {
            self.do_action(Action::Switch);
            return;
        }
        if self.keys.is(&ev, Action::Search) {
            self.do_action(Action::Search);
            return;
        }
        if self.keys.is(&ev, Action::CycleSort) {
            self.do_action(Action::CycleSort);
            return;
        }
        if self.keys.is(&ev, Action::ThemePicker) {
            self.do_action(Action::ThemePicker);
            return;
        }
        if self.keys.is(&ev, Action::ToggleDetails) {
            self.do_action(Action::ToggleDetails);
            return;
        }
        if self.keys.is(&ev, Action::ShowLog) {
            self.do_action(Action::ShowLog);
            return;
        }
        if self.keys.is(&ev, Action::Refresh) {
            self.do_action(Action::Refresh);
            return;
        }
        if self.keys.is(&ev, Action::CommandPalette) {
            self.do_action(Action::CommandPalette);
            return;
        }

        self.main_key(ev);
    }

    fn search_key(&mut self, ev: KeyEvent) {
        match ev.code {
            KeyCode::Esc => {
                self.search_active = false;
                self.filter.clear();
                self.set_status(Level::Info, "Filter cleared", false);
            }
            KeyCode::Enter => {
                self.search_active = false;
                self.set_status(Level::Info, format!("Filter: {}", self.filter), false);
            }
            KeyCode::Backspace => {
                self.filter.pop();
            }
            KeyCode::Char(c) => self.filter.push(c),
            _ => {}
        }
        self.selected = 0;
    }

    fn confirm_key(&mut self, ev: KeyEvent) {
        let yes = matches!(ev.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter);
        let no = matches!(ev.code, KeyCode::Char('n' | 'N') | KeyCode::Esc);
        if !yes && !no {
            return;
        }
        let action = self.confirm.take().map(|c| c.action);
        self.close_overlay();
        if yes {
            match action {
                Some(ConfirmAction::Quit) => {
                    self.dispatch(Command::SetScan(false));
                    self.running = false;
                }
                Some(ConfirmAction::RemoveDevice(addr)) => {
                    self.dispatch(Command::RemoveDevice(addr));
                }
                None => {}
            }
        } else {
            self.set_status(Level::Info, "Cancelled", false);
        }
    }

    fn auth_key(&mut self, ev: KeyEvent) {
        let Some(req) = self.auth.clone() else { return };
        let needs_text = matches!(req.kind, AuthKind::PinCode | AuthKind::Passkey(_));
        match ev.code {
            KeyCode::Esc => {
                self.dispatch(Command::RespondAuth {
                    id: req.id,
                    response: AuthResponse::Reject,
                });
                self.auth = None;
                self.close_overlay();
            }
            KeyCode::Enter => {
                let response = if needs_text {
                    match req.kind {
                        AuthKind::PinCode => AuthResponse::PinCode(self.auth_input.clone()),
                        AuthKind::Passkey(_) => self
                            .auth_input
                            .parse()
                            .map(AuthResponse::Passkey)
                            .unwrap_or(AuthResponse::Reject),
                        _ => AuthResponse::Accept,
                    }
                } else {
                    AuthResponse::Accept
                };
                self.dispatch(Command::RespondAuth {
                    id: req.id,
                    response,
                });
                self.auth = None;
                self.close_overlay();
            }
            KeyCode::Backspace => {
                self.auth_input.pop();
            }
            KeyCode::Char(c)
                if needs_text && (c.is_ascii_digit() || matches!(req.kind, AuthKind::PinCode)) =>
            {
                self.auth_input.push(c);
            }
            _ => {}
        }
    }

    /// Returns true when the key was consumed by the overlay.
    fn overlay_key(&mut self, o: Overlay, ev: KeyEvent) -> bool {
        let n = |app: &App, o: Overlay| -> usize {
            match o {
                Overlay::AdapterChange => app.adapters.len(),
                Overlay::ThemePicker => Theme::name_list().len(),
                Overlay::Profiles => app.profiles.len(),
                Overlay::Network => app.networks.len().max(3),
                Overlay::Progress => app.transfers.len(),
                Overlay::CommandPalette => app.palette_items.len(),
                Overlay::FilePicker => app
                    .file_picker
                    .as_ref()
                    .map(|f| f.entries.len())
                    .unwrap_or(0),
                Overlay::Help | Overlay::Log | Overlay::DeviceInfo | Overlay::About => 400,
                _ => 0,
            }
        };

        if o == Overlay::Menu {
            match ev.code {
                KeyCode::Up => {
                    self.menu_index = self.menu_index.saturating_sub(1);
                    return true;
                }
                KeyCode::Down => {
                    self.menu_index =
                        (self.menu_index + 1).min(self.menu_items.len().saturating_sub(1));
                    return true;
                }
                KeyCode::Right | KeyCode::Tab => {
                    self.set_menu(MenuId::from_index((self.menu_id.index() + 1) % 4));
                    return true;
                }
                KeyCode::Left | KeyCode::BackTab => {
                    self.set_menu(MenuId::from_index((self.menu_id.index() + 3) % 4));
                    return true;
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.menu_select();
                    return true;
                }
                _ => {}
            }
            // Shortcut letters inside a menu (bluetuith behaviour).
            if let KeyCode::Char(c) = ev.code {
                if let Some(i) = self
                    .menu_items
                    .iter()
                    .position(|it| self.keys.get(it.action).code == KeyCode::Char(c) && it.enabled)
                {
                    self.menu_index = i;
                    self.menu_select();
                    return true;
                }
            }
            return false;
        }

        // Generic navigation for list-like overlays.
        let count = n(self, o);
        let up = matches!(ev.code, KeyCode::Up);
        let down = matches!(ev.code, KeyCode::Down);
        if up || down {
            if matches!(
                o,
                Overlay::Help | Overlay::Log | Overlay::DeviceInfo | Overlay::About
            ) {
                self.overlay_scroll = if up {
                    self.overlay_scroll.saturating_sub(1)
                } else {
                    self.overlay_scroll.saturating_add(1)
                };
            } else if count > 0 {
                self.overlay_index = if up {
                    self.overlay_index.saturating_sub(1)
                } else {
                    (self.overlay_index + 1).min(count - 1)
                };
            }
            return true;
        }
        if matches!(ev.code, KeyCode::PageUp) {
            self.overlay_scroll = self.overlay_scroll.saturating_sub(10);
            self.overlay_index = self.overlay_index.saturating_sub(10);
            return true;
        }
        if matches!(ev.code, KeyCode::PageDown) {
            self.overlay_scroll = self.overlay_scroll.saturating_add(10);
            self.overlay_index = (self.overlay_index + 10).min(count.saturating_sub(1));
            return true;
        }

        match o {
            Overlay::AdapterChange => {
                if matches!(ev.code, KeyCode::Enter) {
                    if let Some(a) = self.adapters.get(self.overlay_index).cloned() {
                        self.dispatch(Command::SelectAdapter(a.path));
                        self.selected_adapter = self.overlay_index;
                        self.close_overlay();
                    }
                    return true;
                }
            }
            Overlay::ThemePicker => {
                if matches!(ev.code, KeyCode::Enter) {
                    let names = Theme::name_list();
                    if let Some(name) = names.get(self.overlay_index) {
                        self.config.theme = name.to_string();
                        let mut t = Theme::by_name(name).unwrap_or_else(Theme::midnight);
                        self.config.apply_accent(&mut t);
                        self.theme = t;
                        self.set_status(Level::Success, format!("Theme: {name}"), false);
                    }
                    self.close_overlay();
                    return true;
                }
            }
            Overlay::Profiles => {
                if matches!(ev.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    if let Some(p) = self.profiles.get(self.overlay_index).cloned() {
                        self.dispatch(Command::SetAudioProfile {
                            device: p.device,
                            profile: p.name,
                        });
                    }
                    return true;
                }
            }
            Overlay::Network => {
                if matches!(ev.code, KeyCode::Enter | KeyCode::Char('c')) {
                    if let Some(addr) = self.selected_addr() {
                        let roles = [NetworkRole::Panu, NetworkRole::Nap, NetworkRole::Dun];
                        let role = roles
                            .get(self.overlay_index)
                            .copied()
                            .unwrap_or(NetworkRole::Panu);
                        self.dispatch(Command::ConnectNetwork { device: addr, role });
                    }
                    return true;
                }
                if matches!(ev.code, KeyCode::Char('d')) {
                    if let Some(nw) = self.networks.get(self.overlay_index).cloned() {
                        self.dispatch(Command::DisconnectNetwork(nw.id));
                    }
                    return true;
                }
            }
            Overlay::Progress => {
                let Some(t) = self.transfers.get(self.overlay_index).cloned() else {
                    return false;
                };
                if self.keys.is(&ev, Action::TransferSuspend) {
                    self.dispatch(Command::SuspendTransfer(t.id));
                    return true;
                }
                if self.keys.is(&ev, Action::TransferResume) {
                    self.dispatch(Command::ResumeTransfer(t.id));
                    return true;
                }
                if self.keys.is(&ev, Action::TransferCancel) {
                    self.dispatch(Command::CancelTransfer(t.id));
                    return true;
                }
                if self.keys.is(&ev, Action::ProgressView) {
                    self.close_overlay();
                    return true;
                }
            }
            Overlay::Player => {
                let map = [
                    (Action::PlayerTogglePlay, PlayerCommand::TogglePlay),
                    (Action::PlayerNext, PlayerCommand::Next),
                    (Action::PlayerPrevious, PlayerCommand::Previous),
                    (Action::PlayerSeekForward, PlayerCommand::FastForward),
                    (Action::PlayerSeekBackward, PlayerCommand::Rewind),
                    (Action::PlayerStop, PlayerCommand::Stop),
                    (Action::PlayerVolumeUp, PlayerCommand::VolumeUp),
                    (Action::PlayerVolumeDown, PlayerCommand::VolumeDown),
                ];
                for (a, c) in map {
                    if self.keys.is(&ev, a) {
                        self.dispatch(Command::PlayerCommand(c));
                        return true;
                    }
                }
                if self.keys.is(&ev, Action::PlayerHide) {
                    self.close_overlay();
                    return true;
                }
            }
            Overlay::FilePicker => {
                return self.file_picker_key(ev);
            }
            Overlay::CommandPalette => match ev.code {
                KeyCode::Enter => {
                    if let Some(a) = self.palette_items.get(self.overlay_index).copied() {
                        self.close_overlay();
                        self.do_action(a);
                    }
                    return true;
                }
                KeyCode::Backspace => {
                    self.palette_query.pop();
                    self.rebuild_palette();
                    return true;
                }
                KeyCode::Char(c) => {
                    self.palette_query.push(c);
                    self.rebuild_palette();
                    return true;
                }
                _ => {}
            },
            Overlay::Help | Overlay::Log | Overlay::DeviceInfo | Overlay::About => {
                if matches!(ev.code, KeyCode::Enter | KeyCode::Char('q')) {
                    self.close_overlay();
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn file_picker_key(&mut self, ev: KeyEvent) -> bool {
        let Some(mut fp) = self.file_picker.clone() else {
            return false;
        };
        let entry = fp.entries.get(fp.index).cloned();
        match ev.code {
            KeyCode::Up => {
                fp.index = fp.index.saturating_sub(1);
            }
            KeyCode::Down => {
                fp.index = (fp.index + 1).min(fp.entries.len().saturating_sub(1));
            }
            KeyCode::Right => {
                if let Some(e) = &entry {
                    if e.is_dir() {
                        fp.cwd = e.clone();
                        fp.index = 0;
                        fp.refresh();
                    }
                }
            }
            KeyCode::Left => {
                if let Some(parent) = fp.cwd.parent().map(|p| p.to_path_buf()) {
                    fp.cwd = parent;
                    fp.index = 0;
                    fp.refresh();
                }
            }
            KeyCode::Char(' ') => {
                if let Some(e) = &entry {
                    if !fp.selected.insert(e.clone()) {
                        fp.selected.remove(e);
                    }
                }
            }
            KeyCode::Char('a') => {
                for e in &fp.entries {
                    if !fp.selected.insert(e.clone()) {
                        fp.selected.remove(e);
                    }
                }
            }
            KeyCode::Char('A') => {
                fp.selected = fp.entries.iter().cloned().collect();
            }
            KeyCode::Char('h')
                if ev
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                fp.show_hidden = !fp.show_hidden;
                fp.refresh();
            }
            KeyCode::Char('r')
                if ev
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                fp.refresh();
            }
            KeyCode::Char('s')
                if ev
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                let files: Vec<PathBuf> = fp.selected.iter().cloned().collect();
                if files.is_empty() {
                    self.set_status(Level::Warning, "No files selected", false);
                } else {
                    self.dispatch(Command::SendFiles {
                        device: fp.device.clone(),
                        files: files.clone(),
                    });
                    self.set_status(
                        Level::Busy,
                        format!("Sending {} file(s)…", files.len()),
                        true,
                    );
                    self.close_overlay();
                    self.file_picker = None;
                    return true;
                }
            }
            _ => return false,
        }
        self.file_picker = Some(fp);
        true
    }

    fn main_key(&mut self, ev: KeyEvent) {
        // Navigation
        let visible = self.visible_devices().len();
        if self.keys.is(&ev, Action::NavigateUp) {
            self.selected = self.selected.saturating_sub(1);
            return;
        }
        if self.keys.is(&ev, Action::NavigateDown) {
            self.selected = (self.selected + 1).min(visible.saturating_sub(1));
            return;
        }
        if self.keys.is(&ev, Action::NavigateTop) {
            self.selected = 0;
            return;
        }
        if self.keys.is(&ev, Action::NavigateBottom) {
            self.selected = visible.saturating_sub(1);
            return;
        }
        if matches!(ev.code, KeyCode::Home) {
            self.selected = 0;
            return;
        }
        if matches!(ev.code, KeyCode::End) {
            self.selected = visible.saturating_sub(1);
            return;
        }
        if self.keys.is(&ev, Action::Select) {
            self.do_action(Action::Connect);
            return;
        }

        // All device / adapter actions.
        for a in Action::all() {
            if a.context() == Context::App
                || a.context() == Context::Extra
                || a.context() == Context::Files
                || a.context() == Context::Progress
                || a.context() == Context::Player
            {
                continue;
            }
            if self.keys.is(&ev, a) {
                self.do_action(a);
                return;
            }
        }
    }

    // ---- mouse ------------------------------------------------------------

    pub fn on_mouse(&mut self, ev: MouseEvent) {
        let (x, y) = (ev.column, ev.row);
        match ev.kind {
            MouseEventKind::ScrollUp => {
                self.selected = self.selected.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                self.selected =
                    (self.selected + 1).min(self.visible_devices().len().saturating_sub(1));
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Menu bar
                for (i, r) in self.layout.menu_rects.iter().enumerate() {
                    if x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height {
                        self.menu_id = MenuId::from_index(i);
                        self.menu_index = 0;
                        self.rebuild_menu();
                        self.push(Overlay::Menu);
                        return;
                    }
                }
                // Device table rows
                let r = self.layout.device_rows;
                if x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height {
                    let idx = self.layout.device_scroll + (y - r.y) as usize;
                    if idx < self.visible_devices().len() {
                        let now = Instant::now();
                        let double = self
                            .last_click
                            .map(|(t, i)| i == idx && t.elapsed().as_millis() < 400)
                            .unwrap_or(false);
                        self.selected = idx;
                        self.last_click = Some((now, idx));
                        if double {
                            self.do_action(Action::Connect);
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                let r = self.layout.device_rows;
                if x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height {
                    let idx = self.layout.device_scroll + (y - r.y) as usize;
                    if idx < self.visible_devices().len() {
                        self.selected = idx;
                        self.menu_id = MenuId::Device;
                        self.menu_index = 0;
                        self.rebuild_menu();
                        self.push(Overlay::Menu);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn status_icon(&self) -> &'static str {
        if self.status.busy {
            return util::spinner(self.tick);
        }
        match self.status.level {
            Level::Info => "ℹ",
            Level::Success => "✔",
            Level::Warning => "⚠",
            Level::Error => "✖",
            Level::Busy => util::spinner(self.tick),
        }
    }

    pub fn status_color(&self) -> ratatui::style::Color {
        match self.status.level {
            Level::Info | Level::Busy => self.theme.info,
            Level::Success => self.theme.success,
            Level::Warning => self.theme.warning,
            Level::Error => self.theme.error,
        }
    }
}
