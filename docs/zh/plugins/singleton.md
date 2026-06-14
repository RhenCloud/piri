# Singleton 插件

管理单例窗口，确保全局唯一。切换时聚焦已有实例或启动新进程。

## 配置

使用 `[singleton.{name}]` 格式配置单例：

```toml
[piri.plugins]
singleton = true

[singleton.browser]
command = "google-chrome-stable"

[singleton.term]
command = "GTK_IM_MODULE=wayland ghostty --class=singleton.term"
app_id = "singleton.term"

[singleton.editor]
command = "code"
app_id = "code"
on_created_command = "notify-send '编辑器已打开'"
```

### 配置参数

- `command` (必需): 启动命令（可含环境变量和参数）
- `app_id` (可选): 应用 ID（支持正则，默认从命令提取可执行文件名）
- `on_created_command` (可选): 仅创建新窗口时执行

> **窗口匹配**: `app_id` 使用正则表达式匹配。关于窗口匹配机制的详细说明（包括特殊字符转义），请参阅 [窗口匹配机制文档](../window_matching.md) 和 [插件系统通用配置说明](plugins.md#通用配置说明)

## 使用方法

```bash
# 切换单例（如果存在则聚焦，否则启动）
piri singleton {name} toggle

# 示例
piri singleton browser toggle
piri singleton term toggle
```

## 工作原理

1. 首次切换：检查匹配窗口，找到则聚焦并注册，否则启动应用并等待
2. 窗口创建：新窗口出现后执行 `on_created_command`（如配置）
3. 后续切换：聚焦已存在窗口，否则搜索或重新启动
4. 窗口匹配：使用配置 `app_id` 或从命令提取

## 特性

- ✅ 智能检测，避免重复启动
- ✅ 自动提取 `app_id`（默认从命令提取）
- ✅ 窗口注册，快速查找
- ✅ 健壮匹配，支持非插件启动的窗口

## 使用场景

- 浏览器、终端等单实例应用
- 快速访问常用应用
- 防止资源密集型应用多开

## 注意事项

1. **窗口匹配**: 确保应用设置正确 `app_id`，或明确配置
2. **app_id 提取**: 从命令第一个单词提取（去除路径）
3. **超时**: 启动后最多等待 5 秒，超时后不报错也不聚焦
4. **on_created_command**: 仅创建新窗口时执行，窗口重开会再次执行
