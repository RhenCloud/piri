# Sticky 插件

Sticky 插件用于把**某一个浮动窗口**固定为“跟随窗口”：当你切换焦点工作区时，这个窗口会自动移动到当前焦点工作区。

适合用在常驻小窗（如词典、翻译、日志、播放器控制面板）场景。

## 配置

仅需启用插件：

```toml
[piri.plugins]
sticky = true
```

Sticky 状态为运行时内存状态，不写入配置文件；重启 daemon 后会清空。

## 命令行

```bash
# 将当前焦点窗口设为 sticky（仅同 monitor 跟随）
piri sticky add

# 将当前焦点窗口设为 sticky（允许跨 monitor 跟随）
piri sticky add --cross

# 取消 sticky
piri sticky delete
```

## 行为说明

1. `add` 只接受**当前焦点且为 floating 的窗口**；若当前窗口不是 floating，会返回错误。
2. 仅能同时维护一个 sticky 窗口；再次执行 `add` 会覆盖之前的 sticky 目标。
3. `--cross` 控制是否跨显示器：
   - 不带 `--cross`：只在同 monitor 内跟随工作区；切到别的 monitor 时不会跨屏移动。
   - 带 `--cross`：窗口会跟随到当前焦点 monitor + workspace。
4. `delete` 只清除 sticky 绑定，不会关闭窗口。

## 常见搭配

- 给某个浮动终端/工具窗绑定快捷键，先聚焦该窗后执行 `piri sticky add`。
- 为“跨屏随身窗口”使用 `piri sticky add --cross`。
- 临时结束跟随时执行 `piri sticky delete`。
