//! Application state and input handling. No rendering happens here: `ui`
//! reads `&App`, and every mutation goes through a method on `App`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::backend::{AdapterProperty, Backend, Command, Event};
use crate::config::Config;
use crate::i18n::{fill, Lang, Tr};
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
    pub fn title(self, lang: Lang) -> &'static str {
        let tr = lang.tr();
        match self {
            Overlay::Menu => tr.menu,
            Overlay::Help => tr.keybindings_help,
            Overlay::Confirm => tr.confirm,
            Overlay::AdapterChange => tr.select_adapter,
            Overlay::DeviceInfo => tr.device_info,
            Overlay::FilePicker => tr.send_files,
            Overlay::Progress => tr.transfers,
            Overlay::Player => tr.media_player,
            Overlay::Network => tr.network,
            Overlay::Profiles => tr.audio_profiles,
            Overlay::Auth => tr.pairing_request,
            Overlay::ThemePicker => tr.theme,
            Overlay::Log => tr.event_log,
            Overlay::CommandPalette => tr.command_palette,
            Overlay::About => tr.about,
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
    pub fn label(self, lang: Lang) -> &'static str {
        let tr = lang.tr();
        match self {
            MenuId::Adapter => tr.m_adapter,
            MenuId::Device => tr.m_device,
            MenuId::View => tr.m_view,
            MenuId::Help => tr.m_help,
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
    /// Clickable regions in the bottom hint strip, paired with their action.
    pub help_rects: Vec<(Rect, Action)>,
    /// Outer rect of the topmost overlay (for click-outside-to-close).
    pub overlay_rect: Option<Rect>,
    /// Clickable rows of the topmost overlay, paired with their item index.
    pub overlay_items: Vec<(Rect, usize)>,
}

// ---------------------------------------------------------------------------
// App

/// The active network connection for a role (rows are roles, not ids).
fn network_id_for_role(networks: &[Network], role: NetworkRole) -> Option<usize> {
    networks
        .iter()
        .find(|n| n.role == role && n.connected)
        .map(|n| n.id)
}

/// Actions that must never appear as a command in the palette: pure cursor
/// movement has no effect there, and local dialogs/toggles are not commands.
/// `Suspend` is especially important to exclude — running it raises SIGTSTP
/// and would freeze the whole TUI from a menu that looks harmless.
fn is_palette_excluded(a: Action) -> bool {
    matches!(
        a,
        Action::Select
            | Action::NavigateUp
            | Action::NavigateDown
            | Action::NavigateTop
            | Action::NavigateBottom
            | Action::Close
            | Action::Cancel
            | Action::Switch
            | Action::Suspend
    )
}

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub keys: KeyMap,
    pub backend: Box<dyn Backend>,
    pub lang: Lang,

    pub adapters: Vec<Adapter>,
    pub selected_adapter: usize,
    /// Whether the startup adapter preference has been applied to the UI yet.
    pub adapter_initialized: bool,
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
    /// 0 = category list, 1 = actions inside the chosen category.
    pub palette_stage: u8,
    pub palette_category: Option<Context>,
    pub palette_categories: Vec<Context>,

    pub layout: LayoutInfo,
    pub tick: u64,
    pub running: bool,
    pub suspend_requested: bool,
    pub mouse_enabled: bool,
    pub startup_warning: Option<String>,
    pub last_click: Option<(Instant, usize)>,
    pub last_overlay_click: Option<(Instant, usize)>,
}

impl App {
    pub fn new(config: Config, backend: Box<dyn Backend>) -> Self {
        let keys = KeyMap::from_config(&config.keybindings);
        let mut theme = Theme::by_name(&config.theme).unwrap_or_else(Theme::midnight);
        config.apply_accent(&mut theme);
        let lang = Lang::parse(&config.language);
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
            lang,
            adapters: Vec::new(),
            selected_adapter: 0,
            adapter_initialized: false,
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
                text: lang.tr().msg_ready.into(),
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
            palette_stage: 0,
            palette_category: None,
            palette_categories: Vec::new(),
            layout: LayoutInfo::default(),
            tick: 0,
            running: true,
            suspend_requested: false,
            mouse_enabled: true,
            startup_warning: None,
            last_click: None,
            last_overlay_click: None,
        };

