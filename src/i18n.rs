//! Internationalisation. All user-facing strings live here as one struct per
//! language so the rest of the UI never hard-codes text. `App::tr()` returns
//! the active language's struct by value (every field is `&'static str`).

/// Substitute the single `{}` placeholder in a translated template.
pub fn fill(template: &str, value: impl std::fmt::Display) -> String {
    template.replacen("{}", &value.to_string(), 1)
}

/// Languages the UI can render in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

impl Lang {
    pub fn all() -> &'static [Lang] {
        &[Lang::En, Lang::Zh]
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Zh => "中文",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Zh => "zh",
        }
    }

    pub fn parse(s: &str) -> Lang {
        match s.trim().to_ascii_lowercase().as_str() {
            "zh" | "zh-cn" | "zh_cn" | "zh-tw" | "chinese" | "中文" | "cn" => Lang::Zh,
            _ => Lang::En,
        }
    }

    /// Pick a language from the usual locale variables; defaults to English.
    pub fn detect() -> Lang {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(v) = std::env::var(var) {
                if v.to_ascii_lowercase().starts_with("zh") {
                    return Lang::Zh;
                }
            }
        }
        Lang::En
    }

    pub fn tr(self) -> Tr {
        match self {
            Lang::En => EN,
            Lang::Zh => ZH,
        }
    }
}

