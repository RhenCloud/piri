# EdgePulse Edge Indicator

EdgePulse is a sub-feature of the [Workspace Rule](workspace_rule.md) plugin. It renders left/right edge hints on the screen when the focused column reaches the workspace edge, providing a visual cue for navigation boundaries.

<img width="3837" height="2146" alt="Image" src="https://github.com/user-attachments/assets/3d4f763d-ceec-42c2-a535-c29af36cba63" />

<img width="3837" height="2147" alt="Image" src="https://github.com/user-attachments/assets/8a498dec-eb8d-4648-83dd-e71a453754bb" />

## Configuration

Enable EdgePulse in your configuration file under the workspace rule section:

```toml
[piri.plugins]
workspace_rule = true

[piri.workspace_rule.edge_pulse]
enabled = true
show_left = true
show_right = true
width = 14
height_ratio = 0.42
left_gradient_start = "#68d8ff"
left_gradient_end = "#1f4fff"
right_gradient_start = "#ffd36a"
right_gradient_end = "#ff7a1f"
alpha = 0.85
animation_enabled = false
animation_style = "pulse"
animation_duration = 600
animation_amplitude = 0.8
animation_repeat = 3
```

### Workspace-Specific Configuration

You can override EdgePulse settings per workspace:

```toml
[workspace_rule.main.edge_pulse]
enabled = true
show_left = true
show_right = true
width = 10
height_ratio = 0.5
left_gradient_start = "#80e0ff"
left_gradient_end = "#2060ff"
alpha = 0.9
```

## Configuration Parameters

| Parameter | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | `bool` | `false` | Enable EdgePulse missing-neighbor detection and hints |
| `show_left` | `bool` | `true` | Show left hint when reaching the workspace left edge |
| `show_right` | `bool` | `true` | Show right hint when reaching the workspace right edge |
| `width` | `u32` | `14` | Hint width in pixels |
| `height_ratio` | `f64` | `0.42` | Hint height ratio against output height (0.0-1.0) |
| `left_gradient_start` | `String` | `#68d8ff` | Left hint gradient start color |
| `left_gradient_end` | `String` | `#1f4fff` | Left hint gradient end color |
| `right_gradient_start` | `String` | `#ffd36a` | Right hint gradient start color |
| `right_gradient_end` | `String` | `#ff7a1f` | Right hint gradient end color |
| `alpha` | `f64` | `0.85` | Global alpha (0.0-1.0) |
| `animation_enabled` | `bool` | `false` | Enable animation effects (pulse/fade) |
| `animation_style` | `String` | `"pulse"` | Animation type: `"pulse"` for breathing, `"fade"` for fade-in |
| `animation_duration` | `f64` | `600.0` | Single animation cycle duration in milliseconds |
| `animation_amplitude` | `f64` | `0.8` | Animation intensity 0.0-1.0, controls alpha variation range |
| `animation_repeat` | `u32` | `3` | Number of animation repeats per trigger (0 = infinite until state change) |

## Animation How It Works

When `animation_enabled = true` is set, the indicator animates when the focused column reaches the edge:

- **pulse**: Alpha oscillates using a sine wave for a gentle breathing effect that's least intrusive
- **fade**: Fades in once and stays at full alpha, not annoying

Animation renders at ~60 FPS, driven by `timerfd` + `poll` mechanism. CPU usage is 0% when idle. By default, it plays 3 times per trigger (`animation_repeat = 3`), then stays static.

## How It Works

1. On window focus change, workspace switch, window open/close, the plugin evaluates the focused column's position in the scrolling layout.
2. If the focused column reaches the workspace left (or right) edge — meaning no other tiled columns exist in that direction — a colored indicator is rendered on the corresponding screen edge.
3. The indicator uses a vertical gradient with horizontal alpha falloff for a soft edge appearance.
4. All indicators are rendered as Wayland layer-shell overlay surfaces (via `zwlr_layer_shell_v1`), ensuring they appear above all windows without intercepting input events.

## Behavior Details

- Indicators only appear when there are **2 or more columns** in the workspace. A single-column layout shows no indicators.
- Focusing a floating window preserves the current indicator state.
- Switching workspaces forces re-evaluation; each workspace may have its own EdgePulse style.
- Indicator state is cached to avoid redundant renders.
- Indicators are automatically hidden when EdgePulse is disabled or no focused window exists.

## Requirements

- A wlroots-based compositor (e.g., Niri) that provides `zwlr_layer_shell_v1`.
- The `WAYLAND_DISPLAY` environment variable must be set.
