# 开发指南

## 扩展性

### 添加新插件

1. 在 `src/plugins/` 创建插件文件（如 `myplugin.rs`）
2. 实现 `Plugin` trait：
    ```rust
    use async_trait::async_trait;
    use crate::plugins::Plugin;
    use crate::config::Config;
    use crate::niri::NiriIpc;
    use crate::ipc::IpcRequest;
    use niri_ipc::Event;
    use anyhow::Result;

    pub struct MyPlugin {
        niri: NiriIpc,
        // 插件状态
    }

    impl MyPlugin {
        pub fn new() -> Self {
            Self { niri: NiriIpc::new(None) }
        }
    }

    #[async_trait]
    impl Plugin for MyPlugin {
        fn name(&self) -> &str { "myplugin" }

        async fn init(&mut self, niri: NiriIpc, _config: &Config) -> Result<()> {
            self.niri = niri;
            Ok(())
        }

        // 处理 IPC 请求（可选）
        async fn handle_ipc_request(&mut self, _request: &IpcRequest) -> Result<Option<Result<()>>> {
            Ok(None)
        }

        // 处理 niri 事件（可选，事件驱动插件）
        async fn handle_event(&mut self, event: &Event, _niri: &NiriIpc) -> Result<()> {
            match event {
                Event::WindowOpenedOrChanged { .. } => { /* 处理 */ }
                _ => {}
            }
            Ok(())
        }

        // 声明感兴趣的事件类型
        fn is_interested_in_event(&self, event: &Event) -> bool {
            matches!(event, Event::WindowOpenedOrChanged { .. })
        }

        // 更新配置（可选，热重载）
        async fn update_config(&mut self, _niri: NiriIpc, _config: &Config) -> Result<()> {
            Ok(())
        }

        // 关闭插件（可选）
        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }
    ```
3. 在 `src/plugins/mod.rs` 注册：
    - 添加 `pub mod myplugin;`
    - 在 `PluginManager::init` 中初始化
4. 在 `src/config.rs` 添加配置结构：
    - `PluginsConfig` 中添加启用/禁用选项
5. 在 `src/ipc.rs` 添加 IPC 请求类型（如需要）
6. 在 `src/main.rs` 添加 CLI 命令（如需要）
7. 更新 `config.example.toml`

#### 事件驱动插件

实现 `handle_event` 方法即可，**无需自建事件循环**：

- `PluginManager` 统一监听事件
- 通过 `handle_event` 分发到各插件
- 插件只需关注感兴趣的事件类型

简化开发，确保资源高效利用。

### 添加子命令

1. 在 `src/main.rs` 的 `Commands` 枚举添加命令
2. 在 `async_main` 添加处理逻辑
3. 如需与 daemon 通信：
   - 在 `src/ipc.rs` 的 `IpcRequest` 添加请求类型
    - 或通过插件系统处理
4. 如需直接访问 niri：创建 `NiriIpc` 实例并调用 IPC 方法

### 添加配置项

1. 在 `src/config.rs` 的 `Config` 结构体添加字段
2. 更新 `config.example.toml`

## 代码格式化

使用 `rustfmt`，配置文件 `rustfmt.toml`。

### 安装
```bash
rustup component add rustfmt
```

### 格式化
```bash
cargo fmt              # 格式化所有代码
cargo fmt -- --check  # 检查格式（不修改）
```

## 依赖

- `clap`: 命令行参数解析
- `serde` / `toml`: 配置序列化/反序列化
- `tokio`: 异步运行时
- `anyhow`: 错误处理
- `log` / `env_logger`: 日志系统
- `niri-ipc`: Niri IPC 客户端库
