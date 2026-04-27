mod app;
mod audio;
mod config;
mod error;
mod spotify;
mod ui;

use anyhow::{Context, Result, bail};
use app::{AppState, CrossfadeTick, WebApiEvent};
use config::Config;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use librespot_playback::player::PlayerEvent;
use ratatui::{Terminal, backend::CrosstermBackend};
use spotify::{auth::SpotifyAuth, player::SpotifyPlayer, web_api::SpotifyWebApi};
use std::sync::Arc;
use std::thread;
use std::{io, time::Duration};
use tokio::sync::mpsc;
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
    auth.authenticate()
        .await
        .context("Spotify authentication failed")?;

    let access_token = auth.access_token().await?;
    let web_api = SpotifyWebApi::new(Arc::new(auth.client));

    println!("Authenticated! Connecting to Spotify...");
    let player = SpotifyPlayer::new(&config, access_token)
        .await
        .context("failed to start librespot player")?;

    println!(
        "Connected. Device \"{}\" is now visible in Spotify Connect.",
        config.playback.device_name
    );
    tokio::time::sleep(Duration::from_millis(800)).await;

    run_tui(config, player, web_api).await
}

async fn run_tui(config: Config, player: SpotifyPlayer, web_api: SpotifyWebApi) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(config);
    state.set_status(format!(
        "Device \"{}\" visible in Spotify — cast from your phone/desktop, or [/] to search.",
        state.config.playback.device_name
    ));

    let result = run_event_loop(&mut terminal, &mut state, player, web_api).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    player: SpotifyPlayer,
    web_api: SpotifyWebApi,
) -> Result<()> {
    let mut player_events = player.event_channel();
    let mut bpm_rx = player.bpm_rx.clone();
    let mut bands_rx = player.bands_rx.clone();
    let mut redraw_ticker = interval(Duration::from_millis(100));
    let web_api = Arc::new(web_api);

    // Background tasks send results back here.
    let (web_tx, mut web_rx) = mpsc::channel::<WebApiEvent>(32);

    // Dedicated thread for terminal input so events are never delayed by the redraw tick.
    let (input_tx, mut input_rx) = mpsc::channel::<Event>(64);
    thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if input_tx.blocking_send(ev).is_err() {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            event = player_events.recv() => {
                match event {
                    Some(PlayerEvent::TrackChanged { .. }) => {
                        state.apply_player_event(event.unwrap());
                    }
                    Some(ev) => state.apply_player_event(ev),
                    None => break,
                }
            }

            result = web_rx.recv() => {
                if let Some(ev) = result {
                    state.apply_web_api_event(ev);
                }
            }

            _ = bpm_rx.changed() => {
                if let Some(bpm) = *bpm_rx.borrow() {
                    state.active_deck_mut().bpm = Some(bpm);
                }
            }

            _ = bands_rx.changed() => {
                state.update_fft(bands_rx.borrow().clone());
            }

            Some(ev) = input_rx.recv() => {
                match ev {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        handle_key(key.code, state, &player, &web_api, &web_tx);
                    }
                    Event::Mouse(mouse) => {
                        let size = terminal.size()?;
                        handle_mouse(mouse.kind, mouse.column, mouse.row, size.into(), state);
                    }
                    _ => {}
                }
            }

            _ = redraw_ticker.tick() => {
                // Auto-fade kicks in just before the track ends, if enabled.
                if state.maybe_auto_fade() {
                    let secs = state.config.ui.crossfade_duration_secs;
                    state.set_status(format!("Auto-fading over {secs}s…"));
                }

                // Advance crossfade state machine
                match state.tick_crossfade(100) {
                    CrossfadeTick::PlayTrack(uri) => {
                        let api = Arc::clone(&web_api);
                        let device_id = player.device_id.clone();
                        tokio::spawn(async move {
                            if let Err(e) = api.play_track(&uri, &device_id).await {
                                eprintln!("crossfade play_track error: {e:#}");
                            }
                        });
                    }
                    CrossfadeTick::Complete => state.finish_crossfade(),
                    CrossfadeTick::Continue => {}
                }

                terminal.draw(|f| ui::draw(f, state))?;
            }
        }

        if state.should_quit {
            break;
        }
    }

    player.spirc.shutdown().ok();
    Ok(())
}

fn handle_key(
    code: KeyCode,
    state: &mut AppState,
    player: &SpotifyPlayer,
    web_api: &Arc<SpotifyWebApi>,
    web_tx: &mpsc::Sender<WebApiEvent>,
) {
    use app::UiFocus;

    // Help overlay intercepts everything except its own toggle/close keys.
    if state.show_help {
        match code {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                state.show_help = false;
            }
            _ => {}
        }
        return;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            state.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            state.show_help = true;
            return;
        }
        KeyCode::Tab => {
            state.cycle_focus();
            state.status_message = None;
            return;
        }
        KeyCode::BackTab => {
            state.cycle_focus_back();
            state.status_message = None;
            return;
        }
        KeyCode::Char('a') | KeyCode::Char('A')
            if !matches!(state.focus, UiFocus::Library) || !state.library.is_searching =>
        {
            state.auto_fade = !state.auto_fade;
            // Rearm: a fresh toggle should re-evaluate the current track.
            state.auto_fade_last_fired_uri = None;
            let secs = state.config.ui.crossfade_duration_secs;
            state.set_status(if state.auto_fade {
                format!("Auto-fade ON — fade starts {secs}s before track end")
            } else {
                "Auto-fade OFF".into()
            });
            return;
        }
        _ => {}
    }

    match state.focus {
        UiFocus::Library => handle_library_keys(code, state, web_api, web_tx),
        UiFocus::DeckA | UiFocus::DeckB => handle_deck_keys(code, state, player, web_api),
        UiFocus::Mixer => handle_mixer_keys(code, state, player),
    }
}

