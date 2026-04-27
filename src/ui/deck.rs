use crate::app::DeckState;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, LineGauge, Paragraph},
};
use std::time::Duration;

pub fn draw_deck(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    deck: &DeckState,
    label: &str,
    is_active: bool,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Color::Cyan
    } else if is_active {
        Color::Green
    } else {
        Color::DarkGray
    };

    let status_indicator = if deck.is_playing { "▶" } else { "⏸" };
    let active_tag = if is_active { "[active]" } else { "[cued]" };

    let title = format!(" DECK {label} {active_tag} ");

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // track title + artist
            Constraint::Length(1), // progress bar
            Constraint::Length(1), // time labels
            Constraint::Length(1), // BPM / Key
            Constraint::Min(2),    // visualizer placeholder + volume
        ])
        .split(inner);

    // Track info
    let (title_text, artist_text) = match (&deck.track_title, &deck.track_artist) {
        (Some(t), Some(a)) => (t.as_str(), a.as_str()),
        _ => ("(no track loaded)", ""),
    };

    let track_info = Paragraph::new(vec![
        Line::from(Span::styled(
            title_text,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(artist_text, Style::default().fg(Color::Gray))),
    ]);
    frame.render_widget(track_info, rows[0]);

    // Progress bar
    let progress = if deck.duration_ms > 0 {
        deck.position_ms as f64 / deck.duration_ms as f64
    } else {
        0.0
    };

    let progress_bar = LineGauge::default()
        .ratio(progress.clamp(0.0, 1.0))
        .filled_symbol(symbols::line::THICK.horizontal)
        .unfilled_symbol(symbols::line::NORMAL.horizontal)
        .filled_style(Style::default().fg(if is_active {
            Color::Green
        } else {
            Color::DarkGray
        }));
    frame.render_widget(progress_bar, rows[1]);

    // Time labels
    let pos = format_duration(deck.position_ms);
    let dur = format_duration(deck.duration_ms);
    let time_line = Paragraph::new(format!("{status_indicator} {pos} / {dur}"))
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(time_line, rows[2]);

    // BPM / Key / Energy
    let bpm_str = deck
        .bpm
        .map(|b| format!("{b:.1} BPM"))
        .unwrap_or_else(|| "--- BPM".to_string());
    let key_str = deck.key.as_deref().unwrap_or("---");
    let energy_str = deck
        .energy
        .map(|e| format!("Energy: {:.0}%", e * 100.0))
        .unwrap_or_else(|| "Energy: ---".to_string());

    let meta = Paragraph::new(format!("{bpm_str}  {key_str}  {energy_str}"))
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(meta, rows[3]);

    // Visualizer placeholder + volume gauge
    let viz_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(rows[4]);

    let placeholder =
        Paragraph::new("▁▂▃▄▅▄▃▂▁ ▂▃▅▆▅▃▂ ▁▂▃▄▅▄▂▁").style(Style::default().fg(if is_active {
            Color::Cyan
        } else {
            Color::DarkGray
        }));
    frame.render_widget(placeholder, viz_rows[0]);

    let vol_gauge = Gauge::default()
        .percent(deck.volume as u16)
        .gauge_style(Style::default().fg(Color::Blue))
        .label(format!("Vol {}%", deck.volume));
    frame.render_widget(vol_gauge, viz_rows[1]);
}

fn format_duration(ms: u32) -> String {
    let d = Duration::from_millis(ms as u64);
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}
