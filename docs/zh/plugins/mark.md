# Mark 插件

Mark 插件为窗口提供**可命名的快捷焦点**：把一个字符串（通常是一个字母，如 `a`）绑定到当前窗口，之后用同一名称即可快速切回该窗口。绑定关系保存在 daemon 内存中，**不写配置文件**。

## 配置

只需在插件列表中启用：

```toml
[piri.plugins]
mark = true
```

可选的 mark 设置：

```toml
[piri.mark]
# 若为 true，再次触发已聚焦的标记会跳回之前的窗口
refocus = false
```

所有标记均在运行时通过 CLI 创建或删除。`refocus` 设置支持热重载。

## 命令行

```bash
# 若该标记已指向仍存在窗口 → 聚焦该窗口；否则将当前焦点窗口绑定到该标记
piri mark {name} toggle

# 删除该标记（不存在也视为成功）
piri mark {name} delete

# 将当前焦点窗口绑定到该标记（若已有绑定则直接覆盖）
piri mark {name} add
```

示例：

```bash
piri mark a toggle   # 第一次：把当前窗口标为 a；之后：跳到 a
piri mark a add      # 强制把当前窗口重新标为 a
piri mark a delete   # 取消 a
```

## 行为说明

1. **`toggle`**：若 `name` 已有绑定且对应窗口仍存在，则**聚焦**该窗口；否则（无绑定或窗口已关闭）将**当前焦点窗口**写入 `name`。
2. **`add`**：始终用当前焦点窗口覆盖 `name`，不先判断旧窗口是否仍存在。
3. **`delete`**：移除 `name` 的绑定，幂等。

若要在**已有有效绑定**时改绑到别的窗口，请使用 **`add`**，或先 **`delete`** 再 **`toggle`**。

### 回焦功能（Refocus）

当设置 `[piri.mark] refocus = true` 时：

- 插件会在你触发 mark toggle 时记住之前聚焦的窗口。
- 如果**再次触发相同标记**且该标记的窗口已聚焦，piri 会**跳回之前的窗口**，而不是重新聚焦已标记的窗口。
- 这非常适合在两个窗口之间快速切换（例如代码编辑器和终端）。
- 在空工作区（无聚焦窗口）也能正常工作。

## 与 Niri 快捷键配合

Piri 无法单独「监听下一个按键」；在 Niri 中一般为每个要用的标记配置一条 `spawn`，例如：

```kdl
binds {
    Mod+Shift+A { spawn "piri" "mark" "a" "toggle"; }
    Mod+Shift+B { spawn "piri" "mark" "b" "toggle"; }
}
```

若 Niri 将来支持多键序列或绑定模式，可把多条 `piri mark …` 收拢到同一前缀下，减少与全局快捷键的冲突。详见项目 README 中的相关说明。

## 限制与提示

- **非持久化**：重启 `piri daemon` 后所有标记丢失。
- **无窗口装饰**：Niri IPC 当前无法在窗口上绘制标记字母；若需可视化，可考虑状态栏、通知或外部脚本自行维护列表。
- **依赖焦点**：`toggle` / `add` 依赖「当前聚焦窗口」；调用前请确保目标窗口已聚焦。

## 使用场景

- 临时「书签」几个频繁切换的窗口（终端、浏览器、编辑器等）。
- 与脚本或菜单（如 `fuzzel` 列出 `a`–`z` 后调用 `piri mark …`）组合，减少手写标记名。
