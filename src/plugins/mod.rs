pub mod edge_pulse_renderer;
pub mod empty;
pub mod events;
pub mod fcitx5;
pub mod mark;
pub mod render;
pub mod scratchpads;
pub mod singleton;
pub mod sleepy;
pub mod sticky;
pub mod swallow;
pub mod window_order;
pub mod window_rule;
pub mod window_utils;
pub mod workspace_rule;

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, info, warn};
use tokio::sync::mpsc;
use tokio::time::Duration;

pub use events::PiriEvent;

use crate::config::Config;
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::utils::send_notification;

const DISPLAY_PREFIXES: &[(&str, &str)] = &[
    ("HDMI-A", "HDMI"),
    ("DVI-D", "DVI-D"),
    ("DVI-I", "DVI-I"),
    ("Virtual", "Virtual"),
    ("Unknown", "Unknown"),
    ("USB", "USB"),
    ("eDP", "eDP"),
    ("VGA", "VGA"),
    ("DP", "DP"),
];

fn extract_display_prefix(output: &str) -> Option<&str> {
    DISPLAY_PREFIXES
        .iter()
        .find(|(prefix, _)| output.starts_with(prefix))
        .map(|(_, canonical)| *canonical)
}

/// Plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    type Config: Clone + Send + Sync + FromConfig;

    /// Create a new instance of the plugin
    fn new(niri: NiriIpc, config: Self::Config) -> Self
    where
        Self: Sized;

    async fn handle_ipc_request(&mut self, _request: &IpcRequest) -> Result<Option<Result<()>>> {
        Ok(None)
    }

    async fn handle_event(&mut self, _event: &PiriEvent, _niri: &NiriIpc) -> Result<()> {
        Ok(())
    }

    /// Check if plugin is interested in a specific event type.
    /// Only events that pass this filter will be passed to handle_event().
    fn is_interested_in_event(&self, _event: &PiriEvent) -> bool {
        false
    }

    async fn update_config(&mut self, _config: Self::Config) -> Result<()> {
        Ok(())
    }
}

/// Resolve a workspace config key with monitor-aware lookup.
///
/// Resolution order (first match wins):
/// 1. `"{idx}@{output}"`     — e.g., "1@DP-2" (exact output, most specific)
/// 2. `"{name}@{output}"`    — e.g., "browser@DP-2"
/// 3. `"{idx}@{prefix}"`     — e.g., "1@DP" (display prefix match)
/// 4. `"{name}@{prefix}"`    — e.g., "browser@DP"
/// 5. `"{name}"`             — e.g., "browser" (by name only)
/// 6. `"{idx}"`              — e.g., "1" (fallback, backward compatible)
pub fn resolve_workspace_config<'a, T>(
    map: &'a std::collections::HashMap<String, T>,
    idx: u8,
    name: Option<&str>,
    output: Option<&str>,
) -> Option<&'a T> {
    if let Some(out) = output {
        let key = format!("{}@{}", idx, out);
        if let Some(v) = map.get(&key) {
            return Some(v);
        }
        if let Some(n) = name {
            let key = format!("{}@{}", n, out);
            if let Some(v) = map.get(&key) {
                return Some(v);
            }
        }
        if let Some(pf) = extract_display_prefix(out) {
            let key = format!("{}@{}", idx, pf);
            if let Some(v) = map.get(&key) {
                return Some(v);
            }
            if let Some(n) = name {
                let key = format!("{}@{}", n, pf);
                if let Some(v) = map.get(&key) {
                    return Some(v);
                }
            }
        }
    }
    if let Some(n) = name {
        if let Some(v) = map.get(n) {
            return Some(v);
        }
    }
    map.get(&idx.to_string())
}

pub fn workspace_matches_filter(
    idx: u8,
    name: Option<&str>,
    output: Option<&str>,
    filters: &[String],
) -> bool {
    if filters.is_empty() {
        return true;
    }
    let idx_str = idx.to_string();
    for filter in filters {
        if let Some((left, right)) = filter.split_once('@') {
            let output_matches =
                output.is_some_and(|o| o == right || extract_display_prefix(o) == Some(right));
            if output_matches && (left == idx_str || Some(left) == name) {
                return true;
            }
        }
        // Direct name or idx match
        if Some(filter.as_str()) == name || *filter == idx_str {
            return true;
        }
    }
    false
}

