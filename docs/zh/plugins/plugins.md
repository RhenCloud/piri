# 插件系统

Piri 支持插件系统，允许你扩展功能。插件在 daemon 模式下自动运行。

## 可用插件

### [Scratchpads](scratchpads.md)

快速显示/隐藏常用窗口，支持跨工作区与显示器。

**主要特性**：
- 快速显示/隐藏常用应用程序
- 跨工作区和跨显示器支持
- 可自定义出现方向和大小

### [Empty 插件](empty.md)

空工作区自动化，切换时自动执行命令。

**主要特性**：
- 在空工作区上自动执行命令
- 基于工作区的配置
- 类似于 Hyprland 的 `on-created-empty` 工作区规则

### [Singleton 插件](singleton.md)

管理单例窗口，确保全局唯一。

**主要特性**：
- 智能检测，避免重复启动
- 自动提取 App ID
- 支持创建后执行命令

### [Window Rule 插件](window_rule.md)

基于正则匹配自动归位窗口至指定工作区，支持焦点触发命令。

**主要特性**：
- 正则匹配（`app_id`/`title`，支持列表）
- 支持 workspace 名称/索引
- 焦点触发命令，内置去重

### [Workspace Rule 插件](workspace_rule.md)

工作区布局管理，提供自动宽度、平铺、对齐与最大化。

**主要特性**：
- 自动宽度、平铺、对齐、最大化
- 内置 EdgePulse 边缘提示
- 工作区感知，独立配置

### [Window Order 插件](window_order.md)

按权重自动重排窗口，权重越大越靠左。

**主要特性**：
- 智能排序，最小化移动
- 支持手动/事件驱动触发
- 支持 `app_id` 部分匹配

### [Swallow 插件](swallow.md)

窗口吞噬，子窗口替换父窗口位置。

**主要特性**：
- PID 父子进程匹配（默认）
- 支持规则匹配（`app_id`/`title`/`pid`）
- 智能聚焦窗口队列

### [Mark 插件](mark.md)

为窗口设置命名标记，通过 `piri mark …` 绑定/跳回。绑定存于内存。

**主要特性**：
- `toggle` / `add` / `delete` 操作
- 适合与 Niri `spawn` 或启动器组合
- 可选回焦功能：再次触发相同标记跳回之前的窗口

### [Sticky 插件](sticky.md)

将浮动窗口设为跟随窗口，切换工作区时自动跟随。

**主要特性**：
- `add` / `delete` 命令
- `--cross` 控制跨显示器跟随
- 仅支持 floating 窗口

### [Sleepy 插件](sleepy.md)

在窗口焦点变化时将当前应用状态上报到 Sleepy 服务端，适用于在线状态展示和设备状态同步。

**主要特性**：
- 事件驱动上报（焦点变化触发）
- 支持 Bearer Token 和 secret 两种鉴权方式
- 状态去重，避免重复请求

## 通用配置说明

### 窗口匹配机制

多个插件（如 `window_rule`、`singleton`、`scratchpads`）使用统一的窗口匹配机制，支持通过正则表达式匹配窗口的 `app_id` 和 `title`。

**关键特性**：
- 支持完整的正则表达式语法
- 可以匹配 `app_id` 或 `title`，或两者组合（OR 逻辑）
- 特殊字符需要转义（如 `.` 需要写成 `\\.`）

> **详细说明**: 关于窗口匹配机制的完整文档，请参阅 [窗口匹配机制文档](../window_matching.md)

### Workspace 标识符

多个插件支持通过工作区名称或索引来指定目标工作区：

- **name**: 工作区名称，如 `"main"`, `"work"`, `"dev"`
- **idx**: 工作区索引（1-based），如 `"1"`, `"2"`

**匹配顺序**：name 优先，然后 idx。插件自动识别类型并支持跨类型匹配。

#### 多显示器配置

**显示器接口匹配**：指定 `"[name/idx]@DP"` 会匹配所有以 `DP` 开头的显示器（如 `DP-1`、`DP-2`）。前缀提取基于已知显示器接口命名约定（DP、eDP、HDMI、VGA、DVI-D、DVI-I、Virtual）。

**显示器匹配**：指定 `"[name/idx]@DP-1"` 会匹配到具体连接在`DP-1`的显示器上。


## 插件控制

你可以在配置文件中控制哪些插件启用或禁用：

```toml
[piri.plugins]
scratchpads = true
empty = true
window_rule = true
workspace_rule = true
singleton = true
window_order = true
swallow = true
mark = true
sticky = true
```

**默认行为**：
- 如果未明确指定，插件默认**禁用**（`false`）
- 必须显式将对应项设为 `true` 来启用插件
- `window_rule` 插件例外：如果配置了窗口规则，默认启用（除非显式设置为 `false`）
