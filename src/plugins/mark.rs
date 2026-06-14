use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info};
use std::collections::HashMap;

use crate::config::Config;
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, get_focused_window};
use crate::plugins::FromConfig;

/// Runtime-only plugin: marks are not persisted to disk.
#[derive(Debug, Clone)]
pub struct MarkPluginConfig {
    pub refocus: bool,
}

impl FromConfig for MarkPluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        Some(Self {
            refocus: config.piri.mark.refocus,
        })
    }
}

pub struct MarkPlugin {
    niri: NiriIpc,
    /// Mark name → window id
    marks: HashMap<String, u64>,
    /// Previous focused window id (for refocus feature)
    previous_window: Option<u64>,
    /// Enable refocus feature
    refocus: bool,
}

impl MarkPlugin {
    fn new(niri: NiriIpc, config: MarkPluginConfig) -> Self {
        info!("Mark plugin initialized (refocus: {})", config.refocus);
        Self {
            niri,
            marks: HashMap::new(),
            previous_window: None,
            refocus: config.refocus,
        }
    }

    async fn bind_focused(&mut self, name: &str) -> Result<()> {
        let window = get_focused_window(&self.niri).await?;
        debug!("Mark '{}' → window {}", name, window.id);
        self.marks.insert(name.to_string(), window.id);
        Ok(())
    }

    /// If `name` points to a live window, focus it; otherwise store the current focus under `name`.
    async fn toggle(&mut self, name: &str) -> Result<()> {
        // Fetch windows once for both existence check and focused window lookup
        let windows = self.niri.get_windows_raw().await?;
        let focus_existing = self
            .marks
            .get(name)
            .copied()
            .map(|id| window_utils::window_exists_in_cache(&windows, id))
            .unwrap_or(false);

        if focus_existing {
            let id = self
                .marks
                .get(name)
                .copied()
                .context("internal: mark disappeared after existence check")?;

            // Try refocus first if enabled
            if self.refocus
                && window_utils::try_refocus_to_previous(&self.niri, id, &mut self.previous_window)
                    .await?
            {
                return Ok(());
            }

            // If refocus didn't happen, save current window as previous and focus the mark window
            if let Ok(current) =
                window_utils::get_focused_window_from_cache(&self.niri, &windows).await
            {
                debug!("Saving previous window {} before focusing mark", current.id);
                self.previous_window = Some(current.id);
            } else {
                // No focused window (empty workspace), clear previous_window
                debug!("No focused window, clearing previous_window");
                self.previous_window = None;
            }

            window_utils::focus_window(self.niri.clone(), id).await?;
        } else {
            self.bind_focused(name).await?;
        }
        Ok(())
    }

    fn delete(&mut self, name: &str) {
        self.marks.remove(name);
    }

    async fn add(&mut self, name: &str) -> Result<()> {
        self.bind_focused(name).await
    }
}

#[async_trait]
impl crate::plugins::Plugin for MarkPlugin {
    type Config = MarkPluginConfig;

    fn new(niri: NiriIpc, config: Self::Config) -> Self {
        Self::new(niri, config)
    }

    async fn update_config(&mut self, config: Self::Config) -> Result<()> {
        self.refocus = config.refocus;
        info!("Mark plugin updated (refocus: {})", self.refocus);
        Ok(())
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::MarkToggle { name } => {
                info!("Mark toggle: {}", name);
                self.toggle(name).await?;
                Ok(Some(Ok(())))
            }
            IpcRequest::MarkDelete { name } => {
                info!("Mark delete: {}", name);
                self.delete(name);
                Ok(Some(Ok(())))
            }
            IpcRequest::MarkAdd { name } => {
                info!("Mark add: {}", name);
                self.add(name).await?;
                Ok(Some(Ok(())))
            }
            _ => Ok(None),
        }
    }
}