pub trait FromConfig {
    fn from_config(config: &Config) -> Option<Self>
    where
        Self: Sized;
}

impl FromConfig for () {
    fn from_config(_config: &Config) -> Option<Self> {
        Some(())
    }
}

macro_rules! register_plugins {
    ($($name:expr => $variant:ident($module:ident::$struct:ident)),* $(,)?) => {
        #[allow(clippy::large_enum_variant)]
        pub enum PluginEnum {
            $($variant($module::$struct),)*
        }

        impl PluginEnum {
            pub fn name(&self) -> &str {
                match self {
                    $(PluginEnum::$variant(_) => $name,)*
                }
            }

            async fn handle_event(&mut self, event: &PiriEvent, niri: &NiriIpc) -> Result<()> {
                match self {
                    $(PluginEnum::$variant(p) => p.handle_event(event, niri).await,)*
                }
            }

            fn is_interested_in_event(&self, event: &PiriEvent) -> bool {
                match self {
                    $(PluginEnum::$variant(p) => p.is_interested_in_event(event),)*
                }
            }

            async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
                match self {
                    $(PluginEnum::$variant(p) => p.handle_ipc_request(request).await,)*
                }
            }

            async fn update_config(&mut self, config: &Config) -> Result<()> {
                match self {
                    $(PluginEnum::$variant(p) => {
                        if let Some(plugin_config) = <<$module::$struct as Plugin>::Config as FromConfig>::from_config(config) {
                            p.update_config(plugin_config).await
                        } else {
                            // If from_config returns None, it means the plugin should be disabled.
                            // However, update_config is called on an already existing plugin.
                            // The PluginManager::init will handle disabling/removing the plugin.
                            Ok(())
                        }
                    },)*
                }
            }
        }

        impl PluginManager {
            pub async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
                let p = &config.piri.plugins;
                $(
                    let plugin_config = <<$module::$struct as Plugin>::Config as FromConfig>::from_config(config);
                    let enabled = p.is_enabled($name) && plugin_config.is_some();

                    self.init_or_update_plugin($name, enabled, niri.clone(), config, || {
                        PluginEnum::$variant(<$module::$struct as Plugin>::new(
                            niri.clone(),
                            plugin_config.unwrap(),
                        ))
                    }).await?;
                )*

                // Seed the normalizer with the current window list to avoid
                // false WindowOpened events for already-open windows.
                if let Ok(windows) = niri.get_windows_raw().await {
                    self.normalizer.rebuild_from_piri_windows(&windows);
                }

                Ok(())
            }
        }
    };
}

register_plugins! {
    "empty"        => Empty(empty::EmptyPlugin),
    "window_rule"  => WindowRule(window_rule::WindowRulePlugin),
    "scratchpads"  => Scratchpads(scratchpads::ScratchpadsPlugin),
    "singleton"    => Singleton(singleton::SingletonPlugin),
    "window_order" => WindowOrder(window_order::WindowOrderPlugin),
    "swallow"      => Swallow(swallow::SwallowPlugin),
    "workspace_rule" => WorkspaceRule(workspace_rule::WorkspaceRulePlugin),
    "fcitx5" => Fcitx5(fcitx5::Fcitx5Plugin),
    "sleepy" => Sleepy(sleepy::SleepyPlugin),
    "mark" => Mark(mark::MarkPlugin),
    "sticky" => Sticky(sticky::StickyPlugin),
}

