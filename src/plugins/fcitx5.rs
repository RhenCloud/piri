use anyhow::Result;
use log::{debug, info, warn};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{deserialize_string_or_vec, Config};
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};
use crate::plugins::{FromConfig, PiriEvent};
use crate::utils::Throttle;

/// Fcitx5 input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fcitx5InputMode {
    /// English mode (inactive = 1)
    English,
    /// Chinese mode (active = 2)
    Chinese,
}

impl Fcitx5InputMode {
    /// Convert from fcitx5-remote status code
    /// fcitx5-remote: 0=closed, 1=inactive(English), 2=active(Chinese)
    pub fn from_status(status: i32) -> Option<Self> {
        match status {
            0 | 1 => Some(Fcitx5InputMode::English),
            2 => Some(Fcitx5InputMode::Chinese),
            _ => None,
        }
    }
}

/// Fcitx5 plugin config (for internal use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fcitx5PluginConfig {
    /// List of window rules for fcitx5 input mode switching
    pub rules: Vec<Fcitx5WindowRule>,
}

impl Default for Fcitx5PluginConfig {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
}

impl FromConfig for Fcitx5PluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        if config.fcitx5.is_empty() {
            None
        } else {
            Some(Self {
                rules: config.fcitx5.clone(),
            })
        }
    }
}

/// Fcitx5 window rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fcitx5WindowRule {
    /// Regex pattern(s) to match app_id (optional, can be a string or list of strings)
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub app_id: Option<Vec<String>>,
    /// Regex pattern(s) to match title (optional, can be a string or list of strings)
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub title: Option<Vec<String>>,
    /// Input mode to switch to when this window is focused ("english" or "chinese")
    pub input_mode: String,
}

/// Fcitx5 plugin that automatically switches input mode based on focused window
pub struct Fcitx5Plugin {
    niri: NiriIpc,
    config: Fcitx5PluginConfig,
    /// Window matcher cache for regex pattern matching
    matcher_cache: Arc<WindowMatcherCache>,
    /// Last window ID that triggered input mode switch
    last_focused_window: Option<u64>,
    /// Throttle for input mode switching
    switch_throttle: Throttle,
    /// Last window ID that was processed (for throttling)
    last_handled_window: Option<u64>,
    /// Throttle for handle_focus_change
    handle_throttle: Throttle,
}

impl Fcitx5Plugin {
    /// Get current fcitx5 input mode
    fn get_current_input_mode() -> Result<Option<Fcitx5InputMode>> {
        let output = Command::new("fcitx5-remote")
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute fcitx5-remote: {}", e))?;

        if output.status.success() {
            let status_str = String::from_utf8_lossy(&output.stdout);
            let status = status_str.trim().parse::<i32>().ok();
            Ok(status.and_then(|s| Fcitx5InputMode::from_status(s)))
        } else {
            warn!(
                "fcitx5-remote command failed with status: {}",
                output.status
            );
            Ok(None)
        }
    }

    /// Set fcitx5 input mode
    fn set_input_mode(mode: Fcitx5InputMode) -> Result<()> {
        let arg = match mode {
            Fcitx5InputMode::English => "-c",
            Fcitx5InputMode::Chinese => "-o",
        };
        let output = Command::new("fcitx5-remote")
            .arg(arg)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute fcitx5-remote: {}", e))?;

        if output.status.success() {
            debug!("Set fcitx5 input mode to: {:?}", mode);
            Ok(())
        } else {
            warn!(
                "fcitx5-remote command failed with status: {}",
                output.status
            );
            Err(anyhow::anyhow!("fcitx5-remote command failed"))
        }
    }

    /// Handle focus change for a window
    async fn handle_focus_change(&mut self, window_id: u64) -> Result<()> {
        // Check if this is a programmatic focus change (e.g., from auto_fill)
        if window_utils::should_ignore_focus_change() {
            debug!(
                "Ignoring programmatic focus change for window {}",
                window_id
            );
            return Ok(());
        }

        // Global throttle: prevent processing focus changes too frequently
        if !self.handle_throttle.check_and_update(Duration::from_millis(200)) {
            return Ok(());
        }

        // Update tracking before processing
        self.last_handled_window = Some(window_id);

        let windows = self.niri.get_windows().await?;
        let window = match windows.into_iter().find(|w| w.id == window_id) {
            std::option::Option::Some(w) => w,
            std::option::Option::None => {
                // Window not found - this is normal when a window is closing or has just closed
                // Silently return instead of erroring
                return Ok(());
            }
        };

        let rules = self.config.rules.clone();
        for rule in rules.iter() {
            let matcher = WindowMatcher::new(rule.app_id.as_deref(), rule.title.as_deref());
            if self
                .matcher_cache
                .matches(window.app_id.as_ref(), Some(&window.title), &matcher)?
            {
                // Parse input mode from rule
                let target_mode = match rule.input_mode.to_lowercase().as_str() {
                    "english" | "en" => Fcitx5InputMode::English,
                    "chinese" | "cn" | "zh" => Fcitx5InputMode::Chinese,
                    _ => {
                        warn!("Invalid input mode in rule: {}", rule.input_mode);
                        continue;
                    }
                };

                // Get current mode
                if let Some(current_mode) = Self::get_current_input_mode()? {
                    if current_mode != target_mode {
                        // Throttle input mode switching
                        if self.switch_throttle.check_and_update(Duration::from_millis(200)) {
                            info!(
                                "Switching fcitx5 input mode from {:?} to {:?} for window {} ({:?})",
                                current_mode, target_mode, window_id, window.app_id
                            );
                            Self::set_input_mode(target_mode)?;
                            self.last_focused_window = Some(window_id);
                        }
                    }
                }
                return Ok(());
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for Fcitx5Plugin {
    type Config = Fcitx5PluginConfig;

    fn new(niri: NiriIpc, config: Fcitx5PluginConfig) -> Self {
        info!(
            "Fcitx5 plugin initialized with {} rules",
            config.rules.len()
        );
        Self {
            niri,
            config,
            matcher_cache: Arc::new(WindowMatcherCache::new()),
            last_focused_window: None,
            switch_throttle: Throttle::new(),
            last_handled_window: None,
            handle_throttle: Throttle::new(),
        }
    }

    async fn handle_event(&mut self, event: &PiriEvent, _niri: &NiriIpc) -> Result<()> {
        match event {
            PiriEvent::WindowFocusChanged { id: some_window_id } => {
                if let Some(window_id) = some_window_id {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    self.handle_focus_change(*window_id).await?;
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

    async fn update_config(&mut self, config: Fcitx5PluginConfig) -> Result<()> {
        info!(
            "Updating fcitx5 plugin configuration: {} rules",
            config.rules.len()
        );
        self.config = config;
        self.matcher_cache.clear_cache();
        Ok(())
    }
}
