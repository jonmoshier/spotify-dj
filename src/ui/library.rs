use crate::app::LibraryState;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub fn draw_library(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    library: &LibraryState,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Library ")
        .border_style(Style::default().fg(border_color));

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    // Search box
    let search_prefix = if library.is_searching { "/ " } else { "  " };
    let search_text = if library.search_query.is_empty() {
        if is_focused {
            Span::styled("[/] to search...", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled("", Style::default())
        }
    } else {
        Span::raw(library.search_query.as_str())
    };

    let search = Paragraph::new(Line::from(vec![
        Span::styled(search_prefix, Style::default().fg(Color::Yellow)),
        search_text,
    ]));
    frame.render_widget(search, rows[0]);

    // Track list
    let items: Vec<ListItem> = if library.results.is_empty() {
        vec![ListItem::new(Span::styled(
            "No results",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        library
            .results
            .iter()
            .map(|t| {
                let bpm = t.bpm.map(|b| format!("{b:.0}")).unwrap_or_default();
                let label = format!("{} — {}", t.title, t.artist);
                let bpm_span = Span::styled(format!(" {bpm}"), Style::default().fg(Color::Yellow));
                ListItem::new(Line::from(vec![Span::raw(label), bpm_span]))
            })
            .collect()
    };

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !library.results.is_empty() {
        list_state.select(Some(library.selected));
    }

    frame.render_stateful_widget(list, rows[1], &mut list_state);
}