pub struct PluginManager {
    plugins: Vec<PluginEnum>,
    event_listener_handle: Option<tokio::task::JoinHandle<()>>,
    event_sender: Option<mpsc::UnboundedSender<niri_ipc::Event>>,
    normalizer: events::EventNormalizer,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            event_listener_handle: None,
            event_sender: None,
            normalizer: events::EventNormalizer::new(),
        }
    }

    pub async fn start_event_listener(
        &mut self,
        niri: NiriIpc,
    ) -> Result<mpsc::UnboundedReceiver<niri_ipc::Event>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        self.event_sender = Some(tx);

        let niri_clone = niri.clone();
        let handle = tokio::spawn(async move {
            Self::event_listener_loop(niri_clone, tx_clone).await;
        });

        self.event_listener_handle = Some(handle);
        info!("Plugin manager unified event listener started");
        Ok(rx)
    }

    async fn event_listener_loop(niri: NiriIpc, event_tx: mpsc::UnboundedSender<niri_ipc::Event>) {
        info!("Plugin manager event listener started");

        let mut is_first_connection = true;

        // Outer loop: reconnect on connection failure
        loop {
            let socket = match niri.create_event_stream_socket() {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to create event stream: {}, retrying in 1s", e);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
            };

            let mut read_event = socket.read_events();
            info!("Event stream connected, waiting for events...");

            // Send notification on first successful connection
            if is_first_connection {
                send_notification(
                    "piri",
                    "Started successfully, socket connection established",
                );
                is_first_connection = false;
            }

            while let Ok(event) = read_event() {
                debug!("Raw event received: {:?}", event);

                // Send event to channel for distribution
                if event_tx.send(event).is_err() {
                    warn!("Event channel closed, stopping event listener");
                    return;
                }
            }

            // Connection closed or error - will reconnect in outer loop
            // Refresh outputs cache since state may have changed during disconnection
            if let Err(e) = niri.refresh_outputs().await {
                log::warn!("Failed to refresh outputs cache on reconnect: {}", e);
            }
            warn!("Event stream closed, reconnecting...");
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }

    /// Distribute event to all plugins (called from daemon loop).
    /// Normalizes niri events into PiriEvents before dispatching.
    pub async fn distribute_event(&mut self, event: &niri_ipc::Event, niri: &NiriIpc) {
        let piri_events = self.normalizer.normalize_event(event);

        for piri_event in &piri_events {
            for plugin in &mut self.plugins {
                if plugin.is_interested_in_event(piri_event) {
                    if let Err(e) = plugin.handle_event(piri_event, niri).await {
                        log::warn!("Plugin {} error: {}", plugin.name(), e);
                        send_notification("piri", &format!("Plugin {} error", plugin.name()));
                    }
                }
            }
        }
    }

    /// Initialize or update a single plugin
    /// If the plugin already exists, tries to update it via update_config to preserve runtime state.
    /// If update fails or plugin doesn't exist, creates a new instance.
    async fn init_or_update_plugin<F>(
        &mut self,
        name: &str,
        enabled: bool,
        _niri: NiriIpc,
        config: &Config,
        create_plugin: F,
    ) -> Result<()>
    where
        F: FnOnce() -> PluginEnum,
    {
        let existing_plugin = self.plugins.iter_mut().find(|p| p.name() == name);

        if enabled {
            if let Some(plugin) = existing_plugin {
                debug!("Updating existing plugin configuration: {}", name);
                if let Err(e) = plugin.update_config(config).await {
                    warn!("Failed to update plugin {}, recreating: {}", name, e);
                    self.plugins.retain(|p| p.name() != name);
                    let new_plugin = create_plugin();
                    self.plugins.push(new_plugin);
                }
            } else {
                info!("Initializing new plugin: {}", name);
                let new_plugin = create_plugin();
                self.plugins.push(new_plugin);
            }
        } else {
            if self.plugins.iter().any(|p| p.name() == name) {
                info!("Disabling plugin: {}", name);
                self.plugins.retain(|p| p.name() != name);
            }
        }
        Ok(())
    }

    /// Handle IPC request through plugins
    pub async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        for plugin in &mut self.plugins {
            match plugin.handle_ipc_request(request).await? {
                Some(result) => return Ok(Some(result)),
                Option::None => continue,
            }
        }
        Ok(None)
    }
}