        let mut warnings = Vec::new();
        if !app.keys.conflicts.is_empty() && !app.config.no_warning {
            warnings.push(fill(
                app.tr().msg_key_conflicts,
                app.keys.conflicts.join("; "),
            ));
        }
        // These two options are accepted for config compatibility but do not
        // drive anything yet; say so instead of silently ignoring them.
        if !app.config.gsm_apn.is_empty() || app.config.gsm_number != "*99#" {
            warnings.push(app.tr().msg_gsm_reserved.to_string());
        }
        if !warnings.is_empty() {
            app.startup_warning = Some(warnings.join("; "));
        }
        if app.config.disable_obex_services {
            app.features.obex = false;
            app.features.send_file = false;
            app.features.receive_file = false;
        }
        app.log.push(format!(
            "bluetuith-rs {} started ({} backend)",
            env!("CARGO_PKG_VERSION"),
            app.backend.label()
        ));
        app
    }

    // ---- small helpers ----------------------------------------------------

    /// The active language's string table.
    pub fn tr(&self) -> Tr {
        self.lang.tr()
    }

    /// BlueZ has no API to switch A2DP/HFP profiles; only the simulator can.
    pub fn audio_profiles_writable(&self) -> bool {
        self.backend.is_demo()
    }

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
                    || d.kind()
                        .label(self.lang)
                        .to_ascii_lowercase()
                        .contains(&needle)
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
                    .label(self.lang)
                    .cmp(b.kind().label(self.lang))
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
                self.palette_stage = 0;
                self.palette_category = None;
                self.overlay_index = 0;
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
        let q = self.palette_query.trim().to_ascii_lowercase();
        let all_ctx = [
            Context::App,
            Context::Device,
            Context::Files,
            Context::Progress,
            Context::Player,
            Context::Extra,
        ];
        if self.palette_stage == 0 {
            // Level 1: categories that actually contain actions.
            self.palette_categories = all_ctx
                .into_iter()
                .filter(|c| Action::all().into_iter().any(|a| a.context() == *c))
                .filter(|c| {
                    q.is_empty() || c.label_for(self.lang).to_ascii_lowercase().contains(&q)
                })
                .collect();
            self.overlay_index = self
                .overlay_index
                .min(self.palette_categories.len().saturating_sub(1));
        } else {
            // Level 2: actions in the chosen category.
            let cat = self.palette_category;
            self.palette_items = Action::all()
                .into_iter()
                // Pure navigation/selection keys and local toggles are not
                // "commands"; showing them would give dead or harmful entries.
                .filter(|a| !is_palette_excluded(*a))
                .filter(|a| cat.is_none_or(|c| a.context() == c))
                .filter(|a| {
                    q.is_empty()
                        || a.name().to_ascii_lowercase().contains(&q)
                        || a.name_for(self.lang).to_ascii_lowercase().contains(&q)
                })
                .collect();
            self.overlay_index = self
                .overlay_index
                .min(self.palette_items.len().saturating_sub(1));
        }
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
                // Honour the configured adapter on the first snapshot so the
                // UI selection matches the adapter the backend commands act on.
                // Later snapshots must not undo a user's manual choice.
                if !self.adapter_initialized {
                    self.adapter_initialized = true;
                    let wanted = self.config.adapter.trim();
                    if !wanted.is_empty() {
                        if let Some(i) = self.adapters.iter().position(|a| {
                            a.path == wanted
                                || a.id == wanted
                                || a.address.eq_ignore_ascii_case(wanted)
                        }) {
                            self.selected_adapter = i;
                        }
                    }
                }
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
                let mut s = s;
                s.text = crate::i18n::translate_status(&s.text, self.lang);
                self.log_line(format!("[{:?}] {}", s.level, s.text));
                self.status = s;
                self.status_at = Instant::now();
            }
            Event::Features(f) => {
                self.features = f;
                // `--disable-obex-services` must work for every backend, and
                // must survive later feature refreshes from the worker thread.
                if self.config.disable_obex_services {
                    self.features.obex = false;
                    self.features.send_file = false;
                    self.features.receive_file = false;
                }
            }
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
                        text: self.tr().msg_quit_confirm.into(),
                        action: ConfirmAction::Quit,
                    });
                    self.push(Overlay::Confirm);
                } else {
                    self.running = false;
                }
            }
            Menu => self.open_menu(),
            MenuAdapter => self.open_menu_at(MenuId::Adapter),
            MenuDevice => self.open_menu_at(MenuId::Device),
            MenuView => self.open_menu_at(MenuId::View),
            MenuHelp => self.open_menu_at(MenuId::Help),
            Help => self.push(Overlay::Help),
            Suspend => self.suspend_requested = true,
            Close => {
                if !self.overlays.is_empty() {
                    self.close_overlay();
                } else if !self.filter.is_empty() {
                    self.filter.clear();
                } else {
                    self.set_status(Level::Info, self.tr().msg_ready, false);
                }
            }
            Cancel => {
                // Only a live pairing prompt needs a D-Bus CancelPairing;
                // otherwise Ctrl+x is just a local "clear/escape" so it does
                // not produce a spurious BlueZ error.
                if self.auth.is_some() {
                    if let Some(addr) = self.selected_addr() {
                        self.dispatch(Command::CancelPairing(addr));
                    }
                }
                self.filter.clear();
                self.set_status(Level::Info, self.tr().msg_cancelled, false);
            }
            Switch => {
                self.show_details = !self.show_details;
            }
            Power | Discoverable | Pairable => {
                if let Some(ad) = self.current_adapter() {
                    let (property, value, label) = match a {
                        Power => (AdapterProperty::Powered, !ad.powered, self.tr().power),
                        Discoverable => (
                            AdapterProperty::Discoverable,
                            !ad.discoverable,
                            self.tr().discoverable,
                        ),
                        _ => (AdapterProperty::Pairable, !ad.pairable, self.tr().pairable),
                    };
                    self.set_status(Level::Busy, fill(self.tr().msg_setting, label), true);
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
                self.dispatch(Command::HidePlayer);
            }
            PlayerShow => {
                if self.player.is_none() {
                    self.set_status(Level::Warning, self.tr().p_no_player, false);
                    return;
                }
                self.show_player_bar = true;
                if let Some(addr) = self.selected_addr() {
                    self.dispatch(Command::ShowPlayer(addr));
                }
                self.push(Overlay::Player);
            }
            Connect => {
                if let Some(d) = self.selected_device() {
                    let (addr, connected, name) =
                        (d.address.clone(), d.connected, d.display_name());
                    self.set_status(
                        Level::Busy,
                        fill(
                            if connected {
                                self.tr().msg_disconnecting
                            } else {
                                self.tr().msg_connecting
                            },
                            name,
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
                    self.set_status(Level::Busy, self.tr().msg_pairing, true);
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
                    self.set_status(Level::Warning, self.tr().msg_no_send, false);
                    return;
                }
                if self.selected_device().is_none() {
                    self.set_status(Level::Warning, self.tr().msg_select_device, false);
                    return;
                }
                self.push(Overlay::FilePicker);
            }
            Network => {
                if !self.features.network {
                    self.set_status(Level::Warning, self.tr().msg_unavailable, false);
                    return;
                }
                if let Some(addr) = self.selected_addr() {
                    self.dispatch(Command::RefreshNetworks(addr));
                }
                self.push(Overlay::Network);
            }
            AudioProfiles => {
                if !self.features.audio_profiles {
                    self.set_status(Level::Warning, self.tr().msg_unavailable, false);
                    return;
                }
                if let Some(addr) = self.selected_addr() {
                    self.dispatch(Command::RefreshProfiles(addr));
                }
                self.push(Overlay::Profiles);
            }
            Info => self.push(Overlay::DeviceInfo),
            Remove => {
                if let Some(d) = self.selected_device() {
                    let (addr, name) = (d.address.clone(), d.display_name());
                    self.confirm = Some(ConfirmState {
                        text: fill(self.tr().msg_remove_confirm, name),
                        action: ConfirmAction::RemoveDevice(addr),
                    });
                    self.push(Overlay::Confirm);
                }
            }
            Search => {
                self.search_active = true;
                self.set_status(Level::Info, self.tr().msg_search_hint, false);
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
                self.set_status(
                    Level::Info,
                    format!("{}: {}", self.tr().h_sort, self.sort.label(self.lang)),
                    false,
                );
            }
            ThemePicker => self.push(Overlay::ThemePicker),
            ToggleDetails => self.show_details = !self.show_details,
            ShowLog => self.push(Overlay::Log),
            Refresh => {
                self.dispatch(Command::Refresh);
                self.set_status(Level::Info, self.tr().msg_refreshing, false);
            }
            CommandPalette => self.push(Overlay::CommandPalette),
            About => self.push(Overlay::About),
            Language => {
                self.lang = match self.lang {
                    Lang::En => Lang::Zh,
                    Lang::Zh => Lang::En,
                };
                // Keep the in-memory config consistent with the live UI.
                self.config.language = self.lang.code().to_string();
                // Menu item labels are cached at build time; refresh an open
                // menu so it does not stay in the previous language.
                if self.top_overlay() == Some(Overlay::Menu) {
                    self.rebuild_menu();
                }
                self.set_status(
                    Level::Info,
                    format!("{}: {}", self.tr().language, self.lang.name()),
                    false,
                );
            }
            // Window-scoped actions have no global effect on their own. Route
            // them to the window where they apply, so the command palette
            // never offers a command that appears to do nothing.
            a => match a.context() {
                Context::Files => self.do_action(SendFiles),
                Context::Progress => self.push(Overlay::Progress),
                Context::Player => self.do_action(PlayerShow),
                _ => {}
            },
        }
    }

    // ---- menus ------------------------------------------------------------

    pub fn open_menu(&mut self) {
        self.open_menu_at(MenuId::Adapter);
    }

    /// Open (or switch) the menu overlay to a specific tab, e.g. via F1–F4.
    pub fn open_menu_at(&mut self, id: MenuId) {
        self.menu_id = id;
        self.menu_index = 0;
        self.rebuild_menu();
        if self.top_overlay() != Some(Overlay::Menu) {
            self.push(Overlay::Menu);
        }
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
        let tr = self.tr();
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
                    format!("{}: {}", tr.power, if p { tr.on } else { tr.off }),
                    Action::Power,
                    self.features.adapter,
                ));
                items.push(self.item(
                    format!("{}: {}", tr.discoverable, if disc { tr.on } else { tr.off }),
                    Action::Discoverable,
                    p && self.features.adapter,
                ));
                items.push(self.item(
                    format!("{}: {}", tr.pairable, if pair { tr.on } else { tr.off }),
                    Action::Pairable,
                    p && self.features.adapter,
                ));
                items.push(self.item(
                    if scan { tr.stop_scan } else { tr.start_scan },
                    Action::Scan,
                    p && self.features.adapter,
                ));
                items.push(self.item(
                    tr.change_adapter,
                    Action::ChangeAdapter,
                    self.features.adapter,
                ));
                items.push(self.item(tr.view_downloads, Action::ProgressView, true));
                items.push(self.item(
                    tr.hide_media_player,
                    Action::PlayerHide,
                    self.show_player_bar,
                ));
                items.push(self.item(tr.quit, Action::Quit, true));
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
                let any = d.is_some() && self.features.device;
                items.push(self.item(
                    if connected { tr.disconnect } else { tr.connect },
                    Action::Connect,
                    any,
                ));
                items.push(self.item(tr.pair, Action::Pair, any && !paired));
                items.push(self.item(
                    if trusted { tr.untrust } else { tr.trust },
                    Action::Trust,
                    any,
                ));
                items.push(self.item(
                    if blocked { tr.unblock } else { tr.block },
                    Action::Block,
                    any,
                ));
                items.push(self.item(
                    tr.send_files_menu,
                    Action::SendFiles,
                    any && self.features.send_file && has_obex,
                ));
                items.push(self.item(
                    tr.network_options,
                    Action::Network,
                    any && self.features.network && has_net,
                ));
                items.push(self.item(
                    tr.audio_profiles_menu,
                    Action::AudioProfiles,
                    any && has_audio && self.features.audio_profiles,
                ));
                items.push(self.item(
                    tr.show_media_player,
                    Action::PlayerShow,
                    any && has_avrcp && self.features.media_player,
                ));
                items.push(self.item(tr.device_info_menu, Action::Info, any));
                items.push(self.item(tr.remove, Action::Remove, any));
            }
            MenuId::View => {
                items.push(self.item(tr.search_filter, Action::Search, true));
                items.push(self.item(tr.cycle_sort, Action::CycleSort, true));
                items.push(self.item(
                    if self.show_details {
                        tr.hide_details
                    } else {
                        tr.show_details
                    },
                    Action::ToggleDetails,
                    true,
                ));
                items.push(self.item(tr.theme_picker_menu, Action::ThemePicker, true));
                items.push(self.item(tr.event_log_menu, Action::ShowLog, true));
                items.push(self.item(tr.media_player_menu, Action::PlayerShow, true));
                items.push(self.item(tr.transfers_menu, Action::ProgressView, true));
                items.push(self.item(tr.refresh, Action::Refresh, true));
                items.push(self.item(tr.command_palette_menu, Action::CommandPalette, true));
            }
            MenuId::Help => {
                items.push(self.item(tr.keybindings_help_menu, Action::Help, true));
                items.push(self.item(tr.about_menu, Action::About, true));
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
            self.set_status(Level::Warning, self.tr().msg_unavailable, false);
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
        // When an overlay is open it owns the keyboard: main-UI shortcuts must
        // not fire underneath it. Esc/Close always dismisses the top overlay.
        if let Some(o) = self.top_overlay() {
            // Command palette level 2: Esc steps back to the category list
            // instead of closing the whole palette.
            if o == Overlay::CommandPalette
                && self.palette_stage == 1
                && (matches!(ev.code, KeyCode::Esc) || self.keys.is(&ev, Action::Close))
            {
                self.palette_go_back();
                return;
            }
            if self.keys.is(&ev, Action::Close) || matches!(ev.code, KeyCode::Esc) {
                self.close_overlay();
                return;
            }
            // `overlay_key` may itself handle Close for a specific view.
            if self.overlay_key(o, ev) {
                return;
            }
            // Swallow everything the overlay did not consume.
            return;
        }
        // No overlay: quit and the global shortcuts.
        if self.keys.is(&ev, Action::Quit) {
            self.do_action(Action::Quit);
            return;
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
        for a in [
            Action::MenuAdapter,
            Action::MenuDevice,
            Action::MenuView,
            Action::MenuHelp,
        ] {
            if self.keys.is(&ev, a) {
                self.do_action(a);
                return;
            }
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
        if self.keys.is(&ev, Action::About) {
            self.do_action(Action::About);
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
        if self.keys.is(&ev, Action::Language) {
            self.do_action(Action::Language);
            return;
        }

        self.main_key(ev);
    }

    fn search_key(&mut self, ev: KeyEvent) {
        let tr = self.tr();
        if self.keys.is(&ev, Action::Close) || matches!(ev.code, KeyCode::Esc) {
            self.search_active = false;
            self.filter.clear();
            self.set_status(Level::Info, tr.msg_filter_cleared, false);
        } else if self.keys.is(&ev, Action::Select) || matches!(ev.code, KeyCode::Enter) {
            self.search_active = false;
            self.set_status(Level::Info, fill(tr.msg_filter, &self.filter), false);
        } else if self.keys.is(&ev, Action::NavigateUp) {
            // Keep the list navigable while typing the filter.
            self.move_selected(true);
        } else if self.keys.is(&ev, Action::NavigateDown) {
            self.move_selected(false);
        } else {
            match ev.code {
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.selected = 0;
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.selected = 0;
                }
                _ => {}
            }
        }
    }

    /// Move the device selection one step (with wraparound), used by the main
    /// list and by the search input.
    fn move_selected(&mut self, up: bool) {
        let visible = self.visible_devices().len();
        if visible == 0 {
            return;
        }
        self.selected = if up {
            if self.selected == 0 {
                visible - 1
            } else {
                self.selected - 1
            }
        } else if self.selected + 1 >= visible {
            0
        } else {
            self.selected + 1
        };
    }

    fn confirm_key(&mut self, ev: KeyEvent) {
        let yes = matches!(ev.code, KeyCode::Char('y' | 'Y'))
            || self.keys.is(&ev, Action::Select)
            || matches!(ev.code, KeyCode::Enter);
        let no = matches!(ev.code, KeyCode::Char('n' | 'N'))
            || self.keys.is(&ev, Action::Close)
            || matches!(ev.code, KeyCode::Esc);
        if !yes && !no {
            return;
        }
        let action = self.confirm.take().map(|c| c.action);
        self.close_overlay();
        if yes {
            match action {
                Some(ConfirmAction::Quit) => {
                    // Only stop an actually-running discovery; BlueZ returns
                    // "No discovery started" otherwise, which would leave a
                    // spurious error on the way out.
                    if self
                        .current_adapter()
                        .map(|a| a.discovering)
                        .unwrap_or(false)
                    {
                        self.dispatch(Command::SetScan(false));
                    }
                    self.running = false;
                }
                Some(ConfirmAction::RemoveDevice(addr)) => {
                    self.dispatch(Command::RemoveDevice(addr));
                }
                None => {}
            }
        } else {
            self.set_status(Level::Info, self.tr().msg_cancelled, false);
        }
    }

    fn auth_key(&mut self, ev: KeyEvent) {
        let Some(req) = self.auth.clone() else {
            return;
        };
        let needs_text = matches!(req.kind, AuthKind::PinCode | AuthKind::Passkey(_));
        let close = self.keys.is(&ev, Action::Close) || matches!(ev.code, KeyCode::Esc);
        let accept = self.keys.is(&ev, Action::Select) || matches!(ev.code, KeyCode::Enter);

        if close {
            self.dispatch(Command::RespondAuth {
                id: req.id,
                response: AuthResponse::Reject,
            });
            self.auth = None;
            self.close_overlay();
            return;
        }
        if accept {
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
            return;
        }
        match ev.code {
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
                Overlay::Network => 3, // exactly three selectable roles (PANU/NAP/DUN)
                Overlay::Progress => app.transfers.len(),
                Overlay::CommandPalette => {
                    if app.palette_stage == 0 {
                        app.palette_categories.len()
                    } else {
                        app.palette_items.len()
                    }
                }
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
            // F1–F4 switch directly between menu tabs.
            for (a, id) in [
                (Action::MenuAdapter, MenuId::Adapter),
                (Action::MenuDevice, MenuId::Device),
                (Action::MenuView, MenuId::View),
                (Action::MenuHelp, MenuId::Help),
            ] {
                if self.keys.is(&ev, a) {
                    self.set_menu(id);
                    return true;
                }
            }
            let len = self.menu_items.len();
            // Confirm follows the Select binding (Enter by default); Space is
            // kept as a convenience for menu activation.
            if self.keys.is(&ev, Action::Select) || matches!(ev.code, KeyCode::Char(' ')) {
                self.menu_select();
                return true;
            }
            match ev.code {
                KeyCode::Up => {
                    self.menu_index = if self.menu_index == 0 {
                        len.saturating_sub(1)
                    } else {
                        self.menu_index - 1
                    };
                    return true;
                }
                KeyCode::Down => {
                    self.menu_index = if self.menu_index + 1 >= len {
                        0
                    } else {
                        self.menu_index + 1
                    };
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
                _ => {}
            }
            // Shortcut keys inside a menu (modifier- and case-aware).
            if let Some(i) = self
                .menu_items
                .iter()
                .position(|it| it.enabled && self.keys.is(&ev, it.action))
            {
                self.menu_index = i;
                self.menu_select();
                return true;
            }
            return false;
        }

        // The file picker keeps its own cursor (`fp.index`) and key handling.
        if o == Overlay::FilePicker {
            return self.file_picker_key(ev);
        }
        // The command palette has two levels (categories → actions).
        if o == Overlay::CommandPalette {
            return self.command_palette_key(ev);
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
                    if self.overlay_index == 0 {
                        count - 1
                    } else {
                        self.overlay_index - 1
                    }
                } else if self.overlay_index + 1 >= count {
                    0
                } else {
                    self.overlay_index + 1
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
                if self.keys.is(&ev, Action::Select) {
                    if let Some(a) = self.adapters.get(self.overlay_index).cloned() {
                        self.dispatch(Command::SelectAdapter(a.path));
                        self.selected_adapter = self.overlay_index;
                        self.close_overlay();
                    }
                    return true;
                }
            }
            Overlay::ThemePicker => {
                if self.keys.is(&ev, Action::Select) {
                    let names = Theme::name_list();
                    if let Some(name) = names.get(self.overlay_index) {
                        self.config.theme = name.to_string();
                        let mut t = Theme::by_name(name).unwrap_or_else(Theme::midnight);
                        self.config.apply_accent(&mut t);
                        self.theme = t;
                        self.set_status(Level::Success, fill(self.tr().msg_theme, name), false);
                    }
                    self.close_overlay();
                    return true;
                }
            }
            Overlay::Profiles => {
                if self.keys.is(&ev, Action::Select) {
                    // BlueZ does not own A2DP/HFP switching; only the demo
                    // backend can actually toggle a profile. On real hardware
                    // say so instead of faking a switch.
                    if !self.audio_profiles_writable() {
                        self.set_status(Level::Warning, self.tr().msg_profile_hint, false);
                        return true;
                    }
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
                let roles = [NetworkRole::Panu, NetworkRole::Nap, NetworkRole::Dun];
                if self.keys.is(&ev, Action::Select) || self.keys.is(&ev, Action::Connect) {
                    if let Some(addr) = self.selected_addr() {
                        let role = roles
                            .get(self.overlay_index)
                            .copied()
                            .unwrap_or(NetworkRole::Panu);
                        self.dispatch(Command::ConnectNetwork { device: addr, role });
                    }
                    return true;
                }
                if self.keys.is(&ev, Action::Remove) {
                    // The highlighted row is a role; disconnect the active
                    // connection for that role (the list is indexed by id,
                    // not by row, so a direct index would be wrong).
                    if let Some(role) = roles.get(self.overlay_index).copied() {
                        if let Some(id) = network_id_for_role(&self.networks, role) {
                            self.dispatch(Command::DisconnectNetwork(id));
                        }
                    }
                    return true;
                }
            }
            Overlay::Progress => {
                // Closing must work even when the transfer list is empty.
                if self.keys.is(&ev, Action::ProgressView) {
                    self.close_overlay();
                    return true;
                }
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
                    (Action::PlayerShuffle, PlayerCommand::ToggleShuffle),
                    (Action::PlayerRepeat, PlayerCommand::CycleRepeat),
                ];
                for (a, c) in map {
                    if self.keys.is(&ev, a) {
                        self.dispatch(Command::PlayerCommand(c));
                        return true;
                    }
                }
                if self.keys.is(&ev, Action::PlayerHide) {
                    self.do_action(Action::PlayerHide);
                    return true;
                }
            }
            Overlay::FilePicker => {
                return self.file_picker_key(ev);
            }
            // Enter or the quit key just dismisses an info window.
            Overlay::Help | Overlay::Log | Overlay::DeviceInfo | Overlay::About
                if self.keys.is(&ev, Action::Select) || self.keys.is(&ev, Action::Quit) =>
            {
                self.close_overlay();
                return true;
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
        if self.keys.is(&ev, Action::NavigateUp) {
            let n = fp.entries.len();
            if n > 0 {
                fp.index = if fp.index == 0 { n - 1 } else { fp.index - 1 };
            }
        } else if self.keys.is(&ev, Action::NavigateDown) {
            let n = fp.entries.len();
            if n > 0 {
                fp.index = if fp.index + 1 >= n { 0 } else { fp.index + 1 };
            }
        } else if self.keys.is(&ev, Action::FileDirForward) {
            if let Some(e) = &entry {
                if e.is_dir() {
                    fp.cwd = e.clone();
                    fp.index = 0;
                    fp.refresh();
                }
            }
        } else if self.keys.is(&ev, Action::FileDirBack) {
            if let Some(parent) = fp.cwd.parent().map(|p| p.to_path_buf()) {
                fp.cwd = parent;
                fp.index = 0;
                fp.refresh();
            }
        } else if self.keys.is(&ev, Action::FileSelect) {
            if let Some(e) = &entry {
                if !fp.selected.insert(e.clone()) {
                    fp.selected.remove(e);
                }
            }
        } else if self.keys.is(&ev, Action::FileInvertSelection) {
            for e in &fp.entries {
                if !fp.selected.insert(e.clone()) {
                    fp.selected.remove(e);
                }
            }
        } else if self.keys.is(&ev, Action::FileSelectAll) {
            fp.selected = fp.entries.iter().cloned().collect();
        } else if self.keys.is(&ev, Action::FileToggleHidden) {
            fp.show_hidden = !fp.show_hidden;
            fp.refresh();
        } else if self.keys.is(&ev, Action::FileRefresh) {
            fp.refresh();
        } else if self.keys.is(&ev, Action::FileConfirmSelection) {
            let files: Vec<PathBuf> = fp.selected.iter().cloned().collect();
            if files.is_empty() {
                self.set_status(Level::Warning, self.tr().msg_no_files, false);
            } else {
                self.dispatch(Command::SendFiles {
                    device: fp.device.clone(),
                    files: files.clone(),
                });
                self.set_status(Level::Busy, fill(self.tr().msg_sending, files.len()), true);
                self.close_overlay();
                self.file_picker = None;
                return true;
            }
        } else {
            return false;
        }
        self.file_picker = Some(fp);
        true
    }

    fn main_key(&mut self, ev: KeyEvent) {
        // Navigation
        let visible = self.visible_devices().len();
        if self.keys.is(&ev, Action::NavigateUp) {
            self.move_selected(true);
            return;
        }
        if self.keys.is(&ev, Action::NavigateDown) {
            self.move_selected(false);
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

    /// Two-level command palette: categories (stage 0) → actions (stage 1).
    fn command_palette_key(&mut self, ev: KeyEvent) -> bool {
        let stage0 = self.palette_stage == 0;
        // Confirm honours the Select binding (Enter by default). Raw Enter is
        // also accepted because mouse activation synthesises an Enter event.
        if self.keys.is(&ev, Action::Select) || matches!(ev.code, KeyCode::Enter) {
            if stage0 {
                if let Some(cat) = self.palette_categories.get(self.overlay_index).copied() {
                    self.palette_category = Some(cat);
                    self.palette_stage = 1;
                    self.palette_query.clear();
                    self.overlay_index = 0;
                    self.rebuild_palette();
                }
            } else if let Some(a) = self.palette_items.get(self.overlay_index).copied() {
                self.close_overlay();
                self.do_action(a);
            }
            return true;
        }
        match ev.code {
            KeyCode::Up | KeyCode::Down => {
                let n = if stage0 {
                    self.palette_categories.len()
                } else {
                    self.palette_items.len()
                };
                if n == 0 {
                    return true;
                }
                let up = matches!(ev.code, KeyCode::Up);
                self.overlay_index = if up {
                    if self.overlay_index == 0 {
                        n - 1
                    } else {
                        self.overlay_index - 1
                    }
                } else if self.overlay_index + 1 >= n {
                    0
                } else {
                    self.overlay_index + 1
                };
                true
            }
            KeyCode::Backspace => {
                if !stage0 && self.palette_query.is_empty() {
                    self.palette_go_back();
                } else {
                    self.palette_query.pop();
                    self.rebuild_palette();
                }
                true
            }
            KeyCode::Char(c) => {
                self.palette_query.push(c);
                self.rebuild_palette();
                true
            }
            _ => false,
        }
    }

    /// Leave the action level and return to the category list.
    pub fn palette_go_back(&mut self) {
        self.palette_stage = 0;
        self.palette_category = None;
        self.palette_query.clear();
        self.overlay_index = 0;
        self.rebuild_palette();
    }

    /// Number of selectable items in a list-like overlay.
    fn overlay_len(&self, o: Overlay) -> usize {
        match o {
            Overlay::Menu => self.menu_items.len(),
            Overlay::AdapterChange => self.adapters.len(),
            Overlay::ThemePicker => Theme::name_list().len(),
            Overlay::Profiles => self.profiles.len(),
            Overlay::Network => 3,
            Overlay::Progress => self.transfers.len(),
            Overlay::CommandPalette => {
                if self.palette_stage == 0 {
                    self.palette_categories.len()
                } else {
                    self.palette_items.len()
                }
            }
            Overlay::FilePicker => self
                .file_picker
                .as_ref()
                .map(|f| f.entries.len())
                .unwrap_or(0),
            _ => 0,
        }
    }

    /// Scroll the open overlay (wheel events), never the list underneath.
    fn scroll_overlay(&mut self, o: Overlay, up: bool) {
        if matches!(
            o,
            Overlay::Help | Overlay::Log | Overlay::DeviceInfo | Overlay::About
        ) {
            self.overlay_scroll = if up {
                self.overlay_scroll.saturating_sub(1)
            } else {
                self.overlay_scroll.saturating_add(1)
            };
            return;
        }
        if o == Overlay::FilePicker {
            if let Some(fp) = self.file_picker.as_mut() {
                let n = fp.entries.len();
                if n > 0 {
                    fp.index = if up {
                        if fp.index == 0 {
                            n - 1
                        } else {
                            fp.index - 1
                        }
                    } else if fp.index + 1 >= n {
                        0
                    } else {
                        fp.index + 1
                    };
                }
            }
            return;
        }
        if o == Overlay::Menu {
            let n = self.menu_items.len();
            if n > 0 {
                self.menu_index = if up {
                    if self.menu_index == 0 {
                        n - 1
                    } else {
                        self.menu_index - 1
                    }
                } else if self.menu_index + 1 >= n {
                    0
                } else {
                    self.menu_index + 1
                };
            }
            return;
        }
        let n = self.overlay_len(o);
        if n > 0 {
            self.overlay_index = if up {
                if self.overlay_index == 0 {
                    n - 1
                } else {
                    self.overlay_index - 1
                }
            } else if self.overlay_index + 1 >= n {
                0
            } else {
                self.overlay_index + 1
            };
        }
    }

    /// Activate (or select) a row inside the top overlay on left click.
    fn overlay_item_click(&mut self, o: Overlay, idx: usize) {
        let now = Instant::now();
        let double = self
            .last_overlay_click
            .map(|(t, i)| i == idx && t.elapsed().as_millis() < 400)
            .unwrap_or(false);
        self.last_overlay_click = Some((now, idx));

        match o {
            // Menus act on a single click (standard menu behaviour).
            Overlay::Menu => {
                self.menu_index = idx;
                self.menu_select();
            }
            // The command palette is a launcher: a click activates the row.
            Overlay::CommandPalette => {
                self.overlay_index = idx;
                let _ = self.overlay_key(o, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            }
            // File picker: click selects, double-click enters a folder or
            // toggles a file's checkbox.
            Overlay::FilePicker => {
                if let Some(fp) = self.file_picker.as_mut() {
                    fp.index = idx;
                }
                if double {
                    self.file_picker_activate(idx);
                }
            }
            // Other lists: click selects, double-click activates.
            _ => {
                self.overlay_index = idx;
                if double {
                    let _ = self.overlay_key(o, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
        }
    }

    fn file_picker_activate(&mut self, idx: usize) {
        let Some(mut fp) = self.file_picker.clone() else {
            return;
        };
        let Some(entry) = fp.entries.get(idx).cloned() else {
            return;
        };
        if entry.is_dir() {
            fp.cwd = entry;
            fp.index = 0;
            fp.refresh();
        } else if !fp.selected.insert(entry.clone()) {
            fp.selected.remove(&entry);
        }
        self.file_picker = Some(fp);
    }

    // ---- mouse ------------------------------------------------------------

    pub fn on_mouse(&mut self, ev: MouseEvent) {
        let (x, y) = (ev.column, ev.row);
        match ev.kind {
            MouseEventKind::ScrollUp => {
                if let Some(o) = self.top_overlay() {
                    self.scroll_overlay(o, true);
                    return;
                }
                let n = self.visible_devices().len();
                if n > 0 {
                    self.selected = if self.selected == 0 {
                        n - 1
                    } else {
                        self.selected - 1
                    };
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(o) = self.top_overlay() {
                    self.scroll_overlay(o, false);
                    return;
                }
                let n = self.visible_devices().len();
                if n > 0 {
                    self.selected = if self.selected + 1 >= n {
                        0
                    } else {
                        self.selected + 1
                    };
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // The top menu bar is live when nothing (or only a menu) is
                // open. Never let it cover a confirmation / pairing dialog.
                if self.top_overlay().is_none_or(|o| o == Overlay::Menu) {
                    for (i, r) in self.layout.menu_rects.iter().enumerate() {
                        if x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height {
                            self.open_menu_at(MenuId::from_index(i));
                            return;
                        }
                    }
                }
                // While an overlay is open it owns the mouse.
                if let Some(o) = self.top_overlay() {
                    let inside = match self.layout.overlay_rect {
                        Some(r) => x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height,
                        // Not drawn yet: don't treat the click as "outside".
                        None => true,
                    };
                    if !inside {
                        match o {
                            // Keep the pairing prompt modal to avoid an
                            // accidental reject; everything else dismisses.
                            Overlay::Auth => {}
                            Overlay::Confirm => {
                                self.confirm_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
                            }
                            _ => self.close_overlay(),
                        }
                        return;
                    }
                    let hit = self
                        .layout
                        .overlay_items
                        .iter()
                        .find(|(r, _)| {
                            x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
                        })
                        .map(|(_, i)| *i);
                    if let Some(idx) = hit {
                        self.overlay_item_click(o, idx);
                    }
                    return;
                }
                // Bottom hint strip: clicking a hint runs its action (this is
                // how windows like Info/Transfers/Help can be opened by mouse).
                for (r, a) in &self.layout.help_rects {
                    if x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height {
                        let action = *a;
                        self.do_action(action);
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
                // A modal overlay owns the mouse; right-clicking behind it
                // must not replace it with the device context menu.
                if self.top_overlay().is_some() {
                    return;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_disconnect_targets_the_selected_role() {
        let networks = vec![
            Network {
                id: 7,
                role: NetworkRole::Panu,
                connected: true,
                ..Default::default()
            },
            Network {
                id: 42,
                role: NetworkRole::Nap,
                connected: true,
                ..Default::default()
            },
        ];
        // Row 1 is NAP, so it must select id 42, not networks[1] by index
        // coincidence nor the first connection.
        assert_eq!(network_id_for_role(&networks, NetworkRole::Nap), Some(42));
        assert_eq!(network_id_for_role(&networks, NetworkRole::Dun), None);
    }

    #[test]
    fn local_keys_are_excluded_from_the_palette() {
        assert!(is_palette_excluded(Action::NavigateUp));
        assert!(is_palette_excluded(Action::Select));
        assert!(is_palette_excluded(Action::Suspend));
        assert!(is_palette_excluded(Action::Close));
        assert!(!is_palette_excluded(Action::Connect));
        assert!(!is_palette_excluded(Action::About));
    }
}
