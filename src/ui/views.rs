//! The always-visible chrome: header, menu bar, device table, details panel,
//! mini player, status bar and keybinding strip.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use super::panel;
use crate::app::{App, MenuId, Overlay};
use crate::backend::service_name;
use crate::model::*;
use crate::util;

fn span_width(s: &Span<'_>) -> usize {
    UnicodeWidthStr::width(s.content.as_ref())
}

/// Join left and right spans on one line with a computed gap.
fn line_lr<'a>(left: Vec<Span<'a>>, right: Vec<Span<'a>>, width: u16) -> Vec<Span<'a>> {
    let lw: usize = left.iter().map(span_width).sum();
    let rw: usize = right.iter().map(span_width).sum();
    let gap = (width as usize).saturating_sub(lw + rw);
    let mut out = left;
    if gap > 0 {
        out.push(Span::raw(" ".repeat(gap)));
    }
    out.extend(right);
    out
}

fn chip<'a>(label: &str, on: bool, t: &crate::theme::Theme) -> Span<'a> {
    let dot = if on { "●" } else { "○" };
    let fg = if on { t.success } else { t.muted };
    // Uppercase Latin chip labels to match the compact header style; CJK text
    // is left untouched by `to_ascii_uppercase`.
    Span::styled(
        format!(" {dot} {} ", label.to_ascii_uppercase()),
        Style::new()
            .fg(fg)
            .bg(t.surface_alt)
            .add_modifier(Modifier::BOLD),
    )
}

// ---------------------------------------------------------------------------

pub fn header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let t = &app.theme;
    let tr = app.tr();
    frame.render_widget(Block::default().style(Style::new().bg(t.header_bg)), area);

    let ad = app.current_adapter();
    let adapter_label = ad
        .map(|a| a.display_name())
        .unwrap_or_else(|| tr.msg_no_adapter.into());
    let adapter_id = ad.map(|a| a.id.clone()).unwrap_or_default();
    let n = app.visible_devices().len();
    let connected = app.visible_devices().iter().filter(|d| d.connected).count();

    let row0 = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    let left = vec![
        Span::styled(
            " ◆ ",
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "bluetuith-rs",
            Style::new().fg(t.header_fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            Style::new().fg(t.dim),
        ),
        Span::styled("   │   ", Style::new().fg(t.border)),
        Span::styled("▾ ", Style::new().fg(t.accent)),
        Span::styled(
            adapter_label,
            Style::new().fg(t.header_fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" ({adapter_id})"), Style::new().fg(t.muted)),
    ];
    let right = vec![
        Span::styled(format!("{} {}", n, tr.devices), Style::new().fg(t.muted)),
        Span::styled("  ·  ", Style::new().fg(t.dim)),
        Span::styled(
            format!("{} {}", connected, tr.lg_connected),
            Style::new().fg(t.connected),
        ),
        Span::raw("   "),
        Span::styled(
            format!(" {} ", app.backend.label()),
            Style::new()
                .fg(t.accent_fg)
                .bg(t.accent_soft)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(line_lr(left, right, area.width))),
        row0,
    );

    let row1 = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: 1,
    };
    let powered = ad.map(|a| a.powered).unwrap_or(false);
    let mut spans: Vec<Span<'_>> = vec![Span::raw("  ")];
    for (label, on) in [
        (tr.power, powered),
        (tr.discoverable, ad.map(|a| a.discoverable).unwrap_or(false)),
        (tr.pairable, ad.map(|a| a.pairable).unwrap_or(false)),
        (tr.st_scan, ad.map(|a| a.discovering).unwrap_or(false)),
    ] {
        spans.push(chip(label, on, t));
        spans.push(Span::raw(" "));
    }
    if ad.map(|a| a.discovering).unwrap_or(false) {
        spans.push(Span::styled(
            util::spinner(app.tick),
            Style::new().fg(t.accent),
        ));
        spans.push(Span::styled(
            format!(" {}", tr.st_scanning),
            Style::new().fg(t.accent),
        ));
    }
    if !app.filter.is_empty() {
        spans.push(Span::styled(
            format!("   ⌕ {}", app.filter),
            Style::new().fg(t.warning).add_modifier(Modifier::BOLD),
        ));
    }
    if app.search_active {
        spans.push(Span::styled(
            format!("   ⌕ {}_", app.filter),
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), row1);

    // Decorative rule.
    let row2 = Rect {
        x: area.x,
        y: area.y + 2,
        width: area.width,
        height: 1,
    };
    let w = area.width as usize;
    let lead = (w / 8).max(4);
    let rule = Line::from(vec![
        Span::styled("━".repeat(lead), Style::new().fg(t.accent)),
        Span::styled(
            "─".repeat(w.saturating_sub(lead)),
            Style::new().fg(t.border),
        ),
    ]);
    frame.render_widget(Paragraph::new(rule), row2);
}

