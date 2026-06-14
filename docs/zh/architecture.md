# 架构设计

Piri 采用模块化、状态驱动的事件导向架构，为 Niri 合成器提供高性能、低延迟扩展能力。

## 核心设计理念

### 1. 状态驱动与 Manager 模式
复杂插件（Scratchpads、Singleton 等）遵循 **State-Manager** 模式：
- **State**：聚合静态配置（TOML）与运行时状态（`window_id`、可见性、最后聚焦窗口）
- **Manager**：维护 `HashMap<String, State>`，提供原子化操作
- **插件外壳**：实现 `Plugin` trait，映射 IPC 请求与 Niri 事件到 Manager 操作

### 2. 资源保障机制
"延迟初始化"与"自动恢复"策略，核心方法 `ensure_window_id` 实现：
1. 有效性检查：验证 `window_id` 是否仍存在
2. 自动捕获：按 `app_id` 模式搜索并捕获窗口
3. 自动启动：执行命令并等待窗口出现
4. 初始化设置：浮动、大小、几何定位

### 3. 智能配置热重载
- **配置合并**：保留旧状态中已关联的 `window_id`
- **动态保护**：IPC 动态添加的资源（`is_dynamic`）智能保留
- **缓存清理**：自动清理 `WindowMatcher` 正则缓存

## 核心模块

### 插件系统 (`src/plugins/`)
- `mod.rs`: `Plugin` trait 与事件/IPC 分发总线
- `scratchpads.rs`: 窗口隐藏/显示，跨工作区/显示器
- `singleton.rs`: 单实例保障与快速切换
- `window_rule.rs`: 规则引擎，窗口归位与焦点命令
- `window_utils.rs`: 几何计算与窗口匹配引擎，含正则缓存池

### 通信与事件
- `niri.rs`: 异步 IPC 客户端，封装 Niri 动作
- `daemon.rs`: 事件流分发、信号处理、插件生命周期
- `ipc.rs`: Unix Socket 内部命令协议

## 性能与健壮性

### 统一去重机制
基于 **窗口 ID + 时间戳** 的去重引擎（冷却 200-500ms），避免 `focus_command` 等副作用重复触发。

### 几何计算抽象
屏幕边缘、边距、百分比计算收拢于 `window_utils.rs`，一次性获取 Output 上下文，减少 IPC 往返。

### 正则表达式缓存
`WindowMatcherCache`（`Arc<Mutex<HashMap<String, Regex>>>`）避免重复编译，优化 `window_rule` 等高频匹配场景。
