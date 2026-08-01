use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use mpris::{PlaybackStatus, PlayerFinder};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::niri::NiriIpc;
use crate::plugins::{FromConfig, PiriEvent, Plugin};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepyConfig {
    /// Sleepy server base URL, e.g. "https://sleepy.example.com"
    pub server_url: String,
    /// Device ID in Sleepy
    pub device_id: String,
    /// Display name in Sleepy
    pub device_name: String,
    /// Optional bearer token used by Sleepy auth middleware
    #[serde(default)]
    pub token: Option<String>,
    /// Optional secret used by some Sleepy deployments
    #[serde(default)]
    pub secret: Option<String>,
    /// Use app_id as status text if available (fallback to title)
    #[serde(default)]
    pub prefer_app_id: bool,
    /// Regex matching MPRIS player process/bus name, e.g. "spotify|vlc|firefox"
    #[serde(default)]
    pub media_process_name: Option<String>,
    /// Device ID used when reporting media playback status
    #[serde(default)]
    pub media_device_id: Option<String>,
    /// Display name used when reporting media playback status
    #[serde(default)]
    pub media_device_name: Option<String>,
    /// Polling interval in seconds for media playback status
    #[serde(default = "default_media_poll_interval")]
    pub media_poll_interval: u64,
}

fn default_media_poll_interval() -> u64 {
    5
}

fn media_enabled(config: &SleepyConfig) -> bool {
    !config.media_process_name.as_deref().unwrap_or("").trim().is_empty()
        && !config.media_device_id.as_deref().unwrap_or("").trim().is_empty()
        && !config.media_device_name.as_deref().unwrap_or("").trim().is_empty()
}

impl FromConfig for SleepyConfig {
    fn from_config(config: &Config) -> Option<Self> {
        config.sleepy.clone()
    }
}

pub struct SleepyPlugin {
    niri: NiriIpc,
    config: SleepyConfig,
    client: Client,
    last_status: Option<String>,
    shared_config: Arc<RwLock<SleepyConfig>>,
    media_task: Option<JoinHandle<()>>,
}

impl SleepyPlugin {
    async fn set_status(&self, status: &str) -> Result<()> {
        Self::send_device_status(
            &self.client,
            &self.config,
            &self.config.device_id,
            &self.config.device_name,
            true,
            status,
        )
        .await
    }

    async fn send_device_status(
        client: &Client,
        config: &SleepyConfig,
        device_id: &str,
        device_name: &str,
        using: bool,
        status: &str,
    ) -> Result<()> {
        let mut body = Map::new();
        body.insert("id".to_string(), json!(device_id));
        body.insert("show_name".to_string(), json!(device_name));
        body.insert("using".to_string(), json!(using));
        body.insert("status".to_string(), json!(status));

        if let Some(secret) = &config.secret {
            if !secret.is_empty() {
                body.insert("secret".to_string(), json!(secret));
            }
        }

        let endpoint = format!(
            "{}/api/device/set",
            config.server_url.trim_end_matches('/')
        );

        let mut request = client.post(endpoint).json(&Value::Object(body));
        if let Some(token) = &config.token {
            if !token.is_empty() {
                request = request.bearer_auth(token);
            }
        }

        let response = request.send().await.context("failed to send status request to sleepy")?;

        if !response.status().is_success() {
            let code = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("sleepy api returned {}: {}", code, text);
        }

        Ok(())
    }

    fn status_from_window(&self, window: &crate::niri::Window) -> String {
        if self.config.prefer_app_id {
            if let Some(app_id) = &window.app_id {
                if !app_id.trim().is_empty() {
                    return app_id.trim().to_string();
                }
            }
        }

        let title = window.title.trim();
        if title.is_empty() {
            window
                .app_id
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            title.to_string()
        }
    }

    async fn handle_focus_change(&mut self, window_id: u64) -> Result<()> {
        let windows = self.niri.get_windows().await?;
        let window = match windows.into_iter().find(|w| w.id == window_id) {
            Some(w) => w,
            Option::None => return Ok(()),
        };

        let status = self.status_from_window(&window);
        if self.last_status.as_deref() == Some(status.as_str()) {
            debug!("Sleepy status unchanged, skipping push: {}", status);
            return Ok(());
        }

        self.set_status(&status).await?;
        info!("Sleepy status updated: {}", status);
        self.last_status = Some(status);
        Ok(())
    }

