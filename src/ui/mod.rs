pub mod deck;
pub mod library;
pub mod mixer;
pub mod visualizer;

use crate::app::{AppState, UiFocus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn draw(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Outer vertical split: decks (top 60%) | bottom bar (40%)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Top row: Deck A | Deck B
    let deck_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    deck::draw_deck(
        frame,
        deck_cols[0],
        &state.deck_a,
        "A",
        state.active_deck == crate::app::ActiveDeck::A,
        state.focus == UiFocus::DeckA,
        &state.fft_bands,
        &state.fft_peaks,
    );

    deck::draw_deck(
        frame,
        deck_cols[1],
        &state.deck_b,
        "B",
        state.active_deck == crate::app::ActiveDeck::B,
        state.focus == UiFocus::DeckB,
        &state.fft_bands,
        &state.fft_peaks,
    );

    // Bottom row: library (30%) | mixer + status (70%)
    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(rows[1]);

    library::draw_library(
        frame,
        bottom_cols[0],
        &state.library,
        state.focus == UiFocus::Library,
    );

    let mixer_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(bottom_cols[1]);

    mixer::draw_mixer(
        frame,
        mixer_rows[0],
        state.crossfader,
        state.focus == UiFocus::Mixer,
        state.auto_fade,
    );
    draw_status(frame, mixer_rows[1], state);

    if state.show_help {
        draw_help(frame);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn draw_help(frame: &mut Frame) {
    const LINES: &[(&str, &str)] = &[
        ("Global", ""),
        ("Tab / Shift+Tab", "cycle focus forward / back"),
        ("?", "toggle this help"),
        ("Q", "quit"),
        ("", ""),
        ("Deck (when focused)", ""),
        ("Space", "play / pause"),
        ("← →", "seek ±5s"),
        ("↑ ↓", "volume ±5"),
        ("", ""),
        ("Library", ""),
        ("/", "search"),
        ("↑ ↓  or  scroll", "navigate results"),
        ("L", "load selected → Deck A"),
        ("R", "load selected → Deck B"),
        ("", ""),
        ("Mixer", ""),
        ("← →  or  scroll", "move crossfader"),
        ("X", "start crossfade"),
        ("A", "toggle auto-fade"),
        ("5 / 0 / 3", "crossfade duration 5s / 10s / 30s"),
        ("", ""),
        ("Mouse", ""),
        ("Click panel", "focus that panel"),
        ("Scroll library", "navigate track list"),
        ("Scroll mixer", "move crossfader"),
    ];

    let popup_width: u16 = 52;
    let popup_height: u16 = LINES.len() as u16 + 4;
    let area = centered_rect(popup_width, popup_height, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help  [?] or [Esc] to close ")
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    use ratatui::text::{Line, Span};
    let lines: Vec<Line> = LINES
        .iter()
        .map(|(key, desc)| {
            if desc.is_empty() {
                Line::from(Span::styled(
                    *key,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("{:<20}", key), Style::default().fg(Color::Cyan)),
                    Span::raw(*desc),
                ])
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_status(frame: &mut Frame, area: Rect, state: &AppState) {
    let msg = state
        .status_message
        .as_deref()
        .unwrap_or("[SPC] Play/Pause  [←→] Seek  [TAB] Focus  [L] →DeckA  [R] →DeckB  [X] Crossfade  [A] Auto-fade  [/] Search  [Q] Quit");

    let widget = Paragraph::new(msg).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Keys ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(widget, area);
}
