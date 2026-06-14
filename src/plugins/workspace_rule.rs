use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use niri_ipc::{Action, Reply, Request, SizeChange};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::time::Duration;

use crate::config::{Config, EdgePulseConfig, WorkspaceRuleConfig, WorkspaceRuleSection};
use crate::niri::NiriIpc;
use crate::plugins::edge_pulse_renderer::{EdgePulseRenderState, EdgePulseRenderer};
use crate::plugins::resolve_workspace_config;
use crate::plugins::window_utils::perform_swallow;
use crate::plugins::FromConfig;
use crate::utils::Throttle;
use niri_ipc::ColumnDisplay;

struct AutofillGuard {
    flag: Arc<StdMutex<bool>>,
}

impl AutofillGuard {
    fn new(flag: Arc<StdMutex<bool>>) -> Self {
        Self { flag }
    }
}

impl Drop for AutofillGuard {
    fn drop(&mut self) {
        if let Ok(mut executing) = self.flag.try_lock() {
            *executing = false;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceRulePluginConfig {
    pub default: WorkspaceRuleSection,
    pub workspaces: HashMap<String, WorkspaceRuleConfig>,
}

impl FromConfig for WorkspaceRulePluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        // Check if there's any configuration (either default or workspace-specific)
        let has_default = !config.piri.workspace_rule.auto_width.is_empty()
            || config.piri.workspace_rule.auto_tile
            || config.piri.workspace_rule.auto_fill
            || config.piri.workspace_rule.auto_maximize
            || config.piri.workspace_rule.edge_pulse.enabled;
        let has_workspaces = !config.workspace_rule.is_empty()
            || config
                .workspace_rule
                .values()
                .any(|c| c.auto_tile || c.auto_fill || c.auto_maximize || c.edge_pulse.enabled);

        if !has_default && !has_workspaces {
            return None;
        }

        Some(Self {
            default: config.piri.workspace_rule.clone(),
            workspaces: config.workspace_rule.clone(),
        })
    }
}

pub struct WorkspaceRulePlugin {
    niri: NiriIpc,
    config: WorkspaceRulePluginConfig,
    previous_layouts: HashMap<u64, niri_ipc::WindowLayout>,
    window_floating_state: HashMap<u64, bool>,
    maximized_windows: HashSet<u64>,
    auto_tiled_windows: HashSet<u64>,
    previous_window_sizes: HashMap<u64, (i32, i32)>,
    apply_widths_throttle: Arc<StdMutex<Throttle>>,
    autofill_executing: Arc<StdMutex<bool>>,
    edge_pulse_last_render: Option<EdgePulseRenderState>,
    edge_pulse_renderer: EdgePulseRenderer,
}

impl WorkspaceRulePlugin {
    fn hide_edge_pulse(&mut self) -> Result<()> {
        let hidden = EdgePulseRenderState {
            show_left: false,
            show_right: false,
        };
        if self.edge_pulse_last_render != Some(hidden) {
            self.edge_pulse_renderer
                .render(hidden, &self.config.default.edge_pulse, None, 1)?;
            self.edge_pulse_last_render = Some(hidden);
        }
        Ok(())
    }

    fn parse_width(width_str: &str) -> Result<f64> {
        let percent = width_str
            .strip_suffix('%')
            .with_context(|| format!("Width must end with '%', got: {}", width_str))?
            .parse::<f64>()
            .with_context(|| format!("Invalid number in width '{}'", width_str))?;

        if !(0.0..=100.0).contains(&percent) {
            anyhow::bail!("Width must be 0-100%, got: {}%", percent);
        }

        Ok(percent)
    }

    fn filter_tiled_windows_in_workspace(
        windows: &[crate::niri::Window],
        workspace_id: u64,
    ) -> Vec<&crate::niri::Window> {
        windows
            .iter()
            .filter(|w| !w.floating && w.workspace_id == Some(workspace_id))
            .collect()
    }

    async fn try_execute_autofill(
        &self,
        idx: u8,
        name: Option<&str>,
        output: Option<&str>,
        reason: &str,
    ) -> Result<()> {
        if !self.get_auto_fill(idx, name, output) {
            return Ok(());
        }

        {
            let mut executing = self.autofill_executing.lock().unwrap_or_else(|e| e.into_inner());
            if *executing {
                debug!("Autofill ignored: already executing");
                return Ok(());
            }
            *executing = true;
        }

        info!("Auto_fill: triggered by {} in workspace {}", reason, idx);

        tokio::time::sleep(Duration::from_millis(100)).await;

        self.check_and_align_last_column()
            .await
            .map_err(|e| {
                warn!("Auto_fill: failed to align columns: {}", e);
                e
            })
            .ok();

        Ok(())
    }