/// All translatable strings for one language (every field `&'static str`).
#[derive(Clone, Copy)]
pub struct Tr {
    // Panel / overlay titles
    pub devices: &'static str,
    pub device_details: &'static str,
    pub select_adapter: &'static str,
    pub network: &'static str,
    pub audio_profiles: &'static str,
    pub media_player: &'static str,
    pub transfers: &'static str,
    pub theme: &'static str,
    pub event_log: &'static str,
    pub about: &'static str,
    pub keybindings_help: &'static str,
    pub menu: &'static str,
    pub confirm: &'static str,
    pub device_info: &'static str,
    pub send_files: &'static str,
    pub pairing_request: &'static str,
    pub command_palette: &'static str,
    pub language: &'static str,
    // Menu bar labels
    pub m_adapter: &'static str,
    pub m_device: &'static str,
    pub m_view: &'static str,
    pub m_help: &'static str,
    // Menu item labels
    pub power: &'static str,
    pub discoverable: &'static str,
    pub pairable: &'static str,
    pub on: &'static str,
    pub off: &'static str,
    pub start_scan: &'static str,
    pub stop_scan: &'static str,
    pub change_adapter: &'static str,
    pub view_downloads: &'static str,
    pub hide_media_player: &'static str,
    pub show_media_player: &'static str,
    pub quit: &'static str,
    pub connect: &'static str,
    pub disconnect: &'static str,
    pub pair: &'static str,
    pub untrust: &'static str,
    pub trust: &'static str,
    pub unblock: &'static str,
    pub block: &'static str,
    pub send_files_menu: &'static str,
    pub network_options: &'static str,
    pub audio_profiles_menu: &'static str,
    pub device_info_menu: &'static str,
    pub remove: &'static str,
    pub search_filter: &'static str,
    pub cycle_sort: &'static str,
    pub hide_details: &'static str,
    pub show_details: &'static str,
    pub theme_picker_menu: &'static str,
    pub event_log_menu: &'static str,
    pub media_player_menu: &'static str,
    pub transfers_menu: &'static str,
    pub refresh: &'static str,
    pub command_palette_menu: &'static str,
    pub keybindings_help_menu: &'static str,
    pub about_menu: &'static str,
    // Column headers
    pub col_name: &'static str,
    pub col_address: &'static str,
    pub col_type: &'static str,
    pub col_rssi: &'static str,
    pub col_bat: &'static str,
    pub col_state: &'static str,
    // Details / device info labels
    pub d_connected: &'static str,
    pub d_paired: &'static str,
    pub d_trusted: &'static str,
    pub d_blocked: &'static str,
    pub d_signal: &'static str,
    pub d_battery: &'static str,
    pub d_type: &'static str,
    pub d_class: &'static str,
    pub d_modalias: &'static str,
    pub d_appearance: &'static str,
    pub d_services: &'static str,
    pub d_and_more: &'static str,
    pub d_select_device: &'static str,
    pub d_address: &'static str,
    pub d_adapter: &'static str,
    pub d_icon: &'static str,
    pub d_rssi: &'static str,
    pub d_txpower: &'static str,
    pub d_legacy: &'static str,
    pub d_resolved: &'static str,
    pub d_uuids: &'static str,
    // Legend
    pub lg_title: &'static str,
    pub lg_connected: &'static str,
    pub lg_paired: &'static str,
    pub lg_trusted: &'static str,
    pub lg_blocked: &'static str,
    pub lg_signal: &'static str,
    pub lg_battery: &'static str,
    // yes / no
    pub yes: &'static str,
    pub no: &'static str,
    // Help bar hints
    pub h_navigate: &'static str,
    pub h_connect: &'static str,
    pub h_pair: &'static str,
    pub h_scan: &'static str,
    pub h_power: &'static str,
    pub h_trust: &'static str,
    pub h_block: &'static str,
    pub h_send: &'static str,
    pub h_info: &'static str,
    pub h_remove: &'static str,
    pub h_transfers: &'static str,
    pub h_search: &'static str,
    pub h_sort: &'static str,
    pub h_details: &'static str,
    pub h_theme: &'static str,
    pub h_help: &'static str,
    // Status bar
    pub sb_menu: &'static str,
    pub sb_help: &'static str,
    pub sb_quit: &'static str,
    // Header chips
    pub st_scan: &'static str,
    pub st_scanning: &'static str,
    // Sort labels
    pub sort_name: &'static str,
    pub sort_address: &'static str,
    pub sort_rssi: &'static str,
    pub sort_battery: &'static str,
    pub sort_state: &'static str,
    pub sort_kind: &'static str,
    // Status / action messages
    pub msg_ready: &'static str,
    pub msg_no_adapter: &'static str,
    pub msg_quit_confirm: &'static str,
    pub msg_setting: &'static str,
    pub msg_connecting: &'static str,
    pub msg_disconnecting: &'static str,
    pub msg_pairing: &'static str,
    pub msg_cancelled: &'static str,
    pub msg_no_send: &'static str,
    pub msg_select_device: &'static str,
    pub msg_search_hint: &'static str,
    pub msg_refreshing: &'static str,
    pub msg_unavailable: &'static str,
    pub msg_filter_cleared: &'static str,
    pub msg_filter: &'static str,
    pub msg_remove_confirm: &'static str,
    pub msg_no_files: &'static str,
    pub msg_sending: &'static str,
    pub msg_no_devices: &'static str,
    pub msg_no_devices_filter: &'static str,
    pub msg_select_device_details: &'static str,
    pub msg_filter_active: &'static str,
    pub msg_theme: &'static str,
    pub msg_profile_hint: &'static str,
    pub msg_terminal_small: &'static str,
    pub msg_key_conflicts: &'static str,
    pub msg_gsm_reserved: &'static str,
    pub f_readonly: &'static str,
    pub nr_panu: &'static str,
    pub nr_nap: &'static str,
    pub nr_dun: &'static str,
    pub pf_d_a2dp_src: &'static str,
    pub pf_d_a2dp_sink: &'static str,
    pub pf_d_hsp: &'static str,
    pub pf_d_hfp: &'static str,
    pub pf_d_avrcp: &'static str,
    // Media player
    pub p_now_playing: &'static str,
    pub p_no_player: &'static str,
    pub p_status: &'static str,
    pub p_vol: &'static str,
    // Media player status / controls
    pub p_stopped: &'static str,
    pub p_playing: &'static str,
    pub p_paused: &'static str,
    pub p_seeking_fwd: &'static str,
    pub p_seeking_rev: &'static str,
    pub p_error: &'static str,
    pub p_shuffle: &'static str,
    pub p_repeat: &'static str,
    pub p_hint: &'static str,
    pub p_vol_prefix: &'static str,
    pub p_eq: &'static str,
    pub p_track: &'static str,
    // Network
    pub n_active: &'static str,
    pub n_none: &'static str,
    pub n_available: &'static str,
    // Profiles
    pub pf_none: &'static str,
    // Progress / transfers
    pub pr_none: &'static str,
    // Transfer status labels
    pub ts_active: &'static str,
    pub ts_suspended: &'static str,
    pub ts_done: &'static str,
    pub ts_failed: &'static str,
    pub ts_cancelled: &'static str,
    // File picker
    pub fp_dir: &'static str,
    // Auth
    pub au_confirm: &'static str,
    pub au_verify: &'static str,
    pub au_authorize: &'static str,
    pub au_allow: &'static str,
    pub au_pin: &'static str,
    pub au_type: &'static str,
    pub au_passkey: &'static str,
    pub au_pairing_pin: &'static str,
    pub au_enter: &'static str,
    pub au_enter_passkey: &'static str,
    pub au_expected: &'static str,
    pub au_accept: &'static str,
    pub au_reject: &'static str,
    pub au_file_push: &'static str,
    pub au_file_accept: &'static str,
    // About
    pub ab_desc: &'static str,
    pub ab_desc2: &'static str,
    pub ab_session: &'static str,
    pub ab_backend: &'static str,
    pub ab_adapters: &'static str,
    pub ab_devices: &'static str,
    pub ab_theme: &'static str,
    pub ab_obex: &'static str,
    pub ab_media: &'static str,
    pub ab_receive: &'static str,
    pub ab_license: &'static str,
    // Help view
    pub hv_ref: &'static str,
    pub hv_override: &'static str,
    pub hv_mouse: &'static str,
    // Overlay footers
    pub f_scroll: &'static str,
    pub f_close: &'static str,
    pub f_confirm: &'static str,
    pub f_cancel: &'static str,
    pub f_select: &'static str,
    pub f_use_adapter: &'static str,
    pub f_dir: &'static str,
    pub f_send: &'static str,
    pub f_invert: &'static str,
    pub f_all: &'static str,
    pub f_suspend: &'static str,
    pub f_resume: &'static str,
    pub f_filter: &'static str,
    pub f_run: &'static str,
    pub f_apply: &'static str,
    pub f_track: &'static str,
    pub f_seek: &'static str,
    pub f_volume: &'static str,
    pub f_stop: &'static str,
    pub f_hide: &'static str,
    // More footer verbs
    pub f_toggle: &'static str,
    pub f_connect: &'static str,
    pub f_disconnect: &'static str,
    pub f_accept: &'static str,
    pub f_reject: &'static str,
    pub f_play_pause: &'static str,
    pub f_shuffle: &'static str,
    pub f_repeat: &'static str,
    pub f_select_role: &'static str,
    pub f_type: &'static str,
    pub f_open: &'static str,
    pub f_back: &'static str,
}

