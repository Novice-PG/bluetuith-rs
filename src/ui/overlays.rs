//! Every popup / full-screen view. Overlays are drawn on top of the base
//! frame, deepest first, so a stack (menu → confirm, etc.) composites cleanly.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use super::{centered_rect, modal, panel};
use crate::app::{App, Overlay};
use crate::backend::service_name;
use crate::keybind::Context;
use crate::model::*;
use crate::theme::Theme;
use crate::util;

fn boxed(frame: &mut Frame<'_>, app: &App, title: &str, w: u16, h: u16) -> Rect {
    let area = centered_rect(w, h, frame.area());
    modal(frame, area, title, app)
}

fn footer<'a>(app: &App, hints: &[(&str, &str)]) -> Line<'a> {
    let t = &app.theme;
    let mut v = vec![Span::raw(" ")];
    for (key, label) in hints {
        v.push(Span::styled(
            format!(" {key} "),
            Style::new()
                .fg(t.accent_fg)
                .bg(t.accent_soft)
                .add_modifier(Modifier::BOLD),
        ));
        v.push(Span::styled(format!(" {label}  "), Style::new().fg(t.dim)));
    }
    Line::from(v)
}

/// Render a scrollable 1-line-per-item list with a highlighted selection.
fn list(
    frame: &mut Frame<'_>,
    inner: Rect,
    lines: Vec<Line<'static>>,
    index: usize,
    app: &App,
    footer_line: Line<'static>,
) {
    let t = &app.theme;
    let h = inner.height.saturating_sub(2) as usize;
    let start = if index >= h { index - h + 1 } else { 0 };
    for i in 0..h {
        let idx = start + i;
        let Some(line) = lines.get(idx) else { break };
        let r = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };
        let style = if idx == index {
            Style::new().bg(t.selection_bg)
        } else if idx % 2 == 1 {
            Style::new().bg(t.surface_alt)
        } else {
            Style::new()
        };
        frame.render_widget(Paragraph::new(line.clone()).style(style), r);
    }
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(Paragraph::new(footer_line), fr);
}

pub fn render(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let overlays = app.overlays.clone();
    for o in overlays {
        match o {
            Overlay::Menu => menu(frame, app, area),
            Overlay::Help => help(frame, app),
            Overlay::Confirm => confirm(frame, app),
            Overlay::AdapterChange => adapter_change(frame, app),
            Overlay::DeviceInfo => device_info(frame, app),
            Overlay::FilePicker => file_picker(frame, app),
            Overlay::Progress => progress(frame, app),
            Overlay::Player => player(frame, app),
            Overlay::Network => network(frame, app),
            Overlay::Profiles => profiles(frame, app),
            Overlay::Auth => auth(frame, app),
            Overlay::ThemePicker => theme_picker(frame, app),
            Overlay::Log => log_view(frame, app),
            Overlay::CommandPalette => command_palette(frame, app),
            Overlay::About => about(frame, app),
        }
    }
}

// ---------------------------------------------------------------------------