    /// Get auto_width configuration for a workspace
    fn get_auto_width(
        &self,
        idx: u8,
        name: Option<&str>,
        output: Option<&str>,
    ) -> &Vec<Vec<String>> {
        resolve_workspace_config(&self.config.workspaces, idx, name, output)
            .map(|c| &c.auto_width)
            .unwrap_or(&self.config.default.auto_width)
    }

    fn get_auto_tile(&self, idx: u8, name: Option<&str>, output: Option<&str>) -> bool {
        resolve_workspace_config(&self.config.workspaces, idx, name, output)
            .map(|c| c.auto_tile)
            .unwrap_or(self.config.default.auto_tile)
    }

    fn get_auto_fill(&self, idx: u8, name: Option<&str>, output: Option<&str>) -> bool {
        resolve_workspace_config(&self.config.workspaces, idx, name, output)
            .map(|c| c.auto_fill)
            .unwrap_or(self.config.default.auto_fill)
    }

    fn get_auto_maximize(&self, idx: u8, name: Option<&str>, output: Option<&str>) -> bool {
        resolve_workspace_config(&self.config.workspaces, idx, name, output)
            .map(|c| c.auto_maximize)
            .unwrap_or(self.config.default.auto_maximize)
    }

    fn get_edge_pulse_config(
        &self,
        idx: u8,
        name: Option<&str>,
        output: Option<&str>,
    ) -> &EdgePulseConfig {
        resolve_workspace_config(&self.config.workspaces, idx, name, output)
            .map(|c| &c.edge_pulse)
            .unwrap_or(&self.config.default.edge_pulse)
    }

    fn collect_workspace_columns_by_id(windows: &[crate::niri::Window], ws_id: u64) -> Vec<usize> {
        let mut columns: Vec<usize> = windows
            .iter()
            .filter(|w| !w.floating && w.workspace_id == Some(ws_id))
            .filter_map(|w| {
                w.layout
                    .as_ref()
                    .and_then(|layout| layout.pos_in_scrolling_layout)
                    .map(|(column, _)| column)
            })
            .collect();

        columns.sort_unstable();
        columns.dedup();
        columns
    }

    async fn sync_edge_pulse_indicator(&mut self, workspace_id: Option<u64>) -> Result<()> {
        // Resolve both workspace name (for config lookup) and ID (for window filtering).
        // Workspace ID is globally unique; idx is per-output and not unique across monitors.
        let (ws_idx, ws_name, ws_output, ws_id) = if let Some(id) = workspace_id {
            let workspaces = self.niri.get_workspaces().await?;
            match workspaces.into_iter().find(|ws| ws.id == id) {
                Some(ws) => (ws.idx, ws.name, ws.output, ws.id),
                None => {
                    self.hide_edge_pulse()?;
                    return Ok(());
                }
            }
        } else {
            let workspaces = self.niri.get_workspaces().await?;
            match workspaces.into_iter().find(|ws| ws.is_focused) {
                Some(ws) => (ws.idx, ws.name, ws.output, ws.id),
                None => {
                    self.hide_edge_pulse()?;
                    return Ok(());
                }
            }
        };
        let edge_cfg = self
            .get_edge_pulse_config(ws_idx, ws_name.as_deref(), ws_output.as_deref())
            .clone();

        if !edge_cfg.enabled {
            if let Some(prev) = self.edge_pulse_last_render.take() {
                if prev.show_left || prev.show_right {
                    info!(
                        "EdgePulse disabled in workspace {}, hiding indicator",
                        ws_name.as_deref().unwrap_or("unknown")
                    );
                    self.hide_edge_pulse()?;
                }
            }
            return Ok(());
        }

        let Some(focused_window_id) = self.niri.get_focused_window_id().await? else {
            self.hide_edge_pulse()?;
            return Ok(());
        };

        let windows = self.niri.get_windows_raw().await?;
        let columns = Self::collect_workspace_columns_by_id(&windows, ws_id);

        // Single column — no edge indicators needed
        if columns.len() <= 1 {
            self.hide_edge_pulse()?;
            return Ok(());
        }

        let focused_col = windows
            .iter()
            .find(|w| w.id == focused_window_id && !w.floating && w.workspace_id == Some(ws_id))
            .and_then(|w| w.layout.as_ref())
            .and_then(|layout| layout.pos_in_scrolling_layout.map(|(col, _)| col));

        let Some(focused_col) = focused_col else {
            // Focused window is floating or not tiled — keep current indicator state
            if windows
                .iter()
                .any(|w| w.id == focused_window_id && w.floating && w.workspace_id == Some(ws_id))
            {
                return Ok(());
            }
            self.hide_edge_pulse()?;
            return Ok(());
        };

        let has_left = columns.iter().any(|col| *col < focused_col);
        let has_right = columns.iter().any(|col| *col > focused_col);

        let state = EdgePulseRenderState {
            show_left: edge_cfg.show_left && !has_left,
            show_right: edge_cfg.show_right && !has_right,
        };

        if self.edge_pulse_last_render == Some(state) {
            return Ok(());
        }

        self.edge_pulse_last_render = Some(state);
        let focused_output = self.niri.get_focused_output().await.ok();
        let target_output_name = focused_output.as_ref().map(|o| o.name.clone());
        let output_height = focused_output
            .as_ref()
            .and_then(|o| o.logical.as_ref().map(|l| l.height as i32))
            .unwrap_or(1080);
        self.edge_pulse_renderer.render(
            state,
            &edge_cfg,
            target_output_name.as_deref(),
            output_height,
        )?;
        info!(
            "EdgePulse {} => left={}, right={}, ws={}, focused_col={}, style(width={}, height_ratio={}, alpha={}, left=[{} -> {}], right=[{} -> {}])",            if state.show_left || state.show_right {
                "show"
            } else {
                "hide"
            },
            state.show_left,
            state.show_right,
            ws_idx,
            focused_col,
            edge_cfg.width,
            edge_cfg.height_ratio,
            edge_cfg.alpha,
            edge_cfg.left_gradient_start,
            edge_cfg.left_gradient_end,
            edge_cfg.right_gradient_start,
            edge_cfg.right_gradient_end
        );

        Ok(())
    }

