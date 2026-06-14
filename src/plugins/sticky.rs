use crate::plugins::PiriEvent;
use anyhow::Result;
use async_trait::async_trait;
use log::{debug, info};

use crate::config::Config;
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, StickyFollowResult};
use crate::plugins::FromConfig;

#[derive(Debug, Clone, Default)]
pub struct StickyPluginConfig;

impl FromConfig for StickyPluginConfig {
    fn from_config(_config: &Config) -> Option<Self> {
        Some(Self)
    }
}

pub struct StickyPlugin {
    niri: NiriIpc,
}

impl StickyPlugin {
    fn new(niri: NiriIpc) -> Self {
        info!("Sticky plugin initialized");
        Self { niri }
    }

    /// Add the focused window as sticky via the global registry.
    async fn add(&self, cross_monitor: bool) -> Result<()> {
        let window = window_utils::get_focused_window(&self.niri).await?;
        if !window.floating {
            anyhow::bail!("Focused window is not floating. Sticky only supports floating windows.");
        }
        window_utils::register_sticky_window(window.id, cross_monitor);
        Ok(())
    }

    /// Remove the focused window from the sticky registry.
    async fn delete(&self) -> Result<()> {
        let window = window_utils::get_focused_window(&self.niri).await?;
        window_utils::unregister_sticky_window(window.id);
        Ok(())
    }

    /// Follow the focused workspace for all registered sticky windows.
    async fn follow_focused_workspace(&self) -> Result<()> {
        let windows = window_utils::get_sticky_window_list();
        for (window_id, cross_monitor) in windows {
            // Get window position/size before the move (for proportional resize)
            let pre_move = self.niri.get_window_position_async(window_id).await?;

            match window_utils::sticky_follow_workspace(&self.niri, window_id, cross_monitor)
                .await?
            {
                StickyFollowResult::Keep => {}
                StickyFollowResult::WindowGone | StickyFollowResult::NotFloating => {
                    window_utils::unregister_sticky_window(window_id);
                }
                StickyFollowResult::OutputChanged {
                    old_output_width: old_ow,
                    old_output_height: old_oh,
                    new_output_width: new_ow,
                    new_output_height: new_oh,
                } => {
                    if let Some((old_x, old_y, old_w, old_h)) = pre_move {
                        let ratio_w = new_ow as f64 / old_ow as f64;
                        let ratio_h = new_oh as f64 / old_oh as f64;
                        let new_w = (old_w as f64 * ratio_w) as u32;
                        let new_h = (old_h as f64 * ratio_h) as u32;
                        let new_x = (old_x as f64 * ratio_w) as i32;
                        let new_y = (old_y as f64 * ratio_h) as i32;

                        debug!(
                            "Sticky proportional resize: ({},{}) {}x{} -> ({},{}) {}x{} (output {}x{} -> {}x{})",
                            old_x, old_y, old_w, old_h, new_x, new_y, new_w, new_h,
                            old_ow, old_oh, new_ow, new_oh
                        );

                        self.niri.resize_floating_window(window_id, new_w, new_h).await?;
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                        if let Some((cur_x, cur_y, _, _)) =
                            self.niri.get_window_position_async(window_id).await?
                        {
                            window_utils::move_window_to_position(
                                &self.niri, window_id, cur_x, cur_y, new_x, new_y,
                            )
                            .await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl crate::plugins::Plugin for StickyPlugin {
    type Config = StickyPluginConfig;

    fn new(niri: NiriIpc, _config: Self::Config) -> Self {
        Self::new(niri)
    }

    async fn update_config(&mut self, _config: Self::Config) -> Result<()> {
        Ok(())
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::StickyAdd { cross } => {
                self.add(*cross).await?;
                Ok(Some(Ok(())))
            }
            IpcRequest::StickyDelete => {
                self.delete().await?;
                Ok(Some(Ok(())))
            }
            _ => Ok(None),
        }
    }

    async fn handle_event(&mut self, event: &PiriEvent, _niri: &NiriIpc) -> Result<()> {
        if let PiriEvent::WorkspaceActivated { focused: true, .. } = event {
            self.follow_focused_workspace().await?;
        }
        Ok(())
    }

    fn is_interested_in_event(&self, event: &PiriEvent) -> bool {
        matches!(event, PiriEvent::WorkspaceActivated { focused: true, .. })
    }
}
