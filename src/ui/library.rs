use crate::app::{LibraryState, SearchFocus, SortOrder};
use crate::config::SearchPreset;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub fn draw_library(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    library: &LibraryState,
    presets: &[SearchPreset],
    active_bpm: Option<f32>,
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

    let show_bpm = active_bpm.is_some();
    let show_sort = library.sort != SortOrder::Relevance && !library.results.is_empty();
    let mut constraints = vec![
        Constraint::Length(1), // freetext row
        Constraint::Length(1), // filter row
        Constraint::Min(1),    // results
    ];
    if show_sort {
        constraints.push(Constraint::Length(1));
    }
    if show_bpm {
        constraints.push(Constraint::Length(1));
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // --- Freetext row ---
    let (prefix, prefix_color) = match library.search_focus {
        SearchFocus::Freetext => ("/ ", Color::Yellow),
        _ => ("  ", Color::DarkGray),
    };

    let query_span = if library.search_focus == SearchFocus::Freetext {
        let text = format!("{}▌", library.search_query);
        Span::styled(
            text,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
    } else if library.search_query.is_empty() {
        Span::styled(
            if is_focused {
                "[/] title · artist"
            } else {
                ""
            },
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::raw(library.search_query.as_str())
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(prefix, Style::default().fg(prefix_color)),
            query_span,
        ])),
        rows[0],
    );

    // --- Filter row ---
    frame.render_widget(draw_filter_row(library, presets, is_focused), rows[1]);

    // --- Results list ---
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
                let duration = fmt_duration(t.duration_ms);
                let explicit = if t.explicit { " [E]" } else { "" };
                let popularity = if t.popularity > 0 {
                    format!(" ★{}", t.popularity)
                } else {
                    String::new()
                };
                let bpm = t.bpm.map(|b| format!("  {b:.0}bpm")).unwrap_or_default();

                let line1 = Line::from(vec![
                    Span::styled(
                        format!("{} — {}", t.title, t.artist),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(explicit, Style::default().fg(Color::Red)),
                    Span::styled(popularity, Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("  {duration}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(bpm, Style::default().fg(Color::Yellow)),
                ]);

                let mut meta_parts: Vec<String> = Vec::new();
                if !t.album.is_empty() {
                    meta_parts.push(t.album.clone());
                }
                if let Some(year) = t.release_year {
                    meta_parts.push(year.to_string());
                }
                if !t.genres.is_empty() {
                    let genres = t
                        .genres
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    meta_parts.push(genres);
                }

                let line2 = Line::from(Span::styled(
                    format!("  {}", meta_parts.join(" · ")),
                    Style::default().fg(Color::DarkGray),
                ));

                ListItem::new(Text::from(vec![line1, line2]))
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

    // Each item is 2 lines tall; halve height for centering.
    let item_height = 2usize;
    let visible = (rows[2].height as usize) / item_height;
    let offset = if library.selected < visible / 2 {
        0
    } else {
        library.selected.saturating_sub(visible / 2)
    };
    let mut list_state = ListState::default()
        .with_selected((!library.results.is_empty()).then_some(library.selected))
        .with_offset(offset);
    frame.render_stateful_widget(list, rows[2], &mut list_state);

    // --- Sort indicator ---
    let mut next_row = 3usize;
    if show_sort {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("sorted by ", Style::default().fg(Color::DarkGray)),
                Span::styled(library.sort.label(), Style::default().fg(Color::Cyan)),
                Span::styled("  [S] cycle", Style::default().fg(Color::DarkGray)),
            ])),
            rows[next_row],
        );
        next_row += 1;
    }

    // --- BPM reference ---
    if let (Some(bpm), true) = (active_bpm, show_bpm) {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("Active: ~{bpm:.0} BPM"),
                Style::default().fg(Color::DarkGray),
            )),
            rows[next_row],
        );
    }
}

fn fmt_duration(ms: u32) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

fn draw_filter_row<'a>(
    library: &'a LibraryState,
    presets: &'a [SearchPreset],
    is_focused: bool,
) -> Paragraph<'a> {
    match library.search_focus {
        SearchFocus::Artist | SearchFocus::Title | SearchFocus::Genre | SearchFocus::Year => {
            draw_filter_editing(library)
        }
        _ => draw_filter_idle(library, presets, is_focused),
    }
}

fn field_spans<'a>(
    label: &'a str,
    value: &'a str,
    is_active: bool,
    separator: &'a str,
) -> Vec<Span<'a>> {
    let style = if is_active {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else if value.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let text = if is_active {
        format!("{label}:[{value}▌]")
    } else if value.is_empty() {
        format!("{label}:[···]")
    } else {
        format!("{label}:{value}")
    };

    vec![
        Span::styled(text, style),
        Span::styled(separator, Style::default().fg(Color::DarkGray)),
    ]
}

fn draw_filter_editing(library: &LibraryState) -> Paragraph<'_> {
    let mut spans: Vec<Span> = Vec::new();

    let fields: &[(&str, &str, SearchFocus)] = &[
        ("a", &library.filter_artist, SearchFocus::Artist),
        ("t", &library.filter_title, SearchFocus::Title),
        ("g", &library.filter_genre, SearchFocus::Genre),
        ("y", &library.filter_year, SearchFocus::Year),
    ];

    for (i, (label, value, focus)) in fields.iter().enumerate() {
        let is_active = library.search_focus == *focus;
        let sep = if i + 1 < fields.len() { "  " } else { "" };
        spans.extend(field_spans(label, value, is_active, sep));
    }

    Paragraph::new(Line::from(spans))
}

fn draw_filter_idle<'a>(
    library: &'a LibraryState,
    presets: &'a [SearchPreset],
    is_focused: bool,
) -> Paragraph<'a> {
    let has_artist = !library.filter_artist.is_empty();
    let has_title = !library.filter_title.is_empty();
    let has_genre = !library.filter_genre.is_empty();
    let has_year = !library.filter_year.is_empty();
    let has_tag = library.filter_tag != crate::app::TagFilter::None;

    if has_artist || has_title || has_genre || has_year || has_tag {
        let mut spans: Vec<Span> = Vec::new();
        let mut first = true;

        let text_fields: &[(&str, &str)] = &[
            ("artist", library.filter_artist.as_str()),
            ("title", library.filter_title.as_str()),
            ("genre", library.filter_genre.as_str()),
            ("year", library.filter_year.as_str()),
        ];
        for (label, value) in text_fields.iter().filter(|(_, v)| !v.is_empty()) {
            if !first {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                format!("{label}:{value}"),
                Style::default().fg(Color::Cyan),
            ));
            first = false;
        }
        if let Some(tag_label) = library.filter_tag.label() {
            if !first {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                format!("tag:{tag_label}"),
                Style::default().fg(Color::Cyan),
            ));
        }

        spans.push(Span::styled(
            "  [^X] clear",
            Style::default().fg(Color::DarkGray),
        ));
        Paragraph::new(Line::from(spans))
    } else if !presets.is_empty() && is_focused {
        let spans: Vec<Span> = presets
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, p)| {
                Span::styled(
                    format!("[{}] {}  ", i + 1, p.name),
                    Style::default().fg(Color::DarkGray),
                )
            })
            .collect();
        Paragraph::new(Line::from(spans))
    } else if is_focused {
        Paragraph::new(Line::from(vec![
            Span::styled("[A] artist  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[T] title  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[G] genre  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Y] year  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[N] tag  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[S] sort", Style::default().fg(Color::DarkGray)),
        ]))
    } else {
        Paragraph::new("")
    }
}