    /// Handle auto_tile logic: merge new windows into existing columns (except first column).
    /// Returns `Ok(true)` if the window was merged into an existing column.
    async fn handle_auto_tile(&mut self, new_window: &crate::niri::Window) -> Result<bool> {
        let current_ws = self.niri.get_focused_workspace().await?;
        let ws_idx = current_ws.idx;
        let ws_name = &current_ws.name;
        let ws_output = current_ws.output.as_deref();
        let ws_id = current_ws.id;

        if !self.get_auto_tile(ws_idx, Some(ws_name.as_str()), ws_output) {
            debug!("Auto_tile is not enabled for workspace {}", ws_name);
            return Ok(false);
        }

        info!(
            "Auto_tile: processing new window {} in workspace {}",
            new_window.id, ws_name
        );

        // Get all windows in the workspace (excluding the new window)
        let windows = self.niri.get_windows().await?;
        let ws_windows: Vec<_> = Self::filter_tiled_windows_in_workspace(&windows, ws_id)
            .into_iter()
            .filter(|w| w.id != new_window.id)
            .collect();

        // Group existing windows by column
        let mut columns: HashMap<usize, Vec<&crate::niri::Window>> = HashMap::new();
        for w in &ws_windows {
            if let Some((col, _)) = w.layout.as_ref().and_then(|l| l.pos_in_scrolling_layout) {
                columns.entry(col).or_default().push(w);
            }
        }

        // Find the first non-first column that has exactly one window
        let mut target_col: Option<usize> = None;
        let mut target_window: Option<&crate::niri::Window> = None;

        for (col, windows_in_col) in &columns {
            // Skip first column
            if *col == 1 {
                continue;
            }
            // If this column has exactly one window, we can merge the new window here
            if windows_in_col.len() == 1 {
                target_col = Some(*col);
                target_window = Some(windows_in_col[0]);
                break;
            }
        }

        // If we found a target column, merge the new window into it
        if let (Some(col), Some(parent_window)) = (target_col, target_window) {
            info!(
                "Auto-tiling: merging window {} into column {} with parent window {}",
                new_window.id, col, parent_window.id
            );

            perform_swallow(
                &self.niri,
                parent_window,
                new_window,
                new_window.id,
                ColumnDisplay::Normal,
            )
            .await?;
            Ok(true)
        } else {
            debug!(
                "Auto-tile: no suitable column found for window {} (all non-first columns are full or empty)",
                new_window.id
            );
            Ok(false)
        }
    }

