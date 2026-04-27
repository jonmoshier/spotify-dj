mod app;
mod audio;
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
use spotify::{auth::SpotifyAuth, player::SpotifyPlayer};
use std::io;
use std::time::Duration;
use tokio::time::interval;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load().context("failed to load config")?;

    if config.auth.client_id.is_empty() {
        let config_path = Config::config_path()?;
        bail!(
            "No client_id configured.\n\
            \n\
            1. Go to https://developer.spotify.com/dashboard and create an app.\n\
            2. Set the Redirect URI to: http://127.0.0.1:8888/callback\n\
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

    let access_token = auth.access_token().await?;

    println!("Authenticated! Connecting to Spotify...");
    let player = SpotifyPlayer::new(&config, access_token)
        .await
        .context("failed to start librespot player")?;

    println!(
        "Connected. Device \"{}\" is now visible in Spotify Connect.",
        config.playback.device_name
    );
    tokio::time::sleep(Duration::from_millis(800)).await;

    run_tui(config, player).await
}

async fn run_tui(config: Config, player: SpotifyPlayer) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(config);
    state.set_status(format!(
        "Device \"{}\" visible in Spotify — cast from your phone/desktop, or [/] to search.",
        state.config.playback.device_name
    ));

    let result = run_event_loop(&mut terminal, &mut state, player).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    player: SpotifyPlayer,
) -> Result<()> {
    let mut player_events = player.event_channel();
    // Redraw timer — ensures the TUI ticks even when no events arrive.
    let mut redraw_ticker = interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            // Librespot player event.
            event = player_events.recv() => {
                match event {
                    Some(ev) => state.apply_player_event(ev),
                    None => break, // player shut down
                }
            }

            // Keyboard input (non-blocking poll).
            _ = redraw_ticker.tick() => {
                terminal.draw(|f| ui::draw(f, state))?;

                if event::poll(Duration::ZERO)? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            handle_key(key.code, state, &player);
                        }
                    }
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    player.spirc.shutdown().ok();
    Ok(())
}

fn handle_key(code: KeyCode, state: &mut AppState, player: &SpotifyPlayer) {
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
        UiFocus::DeckA | UiFocus::DeckB => handle_deck_keys(code, state, player),
        UiFocus::Mixer => handle_mixer_keys(code, state, player),
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
            state.set_status("Load to Deck A — search connects in Phase 3");
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            state.set_status("Load to Deck B — search connects in Phase 3");
        }
        _ => {}
    }
}

fn handle_deck_keys(code: KeyCode, state: &mut AppState, player: &SpotifyPlayer) {
    match code {
        KeyCode::Char(' ') => {
            if let Err(e) = player.play_pause() {
                state.set_status(format!("play_pause error: {e}"));
            }
        }
        KeyCode::Left => {
            let pos = state.active_deck_state().position_ms.saturating_sub(5_000);
            if let Err(e) = player.seek(pos) {
                state.set_status(format!("seek error: {e}"));
            }
        }
        KeyCode::Right => {
            let pos = state.active_deck_state().position_ms + 5_000;
            if let Err(e) = player.seek(pos) {
                state.set_status(format!("seek error: {e}"));
            }
        }
        KeyCode::Up => {
            let vol = (state.active_deck_state().volume + 5).min(100);
            state.active_deck_mut().volume = vol;
            if let Err(e) = player.set_volume(vol) {
                state.set_status(format!("volume error: {e}"));
            }
        }
        KeyCode::Down => {
            let vol = state.active_deck_state().volume.saturating_sub(5);
            state.active_deck_mut().volume = vol;
            if let Err(e) = player.set_volume(vol) {
                state.set_status(format!("volume error: {e}"));
            }
        }
        _ => {}
    }
}

fn handle_mixer_keys(code: KeyCode, state: &mut AppState, _player: &SpotifyPlayer) {
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
