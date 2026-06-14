# 窗口匹配机制

Piri 使用统一的正则匹配机制，通过 `app_id` 和/或 `title` 查找窗口。插件 `window_rule`、`singleton`、`scratchpads` 等均使用此机制。

## 匹配方式

### 正则表达式

基于 Rust `regex` crate，支持完整正则语法。

### 匹配字段

- `app_id`: 应用 ID（可选）
- `title`: 窗口标题（可选）

**至少指定其一**，同时指定时任一匹配即可（OR 逻辑）。

## 使用示例

### 基本匹配

```toml
app_id = "code"              # 精确匹配
app_id = ".*chrome.*"        # 包含匹配
app_id = "^firefox"           # 开头匹配
app_id = "^code$"             # 精确锚定
```

### 标题匹配

```toml
title = ".*Chrome.*"          # 包含 "Chrome"
title = "^VS Code"            # 以 "VS Code" 开头
title = ".*\\d+.*"           # 包含数字
```

### 组合匹配

```toml
app_id = "code"
title = ".*VS Code.*"      # app_id 或 title 任一匹配即可
```

## 插件使用示例

### Window Rule

```toml
[[window_rule]]
app_id = ".*firefox.*"
open_on_workspace = "2"
```

> Window Rule 支持列表匹配，详见 [文档](plugins/window_rule.md)。

### Singleton

```toml
[singleton.browser]
command = "google-chrome-stable"
app_id = "google-chrome"
```

### Scratchpads

```toml
[scratchpads.term]
direction = "fromRight"
command = "ghostty"
app_id = "float\\.dropterm"  # 转义点号
```

## 正则语法参考

### 常用模式

| 模式 | 说明 | 示例 |
|------|------|------|
| `.` | 任意字符 | `"c.ode"` → `"code"`, `"cade"` |
| `.*` | 零或多个任意字符 | `".*chrome.*"` → 包含 `chrome` |
| `^` | 字符串开头 | `"^firefox"` → 以 `firefox` 开头 |
| `$` | 字符串结尾 | `"code$"` → 以 `code` 结尾 |
| `[abc]` | 字符集 | `"[abc]ode"` → `"aode"`, `"bode"`, `"code"` |
| `\d` | 数字（同 `[0-9]`） | `"\d+"` → 一个或多个数字 |
| `\w` | 单词字符 | `"\w+"` → 单词 |
| `+` | 一个或多个 | `"[0-9]+"` |
| `*` | 零个或多个 | `".*"` |
| `?` | 零个或一个 | `"colou?r"` → `"color"`/`"colour"` |
| `\|` | 或 | `"firefox\|chrome"` |

### 转义特殊字符

特殊字符（`.`, `*`, `+`, `?`, `[`, `]`, `(`, `)`, `{`, `}`, `^`, `$`, `|`, `\`）需转义：

```toml
app_id = "float\\.dropterm"  # 转义点号
title = ".*\\(.*\\).*"      # 转义括号
```

## 性能与最佳实践

1. **正则缓存**：编译后缓存，避免重复编译
2. **简单模式**：简单明确的模式性能更好
3. **精确匹配**：已知 `app_id` 时用 `^app_id$`
4. **部分匹配**：`".*pattern.*"` 进行部分匹配
5. **转义特殊字符**：包含特殊字符时记得转义
6. **测试模式**：配置前用在线工具验证

## 调试技巧

1. **检查日志**：查看 piri 日志了解匹配过程
2. **验证 app_id/title**：用 `niri-ipc` 查看实际值
3. **测试正则**：使用在线工具测试
4. **简化模式**：先用简单模式验证，再逐步复杂化

## 示例配置

### 匹配多个浏览器

```toml
[[window_rule]]
app_id = ".*(firefox|chrome|chromium).*"
open_on_workspace = "browser"
```

### 匹配开发工具、终端

```toml
[[window_rule]]
app_id = ".*(code|vscode|idea).*"
open_on_workspace = "dev"
```

```toml
[[window_rule]]
app_id = ".*(term|terminal|ghostty|alacritty).*"
open_on_workspace = "1"
```

### 标题匹配

```toml
[[window_rule]]
title = ".*GitHub.*"
open_on_workspace = "dev"
```
