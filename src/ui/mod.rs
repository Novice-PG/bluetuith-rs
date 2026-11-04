//! Frame composition. Everything here reads `&App`; nothing mutates state.

pub mod overlays;
pub mod views;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::theme::Theme;

/// A rounded, titled panel. `focused` picks the brighter border colour.
pub fn panel(title: &str, t: &Theme, focused: bool) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { t.border_focus } else { t.border }))
        .style(Style::new().bg(t.surface))
        .padding(Padding::horizontal(1))
        .title(
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    title.to_string(),
                    Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ])
            .centered(),
        )
}

/// Plain rounded panel without a title.
pub fn plain_panel(t: &Theme) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.border))
        .style(Style::new().bg(t.surface))
        .padding(Padding::horizontal(1))
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width.saturating_sub(2)).max(20);
    let h = height.min(area.height.saturating_sub(2)).max(5);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

/// Clear + block + inner rect, ready for overlay contents.
pub fn modal(frame: &mut Frame<'_>, area: Rect, title: &str, app: &App) -> Rect {
    frame.render_widget(Clear, area);
    let b = panel(title, &app.theme, true);
    let inner = b.inner(area);
    frame.render_widget(b, area);
    inner
}

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < 40 || area.height < 12 {
        let p = Paragraph::new("Terminal too small — please resize to at least 40×12")
            .style(Style::new().fg(app.theme.warning));
        frame.render_widget(p, area);
        return;
    }

    // Background wash.
    frame.render_widget(Block::default().style(Style::new().bg(app.theme.bg)), area);

    let player_h = if app.show_player_bar && app.player.is_some() {
        3
    } else {
        0
    };
    let help_h = if app.config.no_help_display { 0 } else { 1 };
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(player_h),
        Constraint::Length(1),
        Constraint::Length(help_h),
    ])
    .split(area);

    app.layout.header = chunks[0];
    app.layout.menubar = chunks[1];
    app.layout.status = chunks[4];
    app.layout.helpbar = chunks[5];

    views::header(frame, app, chunks[0]);
    views::menubar(frame, app, chunks[1]);

    let body = chunks[2];
    let show_details = app.show_details && body.width >= 96;
    if show_details {
        let right_w = body.width.saturating_sub(52).min(48);
        let c = Layout::horizontal([Constraint::Min(50), Constraint::Length(right_w)]).split(body);
        app.layout.devices = c[0];
        app.layout.details = c[1];
        views::devices(frame, app, c[0]);
        views::details(frame, app, c[1]);
    } else {
        app.layout.devices = body;
        app.layout.details = Rect::default();
        views::devices(frame, app, body);
    }

    if player_h > 0 {
        views::player_bar(frame, app, chunks[3]);
    }
    views::status(frame, app, chunks[4]);
    if help_h > 0 {
        views::helpbar(frame, app, chunks[5]);
    }

    overlays::render(frame, app, area);
}
