# Scratchpads 插件

快速显示/隐藏常用窗口，支持跨 workspace 与 monitor。

## 演示视频

https://github.com/user-attachments/assets/1c40d75b-310a-4503-9d59-92f0479a54d5

## 配置

使用 `[scratchpads.{name}]` 格式配置 scratchpad：

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
swallow_to_focus = true  # 显示时自动吞入当前聚焦的窗口

[scratchpads.note]
direction = "fromTop"
command = "gnome-text-editor"
app_id = "org.gnome.TextEditor"
size = "50% 40%"
margin = 100
sticky = true  # 跟随焦点工作区（由 sticky 插件处理）

[scratchpads.calc2]
direction = "fromBottom"
command = "gnome-calculator"
app_id = "org.gnome.Calculator"
size = "30% 40%"
margin = 50
auto_hide_on_focus_loss = true  # 失去焦点时自动隐藏
```

### 配置参数

- `direction` (必需): 窗口出现方向 (`fromTop`/`fromBottom`/`fromLeft`/`fromRight`)
- `command` (必需): 启动命令（可含环境变量和参数）
- `app_id` (必需): 应用 ID（支持正则，见 [窗口匹配](../window_matching.md)）
- `size` (必需): 窗口大小 `"宽% 高%"`
- `margin` (必需): 边距（像素）
- `swallow_to_focus` (可选): 显示时吞入聚焦窗口（默认 `false`）
- `sticky` (可选): 跟随焦点工作区，委托 sticky 插件（默认 `false`）
- `auto_hide_on_focus_loss` (可选): 失焦自动隐藏（默认 `false`）

> **注意**: `sticky` 与 `auto_hide_on_focus_loss` 不可同时启用。

## 使用方法

### 切换显示/隐藏

```bash
piri scratchpads {name} toggle

# 示例
piri scratchpads term toggle
piri scratchpads calc toggle
```

> **注意**: 切换时，如果目标窗口不是浮动状态（例如被设置为平铺窗口），将直接 focus 到该窗口，不会执行显示/隐藏动画。这是因为 scratchpad 功能仅对浮动窗口生效。

### 动态添加当前窗口

将当前聚焦的窗口快速添加为 scratchpad：

```bash
piri scratchpads {name} add {direction} [--swallow-to-focus]

# 示例
piri scratchpads mypad add fromRight
piri scratchpads mypad add fromRight --swallow-to-focus  # 启用 swallow 功能
```

动态添加的 scratchpad 会使用 `[piri.scratchpad]` 节中设置的默认大小和边距。

> **提示**:
> - 动态添加的窗口仅在第一次注册时调整大小和位置。之后你可以手动调整该窗口的大小和位置（边距），插件在后续切换显示/隐藏时会保持你手动调整后的状态，不再强制重置。
> - 如果 scratchpad 已存在，`add` 命令会自动执行 toggle 操作（显示/隐藏切换），而不是报错。

### 全局默认配置

`[piri.scratchpad]` 节设置动态添加时的默认值：

| 参数 | 说明 | 默认 |
| :--- | :--- | :--- |
| `default_size` | 默认大小 | `"75% 60%"` |
| `default_margin` | 默认边距 | `50` |
| `move_to_workspace` | 隐藏后移至工作区 | 无 |

> **move_to_workspace**: 隐藏时移至指定工作区，显示时返回当前工作区。

## 工作原理

1. 首次启动：窗口不存在时启动应用
2. 窗口注册：找到后设为浮动并移至屏外
3. 切换检查：非浮动窗口直接 focus，不执行动画
4. 显示：移至当前工作区/显示器，按配置定位并聚焦
5. 隐藏：移至屏外，恢复之前焦点

**跨 workspace/monitor**: 自动移至当前聚焦位置。

> **注意**: 仅对浮动窗口生效。非浮动窗口 toggle 时仅直接聚焦。

## 特性

- ✅ 跨 workspace/monitor 快速访问
- ✅ 智能焦点管理（显示聚焦，隐藏恢复）
- ✅ 非浮动窗口直接 focus
- ✅ 灵活配置（大小、位置、方向）
- ✅ 动态添加当前窗口
- ✅ Swallow 集成（`swallow_to_focus`）
- ✅ Sticky 跟随（`sticky`，委托 sticky 插件）
- ✅ 失焦自动隐藏（`auto_hide_on_focus_loss`，仅浮动窗口）