pub const EN: Tr = Tr {
    devices: "Devices",
    device_details: "Device Details",
    select_adapter: "Select Adapter",
    network: "Network Options",
    audio_profiles: "Audio Profiles",
    media_player: "Media Player",
    transfers: "Transfers",
    theme: "Theme",
    event_log: "Event Log",
    about: "About",
    keybindings_help: "Keybindings & Help",
    menu: "Menu",
    confirm: "Confirm",
    device_info: "Device Information",
    send_files: "Send Files",
    pairing_request: "Pairing Request",
    command_palette: "Command Palette",
    language: "Language",
    m_adapter: "Adapter",
    m_device: "Device",
    m_view: "View",
    m_help: "Help",
    power: "Power",
    discoverable: "Discoverable",
    pairable: "Pairable",
    on: "On",
    off: "Off",
    start_scan: "Start Scan",
    stop_scan: "Stop Scan",
    change_adapter: "Change Adapter…",
    view_downloads: "View Downloads…",
    hide_media_player: "Hide Media Player",
    show_media_player: "Show Media Player…",
    quit: "Quit",
    connect: "Connect",
    disconnect: "Disconnect",
    pair: "Pair",
    untrust: "Untrust",
    trust: "Trust",
    unblock: "Unblock",
    block: "Block",
    send_files_menu: "Send Files…",
    network_options: "Network Options…",
    audio_profiles_menu: "Audio Profiles…",
    device_info_menu: "Device Info…",
    remove: "Remove",
    search_filter: "Search / Filter",
    cycle_sort: "Cycle Sort",
    hide_details: "Hide Details",
    show_details: "Show Details",
    theme_picker_menu: "Theme Picker…",
    event_log_menu: "Event Log…",
    media_player_menu: "Media Player…",
    transfers_menu: "Transfers…",
    refresh: "Refresh",
    command_palette_menu: "Command Palette…",
    keybindings_help_menu: "Keybindings & Help",
    about_menu: "About",
    col_name: "NAME",
    col_address: "ADDRESS",
    col_type: "TYPE",
    col_rssi: "RSSI",
    col_bat: "BAT",
    col_state: "STATE",
    d_connected: "Connected",
    d_paired: "Paired",
    d_trusted: "Trusted",
    d_blocked: "Blocked",
    d_signal: "Signal",
    d_battery: "Battery",
    d_type: "Type",
    d_class: "Class",
    d_modalias: "Modalias",
    d_appearance: "Appearance",
    d_services: "Services",
    d_and_more: "and {} more",
    d_select_device: "Select a device to see its details",
    d_address: "Address",
    d_adapter: "Adapter",
    d_icon: "Icon",
    d_rssi: "RSSI",
    d_txpower: "Tx power",
    d_legacy: "Legacy pairing",
    d_resolved: "Services resolved",
    d_uuids: "UUIDs / services",
    lg_title: "legend:",
    lg_connected: "connected",
    lg_paired: "paired",
    lg_trusted: "trusted",
    lg_blocked: "blocked",
    lg_signal: "signal",
    lg_battery: "battery",
    yes: "yes",
    no: "no",
    h_navigate: "navigate",
    h_connect: "connect",
    h_pair: "pair",
    h_scan: "scan",
    h_power: "power",
    h_trust: "trust",
    h_block: "block",
    h_send: "send",
    h_info: "info",
    h_remove: "remove",
    h_transfers: "transfers",
    h_search: "search",
    h_sort: "sort",
    h_details: "details",
    h_theme: "theme",
    h_help: "help",
    sb_menu: "Menu",
    sb_help: "Help",
    sb_quit: "Quit",
    st_scan: "SCAN",
    st_scanning: "scanning…",
    sort_name: "Name",
    sort_address: "Address",
    sort_rssi: "RSSI",
    sort_battery: "Battery",
    sort_state: "State",
    sort_kind: "Kind",
    msg_ready: "Ready",
    msg_no_adapter: "no adapter",
    msg_quit_confirm: "Quit bluetuith-rs?",
    msg_setting: "Setting {}…",
    msg_connecting: "Connecting to {}…",
    msg_disconnecting: "Disconnecting from {}…",
    msg_pairing: "Pairing…",
    msg_cancelled: "Cancelled",
    msg_no_send: "File transfer is unavailable on this session",
    msg_select_device: "Select a device first",
    msg_search_hint: "Search: type to filter, Enter to keep, Esc to clear",
    msg_refreshing: "Refreshing…",
    msg_unavailable: "That action is unavailable here",
    msg_filter_cleared: "Filter cleared",
    msg_filter: "Filter: {}",
    msg_remove_confirm: "Remove {}?",
    msg_no_files: "No files selected",
    msg_sending: "Sending {} file(s)…",
    msg_no_devices: "No devices yet — press s to start scanning",
    msg_no_devices_filter: "No devices match the filter",
    msg_select_device_details: "Select a device to see its details",
    msg_filter_active: "filter",
    msg_theme: "Theme: {}",
    msg_profile_hint:
        "Audio profiles are switched by bluez-alsa/PipeWire/PulseAudio, not by BlueZ itself.",
    msg_terminal_small: "Terminal too small — please resize to at least 40×12",
    msg_key_conflicts: "Keybinding conflicts: {}",
    msg_gsm_reserved: "gsm-apn / gsm-number are reserved for DUN dial-up and have no effect yet",
    f_readonly: "read-only",
    nr_panu: "PANU (client)",
    nr_nap: "NAP (access point)",
    nr_dun: "DUN (dial-up)",
    pf_d_a2dp_src: "High-quality audio output",
    pf_d_a2dp_sink: "Receive audio stream",
    pf_d_hsp: "Headset profile",
    pf_d_hfp: "Hands-free calls",
    pf_d_avrcp: "Remote media control",
    p_now_playing: "NOW PLAYING",
    p_no_player: "No media player is available",
    p_status: "status",
    p_vol: "vol {}%",
    p_stopped: "Stopped",
    p_playing: "Playing",
    p_paused: "Paused",
    p_seeking_fwd: "Seeking ▸▸",
    p_seeking_rev: "Seeking ◂◂",
    p_error: "Error",
    p_shuffle: "shuffle",
    p_repeat: "repeat",
    p_hint: "Space play/pause · < > track · M hide · m player",
    p_vol_prefix: "volume",
    p_eq: "eq",
    p_track: "track",
    n_active: "Active connections",
    n_none: "(none)",
    n_available: "available",
    pf_none: "  No audio profiles advertised by this device",
    pr_none: "No transfers yet — use 'Send Files' on a device",
    ts_active: "Active",
    ts_suspended: "Suspended",
    ts_done: "Done",
    ts_failed: "Failed",
    ts_cancelled: "Cancelled",
    fp_dir: "<DIR>",
    au_confirm: "Confirm pairing",
    au_verify: "Verify the code {} on both devices",
    au_authorize: "Authorize connection",
    au_allow: "Allow this device to connect?",
    au_pin: "Enter PIN code",
    au_type: "Type the PIN shown on the device",
    au_passkey: "Enter passkey",
    au_pairing_pin: "Pairing PIN",
    au_enter: "Enter this PIN on the device: {}",
    au_enter_passkey: "Enter {} on the device",
    au_expected: "Expected up to 6 digits (shown: {})",
    au_accept: "accept / submit",
    au_reject: "reject",
    au_file_push: "Incoming file",
    au_file_accept: "Receive this file: {}?",
    ab_desc: "A full-featured Bluetooth manager for the terminal.",
    ab_desc2: "Written in Rust with ratatui; BlueZ over D-Bus.",
    ab_session: "Session",
    ab_backend: "backend",
    ab_adapters: "adapters",
    ab_devices: "devices",
    ab_theme: "theme",
    ab_obex: "OBEX",
    ab_media: "media",
    ab_receive: "receive files",
    ab_license: "Apache-2.0 licensed · inspired by bluetuith",
    hv_ref: "— keyboard reference",
    hv_override:
        "  Override any binding in ~/.config/bluetuith-rs/config.toml under [keybindings].",
    hv_mouse: "  Mouse: click a menu or a device row, double-click to connect, scroll to navigate.",
    f_scroll: "scroll",
    f_close: "close",
    f_confirm: "confirm",
    f_cancel: "cancel",
    f_select: "select",
    f_use_adapter: "use adapter",
    f_dir: "dir",
    f_send: "send",
    f_invert: "invert",
    f_all: "all",
    f_suspend: "suspend",
    f_resume: "resume",
    f_filter: "filter",
    f_run: "run",
    f_apply: "apply",
    f_track: "track",
    f_seek: "seek",
    f_volume: "volume",
    f_stop: "stop",
    f_hide: "hide",
    f_toggle: "toggle",
    f_connect: "connect",
    f_disconnect: "disconnect",
    f_accept: "accept",
    f_reject: "reject",
    f_play_pause: "play/pause",
    f_shuffle: "shuffle",
    f_repeat: "repeat",
    f_select_role: "select role",
    f_type: "type",
    f_open: "open",
    f_back: "back",
};