    /// Apply width adjustments to windows in current workspace
    /// The logic is based on column count, not window count (a column may have multiple windows)
    async fn apply_widths(&mut self) -> Result<()> {
        let current_ws = self.niri.get_focused_workspace().await?;
        let ws_idx = current_ws.idx;
        let ws_name = &current_ws.name;
        let ws_output = current_ws.output.as_deref();
        let ws_id = current_ws.id;
        let windows = self.niri.get_windows().await?;

        // 1. Filter tiled windows in current workspace
        let ws_windows = Self::filter_tiled_windows_in_workspace(&windows, ws_id);

        // 2. Group windows by column (one window ID per column is enough)
        // Calculate columns early for use throughout the function
        let columns: HashMap<usize, u64> = ws_windows
            .iter()
            .filter_map(|w| {
                w.layout
                    .as_ref()
                    .and_then(|l| l.pos_in_scrolling_layout)
                    .map(|(col, _)| (col, w.id))
            })
            .collect();

        let column_count = columns.len();

        // 3. Handle auto_maximize: maximize when only one window, unmaximize when multiple windows
        if self.get_auto_maximize(ws_idx, Some(ws_name.as_str()), ws_output) {
            match ws_windows.len() {
                0 => return Ok(()), // No windows, nothing to do
                1 => {
                    // Only one window: maximize it to edges
                    let window_id = ws_windows[0].id;

                    // Skip if already maximized to maintain state
                    if self.maximized_windows.contains(&window_id) {
                        debug!("Window {} already maximized, skipping", window_id);
                        return Ok(());
                    }

                    info!(
                        "Auto-maximize: maximizing window {} (only window)",
                        window_id
                    );

                    self.niri
                        .send_action(Action::MaximizeWindowToEdges {
                            id: Some(window_id),
                        })
                        .await
                        .map_err(|e| warn!("Failed to maximize window {}: {}", window_id, e))
                        .ok();

                    self.maximized_windows.insert(window_id);
                    return Ok(());
                }
                _ => {
                    // Multiple windows: check if auto_width is configured
                    let auto_width = self.get_auto_width(ws_idx, Some(ws_name.as_str()), ws_output);
                    let has_width_config = column_count > 0
                        && column_count <= 5
                        && auto_width.get(column_count.saturating_sub(1)).is_some();

                    if !has_width_config {
                        // No auto_width config: unmaximize all maximized windows
                        for window in &ws_windows {
                            if self.maximized_windows.remove(&window.id) {
                                info!(
                                    "Auto-maximize: unmaximizing window {} (multiple windows, no auto_width)",
                                    window.id
                                );
                                // Cancel maximize by toggling (MaximizeWindowToEdges is a toggle)
                                self.niri
                                    .send_action(Action::MaximizeWindowToEdges {
                                        id: Some(window.id),
                                    })
                                    .await
                                    .map_err(|e| {
                                        warn!("Failed to unmaximize window {}: {}", window.id, e)
                                    })
                                    .ok();
                            }
                        }
                        return Ok(());
                    } else {
                        // Has auto_width config: remove maximized tracking (width adjustment will handle)
                        for window in &ws_windows {
                            if self.maximized_windows.remove(&window.id) {
                                info!(
                                    "Auto-maximize: unmaximizing window {} (multiple windows, has auto_width)",
                                    window.id
                                );
                            }
                        }
                    }
                }
            }
        }

        if column_count == 0 || column_count > 5 {
            return Ok(());
        }

        // 4. Get width configuration
        let auto_width = self.get_auto_width(ws_idx, Some(ws_name.as_str()), ws_output);
        let width_config = if let Some(config) = auto_width.get(column_count.saturating_sub(1)) {
            config
        } else {
            // No width config for this column count, nothing to do
            debug!(
                "No width config for {} columns in workspace {}, skipping",
                column_count, ws_name
            );
            return Ok(());
        };

        info!(
            "Applying width adjustment for {} columns ({} windows) in workspace {}: {:?}",
            column_count,
            ws_windows.len(),
            ws_name,
            width_config
        );

        // 5. Sort columns and apply widths
        let mut sorted_cols: Vec<_> = columns.into_iter().collect();
        sorted_cols.sort_unstable_by_key(|(idx, _)| *idx);

        for (i, (col_idx, win_id)) in sorted_cols.into_iter().enumerate() {
            let width_str = width_config
                .get(i)
                .or_else(|| width_config.last())
                .context("Width configuration cannot be empty")?;

            let percent = Self::parse_width(width_str)?;
            debug!(
                "Setting column {} (window {}) width to {}%",
                col_idx, win_id, percent
            );

            self.niri
                .send_action(Action::SetWindowWidth {
                    id: Some(win_id),
                    change: SizeChange::SetProportion(percent),
                })
                .await
                .map_err(|e| warn!("Failed to set column {} width: {}", col_idx, e))
                .ok();
        }

        Ok(())
    }