// ---------------------------------------------------------------------------

pub fn menubar(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let t = app.theme.clone();
    frame.render_widget(Block::default().style(Style::new().bg(t.surface)), area);
    let mut x = area.x + 1;
    for (i, id) in MenuId::all().iter().enumerate() {
        let label = format!("  {}  ", id.label(app.lang));
        let w = UnicodeWidthStr::width(label.as_str()) as u16;
        let rect = Rect {
            x,
            y: area.y,
            width: w,
            height: 1,
        };
        app.layout.menu_rects[i] = rect;
        let active = app.top_overlay() == Some(Overlay::Menu) && app.menu_id == *id;
        let style = if active {
            Style::new()
                .fg(t.accent_fg)
                .bg(t.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(t.muted)
        };
        frame.render_widget(Paragraph::new(Span::styled(label, style)), rect);
        x += w + 1;
    }
    let badge = format!(" {} ", app.sort.label(app.lang));
    let bw = UnicodeWidthStr::width(badge.as_str()) as u16;
    if area.width > bw + 2 {
        frame.render_widget(
            Paragraph::new(Span::styled(badge, Style::new().fg(t.dim))).alignment(Alignment::Right),
            Rect {
                x: area.x,
                y: area.y,
                width: area.width - 1,
                height: 1,
            },
        );
    }
}

// ---------------------------------------------------------------------------

fn signal_color(bars: u8, t: &crate::theme::Theme) -> ratatui::style::Color {
    match bars {
        0 | 1 => t.error,
        2 => t.warning,
        3 => t.accent,
        _ => t.success,
    }
}

pub fn devices(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let t = app.theme.clone();
    let tr = app.tr();
    let visible: Vec<Device> = app.visible_devices().into_iter().cloned().collect();
    let title = format!("{} ({})", tr.devices, visible.len());
    let sort_hint = format!(" {}: {} ", tr.h_sort, app.sort.label(app.lang));
    let block = panel(&title, &t, true)
        .title_bottom(Line::from(Span::styled(sort_hint, Style::new().fg(t.dim))).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 {
        return;
    }
    // Column header + separator.
    let compact = inner.width < 78;
    let hdr_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let sep_rect = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(header_line(app, compact)), hdr_rect);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(inner.width as usize),
            Style::new().fg(t.border),
        ))),
        sep_rect,
    );

    let rows_rect = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    app.layout.device_rows = rows_rect;
    let h = rows_rect.height as usize;
    let start = if app.selected >= h {
        app.selected - h + 1
    } else {
        0
    };
    app.layout.device_scroll = start;

    for i in 0..h {
        let idx = start + i;
        let Some(d) = visible.get(idx) else { break };
        let r = Rect {
            x: rows_rect.x,
            y: rows_rect.y + i as u16,
            width: rows_rect.width,
            height: 1,
        };
        let selected = idx == app.selected;
        let line = device_line(d, compact, app, idx);
        let base = if selected {
            Style::new().bg(t.selection_bg)
        } else if idx % 2 == 1 {
            Style::new().bg(t.surface_alt)
        } else {
            Style::new()
        };
        frame.render_widget(Paragraph::new(line).style(base), r);
    }

    if visible.is_empty() {
        let msg = if app.filter.is_empty() {
            tr.msg_no_devices
        } else {
            tr.msg_no_devices_filter
        };
        frame.render_widget(
            Paragraph::new(Span::styled(msg, Style::new().fg(t.muted)))
                .alignment(Alignment::Center),
            rows_rect,
        );
    } else if h.saturating_sub(visible.len()) >= 2 {
        // Anchor the legend to the bottom of the panel (last two inner rows)
        // instead of leaving a gap beneath it.
        let lr = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(2),
            width: inner.width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "─".repeat(inner.width as usize),
                Style::new().fg(t.border),
            ))),
            lr,
        );
        // Keep the legend to the four state badges only; signal/battery are
        // already explained in the details panel, and a long legend is simply
        // clipped on a narrow device panel.
        let legend = Line::from(vec![
            Span::styled(format!("  {}  ", tr.lg_title), Style::new().fg(t.muted)),
            Span::styled("● ", Style::new().fg(t.connected)),
            Span::styled(format!("{}   ", tr.lg_connected), Style::new().fg(t.dim)),
            Span::styled("◆ ", Style::new().fg(t.paired)),
            Span::styled(format!("{}   ", tr.lg_paired), Style::new().fg(t.dim)),
            Span::styled("★ ", Style::new().fg(t.trusted)),
            Span::styled(format!("{}   ", tr.lg_trusted), Style::new().fg(t.dim)),
            Span::styled("⊘ ", Style::new().fg(t.blocked)),
            Span::styled(tr.lg_blocked, Style::new().fg(t.dim)),
        ]);
        frame.render_widget(
            Paragraph::new(legend),
            Rect {
                x: inner.x,
                y: lr.y + 1,
                width: inner.width,
                height: 1,
            },
        );
    }
}

