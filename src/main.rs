mod app;
mod config;
mod error;
mod spotify;
mod ui;

use anyhow::{bail, Context, Result};
use app::AppState;
use config::Config;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use spotify::auth::SpotifyAuth;
use std::io;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load().context("failed to load config")?;

    if config.auth.client_id.is_empty() {
        let config_path = Config::config_path()?;
        bail!(
            "No client_id configured.\n\
            \n\
            1. Go to https://developer.spotify.com/dashboard and create an app.\n\
            2. Set the Redirect URI to: http://localhost:8888/callback\n\
            3. Copy your Client ID into: {}\n\
            \n\
            Example:\n\
            [auth]\n\
            client_id = \"your_client_id_here\"\n",
            config_path.display()
        );
    }

    let config_dir = Config::config_dir()?;
    let mut auth = SpotifyAuth::new(&config.auth.client_id, config_dir).await?;
    auth.authenticate().await.context("Spotify authentication failed")?;

    println!("Authenticated! Starting spotify-dj...");
    tokio::time::sleep(Duration::from_millis(500)).await;

    run_tui(config).await
}

async fn run_tui(config: Config) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(config);
    state.set_status("Welcome to spotify-dj! Press [/] to search for tracks.");

    let result = run_event_loop(&mut terminal, &mut state).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, state))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(key.code, state);
            }
        }

        if state.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_key(code: KeyCode, state: &mut AppState) {
    use app::UiFocus;

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            state.should_quit = true;
            return;
        }
        KeyCode::Tab => {
            state.cycle_focus();
            state.status_message = None;
            return;
        }
        _ => {}
    }

    match state.focus {
        UiFocus::Library => handle_library_keys(code, state),
        UiFocus::DeckA => handle_deck_keys(code, state),
        UiFocus::DeckB => handle_deck_keys(code, state),
        UiFocus::Mixer => handle_mixer_keys(code, state),
    }
}

fn handle_library_keys(code: KeyCode, state: &mut AppState) {
    match code {
        KeyCode::Char('/') => {
            state.library.is_searching = true;
            state.library.search_query.clear();
            state.set_status("Type to search, [Enter] to confirm, [Esc] to cancel");
        }
        KeyCode::Esc => {
            state.library.is_searching = false;
            state.status_message = None;
        }
        KeyCode::Char(c) if state.library.is_searching => {
            state.library.search_query.push(c);
        }
        KeyCode::Backspace if state.library.is_searching => {
            state.library.search_query.pop();
        }
        KeyCode::Enter if state.library.is_searching => {
            state.library.is_searching = false;
            state.set_status(format!(
                "Searching for \"{}\" — Web API connects in Phase 3",
                state.library.search_query
            ));
        }
        KeyCode::Down => {
            if state.library.selected + 1 < state.library.results.len() {
                state.library.selected += 1;
            }
        }
        KeyCode::Up => {
            state.library.selected = state.library.selected.saturating_sub(1);
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            state.set_status("Load to Deck A — connects in Phase 2");
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            state.set_status("Load to Deck B — connects in Phase 2");
        }
        _ => {}
    }
}

fn handle_deck_keys(code: KeyCode, state: &mut AppState) {
    match code {
        KeyCode::Char(' ') => {
            state.set_status("Play/Pause — librespot connects in Phase 2");
        }
        KeyCode::Left => {
            state.set_status("Seek back — Phase 2");
        }
        KeyCode::Right => {
            state.set_status("Seek forward — Phase 2");
        }
        _ => {}
    }
}

fn handle_mixer_keys(code: KeyCode, state: &mut AppState) {
    match code {
        KeyCode::Left => {
            state.crossfader = (state.crossfader - 0.1).max(-1.0);
        }
        KeyCode::Right => {
            state.crossfader = (state.crossfader + 0.1).min(1.0);
        }
        KeyCode::Char('x') | KeyCode::Char('X') => {
            state.set_status("Auto-crossfade — Phase 5");
        }
        KeyCode::Char('5') => {
            state.config.ui.crossfade_duration_secs = 5;
            state.set_status("Crossfade duration: 5s");
        }
        KeyCode::Char('0') => {
            state.config.ui.crossfade_duration_secs = 10;
            state.set_status("Crossfade duration: 10s");
        }
        KeyCode::Char('3') => {
            state.config.ui.crossfade_duration_secs = 30;
            state.set_status("Crossfade duration: 30s");
        }
        _ => {}
    }
}