pub const ZH: Tr = Tr {
    devices: "设备",
    device_details: "设备详情",
    select_adapter: "选择适配器",
    network: "网络选项",
    audio_profiles: "音频配置",
    media_player: "媒体播放器",
    transfers: "文件传输",
    theme: "主题",
    event_log: "事件日志",
    about: "关于",
    keybindings_help: "快捷键与帮助",
    menu: "菜单",
    confirm: "确认",
    device_info: "设备信息",
    send_files: "发送文件",
    pairing_request: "配对请求",
    command_palette: "命令面板",
    language: "语言",
    m_adapter: "适配器",
    m_device: "设备",
    m_view: "视图",
    m_help: "帮助",
    power: "电源",
    discoverable: "可发现",
    pairable: "可配对",
    on: "开",
    off: "关",
    start_scan: "开始扫描",
    stop_scan: "停止扫描",
    change_adapter: "切换适配器…",
    view_downloads: "查看下载…",
    hide_media_player: "隐藏媒体播放器",
    show_media_player: "显示媒体播放器…",
    quit: "退出",
    connect: "连接",
    disconnect: "断开",
    pair: "配对",
    untrust: "取消信任",
    trust: "信任",
    unblock: "取消屏蔽",
    block: "屏蔽",
    send_files_menu: "发送文件…",
    network_options: "网络选项…",
    audio_profiles_menu: "音频配置…",
    device_info_menu: "设备信息…",
    remove: "移除",
    search_filter: "搜索 / 筛选",
    cycle_sort: "切换排序",
    hide_details: "隐藏详情",
    show_details: "显示详情",
    theme_picker_menu: "主题选择…",
    event_log_menu: "事件日志…",
    media_player_menu: "媒体播放器…",
    transfers_menu: "文件传输…",
    refresh: "刷新",
    command_palette_menu: "命令面板…",
    keybindings_help_menu: "快捷键与帮助",
    about_menu: "关于",
    col_name: "名称",
    col_address: "地址",
    col_type: "类型",
    col_rssi: "信号",
    col_bat: "电量",
    col_state: "状态",
    d_connected: "已连接",
    d_paired: "已配对",
    d_trusted: "已信任",
    d_blocked: "已屏蔽",
    d_signal: "信号",
    d_battery: "电量",
    d_type: "类型",
    d_class: "类别",
    d_modalias: "Modalias",
    d_appearance: "外观",
    d_services: "服务",
    d_and_more: "等 {} 项",
    d_select_device: "选择一个设备以查看详情",
    d_address: "地址",
    d_adapter: "适配器",
    d_icon: "图标",
    d_rssi: "信号强度",
    d_txpower: "发射功率",
    d_legacy: "旧式配对",
    d_resolved: "服务已解析",
    d_uuids: "UUID / 服务",
    lg_title: "图例：",
    lg_connected: "已连接",
    lg_paired: "已配对",
    lg_trusted: "已信任",
    lg_blocked: "已屏蔽",
    lg_signal: "信号",
    lg_battery: "电量",
    yes: "是",
    no: "否",
    h_navigate: "移动",
    h_connect: "连接",
    h_pair: "配对",
    h_scan: "扫描",
    h_power: "电源",
    h_trust: "信任",
    h_block: "屏蔽",
    h_send: "发送",
    h_info: "信息",
    h_remove: "移除",
    h_transfers: "传输",
    h_search: "搜索",
    h_sort: "排序",
    h_details: "详情",
    h_theme: "主题",
    h_help: "帮助",
    sb_menu: "菜单",
    sb_help: "帮助",
    sb_quit: "退出",
    st_scan: "扫描",
    st_scanning: "扫描中…",
    sort_name: "名称",
    sort_address: "地址",
    sort_rssi: "信号",
    sort_battery: "电量",
    sort_state: "状态",
    sort_kind: "类型",
    msg_ready: "就绪",
    msg_no_adapter: "无适配器",
    msg_quit_confirm: "退出 bluetuith-rs？",
    msg_setting: "正在设置 {}…",
    msg_connecting: "正在连接 {}…",
    msg_disconnecting: "正在断开 {}…",
    msg_pairing: "正在配对…",
    msg_cancelled: "已取消",
    msg_no_send: "当前会话不可用文件传输",
    msg_select_device: "请先选择一个设备",
    msg_search_hint: "搜索：输入以筛选，回车保留，Esc 清除",
    msg_refreshing: "刷新中…",
    msg_unavailable: "该操作在此处不可用",
    msg_filter_cleared: "已清除筛选",
    msg_filter: "筛选：{}",
    msg_remove_confirm: "移除 {}？",
    msg_no_files: "未选择文件",
    msg_sending: "正在发送 {} 个文件…",
    msg_no_devices: "暂无设备 — 按 s 开始扫描",
    msg_no_devices_filter: "没有匹配筛选条件的设备",
    msg_select_device_details: "选择一个设备以查看详情",
    msg_filter_active: "筛选",
    msg_theme: "主题：{}",
    msg_profile_hint: "音频配置由 bluez-alsa/PipeWire/PulseAudio 切换，BlueZ 本身不提供该接口。",
    msg_terminal_small: "终端太小 — 请调整到至少 40×12",
    msg_key_conflicts: "快捷键冲突：{}",
    msg_gsm_reserved: "gsm-apn / gsm-number 为 DUN 拨号预留，当前不生效",
    f_readonly: "只读",
    nr_panu: "PANU（客户端）",
    nr_nap: "NAP（接入点）",
    nr_dun: "DUN（拨号）",
    pf_d_a2dp_src: "高质量音频输出",
    pf_d_a2dp_sink: "接收音频流",
    pf_d_hsp: "耳机通话",
    pf_d_hfp: "免提通话",
    pf_d_avrcp: "媒体遥控",
    p_now_playing: "正在播放",
    p_no_player: "没有可用的媒体播放器",
    p_status: "状态",
    p_vol: "音量 {}%",
    p_stopped: "已停止",
    p_playing: "播放中",
    p_paused: "已暂停",
    p_seeking_fwd: "快进中 ▸▸",
    p_seeking_rev: "快退中 ◂◂",
    p_error: "错误",
    p_shuffle: "随机",
    p_repeat: "循环",
    p_hint: "空格 播放/暂停 · < > 曲目 · M 隐藏 · m 播放器",
    p_vol_prefix: "音量",
    p_eq: "均衡器",
    p_track: "曲目",
    n_active: "活动连接",
    n_none: "（无）",
    n_available: "可用",
    pf_none: "  该设备未提供音频配置",
    pr_none: "暂无传输 — 在设备上使用“发送文件”",
    ts_active: "进行中",
    ts_suspended: "已暂停",
    ts_done: "已完成",
    ts_failed: "失败",
    ts_cancelled: "已取消",
    fp_dir: "<目录>",
    au_confirm: "确认配对",
    au_verify: "请确认双方设备上的验证码 {}",
    au_authorize: "授权连接",
    au_allow: "允许此设备连接？",
    au_pin: "输入 PIN 码",
    au_type: "输入设备显示的 PIN 码",
    au_passkey: "输入配对码",
    au_pairing_pin: "配对 PIN 码",
    au_enter: "在设备上输入此 PIN 码：{}",
    au_enter_passkey: "请在设备上输入 {}",
    au_expected: "最多 6 位数字（显示：{}）",
    au_accept: "确认 / 提交",
    au_reject: "拒绝",
    au_file_push: "收到文件",
    au_file_accept: "是否接收文件 {}？",
    ab_desc: "终端里的全功能蓝牙管理器。",
    ab_desc2: "用 Rust 与 ratatui 编写；通过 D-Bus 使用 BlueZ。",
    ab_session: "会话",
    ab_backend: "后端",
    ab_adapters: "适配器",
    ab_devices: "设备",
    ab_theme: "主题",
    ab_obex: "OBEX",
    ab_media: "媒体",
    ab_receive: "接收文件",
    ab_license: "Apache-2.0 许可 · 灵感来自 bluetuith",
    hv_ref: "— 快捷键参考",
    hv_override: "  可在 ~/.config/bluetuith-rs/config.toml 的 [keybindings] 中覆盖任意按键。",
    hv_mouse: "  鼠标：单击菜单或设备行，双击连接，滚动浏览。",
    f_scroll: "滚动",
    f_close: "关闭",
    f_confirm: "确认",
    f_cancel: "取消",
    f_select: "选择",
    f_use_adapter: "使用适配器",
    f_dir: "目录",
    f_send: "发送",
    f_invert: "反选",
    f_all: "全选",
    f_suspend: "暂停",
    f_resume: "继续",
    f_filter: "筛选",
    f_run: "运行",
    f_apply: "应用",
    f_track: "曲目",
    f_seek: "快退/快进",
    f_volume: "音量",
    f_stop: "停止",
    f_hide: "隐藏",
    f_toggle: "切换",
    f_connect: "连接",
    f_disconnect: "断开",
    f_accept: "接受",
    f_reject: "拒绝",
    f_play_pause: "播放/暂停",
    f_shuffle: "随机",
    f_repeat: "循环",
    f_select_role: "选择角色",
    f_type: "输入",
    f_open: "打开",
    f_back: "返回",
};

