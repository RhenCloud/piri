use crate::plugins::PiriEvent;
use anyhow::Result;
use log::{debug, info};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{Config, WindowRuleConfig};
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};
use crate::plugins::FromConfig;
use crate::utils::Throttle;

/// Window rule plugin config (for internal use)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowRulePluginConfig {
    /// List of window rules
    pub rules: Vec<WindowRuleConfig>,
}

impl FromConfig for WindowRulePluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        if config.window_rule.is_empty() {
            None
        } else {
            Some(Self {
                rules: config.window_rule.clone(),
            })
        }
    }
}

/// Window rule plugin that moves windows to workspaces based on app_id and title matching
pub struct WindowRulePlugin {
    niri: NiriIpc,
    config: WindowRulePluginConfig,
    /// Window matcher cache for regex pattern matching
    matcher_cache: Arc<WindowMatcherCache>,
    /// Last window ID that triggered focus command
    last_focused_window: Option<u64>,
    /// Throttle for focus command execution
    execution_throttle: Throttle,
    /// Set of rule indices that have already executed focus_command (when focus_command_once is true)
    executed_rules: HashSet<usize>,
    /// Last window ID that was processed by handle_focus_command (for throttling)
    last_handled_window: Option<u64>,
    /// Throttle for handle_focus_command
    handle_throttle: Throttle,
    /// Windows locked to a specific workspace (window_id -> workspace_name)
    locked_windows: HashMap<u64, String>,
}

impl WindowRulePlugin {
    /// Execute focus command with de-duplication
    async fn execute_focus_rule(
        &mut self,
        window_id: u64,
        focus_command: &str,
        rule_index: usize,
        focus_once: bool,
    ) -> Result<()> {
        // If focus_once is true and this rule has already executed focus_command, skip
        if focus_once && self.executed_rules.contains(&rule_index) {
            return Ok(());
        }

        // Global throttle: prevent executing focus_command too frequently regardless of window ID
        if self.execution_throttle.check_and_update(Duration::from_millis(200)) {
            info!(
                "Executing focus_command for window {}: {}",
                window_id, focus_command
            );
            window_utils::execute_command(focus_command)?;

            // Mark this rule as having executed focus_command if focus_once is true
            if focus_once {
                self.executed_rules.insert(rule_index);
            }

            self.last_focused_window = Some(window_id);
        }

        Ok(())
    }

    /// Handle focus command execution for currently focused window
    async fn handle_focus_command(&mut self, window_id: u64) -> Result<()> {
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

        let windows = self.niri.get_windows_raw().await?;
        let window = match windows.into_iter().find(|w| w.id == window_id) {
            Some(w) => w,
            None => {
                // Window not found - this is normal when a window is closing or has just closed
                // Silently return instead of erroring
                return Ok(());
            }
        };

        // Find matching rule without holding borrows on self
        let matched_rule = {
            let mut found = None;
            for (rule_index, rule) in self.config.rules.iter().enumerate() {
                if let Some(ref focus_command) = rule.focus_command {
                    let matcher = WindowMatcher::new(rule.app_id.as_deref(), rule.title.as_deref());
                    if self.matcher_cache.matches(
                        window.app_id.as_ref(),
                        Some(&window.title),
                        &matcher,
                    )? {
                        found = Some((rule_index, focus_command.clone(), rule.focus_command_once));
                        break;
                    }
                }
            }
            found
        };

        if let Some((rule_index, focus_command, focus_once)) = matched_rule {
            self.execute_focus_rule(window_id, &focus_command, rule_index, focus_once)
                .await?;
        }

        Ok(())
    }

    async fn handle_window_opened(&mut self, window: &niri_ipc::Window) -> Result<()> {
        // Find matching rule without holding borrows on self
        let matched_rule = {
            let mut found = None;
            for (rule_index, rule) in self.config.rules.iter().enumerate() {
                let matcher = WindowMatcher::new(rule.app_id.as_deref(), rule.title.as_deref());
                if self.matcher_cache.matches(
                    window.app_id.as_ref(),
                    window.title.as_ref(),
                    &matcher,
                )? {
                    found = Some((
                        rule_index,
                        rule.open_on_workspace.clone(),
                        rule.focus_command.clone(),
                        rule.focus_command_once,
                    ));
                    break;
                }
            }
            found
        };

        if let Some((rule_index, open_on_workspace, focus_command, focus_once)) = matched_rule {
            // 1. Move to workspace if specified
            if let Some(ref workspace_name) = open_on_workspace {
                // Check for lock suffix '!'
                let (target, locked) = if let Some(name) = workspace_name.strip_suffix('!') {
                    (name, true)
                } else {
                    (workspace_name.as_str(), false)
                };

                window_utils::move_window_to_named_workspace(&self.niri, window, target).await?;

                if locked {
                    info!("Window {} locked to workspace '{}'", window.id, target);
                    self.locked_windows.insert(window.id, target.to_string());
                }
            }

            // 2. Execute focus command if specified (unified de-duplication)
            if let Some(ref focus_command) = focus_command {
                self.execute_focus_rule(window.id, focus_command, rule_index, focus_once)
                    .await?;
            }
        }
        Ok(())
    }

