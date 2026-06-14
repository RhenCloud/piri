use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::plugins::empty::EmptyPluginConfig;

/// Direction from which the scratchpad appears
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Top,
    Bottom,
    Left,
    Right,
}

impl std::str::FromStr for Direction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "fromTop" => Ok(Direction::Top),
            "fromBottom" => Ok(Direction::Bottom),
            "fromLeft" => Ok(Direction::Left),
            "fromRight" => Ok(Direction::Right),
            _ => anyhow::bail!(
                "Invalid direction: {}. Must be one of: fromTop, fromBottom, fromLeft, fromRight",
                s
            ),
        }
    }
}

impl Direction {
    /// Convert Direction to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Top => "fromTop",
            Direction::Bottom => "fromBottom",
            Direction::Left => "fromLeft",
            Direction::Right => "fromRight",
        }
    }
}

impl Serialize for Direction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Direction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub niri: NiriConfig,
    #[serde(default)]
    pub piri: PiriConfig,
    #[serde(default)]
    pub scratchpads: HashMap<String, ScratchpadConfig>,
    #[serde(default)]
    pub empty: HashMap<String, EmptyWorkspaceConfig>,
    #[serde(default)]
    pub singleton: HashMap<String, SingletonConfig>,
    #[serde(default)]
    pub window_rule: Vec<WindowRuleConfig>,
    #[serde(default)]
    pub window_order: HashMap<String, u32>,
    #[serde(default)]
    pub swallow: Vec<crate::plugins::swallow::SwallowRule>,
    #[serde(default)]
    pub workspace_rule: HashMap<String, WorkspaceRuleConfig>,
    #[serde(default)]
    pub fcitx5: Vec<crate::plugins::fcitx5::Fcitx5WindowRule>,
    #[serde(default)]
    pub sleepy: Option<crate::plugins::sleepy::SleepyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowOrderSection {
    #[serde(default = "default_enable_event_listener")]
    pub enable_event_listener: bool,
    #[serde(default = "default_window_order_weight")]
    pub default_weight: u32,
    #[serde(default)]
    pub workspaces: Vec<String>,
}