// ---------------------------------------------------------------------------
// Backend status lines.
//
// The worker threads emit plain English strings (they have no access to the
// UI's language). Keep a small translation table here so the Chinese UI does
// not show English status text, without threading a language through D-Bus
// worker threads. Unknown strings — notably raw D-Bus errors — pass through
// unchanged, which is the right behaviour for technical diagnostics.

const EXACT_STATUS: &[(&str, &str)] = &[
    ("Adapter selected", "已切换适配器"),
    ("Connected", "已连接"),
    ("Disconnected", "已断开"),
    ("Pairing complete", "配对完成"),
    ("Pairing cancelled", "已取消配对"),
    ("Device removed", "已移除设备"),
    ("No media player is available", "没有可用的媒体播放器"),
    ("Scanning for devices…", "正在扫描设备…"),
    ("Scanning stopped", "已停止扫描"),
    ("Device list refreshed", "设备列表已刷新"),
    ("Pairing agent registered", "配对代理已注册"),
    ("BlueZ is unavailable on this system", "本系统没有可用的 BlueZ"),
    ("Transfer suspended", "传输已暂停"),
    ("Transfer resumed", "传输已恢复"),
    ("Transfer cancelled", "传输已取消"),
    ("No such transfer", "找不到该传输"),
    ("OBEX service unavailable", "OBEX 服务不可用"),
    ("OBEX services are disabled", "OBEX 服务已禁用"),
    ("No such network connection", "找不到该网络连接"),
    ("Disconnected network", "已断开网络"),
    (
        "DUN dial-up is not provided by BlueZ Network1 (reserved)",
        "DUN 拨号不由 BlueZ Network1 提供（预留功能）",
    ),
    ("Volume adjusted", "音量已调整"),
    ("Shuffle toggled", "已切换随机播放"),
    ("Repeat cycled", "已切换循环模式"),
    (
        "no MediaTransport1 with a writable Volume property",
        "当前设备没有可调音量的 MediaTransport1 接口，音量由系统音频服务控制。",
    ),
    ("Media player hidden", "已隐藏媒体播放器"),
    ("Media player shown", "已显示媒体播放器"),
    ("Network list refreshed", "网络列表已刷新"),
    ("Audio profiles refreshed", "音频配置已刷新"),
    ("Pairing rejected by user", "用户拒绝了配对"),
    ("Play", "播放"),
    ("Pause", "暂停"),
    ("Stop", "停止"),
    ("Next", "下一首"),
    ("Previous", "上一首"),
    ("FastForward", "快进"),
    ("Rewind", "快退"),
    (
        "Cannot send: OBEX service (obexd) is unavailable. Install bluez-obexd and/or start it with `systemctl --user start obex.service`.",
        "无法发送：OBEX 服务（obexd）不可用。请安装 bluez-obexd，或执行 `systemctl --user start obex.service` 启动。",
    ),
];

