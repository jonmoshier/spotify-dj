pub mod deck;
pub mod library;
pub mod mixer;

use crate::app::{AppState, UiFocus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
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
    );

    deck::draw_deck(
        frame,
        deck_cols[1],
        &state.deck_b,
        "B",
        state.active_deck == crate::app::ActiveDeck::B,
        state.focus == UiFocus::DeckB,
    );

    // Bottom row: library (30%) | mixer + status (70%)
    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(rows[1]);

    library::draw_library(frame, bottom_cols[0], &state.library, state.focus == UiFocus::Library);

    let mixer_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(bottom_cols[1]);

    mixer::draw_mixer(frame, mixer_rows[0], state.crossfader, state.focus == UiFocus::Mixer);
    draw_status(frame, mixer_rows[1], state);
}

fn draw_status(frame: &mut Frame, area: Rect, state: &AppState) {
    let msg = state
        .status_message
        .as_deref()
        .unwrap_or("[SPC] Play/Pause  [←→] Seek  [TAB] Focus  [L] →DeckA  [R] →DeckB  [X] Crossfade  [/] Search  [Q] Quit");

    let widget = Paragraph::new(msg).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Keys ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(widget, area);
}