fn handle_library_keys(
    code: KeyCode,
    state: &mut AppState,
    web_api: &Arc<SpotifyWebApi>,
    web_tx: &mpsc::Sender<WebApiEvent>,
) {
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
            let query = state.library.search_query.clone();
            state.set_status(format!("Searching for \"{query}\"…"));
            let api = Arc::clone(web_api);
            let tx = web_tx.clone();
            tokio::spawn(async move {
                match api.search_tracks(&query).await {
                    Ok(results) => {
                        let _ = tx.send(WebApiEvent::SearchResults(results)).await;
                    }
                    Err(e) => eprintln!("search error: {e}"),
                }
            });
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
            if let Some(track) = state.library.results.get(state.library.selected).cloned() {
                let title = track.title.clone();
                state.load_to_deck(&track, app::ActiveDeck::A);
                state.set_status(format!("Loaded \"{title}\" → Deck A"));
            } else {
                state.set_status("No track selected");
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if let Some(track) = state.library.results.get(state.library.selected).cloned() {
                let title = track.title.clone();
                state.load_to_deck(&track, app::ActiveDeck::B);
                state.set_status(format!("Loaded \"{title}\" → Deck B"));
            } else {
                state.set_status("No track selected");
            }
        }
        _ => {}
    }
}

fn handle_deck_keys(
    code: KeyCode,
    state: &mut AppState,
    player: &SpotifyPlayer,
    web_api: &Arc<SpotifyWebApi>,
) {
    match code {
        KeyCode::Char(' ') => {
            let deck = state.active_deck_state();
            if deck.needs_initial_play {
                if let Some(uri) = deck.track_uri.clone() {
                    // Track loaded via L/R but never started — send to Spotify
                    state.active_deck_mut().needs_initial_play = false;
                    let api = Arc::clone(web_api);
                    let device_id = player.device_id.clone();
                    tokio::spawn(async move {
                        if let Err(e) = api.play_track(&uri, &device_id).await {
                            eprintln!("play_track error: {e:#}");
                        }
                    });
                }
            } else {
                // librespot owns the track — toggle play/pause
                if let Err(e) = player.play_pause() {
                    state.set_status(format!("play_pause error: {e}"));
                }
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

fn handle_mouse(
    kind: MouseEventKind,
    col: u16,
    row: u16,
    size: ratatui::layout::Rect,
    state: &mut AppState,
) {
    use app::UiFocus;

    let deck_bottom = size.height * 60 / 100;
    let lib_right = size.width * 50 / 100;

    let in_deck_area = row < deck_bottom;
    let in_lib = !in_deck_area && col < lib_right;
    let in_mixer = !in_deck_area && col >= lib_right;

    match kind {
        MouseEventKind::Down(MouseButton::Left) => {
            state.status_message = None;
            if in_deck_area {
                state.focus = if col < size.width / 2 {
                    UiFocus::DeckA
                } else {
                    UiFocus::DeckB
                };
            } else if in_lib {
                state.focus = UiFocus::Library;
            } else {
                state.focus = UiFocus::Mixer;
            }
        }
        MouseEventKind::ScrollDown => {
            if in_lib {
                if state.library.selected + 1 < state.library.results.len() {
                    state.library.selected += 1;
                }
            } else if in_mixer {
                state.crossfader = (state.crossfader + 0.05).min(1.0);
            }
        }
        MouseEventKind::ScrollUp => {
            if in_lib {
                state.library.selected = state.library.selected.saturating_sub(1);
            } else if in_mixer {
                state.crossfader = (state.crossfader - 0.05).max(-1.0);
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
            if state.crossfade.is_some() {
                state.set_status("Crossfade already in progress");
            } else if state.inactive_deck_state().track_uri.is_none() {
                state.set_status("Load a track to the other deck first ([/] search, then [L]/[R])");
            } else {
                let secs = state.config.ui.crossfade_duration_secs;
                state.start_crossfade(secs);
                let switch_label = state
                    .crossfade
                    .as_ref()
                    .map(|cf| {
                        let midpoint = cf.total_ms / 2;
                        if cf.switch_at_ms != midpoint {
                            format!(
                                "Crossfading over {secs}s — switching on downbeat at {:.1}s",
                                cf.switch_at_ms as f32 / 1000.0
                            )
                        } else {
                            format!("Crossfading over {secs}s…")
                        }
                    })
                    .unwrap_or_else(|| format!("Crossfading over {secs}s…"));
                state.set_status(switch_label);
            }
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
