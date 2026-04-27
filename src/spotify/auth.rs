use anyhow::{Context, Result};
use rspotify::{
    model::Token,
    prelude::*,
    scopes, AuthCodePkceSpotify, Config as SpotifyConfig, Credentials, OAuth,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;

const REDIRECT_PORT: u16 = 8888;
const REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";
const TOKEN_FILE: &str = "tokens.json";

/// Persisted token data written to ~/.config/spotify-dj/tokens.json.
#[derive(Debug, Serialize, Deserialize)]
struct StoredTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<String>,
    token_type: Option<String>,
    scopes: Vec<String>,
}

pub struct SpotifyAuth {
    pub client: AuthCodePkceSpotify,
    token_path: PathBuf,
}

impl SpotifyAuth {
    pub async fn new(client_id: &str, config_dir: PathBuf) -> Result<Self> {
        let creds = Credentials::new_pkce(client_id);
        let oauth = OAuth {
            redirect_uri: REDIRECT_URI.to_string(),
            scopes: scopes!(
                "streaming",
                "user-read-playback-state",
                "user-modify-playback-state",
                "user-read-currently-playing",
                "user-library-read",
                "playlist-read-private",
                "playlist-read-collaborative"
            ),
            ..Default::default()
        };

        let config = SpotifyConfig {
            token_cached: false,
            token_refreshing: true,
            ..Default::default()
        };

        let client = AuthCodePkceSpotify::with_config(creds, oauth, config);
        let token_path = config_dir.join(TOKEN_FILE);

        Ok(Self { client, token_path })
    }

    /// Load saved tokens, refresh them, or run the full OAuth flow.
    pub async fn authenticate(&mut self) -> Result<()> {
        if let Ok(token) = self.load_tokens() {
            *self.client.token.lock().await.unwrap() = Some(token);
            if self.client.refresh_token().await.is_ok() {
                self.save_tokens().await?;
                return Ok(());
            }
        }

        self.run_oauth_flow().await?;
        self.save_tokens().await?;
        Ok(())
    }

    async fn run_oauth_flow(&mut self) -> Result<()> {
        let url = self
            .client
            .get_authorize_url(None)
            .context("could not build authorize URL")?;

        println!("\nOpening Spotify login in your browser...");
        println!("If it doesn't open, visit:\n  {url}\n");

        if webbrowser::open(&url).is_err() {
            println!("(Could not open browser automatically — paste the URL above.)");
        }

        let code = Self::wait_for_callback()?;
        self.client
            .request_token(&code)
            .await
            .context("token exchange failed")?;

        Ok(())
    }

    fn wait_for_callback() -> Result<String> {
        println!("Waiting for Spotify callback on port {REDIRECT_PORT}...");

        let listener = TcpListener::bind(format!("127.0.0.1:{REDIRECT_PORT}"))
            .with_context(|| format!("could not bind to port {REDIRECT_PORT}"))?;

        let (stream, _) = listener.accept().context("callback accept failed")?;
        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;

        // GET /callback?code=xxx HTTP/1.1
        let code = request_line
            .split_whitespace()
            .nth(1)
            .and_then(|path| url::Url::parse(&format!("http://127.0.0.1{path}")).ok())
            .and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == "code")
                    .map(|(_, v)| v.to_string())
            })
            .context("no code in callback URL")?;

        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><body><h2>spotify-dj authorized!</h2>\
            <p>You can close this tab and return to the terminal.</p></body></html>";
        (&stream as &std::net::TcpStream)
            .write_all(response.as_bytes())
            .ok();

        Ok(code)
    }

    fn load_tokens(&self) -> Result<Token> {
        let text = fs::read_to_string(&self.token_path)
            .with_context(|| format!("no token file at {}", self.token_path.display()))?;

        let stored: StoredTokens =
            serde_json::from_str(&text).context("could not parse token file")?;

        let scopes: std::collections::HashSet<String> = stored.scopes.into_iter().collect();

        Ok(Token {
            access_token: stored.access_token,
            refresh_token: stored.refresh_token,
            expires_at: stored
                .expires_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            scopes,
            expires_in: chrono::TimeDelta::zero(),
        })
    }

    async fn save_tokens(&self) -> Result<()> {
        let locked = self.client.token.lock().await.unwrap();
        let token = locked.as_ref().context("no token to save")?;

        let stored = StoredTokens {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_at: token.expires_at.map(|dt| dt.to_rfc3339()),
            token_type: Some("Bearer".to_string()),
            scopes: token.scopes.iter().cloned().collect(),
        };

        if let Some(parent) = self.token_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let text = serde_json::to_string_pretty(&stored).context("could not serialize tokens")?;

        // Write with restricted permissions (owner read/write only).
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.token_path)?
                .write_all(text.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&self.token_path, text)?;
        }

        Ok(())
    }

    pub fn clear_tokens(&self) {
        fs::remove_file(&self.token_path).ok();
    }

    /// Extract the current access token for use with librespot.
    pub async fn access_token(&self) -> Result<String> {
        let locked = self.client.token.lock().await.unwrap();
        let token = locked.as_ref().context("not authenticated")?;
        Ok(token.access_token.clone())
    }
}
