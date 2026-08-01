# Sleepy 插件

Sleepy 插件会在窗口焦点变化时，将当前应用状态上报到 Sleepy 服务端。可选地，它还可以将 MPRIS 媒体播放状态上报到独立的设备。

参考来源：
- cloud-pyprland 的 sleepy 插件思路（按焦点窗口上报）
- Sleepy 官方 API（`/api/device/set`）

## 功能

- 监听窗口焦点变化事件
- 获取当前焦点窗口信息
- 将状态通过 HTTP POST 上报到 Sleepy
- 自动去重：状态未变化时不重复上报
- 可选：上报 MPRIS 媒体播放状态（使用独立设备）

## 配置

先启用插件：

```toml
[piri.plugins]
sleepy = true
```

然后配置 sleepy：

```toml
[sleepy]
server_url = "https://sleepy.example.com"
device_id = "my-linux"
device_name = "Niri Desktop"
token = ""      # 可选，Bearer Token 鉴权
secret = ""     # 可选，某些部署要求 body 内包含 secret
prefer_app_id = false
```

### 媒体播放状态上报（可选）

如需将 MPRIS 播放状态上报到独立的设备，请添加以下字段：

```toml
[sleepy]
media_process_name = "spotify|vlc|firefox"
media_device_id = "my-linux-media"
media_device_name = "Media Player"
media_poll_interval = 5
```

- `media_process_name`：正则表达式，匹配 MPRIS 播放器的进程/总线名（如 `spotify`、`vlc`、`firefox`）。当插件发现匹配的播放器正在播放时，会向媒体设备上报曲目信息。该字段为启用媒体上报的必要条件。
- `media_device_id`：媒体状态使用的设备 ID
- `media_device_name`：媒体状态使用的设备显示名称
- `media_poll_interval`：轮询播放状态的间隔（秒），默认 `5`

媒体状态格式为 `♪title - artist - album`，与 Sleepy 官方 Windows 客户端一致。未播放时，插件会以 `using: false`、状态 `没有媒体播放` 上报。

## 字段说明

- `server_url`: Sleepy 服务端地址（不带尾部 `/` 更推荐）
- `device_id`: 设备唯一 ID
- `device_name`: 设备显示名称
- `token`: 可选，作为 Authorization Bearer Token 发送
- `secret`: 可选，写入请求体 `secret` 字段
- `prefer_app_id`: 为 `true` 时优先上报 `app_id`，否则优先上报窗口标题

## 上报格式

插件会发送到：`POST {server_url}/api/device/set`

焦点状态请求体示例：

```json
{
  "id": "my-linux",
  "show_name": "Niri Desktop",
  "using": true,
  "status": "VS Code",
  "secret": "optional-secret"
}
```

媒体状态请求体示例：

```json
{
  "id": "my-linux-media",
  "show_name": "Media Player",
  "using": true,
  "status": "♪Song - Artist - Album"
}
```

其中 `secret` 仅在配置非空时发送。

## 注意事项

- 该插件依赖网络连通性，网络异常会记录告警日志
- 若你的服务端启用了鉴权，请确保配置 `token` 或 `secret`
- 如果窗口标题为空，会回退到 `app_id`，再回退到 `Unknown`
- 媒体上报依赖带 MPRIS 支持的 D-Bus 环境（任意兼容 MPRIS 的播放器，如 Spotify、VLC、Firefox）
