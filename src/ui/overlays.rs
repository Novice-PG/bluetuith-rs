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
use crate::i18n::{fill, Lang, Tr};
use crate::keybind::{Action, Context};
use crate::model::*;
use crate::theme::Theme;
use crate::util;

fn boxed(frame: &mut Frame<'_>, app: &mut App, title: &str, w: u16, h: u16) -> Rect {
    let area = centered_rect(w, h, frame.area());
    app.layout.overlay_rect = Some(area);
    modal(frame, area, title, app)
}

/// The live key label for an action (so footers follow user rebindings).
fn key(app: &App, a: Action) -> String {
    app.keys.label(a)
}

/// `↑↓` (or whatever the up/down keys are bound to).
fn nav_keys(app: &App) -> String {
    format!(
        "{}{}",
        key(app, Action::NavigateUp),
        key(app, Action::NavigateDown)
    )
}

/// Localised description for a known audio profile (the backend emits
/// English text, so map by profile name here).
fn profile_desc(name: &str, lang: Lang) -> &'static str {
    let tr = lang.tr();
    match name {
        "A2DP Source" => tr.pf_d_a2dp_src,
        "A2DP Sink" => tr.pf_d_a2dp_sink,
        "HSP" => tr.pf_d_hsp,
        "HFP" => tr.pf_d_hfp,
        "AVRCP" => tr.pf_d_avrcp,
        _ => "",
    }
}

/// Clamp the scroll offset to the content so scrolling past the end cannot
/// leave an empty window, and write the corrected value back to the app.
fn clamp_scroll(app: &mut App, nlines: usize, height: u16) -> u16 {
    let max_off = nlines.saturating_sub(height as usize);
    let off = app.overlay_scroll.min(max_off);
    app.overlay_scroll = off;
    off as u16
}

/// How many terminal rows a wrapped `Paragraph` actually occupies. Using the
/// raw `lines.len()` undercounts wrapped long lines, which would make the
/// scroll clamp stop before the last lines are reachable.
fn wrapped_lines(lines: &[Line<'_>], width: u16) -> usize {
    let w = (width as usize).max(1);
    lines
        .iter()
        .map(|l| {
            let cols: usize = l
                .spans
                .iter()
                .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
                .sum();
            cols.div_ceil(w).max(1)
        })
        .sum()
}

fn footer<'a>(app: &App, hints: &[(String, &'static str)]) -> Line<'a> {
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
    app: &mut App,
    footer_line: Line<'static>,
) {
    let t = app.theme.clone();
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
        app.layout.overlay_items.push((r, idx));
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
        // Each overlay resets the hit-test regions; the topmost (drawn last)
        // therefore owns the mouse.
        app.layout.overlay_rect = None;
        app.layout.overlay_items.clear();
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
    let max_label = items
        .iter()
        .map(|i| unicode_width::UnicodeWidthStr::width(i.label.as_str()))
        .max()
        .unwrap_or(10);
    let max_hint = items
        .iter()
        .map(|i| unicode_width::UnicodeWidthStr::width(i.hint.as_str()))
        .max()
        .unwrap_or(4);
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
    let block = panel(app.menu_id.label(app.lang), &t, true);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    app.layout.overlay_rect = Some(rect);

    // Menus can be taller than the terminal (e.g. the 10-item Adapter menu on
    // a 40x12 screen). Scroll the list so the highlighted row is always
    // visible and hit-testable, instead of silently hiding the last items.
    let h = inner.height as usize;
    let start = if app.menu_index >= h {
        app.menu_index - h + 1
    } else {
        0
    };
    for i in 0..h {
        let idx = start + i;
        let Some(it) = items.get(idx) else { break };
        let r = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };
        let fg = if !it.enabled {
            t.dim
        } else if idx == app.menu_index {
            t.selection_fg
        } else {
            t.fg
        };
        let gap = (inner.width as usize).saturating_sub(
            unicode_width::UnicodeWidthStr::width(it.label.as_str())
                + unicode_width::UnicodeWidthStr::width(it.hint.as_str())
                + 4,
        );
        let line = Line::from(vec![
            Span::styled(
                if idx == app.menu_index {
                    " ▸ "
                } else {
                    "   "
                },
                Style::new().fg(t.accent),
            ),
            Span::styled(
                it.label.clone(),
                Style::new().fg(fg).add_modifier(if idx == app.menu_index {
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
        let style = if idx == app.menu_index {
            Style::new().bg(t.selection_bg)
        } else {
            Style::new()
        };
        frame.render_widget(Paragraph::new(line).style(style), r);
        app.layout.overlay_items.push((r, idx));
    }
}

// ---------------------------------------------------------------------------

fn help(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Help.title(app.lang), 78, 30);
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
            Span::styled(tr.hv_ref, Style::new().fg(t.muted)),
        ]),
        Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::new().fg(t.border),
        )),
        Line::raw(""),
    ];
    let mut current: Option<Context> = None;
    for (ctx, name, key) in app.keys.rows(app.lang) {
        if Some(ctx) != current {
            current = Some(ctx);
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                format!("  {}", ctx.label_for(app.lang)),
                Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
            )));
        }
        lines.push(Line::from(vec![
            Span::styled(
                format!("    {}", util::pad(name, 24)),
                Style::new().fg(t.fg),
            ),
            Span::styled(format!(" {}", key), Style::new().fg(t.muted)),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        tr.hv_override,
        Style::new().fg(t.dim),
    )));
    lines.push(Line::from(Span::styled(
        tr.hv_mouse,
        Style::new().fg(t.dim),
    )));

    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    let off = clamp_scroll(app, wrapped_lines(&lines, body.width), body.height);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((off, 0)),
        body,
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
                (nav_keys(app), tr.f_scroll),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn confirm(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Confirm.title(app.lang), 52, 7);
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
        footer(
            app,
            &[
                (format!("Y / {}", key(app, Action::Select)), tr.f_confirm),
                (format!("N / {}", key(app, Action::Close)), tr.f_cancel),
            ],
        ),
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
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::AdapterChange.title(app.lang), 64, 12);
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
            &[
                (nav_keys(app), tr.f_select),
                (key(app, Action::Select), tr.f_use_adapter),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
    );
}

