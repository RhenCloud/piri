# Sleepy 插件

Sleepy 插件会在窗口焦点变化时，将当前应用状态上报到 Sleepy 服务端。

参考来源：
- cloud-pyprland 的 sleepy 插件思路（按焦点窗口上报）
- Sleepy 官方 API（`/api/device/set`）

## 功能

- 监听窗口焦点变化事件
- 获取当前焦点窗口信息
- 将状态通过 HTTP POST 上报到 Sleepy
- 自动去重：状态未变化时不重复上报

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

## 字段说明

- `server_url`: Sleepy 服务端地址（不带尾部 `/` 更推荐）
- `device_id`: 设备唯一 ID
- `device_name`: 设备显示名称
- `token`: 可选，作为 Authorization Bearer Token 发送
- `secret`: 可选，写入请求体 `secret` 字段
- `prefer_app_id`: 为 `true` 时优先上报 `app_id`，否则优先上报窗口标题

## 上报格式

插件会发送到：`POST {server_url}/api/device/set`

请求体示例：

```json
{
  "id": "my-linux",
  "show_name": "Niri Desktop",
  "using": true,
  "status": "VS Code",
  "secret": "optional-secret"
}
```

其中 `secret` 仅在配置非空时发送。

## 注意事项

- 该插件依赖网络连通性，网络异常会记录告警日志
- 若你的服务端启用了鉴权，请确保配置 `token` 或 `secret`
- 如果窗口标题为空，会回退到 `app_id`，再回退到 `Unknown`