fn header_line(app: &App, compact: bool) -> Line<'static> {
    let t = &app.theme;
    let tr = app.tr();
    let nw = name_width(app.layout.device_rows.width as usize, compact);
    // Column widths are shared verbatim with `device_line`, so headers sit
    // exactly above their data.
    let hi = |label: &str, width: usize, key: SortKey| -> Span<'static> {
        let style = if app.sort == key {
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(t.muted).add_modifier(Modifier::BOLD)
        };
        Span::styled(util::pad(label, width), style)
    };
    let mut v = vec![
        Span::raw("  "), // selection marker
        Span::raw("  "), // kind glyph
        hi(tr.col_name, nw, SortKey::Name),
        Span::raw(" "),
        hi(tr.col_address, 17, SortKey::Address),
    ];
    if !compact {
        v.push(Span::raw(" "));
        v.push(hi(tr.col_type, 9, SortKey::Kind));
    }
    v.push(Span::raw(" "));
    v.push(hi(tr.col_rssi, 4, SortKey::Rssi));
    if !compact {
        v.push(Span::raw(" "));
        v.push(hi(tr.col_bat, 5, SortKey::Battery));
    }
    v.push(Span::raw(" "));
    v.push(hi(tr.col_state, 5, SortKey::State));
    Line::from(v)
}

fn name_width(w: usize, compact: bool) -> usize {
    // marker(2) + glyph(2) + name + 5 separators + address(17) + type(9)
    // + signal(4) + battery(5) + spaced badges(7)
    let fixed = if compact { 35 } else { 51 };
    w.saturating_sub(fixed).clamp(10, 44)
}