impl Default for WindowOrderSection {
    fn default() -> Self {
        Self {
            enable_event_listener: default_enable_event_listener(),
            default_weight: default_window_order_weight(),
            workspaces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwallowSection {
    #[serde(default)]
    pub rules: Vec<crate::plugins::swallow::SwallowRule>,
    #[serde(default = "default_true")]
    pub use_pid_matching: bool,
    #[serde(default)]
    pub exclude: Option<crate::plugins::swallow::SwallowExclude>,
}

fn default_true() -> bool {
    true
}

impl Default for SwallowSection {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            use_pid_matching: default_true(),
            exclude: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NiriConfig {
    /// Path to niri socket (default: $XDG_RUNTIME_DIR/niri or /tmp/niri)
    pub socket_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PiriConfig {
    #[serde(default)]
    pub scratchpad: ScratchpadDefaults,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub window_order: WindowOrderSection,
    #[serde(default)]
    pub swallow: SwallowSection,
    #[serde(default)]
    pub workspace_rule: WorkspaceRuleSection,
    #[serde(default)]
    pub mark: MarkSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarkSection {
    /// If true, toggling a mark that is already focused will return to the previous window
    #[serde(default)]
    pub refocus: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginsConfig {
    #[serde(default)]
    pub scratchpads: Option<bool>,
    #[serde(default)]
    pub empty: Option<bool>,
    #[serde(default)]
    pub window_rule: Option<bool>,
    #[serde(default)]
    pub autofill: Option<bool>,
    #[serde(default)]
    pub singleton: Option<bool>,
    #[serde(default)]
    pub window_order: Option<bool>,
    #[serde(default)]
    pub swallow: Option<bool>,
    #[serde(default)]
    pub workspace_rule: Option<bool>,
    #[serde(default)]
    pub fcitx5: Option<bool>,
    #[serde(default)]
    pub sleepy: Option<bool>,
    #[serde(default)]
    pub mark: Option<bool>,
    #[serde(default)]
    pub sticky: Option<bool>,
    #[serde(rename = "empty_config", default)]
    pub empty_config: Option<EmptyPluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyWorkspaceConfig {
    /// Command to execute when switching to this empty workspace
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingletonConfig {
    /// Command to execute the application (can include environment variables and arguments)
    pub command: String,
    /// Optional app_id pattern to match windows (if not specified, extracted from command)
    pub app_id: Option<String>,
    /// Optional command to execute after the window is created (only executed when window is newly created)
    #[serde(default)]
    pub on_created_command: Option<String>,
}

/// Helper type to deserialize String or Vec<String>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    String(String),
    Vec(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrVec::String(s) => vec![s],
            StringOrVec::Vec(v) => v,
        }
    }
}

/// Window rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRuleConfig {
    /// Regex pattern(s) to match app_id (optional, can be a string or list of strings)
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub app_id: Option<Vec<String>>,
    /// Regex pattern(s) to match title (optional, can be a string or list of strings)
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub title: Option<Vec<String>>,
    /// Workspace to move matching windows to (name or idx, optional if focus_command is specified)
    pub open_on_workspace: Option<String>,
    /// Command to execute when a matching window is focused (optional)
    pub focus_command: Option<String>,
    /// If true, focus_command will only execute on the first focus (default: false)
    #[serde(default)]
    pub focus_command_once: bool,
}

pub(crate) fn deserialize_string_or_vec<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    // Handle missing field case - deserialize as Option first
    let opt: Option<StringOrVec> = Option::deserialize(deserializer)?;
    Ok(opt.map(|sov| sov.into_vec()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadDefaults {
    /// Default size for dynamically added scratchpads (e.g., "40% 60%")
    #[serde(default = "default_size")]
    pub default_size: String,
    /// Default margin for dynamically added scratchpads (pixels)
    #[serde(default = "default_margin")]
    pub default_margin: u32,
    /// Optional workspace to move scratchpads to when hidden
    #[serde(default)]
    pub move_to_workspace: Option<String>,
}

fn default_size() -> String {
    "75% 60%".to_string()
}

fn default_margin() -> u32 {
    50
}

impl Default for ScratchpadDefaults {
    fn default() -> Self {
        Self {
            default_size: default_size(),
            default_margin: default_margin(),
            move_to_workspace: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadConfig {
    /// Direction from which the scratchpad appears
    pub direction: Direction,
    /// Command to execute the application (can include environment variables and arguments)
    pub command: String,
    /// Explicit app_id to match windows (required)
    pub app_id: String,
    /// Size of the scratchpad (e.g., "75% 60%")
    pub size: String,
    /// Margin from the edge in pixels
    pub margin: u32,
    /// If true, swallow the scratchpad window to the focused window when shown
    #[serde(default)]
    pub swallow_to_focus: bool,
    /// If true, scratchpad will follow the focused workspace (delegated to sticky plugin)
    #[serde(default)]
    pub sticky: bool,
    /// If true, scratchpad will automatically hide when it loses focus
    #[serde(default)]
    pub auto_hide_on_focus_loss: bool,
    /// If true, when the scratchpad is visible and focused, toggle will refocus to the previous window
    #[serde(default)]
    pub refocus: bool,
}

impl ScratchpadConfig {
    /// Parse size string (e.g., "75% 60%") into width and height percentages
    pub fn parse_size(&self) -> Result<(f64, f64)> {
        let parts: Vec<&str> = self.size.split_whitespace().collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Size must be in format 'width% height%', got: {}",
                self.size
            );
        }

        let width = parts[0]
            .strip_suffix('%')
            .ok_or_else(|| anyhow::anyhow!("Width must end with %, got: {}", parts[0]))?
            .parse::<f64>()
            .context("Failed to parse width")?;

        let height = parts[1]
            .strip_suffix('%')
            .ok_or_else(|| anyhow::anyhow!("Height must end with %, got: {}", parts[1]))?
            .parse::<f64>()
            .context("Failed to parse height")?;

        Ok((width / 100.0, height / 100.0))
    }
}

impl Config {
    /// Load configuration from file
    /// This is the only method that should be used to load config
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // Create default config if file doesn't exist
        if !path.exists() {
            let default_config = Config::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).context("Failed to create config directory")?;
            }
            let toml = toml::to_string_pretty(&default_config)
                .context("Failed to serialize default config")?;
            fs::write(path, toml).context("Failed to write default config")?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;

        Ok(config)
    }
}

impl PluginsConfig {
    pub fn is_enabled(&self, name: &str) -> bool {
        match name {
            "scratchpads" => self.scratchpads.unwrap_or(false),
            "empty" => self.empty.unwrap_or(false),
            "window_rule" => self.window_rule.unwrap_or(false),
            "singleton" => self.singleton.unwrap_or(false),
            "window_order" => self.window_order.unwrap_or(false),
            "swallow" => self.swallow.unwrap_or(false),
            "workspace_rule" => self.workspace_rule.unwrap_or(false),
            "fcitx5" => self.fcitx5.unwrap_or(false),
            "sleepy" => self.sleepy.unwrap_or(false),
            "mark" => self.mark.unwrap_or(false),
            "sticky" => self.sticky.unwrap_or(false),
            _ => false,
        }
    }
}

fn default_enable_event_listener() -> bool {
    false // Default: event listener disabled
}

fn default_window_order_weight() -> u32 {
    0 // Default: unconfigured windows have weight 0 (rightmost)
}

/// Helper type to deserialize String or Vec<String> for auto_width
/// This allows both "50%" and ["45%", "55%"] formats
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum WidthValue {
    String(String),
    Vec(Vec<String>),
}

impl WidthValue {
    /// Convert to Vec<String>, expanding single string to vec
    fn into_vec(self) -> Vec<String> {
        match self {
            WidthValue::String(s) => vec![s],
            WidthValue::Vec(v) => v,
        }
    }
}

/// Custom deserializer for auto_width array
/// Handles nested arrays: ["100%", "50%"] or ["100%", ["45%", "55%"]]
fn deserialize_auto_width<'de, D>(deserializer: D) -> Result<Vec<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::Deserialize;

    // Deserialize as Vec<WidthValue>
    let values: Vec<WidthValue> = Vec::deserialize(deserializer)?;

    // Convert each element to Vec<String>
    let result: Vec<Vec<String>> = values.into_iter().map(|v| v.into_vec()).collect();

    Ok(result)
}

/// Workspace rule configuration for a specific workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRuleConfig {
    /// Auto width configuration: array where index corresponds to window count (1-based)
    /// Each element can be a string (all windows same width) or array (different widths per window)
    /// Examples:
    ///   ["100%", "50%"] - 1 window: 100%, 2 windows: each 50%
    ///   ["100%", ["45%", "55%"]] - 1 window: 100%, 2 windows: 45% and 55%
    #[serde(deserialize_with = "deserialize_auto_width")]
    pub auto_width: Vec<Vec<String>>,
    /// If true, automatically tile windows: allow up to 2 windows per column (except first column)
    #[serde(default)]
    pub auto_tile: bool,
    /// If true, automatically align last column (autofill)
    #[serde(default, rename = "auto_fill")]
    pub auto_fill: bool,
    /// If true, automatically maximize window when there's only one window, and unmaximize when there are multiple windows
    #[serde(default)]
    pub auto_maximize: bool,
    /// EdgePulse indicator config for this workspace.
    #[serde(default)]
    pub edge_pulse: EdgePulseConfig,
}

/// Workspace rule section in piri config (default settings)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceRuleSection {
    /// Default auto width configuration
    #[serde(deserialize_with = "deserialize_auto_width", default)]
    pub auto_width: Vec<Vec<String>>,
    /// If true, automatically tile windows: allow up to 2 windows per column (except first column)
    #[serde(default)]
    pub auto_tile: bool,
    /// If true, automatically align last column (autofill)
    #[serde(default, rename = "auto_fill")]
    pub auto_fill: bool,
    /// If true, automatically maximize window when there's only one window, and unmaximize when there are multiple windows
    #[serde(default)]
    pub auto_maximize: bool,
    /// Default EdgePulse indicator config for all workspaces.
    #[serde(default)]
    pub edge_pulse: EdgePulseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgePulseConfig {
    /// Enable left/right missing-neighbor indicator.
    #[serde(default)]
    pub enabled: bool,
    /// Show left-side indicator when there is no left neighbor.
    #[serde(default = "default_true")]
    pub show_left: bool,
    /// Show right-side indicator when there is no right neighbor.
    #[serde(default = "default_true")]
    pub show_right: bool,
    /// Indicator width in pixels.
    #[serde(default = "default_edge_pulse_width")]
    pub width: u32,
    /// Indicator height ratio to output height, range 0.0-1.0.
    #[serde(default = "default_edge_pulse_height_ratio")]
    pub height_ratio: f64,
    /// Gradient start color for left edge.
    #[serde(default = "default_left_start")]
    pub left_gradient_start: String,
    /// Gradient end color for left edge.
    #[serde(default = "default_left_end")]
    pub left_gradient_end: String,
    /// Gradient start color for right edge.
    #[serde(default = "default_right_start")]
    pub right_gradient_start: String,
    /// Gradient end color for right edge.
    #[serde(default = "default_right_end")]
    pub right_gradient_end: String,
    /// Global alpha 0.0-1.0.
    #[serde(default = "default_edge_pulse_alpha")]
    pub alpha: f64,
    /// Enable animation effect (pulse/fade).
    #[serde(default)]
    pub animation_enabled: bool,
    /// Animation style: "pulse" | "fade".
    #[serde(default = "default_animation_style")]
    pub animation_style: String,
    /// Animation duration in milliseconds per cycle.
    #[serde(default = "default_animation_duration")]
    pub animation_duration: f64,
    /// Animation amplitude 0.0-1.0, controls intensity.
    #[serde(default = "default_animation_amplitude")]
    pub animation_amplitude: f64,
    /// Number of animation repeats (0 = infinite loop until state changes).
    #[serde(default = "default_animation_repeat")]
    pub animation_repeat: u32,
}

impl Default for EdgePulseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_left: true,
            show_right: true,
            width: default_edge_pulse_width(),
            height_ratio: default_edge_pulse_height_ratio(),
            left_gradient_start: default_left_start(),
            left_gradient_end: default_left_end(),
            right_gradient_start: default_right_start(),
            right_gradient_end: default_right_end(),
            alpha: default_edge_pulse_alpha(),
            animation_enabled: false,
            animation_style: default_animation_style(),
            animation_duration: default_animation_duration(),
            animation_amplitude: default_animation_amplitude(),
            animation_repeat: default_animation_repeat(),
        }
    }
}

fn default_edge_pulse_width() -> u32 {
    14
}

fn default_edge_pulse_height_ratio() -> f64 {
    0.42
}

fn default_edge_pulse_alpha() -> f64 {
    0.85
}

fn default_animation_style() -> String {
    "pulse".to_string()
}

fn default_animation_duration() -> f64 {
    600.0
}

fn default_animation_amplitude() -> f64 {
    0.8
}

fn default_animation_repeat() -> u32 {
    3
}

fn default_left_start() -> String {
    "#68d8ff".to_string()
}

fn default_left_end() -> String {
    "#1f4fff".to_string()
}

fn default_right_start() -> String {
    "#ffd36a".to_string()
}

fn default_right_end() -> String {
    "#ff7a1f".to_string()
}

// Helper to convert TOML table to ScratchpadConfig
impl TryFrom<toml::Table> for ScratchpadConfig {
    type Error = anyhow::Error;

    fn try_from(table: toml::Table) -> Result<Self> {
        let direction = table
            .get("direction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'direction' field"))
            .and_then(Direction::from_str)?;

        let command = table
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' field"))?
            .to_string();

        let size = table
            .get("size")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'size' field"))?
            .to_string();

        let margin = table
            .get("margin")
            .and_then(|v| v.as_integer())
            .ok_or_else(|| anyhow::anyhow!("Missing 'margin' field"))? as u32;

        let app_id = table
            .get("app_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'app_id' field"))?
            .to_string();

        let swallow_to_focus =
            table.get("swallow_to_focus").and_then(|v| v.as_bool()).unwrap_or(false);

        let sticky = table.get("sticky").and_then(|v| v.as_bool()).unwrap_or(false);

        let auto_hide_on_focus_loss =
            table.get("auto_hide_on_focus_loss").and_then(|v| v.as_bool()).unwrap_or(false);

        let refocus = table.get("refocus").and_then(|v| v.as_bool()).unwrap_or(false);

        if sticky && auto_hide_on_focus_loss {
            anyhow::bail!(
                "'sticky' and 'auto_hide_on_focus_loss' cannot both be enabled for a scratchpad"
            );
        }

        Ok(ScratchpadConfig {
            direction,
            command,
            app_id,
            size,
            margin,
            swallow_to_focus,
            sticky,
            auto_hide_on_focus_loss,
            refocus,
        })
    }
}
