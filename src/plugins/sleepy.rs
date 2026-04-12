use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use niri_ipc::Event;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::config::Config;
use crate::niri::NiriIpc;
use crate::plugins::{FromConfig, Plugin};

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
}

impl SleepyPlugin {
    async fn set_status(&self, status: &str) -> Result<()> {
        let mut body = Map::new();
        body.insert("id".to_string(), json!(self.config.device_id));
        body.insert("show_name".to_string(), json!(self.config.device_name));
        body.insert("using".to_string(), json!(true));
        body.insert("status".to_string(), json!(status));

        if let Some(secret) = &self.config.secret {
            if !secret.is_empty() {
                body.insert("secret".to_string(), json!(secret));
            }
        }

        let endpoint = format!(
            "{}/api/device/set",
            self.config.server_url.trim_end_matches('/')
        );

        let mut request = self.client.post(endpoint).json(&Value::Object(body));
        if let Some(token) = &self.config.token {
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
}

#[async_trait]
impl Plugin for SleepyPlugin {
    type Config = SleepyConfig;

    fn new(niri: NiriIpc, config: SleepyConfig) -> Self {
        info!(
            "Sleepy plugin initialized for device '{}' ({})",
            config.device_name, config.device_id
        );
        Self {
            niri,
            config,
            client: Client::new(),
            last_status: None,
        }
    }

    async fn handle_event(&mut self, event: &Event, _niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowFocusChanged { id } => {
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

    fn is_interested_in_event(&self, event: &Event) -> bool {
        match event {
            Event::WindowFocusChanged { .. } => true,
            _ => false,
        }
    }

    async fn update_config(&mut self, config: SleepyConfig) -> Result<()> {
        self.config = config;
        self.last_status = None;
        Ok(())
    }
}