fn device_line(d: &Device, compact: bool, app: &App, idx: usize) -> Line<'static> {
    let t = &app.theme;
    let selected = idx == app.selected;
    let nw = name_width(app.layout.device_rows.width as usize, compact);
    let mut v: Vec<Span<'static>> = vec![
        Span::styled(
            if selected { "▸ " } else { "  " }.to_string(),
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{} ", d.kind().glyph()), Style::new().fg(t.accent)),
        Span::styled(
            util::pad(&d.display_name(), nw),
            Style::new()
                .fg(if selected { t.selection_fg } else { t.fg })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(util::pad(&d.address, 17), Style::new().fg(t.muted)),
    ];
    if !compact {
        v.push(Span::raw(" "));
        v.push(Span::styled(
            util::pad(d.kind().label(app.lang), 9),
            Style::new().fg(t.muted),
        ));
    }
    v.push(Span::raw(" "));
    let bars = d.signal_bars();
    v.push(Span::styled(
        util::signal_glyph(bars).to_string(),
        Style::new().fg(signal_color(bars, t)),
    ));
    if !compact {
        v.push(Span::raw(" "));
        match d.battery {
            Some(p) => {
                let c = if p <= 20 {
                    t.error
                } else if p <= 50 {
                    t.warning
                } else {
                    t.success
                };
                v.push(Span::styled(format!("{:>4}%", p), Style::new().fg(c)));
            }
            None => v.push(Span::styled("    –", Style::new().fg(t.dim))),
        }
    }
    v.push(Span::raw(" "));
    v.push(Span::styled(
        if d.connected { "●" } else { "○" },
        Style::new().fg(if d.connected { t.connected } else { t.dim }),
    ));
    v.push(Span::raw(" "));
    v.push(Span::styled(
        if d.paired { "◆" } else { "◇" },
        Style::new().fg(if d.paired { t.paired } else { t.dim }),
    ));
    v.push(Span::raw(" "));
    v.push(Span::styled(
        if d.trusted { "★" } else { "☆" },
        Style::new().fg(if d.trusted { t.trusted } else { t.dim }),
    ));
    v.push(Span::raw(" "));
    v.push(Span::styled(
        if d.blocked { "⊘" } else { "·" },
        Style::new().fg(if d.blocked { t.blocked } else { t.dim }),
    ));
    v.push(Span::raw(" "));
    Line::from(v)
}

// ---------------------------------------------------------------------------

