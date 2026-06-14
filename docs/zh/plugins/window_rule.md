# Window Rule 插件

基于正则匹配自动归位窗口至指定 workspace，支持焦点触发命令。

## 配置

使用 `[[window_rule]]` 格式配置窗口规则：

```toml
[piri.plugins]
window_rule = true

# 根据 app_id 匹配
[[window_rule]]
app_id = "ghostty"
open_on_workspace = "1"

# 根据 title 匹配
[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "browser"
focus_command = "notify-send 'Focusing on Chrome'"

# 同时指定 app_id 和 title（任一匹配即可）
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"

# 只有 focus_command，不移动窗口
[[window_rule]]
title = ".*Chrome.*"
focus_command = "notify-send 'Chrome focused'"

# focus_command 仅对规则全局执行一次（规则级别，非窗口级别）
[[window_rule]]
app_id = "firefox"
focus_command = "notify-send 'Firefox focused'"
focus_command_once = true

# 正则表达式示例：匹配以 "firefox" 开头的 app_id
[[window_rule]]
app_id = "^firefox"
open_on_workspace = "2"

# 匹配精确的 app_id
[[window_rule]]
app_id = "^code$"
open_on_workspace = "dev"

# app_id 作为列表（任意一个匹配即可）
[[window_rule]]
app_id = ["code", "code-oss", "codium"]
open_on_workspace = "dev"

# title 作为列表（任意一个匹配即可）
[[window_rule]]
title = [".*Chrome.*", ".*Chromium.*", ".*Google Chrome.*"]
open_on_workspace = "browser"
```

## 配置字段

- `app_id` (可选): 正则匹配窗口 `app_id`，支持字符串或列表（OR 逻辑）
- `title` (可选): 正则匹配窗口标题，支持字符串或列表（OR 逻辑）
- `open_on_workspace` (可选): 目标 workspace（名称/索引），支持 `workspace@output` 格式指定显示器。末尾加 `!` 可将窗口锁定在该 workspace（如 `"1@eDP!"`）
- `focus_command` (可选): 窗口获焦时执行命令
- `focus_command_once` (默认 `false`): 规则级单次执行（[issue #1](https://github.com/Asthestarsfalll/piri/issues/1)）

**注意**:
- 至少指定 `app_id`/`title` 之一
- 至少指定 `open_on_workspace`/`focus_command` 之一
- `app_id`/`title` 可单独或列表形式，任一匹配即触发

### 显示器匹配

使用 `workspace@output` 格式将窗口移动到指定显示器上的工作区：

```toml
# 将 Firefox 移到 DP-1 显示器上的工作区 "2"
[[window_rule]]
app_id = "firefox"
open_on_workspace = "2@DP-1"

# 将 Chrome 移到 eDP-1 显示器上的工作区 "browser"
[[window_rule]]
app_id = "chrome"
open_on_workspace = "browser@eDP-1"
```

> **窗口匹配**: 关于窗口匹配机制的详细说明，请参阅 [窗口匹配机制文档](../window_matching.md) 和 [插件系统通用配置说明](plugins.md#通用配置说明)

> **Workspace 标识符**: 关于 workspace 标识符（name/idx）的详细说明，请参阅 [插件系统通用配置说明](plugins.md#workspace-标识符)

## 工作原理

监听 `WindowOpened` 事件：

1. 正则匹配窗口 `app_id`/`title`
2. 匹配后自动移至指定 workspace
3. 按配置顺序检查，**首条匹配规则生效**
4. 如果 `open_on_workspace` 以 `!` 结尾，窗口将被锁定在该 workspace

### Workspace 锁定

在 `open_on_workspace` 末尾加 `!` 可将窗口锁定在指定 workspace，窗口无法被手动移走：

```toml
# 锁定到 workspace 1（eDP 显示器）
[[window_rule]]
app_id = "firefox"
open_on_workspace = "1@eDP!"

# 锁定到名为 "dev" 的 workspace
[[window_rule]]
app_id = "^code$"
open_on_workspace = "dev!"
```

锁定的窗口在每次属性变化时会被检查，如果不在目标 workspace 则自动移回。

## 特性

- ✅ 正则匹配（`app_id`/`title`，支持列表与 OR 逻辑）
- ✅ 正则缓存，提升性能
- ✅ 配置热更新，无需重启
- ✅ Workspace 锁定（`!` 后缀），防止窗口被移走

## focus_command_once

规则级单次执行，非窗口级（[issue #1](https://github.com/Asthestarsfalll/piri/issues/1)）。

## 注意事项

1. **规则顺序**：首条匹配生效，后续不检查
2. **Workspace 不存在**：记录警告，不报错
3. **正则性能**：建议简单明确的模式
4. **focus_command_once**：规则级跟踪，执行后不再触发
