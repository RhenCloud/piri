# Workspace Rule Plugin

The Workspace Rule plugin provides automated window layout management for workspaces, including automatic width adjustment, automatic tiling, automatic alignment, and automatic maximization.

## Features

The Workspace Rule plugin provides the following features:

- **Automatic Width Adjustment** (`auto_width`): Automatically adjust window widths based on window count
- **Automatic Tiling** (`auto_tile`): Automatically merge new windows into existing columns (except first column)
- **Automatic Alignment** (`auto_fill`): Automatically align the last column of windows to the rightmost position
- **Automatic Maximization** (`auto_maximize`): Automatically maximize when there's only one window, and unmaximize when there are multiple windows
- **EdgePulse Edge Indicator** (`edge_pulse`): Trigger left/right edge hints when the focused column reaches the workspace edge

## Demo Videos

https://github.com/user-attachments/assets/092a383c-993b-42b5-9b89-b78b0807b436

https://github.com/user-attachments/assets/b48b0465-0101-4298-9935-22f46d1a2658

https://github.com/user-attachments/assets/3b8dc835-473c-4b6e-b684-02a6951b63f9

https://github.com/user-attachments/assets/2e9e2c86-b9ef-44f1-a896-46473b93b417

## Configuration

### Basic Configuration

Enable the plugin in your configuration file:

```toml
[piri.plugins]
workspace_rule = true
```

### Default Configuration

Use `[piri.workspace_rule]` to configure default settings that apply to all workspaces without specific configuration:

```toml
[piri.workspace_rule]
# Auto width configuration: array index corresponds to window count (1-based)
# Each element can be a string (all windows same width) or array (different widths per window)
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]
# Automatic tiling: allow up to 2 windows per column (except first column)
auto_tile = false
# Automatic alignment: automatically align last column to rightmost position
auto_fill = false
# Automatic maximization: maximize when only one window, unmaximize when multiple windows
auto_maximize = false
```

### Workspace-Specific Configuration

Use `[workspace_rule.{workspace}]` to configure rules for specific workspaces. Workspace identifier can be a name (e.g., `"browser"`) or index (e.g., `"1"`):

```toml
# Workspace index configuration
[workspace_rule.1]
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]

# Workspace name configuration
[workspace_rule.browser]
# 1 window: 100%, 2 windows: 45% and 55%, 3 windows: 33.33% each
auto_width = ["100%", ["45%", "55%"], "33.33%"]

# Enable automatic maximization
[workspace_rule.main]
auto_maximize = true

# Enable automatic alignment
[workspace_rule.dev]
auto_fill = true
```

### Configuration Parameters

| Parameter | Type | Description |
| :--- | :--- | :--- |
| `auto_width` | `Vec<Vec<String>>` | Auto width configuration array, index corresponds to window count (1-based). Each element can be a string (all windows same width) or array (different widths per window). Width values must be in percentage format (e.g., `"50%"`) |
| `auto_tile` | `bool` | If `true`, automatically merge new windows into existing columns (except first column). When a non-first column has only one window, new windows will be merged into that column |
| `auto_fill` | `bool` | If `true`, automatically align the last column of windows to the rightmost position |
| `auto_maximize` | `bool` | If `true`, automatically maximize to edges when workspace has only one window, and unmaximize when there are multiple windows |
| `edge_pulse.enabled` | `bool` | Enable EdgePulse workspace-edge detection and hints |
| `edge_pulse.show_left` | `bool` | Show left hint when reaching the workspace left edge |
| `edge_pulse.show_right` | `bool` | Show right hint when reaching the workspace right edge |
| `edge_pulse.width` | `u32` | Hint width in pixels |
| `edge_pulse.height_ratio` | `f64` | Hint height ratio against output height (0.0-1.0) |
| `edge_pulse.left_gradient_start` | `String` | Left hint gradient start color (e.g. `#68d8ff`) |
| `edge_pulse.left_gradient_end` | `String` | Left hint gradient end color |
| `edge_pulse.right_gradient_start` | `String` | Right hint gradient start color |
| `edge_pulse.right_gradient_end` | `String` | Right hint gradient end color |
| `edge_pulse.alpha` | `f64` | Global alpha (0.0-1.0) |

## How It Works

### Automatic Width Adjustment (`auto_width`)

The plugin applies width configuration based on the **column count** (not window count) in the workspace:

1. Count the number of columns of tiled windows in the current workspace
2. Find the corresponding width configuration based on column count (array index = column count - 1)
3. Set the corresponding width percentage for each column

**Example**:
- Configuration: `auto_width = ["100%", "50%", ["30%", "35%", "35%"]]`
- 1 column: All windows width 100%
- 2 columns: Each column width 50%
- 3 columns: First column 30%, second column 35%, third column 35%

### Automatic Tiling (`auto_tile`)

