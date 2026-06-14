use crate::plugins::PiriEvent;
use anyhow::Result;
use log::info;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::niri::NiriIpc;
use crate::plugins::resolve_workspace_config;
use crate::plugins::{window_utils, FromConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmptyPluginConfig {
    pub workspaces: HashMap<String, String>,
}

impl FromConfig for EmptyPluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        let workspaces = if !config.empty.is_empty() {
            let mut workspaces = HashMap::new();
            for (workspace, cfg) in &config.empty {
                workspaces.insert(workspace.clone(), cfg.command.clone());
            }
            workspaces
        } else {
            config
                .piri
                .plugins
                .empty_config
                .clone()
                .map(|c| c.workspaces)
                .unwrap_or_default()
        };

        if workspaces.is_empty() {
            None
        } else {
            Some(EmptyPluginConfig { workspaces })
        }
    }
}

pub struct EmptyPlugin {
    niri: NiriIpc,
    config: EmptyPluginConfig,
}

impl EmptyPlugin {
    async fn handle_event_internal(&self, event: &PiriEvent) -> Result<()> {
        let (id, focused) = match event {
            PiriEvent::WorkspaceActivated { id, focused } => (*id, *focused),
            _ => return Ok(()),
        };

        if !focused {
            return Ok(());
        }

        if let Some(focused_ws) =
            window_utils::get_focused_workspace_from_event(&self.niri, id).await?
        {
            let is_empty = window_utils::is_workspace_empty(&self.niri, focused_ws.id).await?;

            if is_empty {
                let command_opt = resolve_workspace_config(
                    &self.config.workspaces,
                    focused_ws.idx,
                    focused_ws.name.as_deref(),
                    focused_ws.output.as_deref(),
                );
                let idx_str = focused_ws.idx.to_string();

                if let Some(cmd) = command_opt {
                    info!(
                        "Workspace {} matches empty rule, executing: {}",
                        idx_str, cmd
                    );
                    window_utils::execute_command(cmd)?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for EmptyPlugin {
    type Config = EmptyPluginConfig;

    fn new(niri: NiriIpc, config: EmptyPluginConfig) -> Self {
        info!(
            "Empty plugin initialized with {} rules",
            config.workspaces.len()
        );
        Self { niri, config }
    }

    async fn handle_event(&mut self, event: &PiriEvent, _niri: &NiriIpc) -> Result<()> {
        self.handle_event_internal(event).await
    }

    fn is_interested_in_event(&self, event: &PiriEvent) -> bool {
        matches!(event, PiriEvent::WorkspaceActivated { .. })
    }

    async fn update_config(&mut self, config: EmptyPluginConfig) -> Result<()> {
        info!(
            "Updating empty plugin configuration: {} rules",
            config.workspaces.len()
        );
        self.config = config;
        Ok(())
    }
}