pub fn details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let t = &app.theme;
    let tr = app.tr();
    let block = panel(tr.device_details, t, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(d) = app.selected_device().cloned() else {
        frame.render_widget(
            Paragraph::new(tr.d_select_device).style(Style::new().fg(t.muted)),
            inner,
        );
        return;
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(format!("{} ", d.kind().glyph()), Style::new().fg(t.accent)),
        Span::styled(
            d.display_name(),
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        d.address.clone(),
        Style::new().fg(t.muted),
    )));
    lines.push(Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::new().fg(t.border),
    )));

    // Use the same glyphs as the device-list badges and the legend, so the
    // two panels read consistently (● connection, ◆ paired, ★ trusted, ⊘ blocked).
    let state =
        |label: &str, on: bool, on_glyph: &str, off_glyph: &str, color: ratatui::style::Color| {
            Line::from(vec![
                Span::styled(
                    format!("  {}", util::pad(label, 14)),
                    Style::new().fg(t.muted),
                ),
                Span::styled(
                    format!(
                        "{} {}",
                        if on { on_glyph } else { off_glyph },
                        if on { tr.yes } else { tr.no }
                    ),
                    Style::new()
                        .fg(if on { color } else { t.dim })
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        };
    lines.push(state(tr.d_connected, d.connected, "●", "○", t.connected));
    lines.push(state(tr.d_paired, d.paired, "◆", "◇", t.paired));
    lines.push(state(tr.d_trusted, d.trusted, "★", "☆", t.trusted));
    lines.push(state(tr.d_blocked, d.blocked, "⊘", "·", t.blocked));
    lines.push(Line::raw(""));

    let bars = d.signal_bars();
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}", util::pad(tr.d_signal, 14)),
            Style::new().fg(t.muted),
        ),
        Span::styled(
            util::signal_glyph(bars).to_string(),
            Style::new().fg(signal_color(bars, t)),
        ),
        Span::styled(
            format!(
                "  {}",
                d.rssi
                    .map(|r| format!("{r} dBm"))
                    .unwrap_or_else(|| "—".into())
            ),
            Style::new().fg(t.fg),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}", util::pad(tr.d_battery, 14)),
            Style::new().fg(t.muted),
        ),
        Span::styled(
            d.battery
                .map(|p| format!("{p:>3}%  {}", util::battery_glyph(p)))
                .unwrap_or_else(|| "   –".into()),
            Style::new().fg(d
                .battery
                .map(|p| if p <= 20 { t.error } else { t.success })
                .unwrap_or(t.dim)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}", util::pad(tr.d_type, 14)),
            Style::new().fg(t.muted),
        ),
        Span::styled(d.kind().label(app.lang).to_string(), Style::new().fg(t.fg)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}", util::pad(tr.d_class, 14)),
            Style::new().fg(t.muted),
        ),
        Span::styled(
            d.class
                .map(|c| format!("0x{c:06X}"))
                .unwrap_or_else(|| "—".into()),
            Style::new().fg(t.fg),
        ),
    ]));
    if !d.modalias.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}", util::pad(tr.d_modalias, 14)),
                Style::new().fg(t.muted),
            ),
            Span::styled(d.modalias.clone(), Style::new().fg(t.fg)),
        ]));
    }
    if !d.uuids.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            format!("  {}", tr.d_services),
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        )));
        for u in d.uuids.iter().take(6) {
            let short = if u.len() == 36 { &u[4..8] } else { u.as_str() };
            lines.push(Line::from(vec![
                Span::styled("   • ", Style::new().fg(t.accent_soft)),
                Span::styled(
                    service_name(short, app.lang).to_string(),
                    Style::new().fg(t.muted),
                ),
            ]));
        }
        if d.uuids.len() > 6 {
            lines.push(Line::from(Span::styled(
                format!("   {}", crate::i18n::fill(tr.d_and_more, d.uuids.len() - 6)),
                Style::new().fg(t.dim),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);

    // Battery gauge at the bottom of the panel when it fits.
    if let Some(p) = d.battery {
        if inner.height > 16 {
            let r = Rect {
                x: inner.x,
                y: inner.y + inner.height.saturating_sub(2),
                width: inner.width,
                height: 1,
            };
            frame.render_widget(
                Gauge::default()
                    .ratio((p as f64 / 100.0).clamp(0.0, 1.0))
                    .gauge_style(Style::new().fg(t.success).bg(t.gauge_empty))
                    .label(format!("{} {p}%", tr.d_battery)),
                r,
            );
        }
    }
}

// ---------------------------------------------------------------------------

pub fn player_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let t = &app.theme;
    let tr = app.tr();
    let Some(p) = app.player.as_ref() else { return };
    frame.render_widget(Block::default().style(Style::new().bg(t.surface_alt)), area);
    let r0 = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let left = vec![
        Span::styled(
            " ♫ ",
            Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            util::truncate(&p.title, 32),
            Style::new().fg(t.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" — ", Style::new().fg(t.dim)),
        Span::styled(util::truncate(&p.artist, 22), Style::new().fg(t.muted)),
    ];
    let right = vec![
        Span::styled(
            p.status.label(app.lang).to_string(),
            Style::new().fg(t.info),
        ),
        Span::styled("  ", Style::new()),
        Span::styled(
            format!(
                "{} / {}",
                util::duration(p.position_ms),
                util::duration(p.duration_ms)
            ),
            Style::new().fg(t.muted),
        ),
        Span::styled(
            format!(
                "   {}  {}",
                p.device_name,
                crate::i18n::fill(tr.p_vol, p.volume)
            ),
            Style::new().fg(t.dim),
        ),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(line_lr(left, right, r0.width))),
        r0,
    );

    let r1 = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    frame.render_widget(
        Gauge::default()
            .ratio(p.progress())
            .gauge_style(Style::new().fg(t.accent).bg(t.gauge_empty))
            .label(""),
        r1,
    );

    let r2 = Rect {
        x: area.x + 1,
        y: area.y + 2,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let controls = Line::from(vec![
        Span::styled("  |< ", Style::new().fg(t.fg)),
        Span::styled(
            if p.status == PlayStatus::Playing {
                "▮▮"
            } else {
                "▶ "
            },
            Style::new().fg(t.accent),
        ),
        Span::styled("  >| ", Style::new().fg(t.fg)),
        Span::styled("  ■ ", Style::new().fg(t.fg)),
        Span::styled("   ", Style::new()),
        Span::styled(
            format!(
                "⇄ {} {}",
                tr.p_shuffle,
                if p.shuffle { tr.on } else { tr.off }
            ),
            Style::new().fg(if p.shuffle { t.accent } else { t.dim }),
        ),
        Span::styled("   ", Style::new()),
        Span::styled(
            format!("{} {}", tr.p_repeat, p.repeat.glyph()),
            Style::new().fg(t.muted),
        ),
        Span::styled("   ", Style::new()),
        Span::styled(
            format!("{} / {}", p.track_number, p.track_count),
            Style::new().fg(t.dim),
        ),
        Span::styled("    ", Style::new()),
        Span::styled(tr.p_hint, Style::new().fg(t.dim)),
    ]);
    frame.render_widget(Paragraph::new(controls), r2);
}

// ---------------------------------------------------------------------------

pub fn status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let t = &app.theme;
    let tr = app.tr();
    frame.render_widget(Block::default().style(Style::new().bg(t.surface)), area);
    let icon = app.status_icon().to_string();
    let color = app.status_color();
    let left = vec![
        Span::styled(
            format!(" {icon} "),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            util::truncate(&app.status.text, area.width as usize / 2),
            Style::new().fg(color),
        ),
    ];
    let k = |a: crate::keybind::Action| app.keys.label(a);
    let right = vec![
        Span::styled(
            format!("{} {}", k(crate::keybind::Action::Menu), tr.sb_menu),
            Style::new().fg(t.dim),
        ),
        Span::styled("  ·  ", Style::new().fg(t.border)),
        Span::styled(
            format!("{} {}", k(crate::keybind::Action::Help), tr.sb_help),
            Style::new().fg(t.dim),
        ),
        Span::styled("  ·  ", Style::new().fg(t.border)),
        Span::styled(
            format!("{} {}", k(crate::keybind::Action::Quit), tr.sb_quit),
            Style::new().fg(t.dim),
        ),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(line_lr(left, right, area.width))),
        area,
    );
}