    /// Check if a locked window is still on the correct workspace; move it back if not.
    async fn enforce_workspace_lock(&self, window: &niri_ipc::Window) -> Result<()> {
        let Some(target_workspace) = self.locked_windows.get(&window.id) else {
            return Ok(());
        };

        // Resolve the target workspace ID
        let workspaces = self.niri.get_workspaces_for_mapping().await?;
        let (target_name, want_output) = target_workspace
            .split_once('@')
            .map(|(name, output)| (name, Some(output)))
            .unwrap_or((target_workspace.as_str(), None));

        let focused_output = self.niri.get_focused_output().await.ok().map(|o| o.name);

        let find_on_output = |output_name: &str| -> Option<&niri_ipc::Workspace> {
            workspaces.iter().find(|ws| {
                ws.output.as_deref().is_some_and(|o| {
                    o == output_name || super::extract_display_prefix(o) == Some(output_name)
                }) && (ws.name.as_deref() == Some(target_name) || ws.idx.to_string() == target_name)
            })
        };

        let matched_ws = if let Some(want) = want_output {
            find_on_output(want)
        } else {
            focused_output.as_deref().and_then(find_on_output).or_else(|| {
                workspaces.iter().find(|ws| {
                    ws.name.as_deref() == Some(target_name) || ws.idx.to_string() == target_name
                })
            })
        };

        if let Some(target) = matched_ws {
            if window.workspace_id != Some(target.id) {
                info!(
                    "Window {} escaped workspace '{}', moving back",
                    window.id, target_workspace
                );
                window_utils::move_window_to_named_workspace(&self.niri, window, target_workspace)
                    .await?;
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for WindowRulePlugin {
    type Config = WindowRulePluginConfig;

    fn new(niri: NiriIpc, config: WindowRulePluginConfig) -> Self {
        info!(
            "Window rule plugin initialized with {} rules",
            config.rules.len()
        );
        Self {
            niri,
            config,
            matcher_cache: Arc::new(WindowMatcherCache::new()),
            last_focused_window: None,
            execution_throttle: Throttle::new(),
            executed_rules: HashSet::new(),
            last_handled_window: None,
            handle_throttle: Throttle::new(),
            locked_windows: HashMap::new(),
        }
    }

    async fn handle_event(&mut self, event: &PiriEvent, _niri: &NiriIpc) -> Result<()> {
        match event {
            PiriEvent::WindowFocusChanged {
                id: Some(window_id),
            } => {
                tokio::time::sleep(Duration::from_millis(10)).await;
                self.handle_focus_command(*window_id).await?;
            }
            PiriEvent::WindowOpened { window } => {
                self.handle_window_opened(window).await?;
            }
            PiriEvent::WindowChanged { window } => {
                self.enforce_workspace_lock(window).await.ok();
            }
            PiriEvent::WindowClosed { id } => {
                self.locked_windows.remove(id);
            }
            _ => {}
        }
        Ok(())
    }

    fn is_interested_in_event(&self, event: &PiriEvent) -> bool {
        matches!(
            event,
            PiriEvent::WindowOpened { .. }
                | PiriEvent::WindowChanged { .. }
                | PiriEvent::WindowClosed { .. }
                | PiriEvent::WindowFocusChanged { id: Some(_) }
        )
    }

    async fn update_config(&mut self, config: WindowRulePluginConfig) -> Result<()> {
        info!(
            "Updating window rule plugin configuration: {} rules",
            config.rules.len()
        );
        self.config = config;
        self.matcher_cache.clear_cache();
        // Clear executed rules tracking since rule indices may have changed
        self.executed_rules.clear();
        Ok(())
    }
}
