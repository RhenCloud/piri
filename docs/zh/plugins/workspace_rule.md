# Workspace Rule 插件

工作区布局管理，提供自动宽度、auto_tile、auto_fill 与最大化。

## 功能特性

- **auto_width**: 按窗口数自动调整宽度
- **auto_tile**: 新窗口自动合并到现有列（第一列除外）
- **auto_fill**: 窗口关闭后自动对齐到最右侧
- **auto_maximize**: 单窗口最大化，多窗口取消最大化
- **EdgePulse**: 聚焦列达边缘时触发边缘提示

## 演示视频

https://github.com/user-attachments/assets/092a383c-993b-42b5-9b89-b78b0807b436

https://github.com/user-attachments/assets/b48b0465-0101-4298-9935-22f46d1a2658

https://github.com/user-attachments/assets/3b8dc835-473c-4b6e-b684-02a6951b63f9

https://github.com/user-attachments/assets/2e9e2c86-b9ef-44f1-a896-46473b93b417

## 配置

### 基本配置

在配置文件中启用插件：

```toml
[piri.plugins]
workspace_rule = true
```

### 默认配置

使用 `[piri.workspace_rule]` 配置默认设置，这些设置会应用到所有没有特定配置的工作区：

```toml
[piri.workspace_rule]
# 自动宽度配置：数组索引对应窗口数量（从 1 开始）
# 每个元素可以是字符串（所有窗口相同宽度）或数组（每个窗口不同宽度）
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]
# auto_tile：允许每列最多 2 个窗口（第一列除外）
auto_tile = false
# 自动对齐：自动将最后一列对齐到最右侧
auto_fill = false
# 自动最大化：只有一个窗口时最大化，多个窗口时取消最大化
auto_maximize = false
```

### 工作区特定配置

使用 `[workspace_rule.{workspace}]` 为特定工作区配置规则，工作区标识符可以是名称（如 `"browser"`）或索引（如 `"1"`）：

```toml
# 工作区索引配置
[workspace_rule.1]
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]

# 工作区名称配置
[workspace_rule.browser]
# 1 个窗口：100%，2 个窗口：45% 和 55%，3 个窗口：33.33% 每个
auto_width = ["100%", ["45%", "55%"], "33.33%"]

# 启用自动最大化
[workspace_rule.main]
auto_maximize = true

# 启用自动对齐
[workspace_rule.dev]
auto_fill = true
```

### 配置参数说明

| 参数 | 类型 | 说明 |
| :--- | :--- | :--- |
| `auto_width` | `Vec<Vec<String>>` | 宽度配置数组，索引=窗口数-1。元素可为字符串（均宽）或数组（差宽），需百分比格式 |
| `auto_tile` | `bool` | 新窗口合并到非首列的单窗口列（首列除外） |
| `auto_fill` | `bool` | 窗口关闭后对齐到最右侧 |
| `auto_maximize` | `bool` | 单窗口最大化，多窗口取消最大化 |
| `edge_pulse.*` | 见 edge_pulse.md | EdgePulse 边缘提示配置 |

## 工作原理

### auto_width

按**列数**（非窗口数）应用宽度配置：

1. 统计平铺窗口列数
2. 按列数查找宽度配置（索引 = 列数 - 1）
3. 为每列设置宽度百分比

例：`["100%", "50%", ["30%", "35%", "35%"]]`
- 1 列：100%
- 2 列：各 50%
- 3 列：30%、35%、35%

### auto_tile

新窗口打开时，合并到非首列的单窗口列（使用 swallow）。首列不合并。

### auto_fill

**功能**：窗口关闭或布局改变时，自动将最后一列对齐到工作区最右侧，消除空隙。

**触发时机**：
- 窗口关闭（`WindowClosed` 事件）
- 布局变化（`WindowLayoutsChanged` 事件）

**实现方式**：
1. 保存当前焦点窗口
2. 聚焦首列（`focus-window` 到第一列）
3. 聚焦末列（使所有列右对齐）
4. 恢复原始焦点

**配置示例**：
```toml
[piri.workspace_rule]
auto_fill = true  # 全局启用

[workspace_rule.main]
auto_fill = true  # 仅对 main 工作区启用
```

### auto_maximize

- 单窗口：自动最大化到边缘
- 多窗口：取消最大化，恢复正常宽度

插件跟踪已最大化窗口，避免重复处理。

## 事件处理

插件监听以下事件：

- `WindowOpenedOrChanged`: 处理新窗口打开和窗口状态变化
- `WindowClosed`: 处理窗口关闭
- `WindowLayoutsChanged`: 处理布局变化（用于自动对齐）

### 窗口状态跟踪

插件会跟踪以下状态：

- **已见窗口** (`seen_windows`): 区分新窗口和已存在的窗口
- **窗口浮动状态** (`window_floating_state`): 检测浮动/平铺状态变化
- **已最大化窗口** (`maximized_windows`): 跟踪由 `auto_maximize` 最大化的窗口

### 节流机制

`apply_widths` 函数使用 400ms 的节流机制，避免频繁触发：

- 第一个请求立即执行
- 400ms 内的后续请求会被忽略

## 配置示例

**基础配置**：
```toml
[piri.plugins]
workspace_rule = true

[piri.workspace_rule]
auto_width = ["100%", "50%", "33.33%"]  # 1/2/3 窗口宽度
auto_fill = true
```

**工作区特定配置**：
```toml
[workspace_rule.main]
auto_maximize = true

[workspace_rule.dev]
auto_width = ["100%", ["45%", "55%"], ["30%", "35%", "35%"]]
auto_tile = true
```

## 特性

- ✅ 工作区感知，独立配置
- ✅ 灵活配置（默认/工作区特定）
- ✅ 事件驱动，实时响应
- ✅ 节流优化，避免频繁触发
- ✅ 状态跟踪，避免重复处理

## 使用场景

- 多窗口布局管理，保持整洁
- 单窗口最大化，提升专注度
- 窗口关闭后自动对齐
- 新窗口自动合并，优化空间

## 技术细节

### 列数统计

通过 `pos_in_scrolling_layout` 统计列数（仅平铺窗口，索引从 1 开始）。

### 宽度解析

百分比格式（如 `"50%"`），支持小数（如 `"33.33%"`），必须以 `%` 结尾。

## 限制

- 宽度配置最多支持 5 列（索引 0-4）
- 浮动窗口不参与宽度调整和 auto_tile
- 自动最大化仅适用于平铺窗口
