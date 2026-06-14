# Scratchpads Plugin

Scratchpads allows you to quickly show and hide windows of frequently used applications, with support for cross-workspace and cross-monitor functionality.

## Demo Video

https://github.com/user-attachments/assets/1c40d75b-310a-4503-9d59-92f0479a54d5

## Configuration

Use the `[scratchpads.{name}]` format to configure scratchpads:

```toml
[piri.plugins]
scratchpads = true

[scratchpads.term]
direction = "fromRight"
command = "GTK_IM_MODULE=wayland ghostty --class=float.dropterm"
app_id = "float.dropterm"
size = "40% 60%"
margin = 50

[scratchpads.calc]
direction = "fromBottom"
command = "gnome-calculator"
app_id = "gnome-calculator"
size = "50% 40%"
margin = 100

[scratchpads.preview]
direction = "fromRight"
command = "imv"
app_id = "imv"
size = "60% 80%"
margin = 50
swallow_to_focus = true  # Automatically swallow into focused window when shown

[scratchpads.note]
direction = "fromTop"
command = "gnome-text-editor"
app_id = "org.gnome.TextEditor"
size = "50% 40%"
margin = 100
sticky = true  # Follow focused workspace (handled by sticky plugin)

[scratchpads.calc2]
direction = "fromBottom"
command = "gnome-calculator"
app_id = "org.gnome.Calculator"
size = "30% 40%"
margin = 50
auto_hide_on_focus_loss = true  # Auto-hide when window loses focus
```

### Configuration Parameters

- `direction` (required): Direction from which the window appears
  - `fromTop`: Slide in from top
  - `fromBottom`: Slide in from bottom
  - `fromLeft`: Slide in from left
  - `fromRight`: Slide in from right
- `command` (required): Full command string to launch the application, can include environment variables and arguments
- `app_id` (required): Application ID used to match windows (supports regular expressions)
- `size` (required): Window size in format `"width% height%"`
- `margin` (required): Margin from screen edge in pixels
- `swallow_to_focus` (optional): If `true`, when showing, the scratchpad window will be swallowed into the currently focused window. When hiding, the window will be set to floating first, then execute the normal hide logic. Defaults to `false`
- `sticky` (optional): If `true`, the scratchpad window will follow the focused workspace (like sticky windows). This behavior is delegated to the sticky plugin via a global registry; scratchpads only registers the window. Defaults to `false`
- `auto_hide_on_focus_loss` (optional): If `true`, the scratchpad window will automatically hide when it loses focus. Defaults to `false`

> **Note**: `sticky` and `auto_hide_on_focus_loss` cannot both be enabled for the same scratchpad. Attempting to do so will result in a configuration error.

> **Note**: `app_id` uses regular expression matching. If `app_id` contains special characters (such as `.`, `*`, etc.), they need to be escaped. For example: `app_id = "float\\.dropterm"`
>
> **Reference**: For detailed information about the window matching mechanism, see [Window Matching Mechanism](../window_matching.md)

## Usage

### Toggle Visibility

```bash
piri scratchpads {name} toggle

# Examples
piri scratchpads term toggle
piri scratchpads calc toggle
```

> **Note**: When toggling, if the target window is not in floating state (e.g., set to tiled mode), it will directly focus the window without executing show/hide animations. This is because scratchpad functionality only works with floating windows.

### Add Current Window

Quickly add the currently focused window as a scratchpad:

```bash
piri scratchpads {name} add {direction} [--swallow-to-focus]

# Examples
piri scratchpads mypad add fromRight
piri scratchpads mypad add fromRight --swallow-to-focus  # Enable swallow feature
```

Dynamically added scratchpads will use the default size and margin set in the `[piri.scratchpad]` section.

> **Note**:
> - Dynamically added windows are only resized and positioned once during initial registration. After that, you can manually resize or move the window, and the plugin will maintain your custom size and margin (position) during subsequent show/hide toggles without overriding it.
> - If the scratchpad already exists, the `add` command will automatically execute a toggle operation (show/hide) instead of reporting an error.

### Global Configuration

You can set global defaults in the `[piri.scratchpad]` section:

| Parameter | Description | Default |
| :--- | :--- | :--- |
| `default_size` | Default size for dynamic addition | `"75% 60%"` |
| `default_margin` | Default margin for dynamic addition | `50` |
| `move_to_workspace` | (Optional) Workspace to move windows to when hidden | `None` |

> **move_to_workspace**: If specified, hidden scratchpad windows will be moved to this workspace. This keeps hidden windows out of the current workspace's window stack. When shown, the window will still automatically move to the currently active workspace.

## How It Works

1. **First Launch**: If the window doesn't exist, launches the application specified in the configuration
2. **Window Registration**: After finding the window, sets it to floating mode and moves it off-screen
3. **Toggle Check**: When executing toggle, if the window is not floating, directly focuses the window and returns immediately without animation
4. **Show**: Moves the window to the currently focused output and workspace, positions it according to configured direction and size, and focuses the window
5. **Hide**: Moves the window off-screen and intelligently restores previous focus

**Cross-workspace and cross-monitor**: Regardless of which workspace or monitor the scratchpad window was originally on, it will automatically move to the currently focused location.

> **Note**: Scratchpad functionality only works with floating windows. Features like `auto_hide_on_focus_loss` also only apply to floating windows. If a window is changed to tiled mode, toggle will only directly focus that window.

## Features

- ✅ **Cross-workspace**: Quick access from any workspace
- ✅ **Cross-monitor**: Automatically appears on the currently focused monitor
- ✅ **Smart focus management**: Automatically focuses when showing, restores previous focus when hiding
- ✅ **Non-floating window handling**: When toggling, if window is not floating, directly focus without animation
- ✅ **Flexible configuration**: Customize window size, position, and animation direction
- ✅ **Dynamic addition**: Quickly add the currently focused window as a scratchpad
- ✅ **Swallow integration**: Support swallowing scratchpad window into the currently focused window (`swallow_to_focus` option)
- ✅ **Sticky behavior**: Follow focused workspace via sticky plugin integration (`sticky` option)
- ✅ **Auto-hide on focus loss**: Automatically hide when window loses focus (`auto_hide_on_focus_loss` option, only works with floating windows)