    async fn media_poll_loop(shared: Arc<RwLock<SleepyConfig>>, client: Client) {
        let mut last_playing = false;
        let mut last_content = String::new();

        loop {
            let interval = {
                let config = shared.read().await;
                config.media_poll_interval.max(1)
            };

            let result = Self::poll_media(&shared, &client, &mut last_playing, &mut last_content).await;
            if let Err(e) = result {
                warn!("Failed to poll media status: {}", e);
            }

            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }

    async fn poll_media(
        shared: &Arc<RwLock<SleepyConfig>>,
        client: &Client,
        last_playing: &mut bool,
        last_content: &mut String,
    ) -> Result<()> {
        let config = shared.read().await.clone();
        if !media_enabled(&config) {
            return Ok(());
        }

        let pattern = config.media_process_name.clone().unwrap_or_default();
        let (playing, content) = tokio::task::spawn_blocking(move || query_media(&pattern))
            .await
            .context("media query task panicked")??;

        let changed = playing != *last_playing || (playing && content != *last_content);
        if !changed {
            return Ok(());
        }

        let status = if playing { content.clone() } else { "没有媒体播放".to_string() };
        Self::send_device_status(
            client,
            &config,
            config.media_device_id.as_deref().unwrap_or_default(),
            config.media_device_name.as_deref().unwrap_or_default(),
            playing,
            &status,
        )
        .await?;

        info!("Sleepy media status updated: {}", status);
        *last_playing = playing;
        *last_content = content;
        Ok(())
    }
}

fn query_media(pattern: &str) -> Result<(bool, String)> {
    let re = Regex::new(pattern)?;
    let finder = PlayerFinder::new()?;
    let players = finder.find_all()?;

    let matching: Vec<_> = players
        .into_iter()
        .filter(|p| {
            re.is_match(p.bus_name_trimmed()) || re.is_match(p.identity())
        })
        .collect();

    if matching.is_empty() {
        return Ok((false, String::new()));
    }

    let player = matching
        .iter()
        .find(|p| matches!(p.get_playback_status(), Ok(PlaybackStatus::Playing)))
        .or_else(|| matching.first());
    let player = player.unwrap();

    let status = player.get_playback_status()?;
    if !matches!(status, PlaybackStatus::Playing) {
        return Ok((false, String::new()));
    }

    let meta = player.get_metadata()?;
    let title = meta.title().unwrap_or("").trim().to_string();
    let artist = meta
        .artists()
        .map(|a| a.join(", "))
        .unwrap_or_default();
    let album = meta.album_name().unwrap_or("").trim().to_string();

    let mut content = format!("♪{}", title);
    if !artist.is_empty() && artist != title {
        content.push_str(&format!(" - {}", artist));
    }
    if !album.is_empty() && album != title && album != artist {
        content.push_str(&format!(" - {}", album));
    }

    Ok((true, content))
}

#[async_trait]
impl Plugin for SleepyPlugin {
    type Config = SleepyConfig;

    fn new(niri: NiriIpc, config: SleepyConfig) -> Self {
        info!(
            "Sleepy plugin initialized for device '{}' ({})",
            config.device_name, config.device_id
        );
        let client = Client::new();
        let shared_config = Arc::new(RwLock::new(config.clone()));
        let media_task = if media_enabled(&config) {
            let shared = shared_config.clone();
            let client = client.clone();
            Some(tokio::spawn(async move {
                Self::media_poll_loop(shared, client).await;
            }))
        } else {
            None
        };
        Self {
            niri,
            config,
            client,
            last_status: None,
            shared_config,
            media_task,
        }
    }

    async fn handle_event(&mut self, event: &PiriEvent, _niri: &NiriIpc) -> Result<()> {
        match event {
            PiriEvent::WindowFocusChanged { id } => {
                if let Some(window_id) = id {
                    if let Err(e) = self.handle_focus_change(*window_id).await {
                        warn!("Failed to update sleepy status: {}", e);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn is_interested_in_event(&self, event: &PiriEvent) -> bool {
        match event {
            PiriEvent::WindowFocusChanged { .. } => true,
            _ => false,
        }
    }

    async fn update_config(&mut self, config: SleepyConfig) -> Result<()> {
        let was_enabled = self.media_task.is_some();
        let now_enabled = media_enabled(&config);

        *self.shared_config.write().await = config.clone();
        self.config = config;
        self.last_status = None;

        match (was_enabled, now_enabled) {
            (true, false) => {
                if let Some(task) = self.media_task.take() {
                    task.abort();
                }
            }
            (false, true) => {
                let shared = self.shared_config.clone();
                let client = self.client.clone();
                self.media_task = Some(tokio::spawn(async move {
                    Self::media_poll_loop(shared, client).await;
                }));
            }
            _ => {}
        }

        Ok(())
    }
}