fn menu(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let t = app.theme.clone();
    let items = app.menu_items.clone();
    let max_label = items.iter().map(|i| i.label.len()).max().unwrap_or(10);
    let max_hint = items.iter().map(|i| i.hint.len()).max().unwrap_or(4);
    let w = (max_label + max_hint + 10) as u16;
    let w = w.clamp(32, 52).min(area.width.saturating_sub(2));
    let h = (items.len() as u16 + 2).min(area.height.saturating_sub(3));
    let x = app.layout.menu_rects[app.menu_id.index()]
        .x
        .min(area.width.saturating_sub(w));
    let y = app.layout.menubar.y + 1;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, rect);
    let block = panel(app.menu_id.label(), &t, true);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    for (i, it) in items.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let r = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };
        let fg = if !it.enabled {
            t.dim
        } else if i == app.menu_index {
            t.selection_fg
        } else {
            t.fg
        };
        let gap = (inner.width as usize).saturating_sub(it.label.len() + it.hint.len() + 4);
        let line = Line::from(vec![
            Span::styled(
                if i == app.menu_index { " ▸ " } else { "   " },
                Style::new().fg(t.accent),
            ),
            Span::styled(
                it.label.clone(),
                Style::new().fg(fg).add_modifier(if i == app.menu_index {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::raw(" ".repeat(gap.max(1))),
            Span::styled(
                it.hint.clone(),
                Style::new().fg(if it.enabled { t.accent } else { t.dim }),
            ),
            Span::raw(" "),
        ]);
        let style = if i == app.menu_index {
            Style::new().bg(t.selection_bg)
        } else {
            Style::new()
        };
        frame.render_widget(Paragraph::new(line).style(style), r);
    }
}

// ---------------------------------------------------------------------------

fn help(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Help.title(), 78, 30);
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(vec![
            Span::styled(
                "  bluetuith-rs ",
                Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("v{}  ", env!("CARGO_PKG_VERSION")),
                Style::new().fg(t.dim),
            ),
            Span::styled("— keyboard reference", Style::new().fg(t.muted)),
        ]),
        Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::new().fg(t.border),
        )),
        Line::raw(""),
    ];
    let mut current = Context::App;
    for (ctx, name, key) in app.keys.rows() {
        if ctx != current {
            current = ctx;
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                format!("  {}", ctx.label()),
                Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
            )));
        }
        lines.push(Line::from(vec![
            Span::styled(format!("    {:<24}", name), Style::new().fg(t.fg)),
            Span::styled(format!(" {}", key), Style::new().fg(t.muted)),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  Override any binding in ~/.config/bluetuith-rs/config.toml under [keybindings].",
        Style::new().fg(t.dim),
    )));
    lines.push(Line::from(Span::styled(
        "  Mouse: click a menu or a device row, double-click to connect, scroll to navigate.",
        Style::new().fg(t.dim),
    )));

    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((app.overlay_scroll as u16, 0)),
        body,
    );
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(footer(app, &[("↑↓", "scroll"), ("Esc", "close")]), fr);
}

// ---------------------------------------------------------------------------

fn confirm(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Confirm.title(), 52, 7);
    let text = app
        .confirm
        .as_ref()
        .map(|c| c.text.clone())
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            text,
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 2,
        },
    );
    frame.render_widget(
        footer(app, &[("Y / Enter", "confirm"), ("N / Esc", "cancel")]),
        Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(2),
            width: inner.width,
            height: 2,
        },
    );
}

// ---------------------------------------------------------------------------

fn adapter_change(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::AdapterChange.title(), 64, 12);
    let lines: Vec<Line<'static>> = app
        .adapters
        .iter()
        .map(|a| {
            Line::from(vec![
                Span::styled(
                    if a.powered { " ● " } else { " ○ " },
                    Style::new().fg(if a.powered { t.success } else { t.dim }),
                ),
                Span::styled(
                    util::pad(&a.display_name(), 22),
                    Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(util::pad(&a.address, 18), Style::new().fg(t.muted)),
                Span::styled(format!(" {} ", a.id), Style::new().fg(t.accent)),
            ])
        })
        .collect();
    list(
        frame,
        inner,
        lines,
        app.overlay_index,
        app,
        footer(
            app,
            &[("↑↓", "select"), ("Enter", "use adapter"), ("Esc", "close")],
        ),
    );
}

// ---------------------------------------------------------------------------