pub fn helpbar(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let t = app.theme.clone();
    let tr = app.tr();
    frame.render_widget(Block::default().style(Style::new().bg(t.bg)), area);
    use crate::keybind::Action;
    let k = |a: Action| app.keys.label(a);
    // Every hint is derived from the live keymap, so rebinding an action also
    // updates this strip (and it stays consistent with menus/help/palette).
    let nav = format!("{}{}", k(Action::NavigateUp), k(Action::NavigateDown));
    let hints: Vec<(String, &'static str, Option<Action>)> = vec![
        (nav, tr.h_navigate, None),
        (k(Action::Connect), tr.h_connect, Some(Action::Connect)),
        (k(Action::Pair), tr.h_pair, Some(Action::Pair)),
        (k(Action::Scan), tr.h_scan, Some(Action::Scan)),
        (k(Action::Power), tr.h_power, Some(Action::Power)),
        (k(Action::Trust), tr.h_trust, Some(Action::Trust)),
        (k(Action::Block), tr.h_block, Some(Action::Block)),
        (k(Action::SendFiles), tr.h_send, Some(Action::SendFiles)),
        (k(Action::Info), tr.h_info, Some(Action::Info)),
        (
            k(Action::ProgressView),
            tr.h_transfers,
            Some(Action::ProgressView),
        ),
        (k(Action::Search), tr.h_search, Some(Action::Search)),
        (k(Action::Help), tr.h_help, Some(Action::Help)),
    ];
    let mut spans: Vec<Span<'_>> = vec![Span::raw(" ")];
    let mut rects: Vec<(Rect, Action)> = Vec::new();
    let mut x = area.x + 1;
    for (key, label, action) in hints {
        let key_txt = format!(" {key} ");
        let lab_txt = format!("{label} ");
        let w = UnicodeWidthStr::width(key_txt.as_str()) as u16
            + UnicodeWidthStr::width(lab_txt.as_str()) as u16;
        // Stop cleanly at the edge instead of rendering half a hint.
        if x + w > area.x + area.width {
            break;
        }
        // Record the whole hint (key + label) as a click target.
        if let Some(a) = action {
            rects.push((
                Rect {
                    x,
                    y: area.y,
                    width: w,
                    height: 1,
                },
                a,
            ));
        }
        spans.push(Span::styled(
            key_txt,
            Style::new()
                .fg(t.accent_fg)
                .bg(t.accent_soft)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(lab_txt, Style::new().fg(t.dim)));
        x += w;
    }
    app.layout.help_rects = rects;
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
