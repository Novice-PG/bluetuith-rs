//! Configurable keybindings, including the conflict checker that bluetuith
//! exposes through `[keybindings]`.

use std::collections::HashMap;

use crate::i18n::Lang;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Context {
    App,
    Device,
    Files,
    Progress,
    Player,
    Extra,
}

impl Context {
    pub fn label(self) -> &'static str {
        use crate::i18n::Lang;
        self.label_for(Lang::En)
    }
    pub fn label_for(self, lang: crate::i18n::Lang) -> &'static str {
        match (self, lang) {
            (Context::App, crate::i18n::Lang::Zh) => "全局",
            (Context::Device, crate::i18n::Lang::Zh) => "设备 / 适配器",
            (Context::Files, crate::i18n::Lang::Zh) => "文件选择",
            (Context::Progress, crate::i18n::Lang::Zh) => "传输",
            (Context::Player, crate::i18n::Lang::Zh) => "媒体播放器",
            (Context::Extra, crate::i18n::Lang::Zh) => "扩展",
            (Context::App, _) => "Global",
            (Context::Device, _) => "Device / Adapter",
            (Context::Files, _) => "File picker",
            (Context::Progress, _) => "Transfers",
            (Context::Player, _) => "Media player",
            (Context::Extra, _) => "Extras",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    // Global / App
    Menu,
    MenuAdapter,
    MenuDevice,
    MenuView,
    MenuHelp,
    Select,
    Cancel,
    Suspend,
    Quit,
    Switch,
    Close,
    Help,
    NavigateUp,
    NavigateDown,
    NavigateTop,
    NavigateBottom,
    // Adapter / Device
    Power,
    Discoverable,
    Pairable,
    Scan,
    ChangeAdapter,
    ProgressView,
    PlayerHide,
    PlayerShow,
    Connect,
    Pair,
    Trust,
    Block,
    SendFiles,
    Network,
    AudioProfiles,
    Info,
    Remove,
    // File browser
    FileDirForward,
    FileDirBack,
    FileSelect,
    FileInvertSelection,
    FileSelectAll,
    FileRefresh,
    FileToggleHidden,
    FileConfirmSelection,
    // Transfers
    TransferSuspend,
    TransferResume,
    TransferCancel,
    // Player
    PlayerTogglePlay,
    PlayerNext,
    PlayerPrevious,
    PlayerSeekForward,
    PlayerSeekBackward,
    PlayerStop,
    PlayerVolumeUp,
    PlayerVolumeDown,
    PlayerShuffle,
    PlayerRepeat,
    // Extras (not in bluetuith)
    Search,
    CycleSort,
    ThemePicker,
    ToggleDetails,
    ShowLog,
    Refresh,
    CommandPalette,
    About,
    Language,
}