fn device_info(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::DeviceInfo.title(), 76, 28);
    let Some(d) = app.selected_device().cloned() else {
        return;
    };
    let row = |k: &str, v: String, color: ratatui::style::Color| {
        Line::from(vec![
            Span::styled(format!("  {:<22}", k), Style::new().fg(t.muted)),
            Span::styled(v, Style::new().fg(color)),
        ])
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("  {} ", d.kind().glyph()),
                Style::new().fg(t.accent),
            ),
            Span::styled(
                d.display_name(),
                Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::new().fg(t.border),
        )),
        row("Address", d.address.clone(), t.fg),
        row("Adapter", d.adapter.clone(), t.muted),
        row("Type", d.kind().label().into(), t.fg),
        row("Icon", d.icon.clone(), t.muted),
        row(
            "Class",
            d.class
                .map(|c| format!("0x{c:06X}"))
                .unwrap_or_else(|| "—".into()),
            t.fg,
        ),
        row(
            "Appearance",
            d.appearance
                .map(|a| a.to_string())
                .unwrap_or_else(|| "—".into()),
            t.fg,
        ),
        row("Modalias", d.modalias.clone(), t.fg),
        row(
            "RSSI",
            d.rssi
                .map(|r| format!("{r} dBm"))
                .unwrap_or_else(|| "—".into()),
            signal_color(d.signal_bars(), &t),
        ),
        row(
            "Tx power",
            d.tx_power
                .map(|r| format!("{r} dBm"))
                .unwrap_or_else(|| "—".into()),
            t.fg,
        ),
        row(
            "Battery",
            d.battery
                .map(|p| format!("{p}%"))
                .unwrap_or_else(|| "—".into()),
            t.success,
        ),
        row("Paired", yesno(d.paired), t.paired),
        row("Connected", yesno(d.connected), t.connected),
        row("Trusted", yesno(d.trusted), t.trusted),
        row("Blocked", yesno(d.blocked), t.blocked),
        row("Legacy pairing", yesno(d.legacy_pairing), t.muted),
        row("Services resolved", yesno(d.services_resolved), t.muted),
        Line::raw(""),
        Line::from(Span::styled(
            "  UUIDs / services",
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
    ];
    for u in &d.uuids {
        let short = if u.len() == 36 { &u[4..8] } else { u.as_str() };
        lines.push(Line::from(vec![
            Span::styled("   • ", Style::new().fg(t.accent_soft)),
            Span::styled(util::pad(service_name(short), 24), Style::new().fg(t.muted)),
            Span::styled(u.clone(), Style::new().fg(t.dim)),
        ]));
    }
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((app.overlay_scroll as u16, 0)),
        body,
    );
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(footer(app, &[("↑↓", "scroll"), ("Esc", "close")]), fr);
}

fn yesno(b: bool) -> String {
    if b {
        "yes".into()
    } else {
        "no".into()
    }
}

fn signal_color(bars: u8, t: &Theme) -> ratatui::style::Color {
    match bars {
        0 | 1 => t.error,
        2 => t.warning,
        3 => t.accent,
        _ => t.success,
    }
}

// ---------------------------------------------------------------------------