// ---------------------------------------------------------------------------

fn device_info(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::DeviceInfo.title(app.lang), 76, 28);
    let Some(d) = app.selected_device().cloned() else {
        return;
    };
    let row = |k: &str, v: String, color: ratatui::style::Color| {
        Line::from(vec![
            Span::styled(format!("  {}", util::pad(k, 22)), Style::new().fg(t.muted)),
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
        row(tr.d_address, d.address.clone(), t.fg),
        row(tr.d_adapter, d.adapter.clone(), t.muted),
        row(tr.d_type, d.kind().label(app.lang).into(), t.fg),
        row(tr.d_icon, d.icon.clone(), t.muted),
        row(
            tr.d_class,
            d.class
                .map(|c| format!("0x{c:06X}"))
                .unwrap_or_else(|| "—".into()),
            t.fg,
        ),
        row(
            tr.d_appearance,
            d.appearance
                .map(|a| a.to_string())
                .unwrap_or_else(|| "—".into()),
            t.fg,
        ),
        row(tr.d_modalias, d.modalias.clone(), t.fg),
        row(
            tr.d_rssi,
            d.rssi
                .map(|r| format!("{r} dBm"))
                .unwrap_or_else(|| "—".into()),
            util::signal_color(d.signal_bars(), &t),
        ),
        row(
            tr.d_txpower,
            d.tx_power
                .map(|r| format!("{r} dBm"))
                .unwrap_or_else(|| "—".into()),
            t.fg,
        ),
        row(
            tr.d_battery,
            d.battery
                .map(|p| format!("{p}%"))
                .unwrap_or_else(|| "—".into()),
            t.success,
        ),
        row(
            tr.d_paired,
            format!(
                "{} {}",
                if d.paired { "◆" } else { "◇" },
                if d.paired { tr.yes } else { tr.no }
            ),
            t.paired,
        ),
        row(
            tr.d_connected,
            format!(
                "{} {}",
                if d.connected { "●" } else { "○" },
                if d.connected { tr.yes } else { tr.no }
            ),
            t.connected,
        ),
        row(
            tr.d_trusted,
            format!(
                "{} {}",
                if d.trusted { "★" } else { "☆" },
                if d.trusted { tr.yes } else { tr.no }
            ),
            t.trusted,
        ),
        row(
            tr.d_blocked,
            format!(
                "{} {}",
                if d.blocked { "⊘" } else { "·" },
                if d.blocked { tr.yes } else { tr.no }
            ),
            t.blocked,
        ),
        row(tr.d_legacy, yesno(d.legacy_pairing, &tr), t.muted),
        row(tr.d_resolved, yesno(d.services_resolved, &tr), t.muted),
        Line::raw(""),
        Line::from(Span::styled(
            format!("  {}", tr.d_uuids),
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
    ];
    for u in &d.uuids {
        let short = if u.len() == 36 { &u[4..8] } else { u.as_str() };
        lines.push(Line::from(vec![
            Span::styled("   • ", Style::new().fg(t.accent_soft)),
            Span::styled(
                util::pad(service_name(short, app.lang), 24),
                Style::new().fg(t.muted),
            ),
            Span::styled(u.clone(), Style::new().fg(t.dim)),
        ]));
    }
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    let off = clamp_scroll(app, wrapped_lines(&lines, body.width), body.height);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((off, 0)),
        body,
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
                (nav_keys(app), tr.f_scroll),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        fr,
    );
}

fn yesno(b: bool, tr: &Tr) -> String {
    if b {
        tr.yes.into()
    } else {
        tr.no.into()
    }
}

// ---------------------------------------------------------------------------

fn file_picker(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::FilePicker.title(app.lang), 78, 26);
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
                    tr.fp_dir.into()
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
        app.layout.overlay_items.push((r, idx));
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
                (key(app, Action::FileSelect), tr.f_select),
                (key(app, Action::FileInvertSelection), tr.f_invert),
                (key(app, Action::FileSelectAll), tr.f_all),
                (
                    format!(
                        "{}{}",
                        key(app, Action::FileDirBack),
                        key(app, Action::FileDirForward)
                    ),
                    tr.f_dir,
                ),
                (key(app, Action::FileConfirmSelection), tr.f_send),
                (key(app, Action::Close), tr.f_close),
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
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Progress.title(app.lang), 96, 22);
    if app.transfers.is_empty() {
        frame.render_widget(
            Paragraph::new(tr.pr_none)
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
                format!("{:<9}", tr.status.label(app.lang)),
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
                (nav_keys(app), tr.f_select),
                (key(app, Action::TransferSuspend), tr.f_suspend),
                (key(app, Action::TransferResume), tr.f_resume),
                (key(app, Action::TransferCancel), tr.f_cancel),
                (key(app, Action::Close), tr.f_close),
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
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Player.title(app.lang), 84, 16);
    let Some(p) = app.player.clone() else {
        frame.render_widget(
            Paragraph::new(tr.p_no_player)
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
            tr.p_now_playing,
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
            format!(
                "{}  ·  {} {}/{}",
                p.genre, tr.p_track, p.track_number, p.track_count
            ),
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
                "{}  ·  {}  ·  {} {}  ·  {} {}",
                p.status.label(app.lang),
                p.device_name,
                tr.p_vol_prefix,
                p.volume,
                tr.p_eq,
                p.equalizer
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
    // The footer area is two rows tall; use both so shuffle/repeat and close
    // hints fit instead of being clipped off the right edge.
    let row_a = Rect {
        x: fr.x,
        y: fr.y,
        width: fr.width,
        height: 1,
    };
    let row_b = Rect {
        x: fr.x,
        y: fr.y + 1,
        width: fr.width,
        height: 1,
    };
    frame.render_widget(
        footer(
            app,
            &[
                (key(app, Action::PlayerTogglePlay), tr.f_play_pause),
                (
                    format!(
                        "{} {}",
                        key(app, Action::PlayerPrevious),
                        key(app, Action::PlayerNext)
                    ),
                    tr.f_track,
                ),
                (
                    format!(
                        "{}{}",
                        key(app, Action::PlayerSeekBackward),
                        key(app, Action::PlayerSeekForward)
                    ),
                    tr.f_seek,
                ),
                (
                    format!(
                        "{} {}",
                        key(app, Action::PlayerVolumeUp),
                        key(app, Action::PlayerVolumeDown)
                    ),
                    tr.f_volume,
                ),
            ],
        ),
        row_a,
    );
    frame.render_widget(
        footer(
            app,
            &[
                (key(app, Action::PlayerStop), tr.f_stop),
                (key(app, Action::PlayerShuffle), tr.f_shuffle),
                (key(app, Action::PlayerRepeat), tr.f_repeat),
                (key(app, Action::PlayerHide), tr.f_hide),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        row_b,
    );
}

// ---------------------------------------------------------------------------

fn network(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Network.title(app.lang), 74, 16);
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
                util::pad(role.label_for(app.lang), 22),
                Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(util::pad(role.uuid(), 8), Style::new().fg(t.dim)),
            Span::styled(
                if active {
                    format!("● {}", tr.lg_connected)
                } else {
                    format!("○ {}", tr.n_available)
                },
                Style::new().fg(if active { t.success } else { t.dim }),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        format!("  {}", tr.n_active),
        Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
    )));
    if app.networks.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("   {}", tr.n_none),
            Style::new().fg(t.dim),
        )));
    }
    // Only live connections belong in the "active connections" list. A
    // disconnected Network record is kept by the backend as history, but
    // showing it here would read as still connected.
    for n in app.networks.iter().filter(|n| n.connected) {
        lines.push(Line::from(vec![
            Span::styled("   • ", Style::new().fg(t.accent_soft)),
            Span::styled(util::pad(&n.device_name, 20), Style::new().fg(t.fg)),
            Span::styled(
                util::pad(n.role.label_for(app.lang), 22),
                Style::new().fg(t.muted),
            ),
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
    // Only the three role rows are selectable; the header/connection lines
    // below them must not become mouse targets with a bogus index.
    app.layout
        .overlay_items
        .retain(|(_, idx)| *idx < roles.len());
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
                (nav_keys(app), tr.f_select_role),
                (
                    format!(
                        "{} / {}",
                        key(app, Action::Select),
                        key(app, Action::Connect)
                    ),
                    tr.f_connect,
                ),
                (key(app, Action::Remove), tr.f_disconnect),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn profiles(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Profiles.title(app.lang), 72, 16);
    let lines: Vec<Line<'static>> = if app.profiles.is_empty() {
        vec![Line::from(Span::styled(
            tr.pf_none,
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
                    Span::styled(
                        util::pad(profile_desc(&p.name, app.lang), 32),
                        Style::new().fg(t.muted),
                    ),
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
            &[
                (nav_keys(app), tr.f_select),
                (
                    key(app, Action::Select),
                    if app.audio_profiles_writable() {
                        tr.f_toggle
                    } else {
                        tr.f_readonly
                    },
                ),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn auth(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Auth.title(app.lang), 60, 11);
    let Some(req) = app.auth.clone() else { return };
    let (heading, detail) = match &req.kind {
        AuthKind::Confirmation(passkey) => (
            tr.au_confirm.to_string(),
            fill(tr.au_verify, format!("{passkey:06}")),
        ),
        AuthKind::Authorization => (tr.au_authorize.into(), tr.au_allow.into()),
        AuthKind::PinCode => (tr.au_pin.into(), tr.au_type.into()),
        AuthKind::Passkey(k) => (tr.au_passkey.into(), fill(tr.au_expected, k)),
        AuthKind::DisplayPinCode(pin) => (tr.au_pairing_pin.into(), fill(tr.au_enter, pin)),
        AuthKind::DisplayPasskey(k) => (
            tr.au_pairing_pin.into(),
            fill(tr.au_enter_passkey, format!("{k:06}")),
        ),
        AuthKind::FilePush(name) => (tr.au_file_push.to_string(), fill(tr.au_file_accept, name)),
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
        footer(
            app,
            &[
                (key(app, Action::Select), tr.f_accept),
                (key(app, Action::Close), tr.f_reject),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn theme_picker(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::ThemePicker.title(app.lang), 48, 15);
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
            &[
                (nav_keys(app), tr.f_select),
                (key(app, Action::Select), tr.f_apply),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn log_view(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::Log.title(app.lang), 84, 24);
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
    let off = clamp_scroll(app, wrapped_lines(&lines, body.width), body.height);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((off, 0)),
        body,
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
                (nav_keys(app), tr.f_scroll),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        fr,
    );
}

// ---------------------------------------------------------------------------

fn command_palette(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let stage0 = app.palette_stage == 0;
    let title = if stage0 {
        tr.command_palette.to_string()
    } else {
        let cat = app
            .palette_category
            .map(|c| c.label_for(app.lang))
            .unwrap_or("");
        format!("{} · {}", tr.command_palette, cat)
    };
    let inner = boxed(frame, app, &title, 62, 22);
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
    let lines: Vec<Line<'static>> = if stage0 {
        // Level 1: categories with how many commands each contains.
        app.palette_categories
            .iter()
            .map(|c| {
                let count = Action::all()
                    .into_iter()
                    .filter(|a| a.context() == *c)
                    .count();
                Line::from(vec![
                    Span::styled(
                        format!("   {}", util::pad(c.label_for(app.lang), 26)),
                        Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{count:>3}  "), Style::new().fg(t.muted)),
                    Span::styled("▸", Style::new().fg(t.dim)),
                ])
            })
            .collect()
    } else {
        // Level 2: the actions in the selected category.
        app.palette_items
            .iter()
            .map(|a| {
                Line::from(vec![
                    Span::styled(
                        format!("   {}", util::pad(a.name_for(app.lang), 30)),
                        Style::new().fg(t.fg),
                    ),
                    Span::styled(app.keys.label(*a), Style::new().fg(t.accent)),
                ])
            })
            .collect()
    };
    list(frame, body, lines, app.overlay_index, app, Line::raw(""));
    let fr = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    let hints: Vec<(String, &'static str)> = if stage0 {
        vec![
            (nav_keys(app), tr.f_select),
            (key(app, Action::Select), tr.f_open),
            (key(app, Action::Close), tr.f_close),
        ]
    } else {
        vec![
            (tr.f_type.into(), tr.f_filter),
            (nav_keys(app), tr.f_select),
            (key(app, Action::Select), tr.f_run),
            (key(app, Action::Close), tr.f_back),
        ]
    };
    frame.render_widget(footer(app, &hints), fr);
}

// ---------------------------------------------------------------------------

fn about(frame: &mut Frame<'_>, app: &mut App) {
    let t = app.theme.clone();
    let tr = app.tr();
    let inner = boxed(frame, app, Overlay::About.title(app.lang), 62, 19);
    let features = app.features;
    // Align the value column by display width so CJK and Latin labels line up.
    let kv = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!("   {}", util::pad(k, 10)), Style::new().fg(t.muted)),
            Span::styled(v, Style::new().fg(t.muted)),
        ])
    };
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
            format!("  {}", tr.ab_desc),
            Style::new().fg(t.fg),
        )),
        Line::from(Span::styled(
            format!("  {}", tr.ab_desc2),
            Style::new().fg(t.muted),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            format!("  {}", tr.ab_session),
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        )),
        kv(tr.ab_backend, app.backend.label().to_string()),
        kv(tr.ab_adapters, app.adapters.len().to_string()),
        kv(tr.ab_devices, app.devices.len().to_string()),
        kv(tr.ab_theme, app.config.theme.clone()),
        kv(tr.ab_obex, yesno(features.obex, &tr)),
        kv(tr.ab_receive, yesno(features.receive_file, &tr)),
        kv(tr.ab_media, yesno(features.media_player, &tr)),
        Line::raw(""),
        Line::from(Span::styled(
            format!("  {}", tr.ab_license),
            Style::new().fg(t.dim),
        )),
    ];
    let body = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    let off = clamp_scroll(app, wrapped_lines(&lines, body.width), body.height);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((off, 0)),
        body,
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
                (nav_keys(app), tr.f_scroll),
                (key(app, Action::Close), tr.f_close),
            ],
        ),
        fr,
    );
}