    async fn check_and_align_last_column(&self) -> Result<()> {
        debug!("Autofill: aligning columns in current workspace");

        crate::plugins::window_utils::mark_programmatic_focus_start();

        let _guard = AutofillGuard::new(Arc::clone(&self.autofill_executing));

        self.niri
            .execute_batch(|socket| {
                let focused_window_id =
                    socket.send(Request::FocusedWindow).ok().and_then(|reply| match reply {
                        Reply::Ok(niri_ipc::Response::FocusedWindow(Some(w))) => Some(w.id),
                        _ => None,
                    });

                let _ = socket.send(Request::Action(Action::FocusColumnFirst {}))?;

                let action = if let Some(window_id) = focused_window_id {
                    Action::FocusWindow { id: window_id }
                } else {
                    Action::FocusColumnLast {}
                };
                let _ = socket.send(Request::Action(action))?;

                Ok(())
            })
            .await
    }

    async fn schedule_apply_widths(&mut self) -> Result<()> {
        let should_run = self
            .apply_widths_throttle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .check_and_update_no_reset(Duration::from_millis(200));

        if should_run {
            self.apply_widths().await?;
        }
        Ok(())
    }

    async fn on_window_opened(&mut self, window: &niri_ipc::Window) -> Result<()> {
        let current_ws = self.niri.get_focused_workspace().await?;
        let ws_idx = current_ws.idx;
        let ws_name = &current_ws.name;
        let ws_output = current_ws.output.as_deref();

        self.previous_window_sizes.insert(window.id, window.layout.window_size);
        self.window_floating_state.insert(window.id, window.is_floating);

        if window.is_floating {
            debug!("New floating window: {}", window.id);
        } else {
            debug!("New tiled window: {}", window.id);
            let windows = self.niri.get_windows_raw().await?;
            if let Some(full_window) = windows.iter().find(|w| w.id == window.id) {
                let auto_tiled = self.handle_auto_tile(full_window).await.unwrap_or(false);
                if auto_tiled {
                    self.auto_tiled_windows.insert(window.id);
                }
                if !auto_tiled {
                    self.schedule_apply_widths().await?;
                    self.try_execute_autofill(
                        ws_idx,
                        Some(ws_name.as_str()),
                        ws_output,
                        "window opened",
                    )
                    .await?;
                } else {
                    self.schedule_apply_widths().await?;
                }
            }
        }

        self.sync_edge_pulse_indicator(None).await?;
        Ok(())
    }

    async fn on_window_changed(&mut self, _window: &niri_ipc::Window) -> Result<()> {
        self.sync_edge_pulse_indicator(None).await?;
        Ok(())
    }

    async fn on_window_toggle_floating(&mut self, window: &niri_ipc::Window) -> Result<()> {
        self.window_floating_state.insert(window.id, window.is_floating);

        let current_ws = self.niri.get_focused_workspace().await?;
        let ws_idx = current_ws.idx;
        let ws_name = &current_ws.name;
        let ws_output = current_ws.output.as_deref();

        self.schedule_apply_widths().await?;

        if !window.is_floating {
            // Moved to tiled — try auto_tile and auto_fill
            let windows = self.niri.get_windows_raw().await?;
            if let Some(full_window) = windows.iter().find(|w| w.id == window.id) {
                let auto_tiled = self.handle_auto_tile(full_window).await.unwrap_or(false);
                if auto_tiled {
                    self.auto_tiled_windows.insert(window.id);
                }
                if !auto_tiled {
                    self.try_execute_autofill(
                        ws_idx,
                        Some(ws_name.as_str()),
                        ws_output,
                        "window moved to tiled",
                    )
                    .await?;
                }
            }
        }

        self.sync_edge_pulse_indicator(None).await?;
        Ok(())
    }