fn file_picker(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::FilePicker.title(), 78, 26);
    let Some(fp) = app.file_picker.clone() else {
        return;
    };
    let cwd = fp.cwd.display().to_string();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::new()),
            Span::styled("▸ ", Style::new().fg(t.accent)),
            Span::styled(
                util::truncate(&cwd, inner.width as usize - 6),
                Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
        ])),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );
    let body = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(3),
    };
    let h = body.height as usize;
    let start = if fp.index >= h { fp.index - h + 1 } else { 0 };
    for i in 0..h {
        let idx = start + i;
        let Some(path) = fp.entries.get(idx) else {
            break;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_dir = path.is_dir();
        let checked = fp.selected.contains(path);
        let r = Rect {
            x: body.x,
            y: body.y + i as u16,
            width: body.width,
            height: 1,
        };
        let line = Line::from(vec![
            Span::styled(
                if idx == fp.index { " ▸ " } else { "   " },
                Style::new().fg(t.accent),
            ),
            Span::styled(
                if checked { "[×] " } else { "[ ] " },
                Style::new().fg(if checked { t.success } else { t.dim }),
            ),
            Span::styled(
                if is_dir { "▸ " } else { "  " },
                Style::new().fg(t.accent_soft),
            ),
            Span::styled(
                util::pad(&format!("{}{}", name, if is_dir { "/" } else { "" }), 40),
                Style::new()
                    .fg(if is_dir { t.accent } else { t.fg })
                    .add_modifier(if is_dir {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(
                if is_dir {
                    "<DIR>".into()
                } else {
                    util::human_bytes(std::fs::metadata(path).map(|m| m.len()).unwrap_or(0))
                },
                Style::new().fg(t.dim),
            ),
        ]);
        let style = if idx == fp.index {
            Style::new().bg(t.selection_bg)
        } else {
            Style::new()
        };
        frame.render_widget(Paragraph::new(line).style(style), r);
    }
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(
            app,
            &[
                ("Space", "select"),
                ("a", "invert"),
                ("A", "all"),
                ("←→", "dir"),
                ("Ctrl+s", "send"),
                ("Esc", "close"),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn bar<'a>(ratio: f64, width: usize, t: &Theme) -> Vec<Span<'a>> {
    let filled = (ratio.clamp(0.0, 1.0) * width as f64).round() as usize;
    vec![
        Span::styled("█".repeat(filled), Style::new().fg(t.accent)),
        Span::styled(
            "░".repeat(width.saturating_sub(filled)),
            Style::new().fg(t.gauge_empty),
        ),
    ]
}

fn progress(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Progress.title(), 96, 22);
    if app.transfers.is_empty() {
        frame.render_widget(
            Paragraph::new("No transfers yet — use 'Send Files' on a device")
                .alignment(Alignment::Center)
                .style(Style::new().fg(t.muted)),
            inner,
        );
    }
    let lines: Vec<Line<'static>> = app
        .transfers
        .iter()
        .map(|tr| {
            let mut v = vec![
                Span::styled(
                    if tr.direction == Direction::Outgoing {
                        " ↑ "
                    } else {
                        " ↓ "
                    },
                    Style::new().fg(t.info),
                ),
                Span::styled(
                    util::pad(&tr.name, 22),
                    Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(util::pad(&tr.device_name, 16), Style::new().fg(t.muted)),
            ];
            v.extend(bar(tr.progress(), 12, &t));
            v.push(Span::styled(
                format!(" {:>5.1}% ", tr.progress() * 100.0),
                Style::new().fg(t.accent),
            ));
            v.push(Span::styled(
                format!("{:<9}", tr.status.label()),
                Style::new().fg(match tr.status {
                    TransferStatus::Complete => t.success,
                    TransferStatus::Error | TransferStatus::Cancelled => t.error,
                    TransferStatus::Suspended => t.warning,
                    _ => t.info,
                }),
            ));
            v.push(Span::raw(" "));
            v.push(Span::styled(
                format!(
                    "{}/{}",
                    util::human_bytes(tr.transferred),
                    util::human_bytes(tr.size)
                ),
                Style::new().fg(t.dim),
            ));
            Line::from(v)
        })
        .collect();
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    list(frame, body, lines, app.overlay_index, app, Line::raw(""));
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(
            app,
            &[
                ("↑↓", "select"),
                ("z", "suspend"),
                ("g", "resume"),
                ("x", "cancel"),
                ("Esc", "close"),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn centered_line(frame: &mut Frame<'_>, inner: Rect, line: Line<'static>, y: u16) {
    frame.render_widget(
        Paragraph::new(line).alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: inner.y + y,
            width: inner.width,
            height: 1,
        },
    );
}

fn player(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Player.title(), 64, 16);
    let Some(p) = app.player.clone() else {
        frame.render_widget(
            Paragraph::new("No media player is available")
                .alignment(Alignment::Center)
                .style(Style::new().fg(t.muted)),
            inner,
        );
        return;
    };
    centered_line(
        frame,
        inner,
        Line::from(Span::styled(
            "♪  NOW PLAYING",
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
        0,
    );
    centered_line(
        frame,
        inner,
        Line::from(Span::styled(
            p.title.clone(),
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        )),
        2,
    );
    centered_line(
        frame,
        inner,
        Line::from(Span::styled(p.artist.clone(), Style::new().fg(t.muted))),
        3,
    );
    centered_line(
        frame,
        inner,
        Line::from(Span::styled(p.album.clone(), Style::new().fg(t.dim))),
        4,
    );
    centered_line(
        frame,
        inner,
        Line::from(Span::styled(
            format!("{}  ·  track {}/{}", p.genre, p.track_number, p.track_count),
            Style::new().fg(t.dim),
        )),
        5,
    );

    frame.render_widget(
        Gauge::default()
            .ratio(p.progress())
            .gauge_style(Style::new().fg(t.accent).bg(t.gauge_empty))
            .label(format!(
                "{} / {}",
                util::duration(p.position_ms),
                util::duration(p.duration_ms)
            )),
        Rect {
            x: inner.x + 2,
            y: inner.y + 7,
            width: inner.width.saturating_sub(4),
            height: 1,
        },
    );
    centered_line(
        frame,
        inner,
        Line::from(vec![
            Span::styled("  |<  ", Style::new().fg(t.fg)),
            Span::styled(
                if p.status == PlayStatus::Playing {
                    " ▮▮  "
                } else {
                    " ▶   "
                },
                Style::new().fg(t.accent),
            ),
            Span::styled(" >|   ■  ", Style::new().fg(t.fg)),
            Span::styled(
                format!("{} {}", if p.shuffle { "↔" } else { "→" }, p.repeat.glyph()),
                Style::new().fg(t.muted),
            ),
        ]),
        9,
    );
    centered_line(
        frame,
        inner,
        Line::from(Span::styled(
            format!(
                "{}  ·  {}  ·  volume {}%  ·  eq {}",
                p.status, p.device_name, p.volume, p.equalizer
            ),
            Style::new().fg(t.dim),
        )),
        10,
    );
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(
            app,
            &[
                ("Space", "play/pause"),
                ("< >", "track"),
                ("←→", "seek"),
                ("+ -", "volume"),
                ("]", "stop"),
                ("M", "hide"),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn network(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Network.title(), 74, 16);
    let roles = [NetworkRole::Panu, NetworkRole::Nap, NetworkRole::Dun];
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, role) in roles.iter().enumerate() {
        let active = app.networks.iter().any(|n| n.role == *role && n.connected);
        lines.push(Line::from(vec![
            Span::styled(
                if app.overlay_index == i {
                    " ▸ "
                } else {
                    "   "
                },
                Style::new().fg(t.accent),
            ),
            Span::styled(
                util::pad(role.label(), 22),
                Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(util::pad(role.uuid(), 8), Style::new().fg(t.dim)),
            Span::styled(
                if active {
                    "● connected"
                } else {
                    "○ available"
                },
                Style::new().fg(if active { t.success } else { t.dim }),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  Active connections",
        Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
    )));
    if app.networks.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (none)",
            Style::new().fg(t.dim),
        )));
    }
    for n in &app.networks {
        lines.push(Line::from(vec![
            Span::styled("   • ", Style::new().fg(t.accent_soft)),
            Span::styled(util::pad(&n.device_name, 20), Style::new().fg(t.fg)),
            Span::styled(util::pad(n.role.label(), 22), Style::new().fg(t.muted)),
            Span::styled(n.interface.clone(), Style::new().fg(t.info)),
        ]));
    }
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    list(frame, body, lines, app.overlay_index, app, Line::raw(""));
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(
            app,
            &[
                ("↑↓", "select role"),
                ("Enter / c", "connect"),
                ("d", "disconnect"),
                ("Esc", "close"),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn profiles(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Profiles.title(), 72, 16);
    let lines: Vec<Line<'static>> = if app.profiles.is_empty() {
        vec![Line::from(Span::styled(
            "  No audio profiles advertised by this device",
            Style::new().fg(t.muted),
        ))]
    } else {
        app.profiles
            .iter()
            .map(|p| {
                Line::from(vec![
                    Span::styled(
                        if p.active { " ● " } else { " ○ " },
                        Style::new().fg(if p.active { t.success } else { t.dim }),
                    ),
                    Span::styled(
                        util::pad(&p.name, 18),
                        Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(util::pad(&p.description, 32), Style::new().fg(t.muted)),
                    Span::styled(p.device.clone(), Style::new().fg(t.dim)),
                ])
            })
            .collect()
    };
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    list(frame, body, lines, app.overlay_index, app, Line::raw(""));
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(
            app,
            &[("↑↓", "select"), ("Enter", "toggle"), ("Esc", "close")],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn auth(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Auth.title(), 60, 11);
    let Some(req) = app.auth.clone() else { return };
    let (heading, detail) = match &req.kind {
        AuthKind::Confirmation(passkey) => (
            "Confirm pairing".to_string(),
            format!("Verify the code {passkey:06} on both devices"),
        ),
        AuthKind::Authorization => (
            "Authorize connection".into(),
            "Allow this device to connect?".into(),
        ),
        AuthKind::PinCode => (
            "Enter PIN code".into(),
            "Type the PIN shown on the device".into(),
        ),
        AuthKind::Passkey(k) => (
            "Enter passkey".into(),
            format!("Expected up to 6 digits (shown: {k})"),
        ),
        AuthKind::DisplayPinCode(pin) => (
            "Pairing PIN".into(),
            format!("Enter this PIN on the device: {pin}"),
        ),
        AuthKind::DisplayPasskey(k) => (
            "Pairing passkey".into(),
            format!("Enter {k:06} on the device"),
        ),
    };
    let lines = vec![
        Line::from(Span::styled(
            format!("  {}", req.device_name),
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("  {heading}"),
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            format!("  {detail}"),
            Style::new().fg(t.muted),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  > ", Style::new().fg(t.accent)),
            Span::styled(
                if app.auth_input.is_empty() {
                    "▏".into()
                } else {
                    app.auth_input.clone()
                },
                Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(app, &[("Enter", "accept / submit"), ("Esc", "reject")]),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn theme_picker(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::ThemePicker.title(), 48, 15);
    let themes = Theme::catalogue();
    let lines: Vec<Line<'static>> = themes
        .iter()
        .map(|th| {
            Line::from(vec![
                Span::styled(
                    if th.name == app.config.theme {
                        " ● "
                    } else {
                        "   "
                    },
                    Style::new().fg(t.accent),
                ),
                Span::styled(
                    util::pad(th.name, 16),
                    Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled("████", Style::new().fg(th.accent)),
                Span::styled("████", Style::new().fg(th.success)),
                Span::styled("████", Style::new().fg(th.warning)),
                Span::styled("████", Style::new().fg(th.error)),
            ])
        })
        .collect();
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    list(frame, body, lines, app.overlay_index, app, Line::raw(""));
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(
            app,
            &[("↑↓", "select"), ("Enter", "apply"), ("Esc", "close")],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn log_view(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::Log.title(), 84, 24);
    let lines: Vec<Line<'static>> = app
        .log
        .iter()
        .map(|l| {
            let color = if l.contains("[Error]") {
                t.error
            } else if l.contains("[Warning]") {
                t.warning
            } else if l.contains("[Success]") {
                t.success
            } else {
                t.muted
            };
            Line::from(Span::styled(
                util::truncate(l, inner.width as usize),
                Style::new().fg(color),
            ))
        })
        .collect();
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((app.overlay_scroll as u16, 0)),
        body,
    );
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(footer(app, &[("↑↓", "scroll"), ("Esc", "close")]), fr);
}

// ---------------------------------------------------------------------------

fn command_palette(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::CommandPalette.title(), 62, 22);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ❯ ", Style::new().fg(t.accent)),
            Span::styled(
                app.palette_query.clone(),
                Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled("▏", Style::new().fg(t.accent)),
        ])),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );
    let body = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(3),
    };
    let lines: Vec<Line<'static>> = app
        .palette_items
        .iter()
        .map(|a| {
            Line::from(vec![
                Span::styled(format!("   {:<30}", a.name()), Style::new().fg(t.fg)),
                Span::styled(app.keys.label(*a), Style::new().fg(t.accent)),
                Span::styled(
                    format!("   {}", a.context().label()),
                    Style::new().fg(t.dim),
                ),
            ])
        })
        .collect();
    list(frame, body, lines, app.overlay_index, app, Line::raw(""));
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    frame.render_widget(
        footer(
            app,
            &[
                ("type", "filter"),
                ("↑↓", "select"),
                ("Enter", "run"),
                ("Esc", "close"),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn about(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let inner = boxed(frame, app, Overlay::About.title(), 62, 18);
    let features = app.features;
    let lines = vec![
        Line::from(vec![
            Span::styled("  ◆ ", Style::new().fg(t.accent)),
            Span::styled(
                "bluetuith-rs",
                Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  v{}", env!("CARGO_PKG_VERSION")),
                Style::new().fg(t.dim),
            ),
        ]),
        Line::raw(""),
        Line::from(Span::styled(
            "  A full-featured Bluetooth manager for the terminal.",
            Style::new().fg(t.fg),
        )),
        Line::from(Span::styled(
            "  Written in Rust with ratatui; BlueZ over D-Bus.",
            Style::new().fg(t.muted),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "  Session",
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("   backend      {}", app.backend.label()),
            Style::new().fg(t.muted),
        )),
        Line::from(Span::styled(
            format!("   adapters     {}", app.adapters.len()),
            Style::new().fg(t.muted),
        )),
        Line::from(Span::styled(
            format!("   devices      {}", app.devices.len()),
            Style::new().fg(t.muted),
        )),
        Line::from(Span::styled(
            format!("   theme        {}", app.config.theme),
            Style::new().fg(t.muted),
        )),
        Line::from(Span::styled(
            format!("   OBEX         {}", yesno(features.obex)),
            Style::new().fg(t.muted),
        )),
        Line::from(Span::styled(
            format!("   media        {}", yesno(features.media_player)),
            Style::new().fg(t.muted),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "  Apache-2.0 licensed · inspired by bluetuith",
            Style::new().fg(t.dim),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
