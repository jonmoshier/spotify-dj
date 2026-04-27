use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, LineGauge, Paragraph},
};

pub fn draw_mixer(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    crossfader: f32,
    is_focused: bool,
    auto_fade: bool,
) {
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if auto_fade {
        " Mixer  [AUTO] "
    } else {
        " Mixer "
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    // Crossfader label
    let label = Paragraph::new(Line::from(vec![
        Span::styled(" A ", Style::default().fg(Color::Green)),
        Span::raw("─────── CROSSFADER ─────── "),
        Span::styled("B ", Style::default().fg(Color::Blue)),
    ]));
    frame.render_widget(label, rows[0]);

    // Crossfader gauge: -1.0 (full A) → 1.0 (full B), map to 0.0–1.0
    let ratio = ((crossfader + 1.0) / 2.0) as f64;
    let xfader = LineGauge::default()
        .ratio(ratio.clamp(0.0, 1.0))
        .filled_style(Style::default().fg(Color::White));
    frame.render_widget(xfader, rows[1]);

    // Crossfade hints
    let hints = Paragraph::new("[←] Fade to A    [X] Crossfade now    [A] Auto    [→] Fade to B")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hints, rows[2]);

    // Auto-fade duration options placeholder
    let durations = Paragraph::new("Duration: [5] 5s  [0] 10s  [3] 30s")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(durations, rows[3]);
}