    async fn handle_window_closed(&mut self, window_id: u64) -> Result<()> {
        self.previous_layouts.remove(&window_id);
        self.window_floating_state.remove(&window_id);
        self.maximized_windows.remove(&window_id);
        self.auto_tiled_windows.remove(&window_id);
        self.previous_window_sizes.remove(&window_id);

        debug!("Window {} closed, applying width adjustments", window_id);
        self.schedule_apply_widths().await?;

        let current_ws = self.niri.get_focused_workspace().await?;
        let ws_idx = current_ws.idx;
        let ws_name = &current_ws.name;
        let ws_output = current_ws.output.as_deref();
        self.try_execute_autofill(ws_idx, Some(ws_name.as_str()), ws_output, "window closed")
            .await?;
        self.sync_edge_pulse_indicator(None).await?;

        Ok(())
    }
}

#[async_trait]
impl crate::plugins::Plugin for WorkspaceRulePlugin {
    type Config = WorkspaceRulePluginConfig;

    fn new(niri: NiriIpc, config: WorkspaceRulePluginConfig) -> Self {
        info!(
            "Workspace rule plugin initialized ({} rules)",
            config.workspaces.len()
        );
        Self {
            niri,
            config,
            previous_layouts: HashMap::new(),
            window_floating_state: HashMap::new(),
            maximized_windows: HashSet::new(),
            auto_tiled_windows: HashSet::new(),
            previous_window_sizes: HashMap::new(),
            apply_widths_throttle: Arc::new(StdMutex::new(Throttle::new())),
            autofill_executing: Arc::new(StdMutex::new(false)),
            edge_pulse_last_render: None,
            edge_pulse_renderer: EdgePulseRenderer::new(),
        }
    }

    async fn handle_event(
        &mut self,
        event: &crate::plugins::PiriEvent,
        _niri: &NiriIpc,
    ) -> Result<()> {
        match event {
            crate::plugins::PiriEvent::WindowOpened { window } => {
                self.on_window_opened(window).await?;
            }
            crate::plugins::PiriEvent::WindowChanged { window } => {
                self.on_window_changed(window).await?;
            }
            crate::plugins::PiriEvent::WindowToggleFloating { window } => {
                self.on_window_toggle_floating(window).await?;
            }
            crate::plugins::PiriEvent::WindowClosed { id } => {
                self.handle_window_closed(*id).await?;
            }
            crate::plugins::PiriEvent::WindowFocusChanged { id: Some(_) } => {
                self.sync_edge_pulse_indicator(None).await?;
            }
            crate::plugins::PiriEvent::WindowLayoutsChanged { changes } => {
                let current_ws = self.niri.get_focused_workspace().await?;
                let ws_name = &current_ws.name;

                let has_size_change = changes.iter().any(|(win_id, layout)| {
                    let is_floating =
                        self.window_floating_state.get(win_id).copied().unwrap_or(false);
                    let changed = self
                        .previous_window_sizes
                        .get(win_id)
                        .map(|prev| prev != &layout.window_size)
                        .unwrap_or(false);
                    if !is_floating && changed {
                        self.previous_window_sizes.insert(*win_id, layout.window_size);
                    }
                    !is_floating && changed
                });

                if has_size_change {
                    self.try_execute_autofill(
                        current_ws.idx,
                        Some(ws_name.as_str()),
                        current_ws.output.as_deref(),
                        "window resized",
                    )
                    .await?;
                    self.sync_edge_pulse_indicator(None).await?;
                }
            }
            crate::plugins::PiriEvent::WorkspaceActivated { id, focused: true } => {
                self.edge_pulse_last_render = None;
                self.sync_edge_pulse_indicator(Some(*id)).await?;
            }
            _ => {}
        }
        Ok(())
    }

    fn is_interested_in_event(&self, event: &crate::plugins::PiriEvent) -> bool {
        matches!(
            event,
            crate::plugins::PiriEvent::WindowOpened { .. }
                | crate::plugins::PiriEvent::WindowChanged { .. }
                | crate::plugins::PiriEvent::WindowToggleFloating { .. }
                | crate::plugins::PiriEvent::WindowClosed { .. }
                | crate::plugins::PiriEvent::WindowFocusChanged { id: Some(_) }
                | crate::plugins::PiriEvent::WorkspaceActivated { .. }
                | crate::plugins::PiriEvent::WindowLayoutsChanged { .. }
        )
    }

    async fn update_config(&mut self, config: WorkspaceRulePluginConfig) -> Result<()> {
        info!("Updating workspace rule plugin configuration");
        self.config = config;
        self.edge_pulse_last_render = None;
        self.edge_pulse_renderer.shutdown();
        Ok(())
    }
}
