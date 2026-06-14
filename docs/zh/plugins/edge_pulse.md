# EdgePulse 边缘提示

EdgePulse 是 [Workspace Rule](workspace_rule.md) 插件的子功能。当当前聚焦列到达工作区左/右边缘时，在屏幕对应边缘渲染提示指示器，提供导航边界的视觉提示。

<img width="3837" height="2146" alt="Image" src="https://github.com/user-attachments/assets/3d4f763d-ceec-42c2-a535-c29af36cba63" />

<img width="3837" height="2147" alt="Image" src="https://github.com/user-attachments/assets/8a498dec-eb8d-4648-83dd-e71a453754bb" />

## 配置

在配置文件的 workspace rule 段启用 EdgePulse：

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

### 工作区级别配置

可以为特定工作区覆盖 EdgePulse 设置：

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

## 配置参数

| 参数 | 类型 | 默认值 | 说明 |
| :--- | :--- | :--- | :--- |
| `enabled` | `bool` | `false` | 是否启用 EdgePulse 左右侧缺邻居检测与提示 |
| `show_left` | `bool` | `true` | 到达工作区左边缘时是否显示左侧提示 |
| `show_right` | `bool` | `true` | 到达工作区右边缘时是否显示右侧提示 |
| `width` | `u32` | `14` | 提示宽度（像素） |
| `height_ratio` | `f64` | `0.42` | 提示高度占输出高度的比例（0.0-1.0） |
| `left_gradient_start` | `String` | `#68d8ff` | 左侧提示渐变起始色 |
| `left_gradient_end` | `String` | `#1f4fff` | 左侧提示渐变结束色 |
| `right_gradient_start` | `String` | `#ffd36a` | 右侧提示渐变起始色 |
| `right_gradient_end` | `String` | `#ff7a1f` | 右侧提示渐变结束色 |
| `alpha` | `f64` | `0.85` | 全局透明度（0.0-1.0） |
| `animation_enabled` | `bool` | `false` | 是否启用动效（pulse/fade） |
| `animation_style` | `String` | `"pulse"` | 动效类型：`"pulse"` 呼吸，`"fade"` 淡入 |
| `animation_duration` | `f64` | `600.0` | 单次动效周期（毫秒） |
| `animation_amplitude` | `f64` | `0.8` | 动效强度 0.0-1.0，影响透明度变化范围 |
| `animation_repeat` | `u32` | `3` | 每次触发播放次数（0=无限循环直到状态变化） |

## 动效工作原理

启用 `animation_enabled = true` 后，当聚焦列到达边缘时，指示器将以动画形式呈现：

- **pulse（脉冲）**：透明度呈正弦波呼吸，柔和提醒，最不显眼
- **fade（淡入）**：仅淡入一次后保持常亮，不烦人

动画以 ~60 FPS 帧率渲染，通过 `timerfd` + `poll` 机制驱动，空闲时 CPU 占用 0%。每次触发默认播放 3 次（`animation_repeat = 3`），然后保持静态常亮。

## 工作原理

1. 在窗口焦点变化、工作区切换、窗口打开/关闭时，插件评估当前聚焦列在滚动布局中的位置。
2. 如果聚焦列到达工作区左（或右）边缘（即该方向没有其他平铺列），则在对应屏幕边缘渲染彩色指示器。
3. 指示器使用垂直渐变配合水平 alpha 衰减，呈现柔和的边缘效果。
4. 所有指示器均以 Wayland layer-shell overlay surface 渲染（通过 `zwlr_layer_shell_v1`），确保显示在所有窗口上方且不拦截输入事件。

## 行为说明

- 仅当工作区中存在 **2 个或更多列**时才显示指示器，单列布局不显示。
- 聚焦浮动窗口时保持当前指示器状态不变。
- 切换工作区时强制重新评估；每个工作区可使用独立的 EdgePulse 样式。
- 指示器状态会被缓存，避免冗余渲染。
- 禁用 EdgePulse 或无聚焦窗口时，指示器自动隐藏。

## 系统要求

- 基于 wlroots 的合成器（如 Niri），需提供 `zwlr_layer_shell_v1`。
- 必须设置 `WAYLAND_DISPLAY` 环境变量。