const PREFIX_STATUS: &[(&str, &str)] = &[
    ("Cannot open system D-Bus: ", "无法打开系统 D-Bus："),
    ("BlueZ: ", "BlueZ："),
    ("Pairing agent unavailable: ", "配对代理不可用："),
    ("Connected to ", "已连接到 "),
    ("Disconnected from ", "已断开 "),
    ("Switched to ", "已切换到 "),
    ("Removed ", "已移除 "),
    ("Cancelled pairing with ", "已取消配对："),
    ("Unknown profile ", "未知音频配置 "),
    ("Media player: ", "媒体播放器："),
];

const SUFFIX_STATUS: &[(&str, &str)] = &[
    (" is already paired", " 已配对"),
    (" is trusted", " 已信任"),
    (" is untrusted", " 已取消信任"),
    (" is blocked", " 已屏蔽"),
    (" is unblocked", " 已取消屏蔽"),
];

/// Translate a backend status line for the active language.
pub fn translate_status(text: &str, lang: Lang) -> String {
    if lang != Lang::Zh {
        return text.to_string();
    }
    // A few parameterised diagnostics carry an English explanatory tail.
    let text = text
        .replace(
            "; is the device paired and in range?",
            "；设备是否已配对并在范围内？",
        )
        .replace("OBEX CreateSession failed", "OBEX 建立会话失败")
        .replace("OBEX SendFile failed", "OBEX 发送文件失败");
    let text = text.as_str();
    // Adapter properties: "Powered is on" / "Discoverable is off" …
    for (suffix, value) in [(" is on", "开启"), (" is off", "关闭")] {
        if let Some(key) = text.strip_suffix(suffix) {
            let key = match key {
                "Powered" => "电源",
                "Discoverable" => "可发现",
                "Pairable" => "可配对",
                _ => key,
            };
            return format!("{key}已{value}");
        }
    }
    for (prefix, zh) in [("Trusted = ", "已信任："), ("Blocked = ", "已屏蔽：")] {
        if let Some(v) = text.strip_prefix(prefix) {
            let v = match v {
                "true" => "是",
                "false" => "否",
                _ => v,
            };
            return format!("{zh}{v}");
        }
    }
    if let Some(name) = text.strip_prefix("Paired with ") {
        return format!("已与 {name} 配对");
    }
    if let Some(rest) = text.strip_prefix("Pairing with ") {
        if let Some(name) = rest.strip_suffix(" rejected") {
            return format!("与 {name} 的配对被拒绝");
        }
    }
    for (en, zh) in EXACT_STATUS {
        if text == *en {
            return (*zh).to_string();
        }
    }
    // Parameterised messages: keep the argument, translate the frame.
    if let Some((key, value)) = text.split_once(" set to ") {
        let k = match key {
            "Powered" => "电源",
            "Discoverable" => "可发现",
            "Pairable" => "可配对",
            _ => key,
        };
        let v = if value == "true" { "开启" } else { "关闭" };
        return format!("{k} 已设为{v}");
    }
    if let Some(rest) = text.strip_prefix("Queued ") {
        if let Some((n, _)) = rest.split_once(" file(s) via OBEX Object Push") {
            return format!("已通过 OBEX 加入 {n} 个文件到发送队列");
        }
    }
    if let Some(rest) = text.strip_prefix("Sending ") {
        if let Some((n, tail)) = rest.split_once(" file(s) to ") {
            return format!("正在向 {tail} 发送 {n} 个文件");
        }
    }
    if let Some(rest) = text.strip_prefix("Connected ") {
        if let Some(tail) = rest.strip_suffix(" network") {
            return format!("已连接 {tail} 网络");
        }
        if let Some((name, role)) = rest.split_once(" as ") {
            return format!("已将 {name} 连接为 {role}");
        }
    }
    if let Some(rest) = text.strip_prefix("Toggled ") {
        if let Some((profile, name)) = rest.split_once(" for ") {
            return format!("已为 {name} 切换 {profile}");
        }
    }
    if let Some(rest) = text.strip_prefix("Switching ") {
        if let Some((profile, _)) = rest.split_once(" needs bluez-alsa") {
            return format!(
                "切换 {profile} 需通过 bluez-alsa/PipeWire/PulseAudio，请在系统音频服务中设置"
            );
        }
    }
    for (en, zh) in PREFIX_STATUS {
        if let Some(rest) = text.strip_prefix(en) {
            return format!("{zh}{rest}");
        }
    }
    for (en, zh) in SUFFIX_STATUS {
        if let Some(head) = text.strip_suffix(en) {
            return format!("{head}{zh}");
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_translation_covers_common_messages() {
        assert_eq!(translate_status("Connected", Lang::Zh), "已连接");
        assert_eq!(translate_status("Trusted = true", Lang::Zh), "已信任：是");
        assert_eq!(translate_status("Blocked = false", Lang::Zh), "已屏蔽：否");
        assert_eq!(translate_status("Powered is on", Lang::Zh), "电源已开启");
        assert_eq!(
            translate_status("Queued 3 file(s) via OBEX Object Push", Lang::Zh),
            "已通过 OBEX 加入 3 个文件到发送队列"
        );
        assert_eq!(
            translate_status("Connected PANU (client) network", Lang::Zh),
            "已连接 PANU (client) 网络"
        );
        assert_eq!(
            translate_status(
                "Switching HFP needs bluez-alsa/PulseAudio; use your audio server",
                Lang::Zh
            ),
            "切换 HFP 需通过 bluez-alsa/PipeWire/PulseAudio，请在系统音频服务中设置"
        );
        // Technical diagnostics are intentionally left in English.
        assert_eq!(
            translate_status("org.freedesktop.DBus.Error.Failed: nope", Lang::Zh),
            "org.freedesktop.DBus.Error.Failed: nope"
        );
        // English UI is untouched.
        assert_eq!(translate_status("Connected", Lang::En), "Connected");
    }

    #[test]
    fn fill_replaces_only_the_first_placeholder() {
        assert_eq!(fill("{} and {}", 1), "1 and {}");
    }
}