impl Action {
    pub fn name(self) -> &'static str {
        use Action::*;
        match self {
            Menu => "Menu",
            MenuAdapter => "Adapter Menu",
            MenuDevice => "Device Menu",
            MenuView => "View Menu",
            MenuHelp => "Help Menu",
            Select => "Select",
            Cancel => "Cancel",
            Suspend => "Suspend",
            Quit => "Quit",
            Switch => "Switch",
            Close => "Close",
            Help => "Help",
            NavigateUp => "Navigate Up",
            NavigateDown => "Navigate Down",
            NavigateTop => "Navigate Top",
            NavigateBottom => "Navigate Bottom",
            Power => "Power",
            Discoverable => "Discoverable",
            Pairable => "Pairable",
            Scan => "Scan",
            ChangeAdapter => "Change Adapter",
            ProgressView => "View Downloads",
            PlayerHide => "Hide Media Player",
            PlayerShow => "Show Media Player",
            Connect => "Connect",
            Pair => "Pair",
            Trust => "Trust",
            Block => "Block",
            SendFiles => "Send Files",
            Network => "Network Options",
            AudioProfiles => "Audio Profiles",
            Info => "Info",
            Remove => "Remove",
            FileDirForward => "Go Forward",
            FileDirBack => "Go Back",
            FileSelect => "Select",
            FileInvertSelection => "Invert Selection",
            FileSelectAll => "Select All",
            FileRefresh => "Refresh",
            FileToggleHidden => "Hidden",
            FileConfirmSelection => "Confirm Selection",
            TransferSuspend => "Suspend Transfer",
            TransferResume => "Resume Transfer",
            TransferCancel => "Cancel Transfer",
            PlayerTogglePlay => "Play/Pause",
            PlayerNext => "Next",
            PlayerPrevious => "Previous",
            PlayerSeekForward => "Seek Forward",
            PlayerSeekBackward => "Seek Backward",
            PlayerStop => "Stop",
            PlayerVolumeUp => "Volume Up",
            PlayerVolumeDown => "Volume Down",
            PlayerShuffle => "Toggle Shuffle",
            PlayerRepeat => "Cycle Repeat",
            Search => "Search / Filter",
            CycleSort => "Cycle Sort",
            ThemePicker => "Theme Picker",
            ToggleDetails => "Toggle Details",
            ShowLog => "Event Log",
            Refresh => "Refresh",
            CommandPalette => "Command Palette",
            About => "About",
            Language => "Language",
        }
    }

    pub fn name_for(self, lang: crate::i18n::Lang) -> &'static str {
        use crate::i18n::Lang;
        use Action::*;
        match (self, lang) {
            (Menu, Lang::Zh) => "菜单",
            (MenuAdapter, Lang::Zh) => "适配器菜单",
            (MenuDevice, Lang::Zh) => "设备菜单",
            (MenuView, Lang::Zh) => "视图菜单",
            (MenuHelp, Lang::Zh) => "帮助菜单",
            (Select, Lang::Zh) => "选择",
            (Cancel, Lang::Zh) => "取消",
            (Suspend, Lang::Zh) => "挂起",
            (Quit, Lang::Zh) => "退出",
            (Switch, Lang::Zh) => "切换",
            (Close, Lang::Zh) => "关闭",
            (Help, Lang::Zh) => "帮助",
            (NavigateUp, Lang::Zh) => "上移",
            (NavigateDown, Lang::Zh) => "下移",
            (NavigateTop, Lang::Zh) => "顶部",
            (NavigateBottom, Lang::Zh) => "底部",
            (Power, Lang::Zh) => "电源",
            (Discoverable, Lang::Zh) => "可发现",
            (Pairable, Lang::Zh) => "可配对",
            (Scan, Lang::Zh) => "扫描",
            (ChangeAdapter, Lang::Zh) => "切换适配器",
            (ProgressView, Lang::Zh) => "查看下载",
            (PlayerHide, Lang::Zh) => "隐藏媒体播放器",
            (PlayerShow, Lang::Zh) => "显示媒体播放器",
            (Connect, Lang::Zh) => "连接",
            (Pair, Lang::Zh) => "配对",
            (Trust, Lang::Zh) => "信任",
            (Block, Lang::Zh) => "屏蔽",
            (SendFiles, Lang::Zh) => "发送文件",
            (Network, Lang::Zh) => "网络选项",
            (AudioProfiles, Lang::Zh) => "音频配置",
            (Info, Lang::Zh) => "信息",
            (Remove, Lang::Zh) => "移除",
            (FileDirForward, Lang::Zh) => "进入目录",
            (FileDirBack, Lang::Zh) => "返回上级",
            (FileSelect, Lang::Zh) => "选择",
            (FileInvertSelection, Lang::Zh) => "反选",
            (FileSelectAll, Lang::Zh) => "全选",
            (FileRefresh, Lang::Zh) => "刷新",
            (FileToggleHidden, Lang::Zh) => "显示隐藏文件",
            (FileConfirmSelection, Lang::Zh) => "确认选择",
            (TransferSuspend, Lang::Zh) => "暂停传输",
            (TransferResume, Lang::Zh) => "继续传输",
            (TransferCancel, Lang::Zh) => "取消传输",
            (PlayerTogglePlay, Lang::Zh) => "播放/暂停",
            (PlayerNext, Lang::Zh) => "下一首",
            (PlayerPrevious, Lang::Zh) => "上一首",
            (PlayerSeekForward, Lang::Zh) => "快进",
            (PlayerSeekBackward, Lang::Zh) => "快退",
            (PlayerStop, Lang::Zh) => "停止",
            (PlayerVolumeUp, Lang::Zh) => "音量+",
            (PlayerVolumeDown, Lang::Zh) => "音量-",
            (PlayerShuffle, Lang::Zh) => "随机播放",
            (PlayerRepeat, Lang::Zh) => "循环模式",
            (Search, Lang::Zh) => "搜索 / 筛选",
            (CycleSort, Lang::Zh) => "切换排序",
            (ThemePicker, Lang::Zh) => "主题选择",
            (ToggleDetails, Lang::Zh) => "切换详情",
            (ShowLog, Lang::Zh) => "事件日志",
            (Refresh, Lang::Zh) => "刷新",
            (CommandPalette, Lang::Zh) => "命令面板",
            (About, Lang::Zh) => "关于",
            (Language, Lang::Zh) => "语言",
            _ => self.name(),
        }
    }

    pub fn context(self) -> Context {
        use Action::*;
        match self {
            Menu | MenuAdapter | MenuDevice | MenuView | MenuHelp | Select | Cancel | Suspend
            | Quit | Switch | Close | Help | Language | NavigateUp | NavigateDown | NavigateTop
            | NavigateBottom => Context::App,
            Power | Discoverable | Pairable | Scan | ChangeAdapter | ProgressView | PlayerHide
            | PlayerShow | Connect | Pair | Trust | Block | SendFiles | Network | AudioProfiles
            | Info | Remove => Context::Device,
            FileDirForward | FileDirBack | FileSelect | FileInvertSelection | FileSelectAll
            | FileRefresh | FileToggleHidden | FileConfirmSelection => Context::Files,
            TransferSuspend | TransferResume | TransferCancel => Context::Progress,
            PlayerTogglePlay | PlayerNext | PlayerPrevious | PlayerSeekForward
            | PlayerSeekBackward | PlayerStop | PlayerVolumeUp | PlayerVolumeDown
            | PlayerShuffle | PlayerRepeat => Context::Player,
            Search | CycleSort | ThemePicker | ToggleDetails | ShowLog | Refresh
            | CommandPalette | About => Context::Extra,
        }
    }

    pub fn all() -> Vec<Action> {
        use Action::*;
        vec![
            Menu,
            MenuAdapter,
            MenuDevice,
            MenuView,
            MenuHelp,
            Select,
            Cancel,
            Suspend,
            Quit,
            Switch,
            Close,
            Help,
            NavigateUp,
            NavigateDown,
            NavigateTop,
            NavigateBottom,
            Power,
            Discoverable,
            Pairable,
            Scan,
            ChangeAdapter,
            ProgressView,
            PlayerHide,
            PlayerShow,
            Connect,
            Pair,
            Trust,
            Block,
            SendFiles,
            Network,
            AudioProfiles,
            Info,
            Remove,
            FileDirForward,
            FileDirBack,
            FileSelect,
            FileInvertSelection,
            FileSelectAll,
            FileRefresh,
            FileToggleHidden,
            FileConfirmSelection,
            TransferSuspend,
            TransferResume,
            TransferCancel,
            PlayerTogglePlay,
            PlayerNext,
            PlayerPrevious,
            PlayerSeekForward,
            PlayerSeekBackward,
            PlayerStop,
            PlayerVolumeUp,
            PlayerVolumeDown,
            PlayerShuffle,
            PlayerRepeat,
            Search,
            CycleSort,
            ThemePicker,
            ToggleDetails,
            ShowLog,
            Refresh,
            CommandPalette,
            About,
            Language,
        ]
    }

    pub fn from_name(name: &str) -> Option<Action> {
        Action::all()
            .into_iter()
            .find(|a| a.name().eq_ignore_ascii_case(name.trim()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeySpec {
    fn new(code: KeyCode) -> Self {
        Self {
            code,
            mods: KeyModifiers::NONE,
        }
    }
    fn ctrl(code: KeyCode) -> Self {
        Self {
            code,
            mods: KeyModifiers::CONTROL,
        }
    }
    fn alt(code: KeyCode) -> Self {
        Self {
            code,
            mods: KeyModifiers::ALT,
        }
    }

    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl".to_string());
        }
        if self.mods.contains(KeyModifiers::ALT) {
            parts.push("Alt".to_string());
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            parts.push("Shift".to_string());
        }
        let key = match self.code {
            KeyCode::Char(' ') => "Space".into(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "Enter".into(),
            KeyCode::Esc => "Esc".into(),
            KeyCode::Tab => "Tab".into(),
            KeyCode::BackTab => "BackTab".into(),
            KeyCode::Backspace => "Backspace".into(),
            KeyCode::Up => "↑".into(),
            KeyCode::Down => "↓".into(),
            KeyCode::Left => "←".into(),
            KeyCode::Right => "→".into(),
            KeyCode::Home => "Home".into(),
            KeyCode::End => "End".into(),
            KeyCode::PageUp => "PgUp".into(),
            KeyCode::PageDown => "PgDn".into(),
            KeyCode::Delete => "Del".into(),
            KeyCode::Insert => "Ins".into(),
            KeyCode::F(n) => format!("F{n}"),
            _ => "?".into(),
        };
        parts.push(key);
        parts.join("+")
    }
}

/// Normalise an event so letter keys are case-insensitive: `Shift+q`, `Q`
/// and `q` all compare equal (with the Shift modifier folded away).
fn normalize(code: KeyCode, mods: KeyModifiers) -> (KeyCode, KeyModifiers) {
    match code {
        KeyCode::Char(c) => {
            let c = if c.is_ascii_alphabetic() {
                c.to_ascii_lowercase()
            } else {
                c
            };
            (KeyCode::Char(c), mods - KeyModifiers::SHIFT)
        }
        _ => (code, mods),
    }
}

/// A `KeySpec` with letter case/Shift folded in, for grouping in conflict checks.
fn normalize_spec(spec: KeySpec) -> KeySpec {
    let (code, mods) = normalize(spec.code, spec.mods);
    KeySpec { code, mods }
}

pub fn matches(ev: &KeyEvent, spec: &KeySpec) -> bool {
    let (a_code, a_mods) = normalize(ev.code, ev.modifiers);
    let (b_code, b_mods) = normalize(spec.code, spec.mods);
    a_code == b_code && a_mods == b_mods
}

/// Parse a binding such as `Alt+m`, `Ctrl+x`, `PgUp`, `F3`, `?`, `Space`.
pub fn parse_key(input: &str) -> Option<KeySpec> {
    let tokens: Vec<&str> = input
        .split(|c: char| c == '+' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let mut mods = KeyModifiers::NONE;
    let mut last = tokens[0];
    for (i, tok) in tokens.iter().enumerate() {
        let is_mod = match tok.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => {
                mods |= KeyModifiers::CONTROL;
                true
            }
            "alt" => {
                mods |= KeyModifiers::ALT;
                true
            }
            "shift" => {
                mods |= KeyModifiers::SHIFT;
                true
            }
            _ => false,
        };
        if !is_mod {
            last = tok;
            // Anything after a non-modifier token is invalid.
            if i + 1 < tokens.len() {
                return None;
            }
        }
    }
    let lower = last.to_ascii_lowercase();
    let code = match lower.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "backspace" | "bs" => KeyCode::Backspace,
        "space" | "spc" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pgdown" | "pagedown" => KeyCode::PageDown,
        "del" | "delete" => KeyCode::Delete,
        "ins" | "insert" => KeyCode::Insert,
        _ if lower.starts_with('f') && lower.len() <= 3 => {
            let n: u8 = lower[1..].parse().ok()?;
            if !(1..=12).contains(&n) {
                return None;
            }
            KeyCode::F(n)
        }
        _ => {
            let mut chars = last.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            KeyCode::Char(c)
        }
    };
    Some(KeySpec { code, mods })
}

#[derive(Clone, Debug)]
pub struct KeyMap {
    bindings: HashMap<Action, KeySpec>,
    pub conflicts: Vec<String>,
}

impl Default for KeyMap {
    fn default() -> Self {
        use Action::*;
        // Explicit list: `crossterm::KeyCode::Menu` would clash with `Action::Menu`.
        use KeyCode::{Char, Down, Enter, Esc, Left, PageDown, PageUp, Right, Tab, Up, F};
        let mut b: HashMap<Action, KeySpec> = HashMap::new();
        let ins = |b: &mut HashMap<Action, KeySpec>, a, k| {
            b.insert(a, k);
        };
        ins(&mut b, Menu, KeySpec::alt(Char('m')));
        ins(&mut b, MenuAdapter, KeySpec::new(F(1)));
        ins(&mut b, MenuDevice, KeySpec::new(F(2)));
        ins(&mut b, MenuView, KeySpec::new(F(3)));
        ins(&mut b, MenuHelp, KeySpec::new(F(4)));
        ins(&mut b, Select, KeySpec::new(Enter));
        ins(&mut b, Cancel, KeySpec::ctrl(Char('x')));
        ins(&mut b, Suspend, KeySpec::ctrl(Char('z')));
        ins(&mut b, Quit, KeySpec::new(Char('q')));
        ins(&mut b, Switch, KeySpec::new(Tab));
        ins(&mut b, Close, KeySpec::new(Esc));
        ins(&mut b, Help, KeySpec::new(Char('?')));
        ins(&mut b, NavigateUp, KeySpec::new(Up));
        ins(&mut b, NavigateDown, KeySpec::new(Down));
        ins(&mut b, NavigateTop, KeySpec::new(PageUp));
        ins(&mut b, NavigateBottom, KeySpec::new(PageDown));

        ins(&mut b, Power, KeySpec::new(Char('o')));
        ins(&mut b, Discoverable, KeySpec::alt(Char('s')));
        ins(&mut b, Pairable, KeySpec::alt(Char('p')));
        ins(&mut b, Scan, KeySpec::new(Char('s')));
        ins(&mut b, ChangeAdapter, KeySpec::new(Char('a')));
        ins(&mut b, ProgressView, KeySpec::new(Char('v')));
        ins(&mut b, PlayerHide, KeySpec::alt(Char('h')));
        ins(&mut b, PlayerShow, KeySpec::new(Char('m')));
        ins(&mut b, Connect, KeySpec::new(Char('c')));
        ins(&mut b, Pair, KeySpec::new(Char('p')));
        ins(&mut b, Trust, KeySpec::new(Char('t')));
        ins(&mut b, Block, KeySpec::new(Char('b')));
        ins(&mut b, SendFiles, KeySpec::new(Char('f')));
        ins(&mut b, Network, KeySpec::new(Char('n')));
        ins(&mut b, AudioProfiles, KeySpec::alt(Char('a')));
        ins(&mut b, Info, KeySpec::new(Char('i')));
        ins(&mut b, Remove, KeySpec::new(Char('d')));

        ins(&mut b, FileDirForward, KeySpec::new(Right));
        ins(&mut b, FileDirBack, KeySpec::new(Left));
        ins(&mut b, FileSelect, KeySpec::new(Char(' ')));
        ins(&mut b, FileInvertSelection, KeySpec::new(Char('a')));
        ins(&mut b, FileSelectAll, KeySpec::alt(Char('a')));
        ins(&mut b, FileRefresh, KeySpec::ctrl(Char('r')));
        ins(&mut b, FileToggleHidden, KeySpec::ctrl(Char('h')));
        ins(&mut b, FileConfirmSelection, KeySpec::ctrl(Char('s')));

        ins(&mut b, TransferSuspend, KeySpec::new(Char('z')));
        ins(&mut b, TransferResume, KeySpec::new(Char('g')));
        ins(&mut b, TransferCancel, KeySpec::new(Char('x')));

        ins(&mut b, PlayerTogglePlay, KeySpec::new(Char(' ')));
        ins(&mut b, PlayerNext, KeySpec::new(Char('>')));
        ins(&mut b, PlayerPrevious, KeySpec::new(Char('<')));
        ins(&mut b, PlayerSeekForward, KeySpec::new(Right));
        ins(&mut b, PlayerSeekBackward, KeySpec::new(Left));
        ins(&mut b, PlayerStop, KeySpec::new(Char(']')));
        ins(&mut b, PlayerVolumeUp, KeySpec::new(Char('+')));
        ins(&mut b, PlayerVolumeDown, KeySpec::new(Char('-')));
        ins(&mut b, PlayerShuffle, KeySpec::new(Char('s')));
        ins(&mut b, PlayerRepeat, KeySpec::new(Char('r')));

        ins(&mut b, Search, KeySpec::new(Char('/')));
        ins(&mut b, CycleSort, KeySpec::new(F(5)));
        ins(&mut b, ToggleDetails, KeySpec::new(F(6)));
        ins(&mut b, ThemePicker, KeySpec::new(F(7)));
        ins(&mut b, ShowLog, KeySpec::new(F(8)));
        ins(&mut b, About, KeySpec::new(F(9)));
        ins(&mut b, Refresh, KeySpec::ctrl(Char('r')));
        ins(&mut b, CommandPalette, KeySpec::ctrl(Char('p')));
        ins(&mut b, Language, KeySpec::new(Char('l')));

        let mut map = Self {
            bindings: b,
            conflicts: Vec::new(),
        };
        map.conflicts = map.detect_conflicts();
        map
    }
}

impl KeyMap {
    /// Overlay `[keybindings]` from the config. Unknown names are reported so
    /// a typo is visible instead of silently ignored.
    pub fn from_config(overrides: &HashMap<String, String>) -> Self {
        let mut map = KeyMap::default();
        let mut unknown = Vec::new();
        let mut invalid = Vec::new();
        for (name, keys) in overrides {
            let Some(action) = Action::from_name(name) else {
                unknown.push(name.clone());
                continue;
            };
            let Some(spec) = parse_key(keys) else {
                invalid.push(format!("{name} = {keys:?}"));
                continue;
            };
            map.bindings.insert(action, spec);
        }
        map.conflicts = map.detect_conflicts();
        for u in unknown {
            map.conflicts
                .push(format!("unknown keybinding action: {u}"));
        }
        for i in invalid {
            map.conflicts.push(format!("invalid keybinding: {i}"));
        }
        map
    }

    pub fn get(&self, action: Action) -> KeySpec {
        self.bindings[&action]
    }

    pub fn label(&self, action: Action) -> String {
        self.get(action).label()
    }

    pub fn is(&self, ev: &KeyEvent, action: Action) -> bool {
        matches(ev, &self.get(action))
    }

    /// Report duplicate keys *within the same context* (bluetuith does this
    /// at startup and refuses conflicting configs).
    fn detect_conflicts(&self) -> Vec<String> {
        let mut by_ctx: HashMap<Context, HashMap<KeySpec, Vec<Action>>> = HashMap::new();
        for action in Action::all() {
            // Group by the case-folded spec so `S` and `s` are seen as the
            // same key.
            let spec = normalize_spec(self.get(action));
            by_ctx
                .entry(action.context())
                .or_default()
                .entry(spec)
                .or_default()
                .push(action);
        }
        let mut out = Vec::new();
        for (ctx, keys) in by_ctx {
            for (spec, actions) in keys {
                if actions.len() > 1 {
                    let names: Vec<&str> = actions.iter().map(|a| a.name()).collect();
                    out.push(format!(
                        "{}: {} share '{}'",
                        ctx.label(),
                        names.join(", "),
                        spec.label()
                    ));
                }
            }
        }
        out.sort();
        out
    }

    pub fn rows(&self, lang: Lang) -> Vec<(Context, &'static str, String)> {
        Action::all()
            .into_iter()
            .map(|a| (a.context(), a.name_for(lang), self.label(a)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn parses_simple_and_modified_keys() {
        assert_eq!(parse_key("q").unwrap().code, KeyCode::Char('q'));
        assert_eq!(parse_key("Alt+m").unwrap().mods, KeyModifiers::ALT);
        assert_eq!(parse_key("Ctrl+x").unwrap().mods, KeyModifiers::CONTROL);
        assert_eq!(parse_key("Enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key("PgUp").unwrap().code, KeyCode::PageUp);
        assert_eq!(parse_key("F3").unwrap().code, KeyCode::F(3));
        assert!(parse_key("Ctrl+Alt+Delete+Extra").is_none());
    }

    #[test]
    fn letters_are_case_insensitive() {
        let map = KeyMap::default();
        // q and Q both quit.
        assert!(matches(
            &ev(KeyCode::Char('q'), KeyModifiers::NONE),
            &map.get(Action::Quit)
        ));
        assert!(matches(
            &ev(KeyCode::Char('Q'), KeyModifiers::NONE),
            &map.get(Action::Quit)
        ));
        assert!(matches(
            &ev(KeyCode::Char('Q'), KeyModifiers::SHIFT),
            &map.get(Action::Quit)
        ));
        // Uppercase S now means scan; discoverable is Alt+s.
        assert!(matches(
            &ev(KeyCode::Char('S'), KeyModifiers::NONE),
            &map.get(Action::Scan)
        ));
        assert!(!matches(
            &ev(KeyCode::Char('S'), KeyModifiers::NONE),
            &map.get(Action::Discoverable)
        ));
        assert!(matches(
            &ev(KeyCode::Char('s'), KeyModifiers::ALT),
            &map.get(Action::Discoverable)
        ));
    }

    #[test]
    fn default_bindings_have_no_conflicts() {
        let map = KeyMap::default();
        assert!(
            map.conflicts.is_empty(),
            "unexpected conflicts: {:?}",
            map.conflicts
        );
    }

    #[test]
    fn config_overrides_and_reports_unknown_actions() {
        let mut kv = HashMap::new();
        kv.insert("Quit".to_string(), "Ctrl+q".to_string());
        kv.insert("Bogus".to_string(), "x".to_string());
        let map = KeyMap::from_config(&kv);
        assert_eq!(map.get(Action::Quit).mods, KeyModifiers::CONTROL);
        assert!(map
            .conflicts
            .iter()
            .any(|c| c.contains("unknown keybinding")));
    }

    #[test]
    fn duplicate_keys_in_one_context_are_reported() {
        let mut kv = HashMap::new();
        kv.insert("Connect".to_string(), "p".to_string()); // clashes with Pair
        let map = KeyMap::from_config(&kv);
        assert!(map.conflicts.iter().any(|c| c.contains("share")));
    }
}