When a new window opens:

1. Check columns in the current workspace (excluding first column)
2. Find columns with only one window
3. Merge the new window into that column (using swallow mechanism)

**Note**: The first column will not be merged; new windows will create new columns.

### Automatic Alignment (`auto_fill`)

**Functionality**: Automatically align the last column to the rightmost position when windows are closed or layout changes, eliminating gaps.

**Triggers**:
- Window closed (`WindowClosed` event)
- Layout changed (`WindowLayoutsChanged` event)

**Implementation**:
1. Save currently focused window
2. Focus first column (`focus-window` to first column)
3. Focus last column (aligns all columns to right)
4. Restore original focus

**Configuration Examples**:
```toml
[piri.workspace_rule]
auto_fill = true  # Enable globally

[workspace_rule.main]
auto_fill = true  # Enable only for main workspace
```

### Automatic Maximization (`auto_maximize`)

When window count changes:

1. **Only one window**: Automatically maximize to edges
2. **Multiple windows**: Automatically unmaximize, restoring normal width adjustment

**Note**: The plugin tracks maximized windows to avoid duplicate processing.

## Event Handling

The plugin listens to the following events:

- `WindowOpenedOrChanged`: Handles new window opening and window state changes
- `WindowClosed`: Handles window closing
- `WindowLayoutsChanged`: Handles layout changes (for automatic alignment)

### Window State Tracking

The plugin tracks the following states:

- **Seen Windows** (`seen_windows`): Distinguishes new windows from existing windows
- **Window Floating State** (`window_floating_state`): Detects floating/tiling state changes
- **Maximized Windows** (`maximized_windows`): Tracks windows maximized by `auto_maximize`

### Throttling Mechanism

The `apply_widths` function uses a 400ms throttling mechanism to avoid frequent triggers:

- First request executes immediately
- Subsequent requests within 400ms are ignored

## Configuration Examples

### Example 1: Basic Width Configuration

```toml
[piri.plugins]
workspace_rule = true

[piri.workspace_rule]
# 1 window: 100%, 2 windows: 50% each, 3 windows: 33.33% each
auto_width = ["100%", "50%", "33.33%"]
```

### Example 2: Custom Width Configuration

```toml
[workspace_rule.dev]
# 1 window: 100%, 2 windows: 45% and 55%, 3 windows: 30%, 35%, 35%
auto_width = ["100%", ["45%", "55%"], ["30%", "35%", "35%"]]
```

### Example 3: Enable Automatic Maximization

```toml
[workspace_rule.main]
auto_maximize = true
```

### Example 4: Enable Automatic Alignment

```toml
[workspace_rule.browser]
auto_fill = true
```

### Example 5: Enable Automatic Tiling

```toml
[workspace_rule.work]
auto_tile = true
```

### Example 6: Combined Configuration

```toml
[piri.workspace_rule]
# Default configuration
auto_width = ["100%", "50%", "33.33%"]
auto_fill = true

[workspace_rule.main]
# Main workspace: enable automatic maximization
auto_maximize = true

[workspace_rule.dev]
# Development workspace: custom width + automatic tiling
auto_width = ["100%", ["45%", "55%"], ["30%", "35%", "35%"]]
auto_tile = true
```

## Features

- ✅ **Workspace-Aware**: Each workspace can be configured independently
- ✅ **Flexible Configuration**: Supports default and workspace-specific configuration
- ✅ **Event-Driven**: Real-time response to window changes
- ✅ **Throttling Optimization**: Avoids frequent triggers, improves performance
- ✅ **State Tracking**: Intelligently tracks window state to avoid duplicate processing
- ✅ **Integrated Functionality**: Comprehensive workspace window management

## Use Cases

- **Multi-Window Layout Management**: Automatically adjust window widths to maintain clean layouts
- **Single Window Maximization**: Automatically maximize when only one window, improving focus
- **Automatic Alignment**: Automatically align after closing windows, keeping interface clean
- **Smart Tiling**: Automatically merge new windows into existing columns, optimizing space usage

## Technical Details

### Column Count Statistics

The plugin counts columns through the `pos_in_scrolling_layout` field:

- Only counts tiled windows (non-floating windows)
- Each column only needs one window ID to set width
- Column index starts from 1

### Width Parsing

Width values must be in percentage format (e.g., `"50%"`), range 0-100%:

- Supports decimals (e.g., `"33.33%"`)
- Must end with `%`
- Parsing failures will log warnings and skip

### Window Matching

The plugin uses workspace name or ID to match windows:

- Prefers `workspace` field (name)
- If not present, uses `workspace_id` field (ID)

## Limitations

- Width configuration supports up to 5 columns (index 0-4)
- Layouts with more than 5 columns will not apply width adjustment
- Floating windows do not participate in width adjustment and automatic tiling
- Automatic maximization only works for tiled windows
