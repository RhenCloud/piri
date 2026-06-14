# 细粒度事件拆分

Piri 在事件分发层对 Niri 的粗粒度事件进行自动拆分，将一个复合事件拆分为多个语义明确的子事件，插件只需关心自己需要的事件类型。

## 与 Niri 原生的区别

Niri 的 `WindowOpenedOrChanged` 事件粒度较粗——同一个事件同时涵盖新窗口打开、浮动/平铺状态切换、窗口属性变化（title、app_id 等）。每个需要区分这些情况的插件都必须自行维护 `seen_windows`、`window_floating_state` 等状态来判断发生了什么。

Piri 的做法：在事件分发层自动将其拆分为三个独立的子事件，插件无需自行追踪状态。

## PiriEvent 事件枚举

Piri 定义了自己的事件枚举 `PiriEvent`，将 Niri 原始事件拆分为细粒度子事件：

### 子事件（从 WindowOpenedOrChanged 拆分）

| 事件 | 说明 |
|------|------|
| `WindowOpened` | 新窗口首次出现 |
| `WindowChanged` | 已有窗口属性变化（title、app_id、layout 等），但浮动/平铺状态未变 |
| `WindowToggleFloating` | 窗口在浮动和平铺之间切换 |

### 透传事件

以下事件直接从 Niri 透传，语义不变：

| 事件 | 说明 |
|------|------|
| `WindowClosed` | 窗口关闭 |
| `WindowFocusChanged` | 窗口焦点变化 |
| `WindowLayoutsChanged` | 窗口布局变化（resize 等） |
| `WindowFocusTimestampChanged` | 窗口焦点时间戳变化 |
| `WindowUrgencyChanged` | 窗口紧急状态变化 |
| `WindowsChanged` | 窗口列表全量更新 |
| `WorkspaceActivated` | 工作区激活 |
| `WorkspaceActiveWindowChanged` | 工作区活动窗口变化 |
| `WorkspacesChanged` | 工作区配置变化 |
| `KeyboardLayoutsChanged` | 键盘布局配置变化 |
| `KeyboardLayoutSwitched` | 键盘布局切换 |
| `ConfigLoaded` | 配置加载 |

## 分类逻辑

`EventNormalizer` 内部维护窗口状态追踪：

```
WindowOpenedOrChanged { window } 到达时：

1. window.id 不在已知窗口列表中
   → 发出 WindowOpened
   → 记录 window.id 和 is_floating 状态

2. window.id 在已知窗口列表中，且 is_floating 与记录不同
   → 发出 WindowToggleFloating
   → 更新 is_floating 记录

3. 其他情况
   → 发出 WindowChanged
```

状态清理：
- `WindowClosed` 时移除窗口记录
- `WindowsChanged` 时从全量窗口列表重建状态
- 启动时从初始窗口列表播种，避免已有窗口误触发 `WindowOpened`

## 对插件的影响

所有插件现在接收 `PiriEvent` 而非 `niri_ipc::Event`。插件只需在 `is_interested_in_event` 中声明关心的子事件，无需自行追踪窗口状态。

**迁移前**（需要自行追踪状态）：
```rust
// 插件内部需要维护 seen_windows、window_floating_state 等
fn handle_event(&mut self, event: &niri_ipc::Event) {
    match event {
        Event::WindowOpenedOrChanged { window } => {
            let is_new = !self.seen_windows.contains(&window.id);
            let floating_changed = ...; // 需要自行比较
            // 复杂的分类逻辑
        }
    }
}
```

**迁移后**（事件已规范化）：
```rust
fn handle_event(&mut self, event: &PiriEvent) {
    match event {
        PiriEvent::WindowOpened { window } => { /* 新窗口 */ }
        PiriEvent::WindowToggleFloating { window } => { /* 浮动切换 */ }
        PiriEvent::WindowChanged { window } => { /* 属性变化 */ }
        _ => {}
    }
}
```
