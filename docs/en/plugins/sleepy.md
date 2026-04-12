# Sleepy Plugin

The Sleepy plugin reports your currently focused app/window to a Sleepy server whenever focus changes.

References:
- cloud-pyprland sleepy plugin behavior (focus-based reporting)
- Sleepy official API (`/api/device/set`)

## Features

- Listens to window focus change events
- Resolves focused window information
- Sends status to Sleepy via HTTP POST
- Built-in deduplication: skips unchanged statuses

## Configuration

Enable the plugin first:

```toml
[piri.plugins]
sleepy = true
```

Then configure sleepy:

```toml
[sleepy]
server_url = "https://sleepy.example.com"
device_id = "my-linux"
device_name = "Niri Desktop"
token = ""      # optional, Bearer token auth
secret = ""     # optional, some deployments require secret in request body
prefer_app_id = false
```

## Field Notes

- `server_url`: Sleepy server base URL (without trailing `/` is recommended)
- `device_id`: unique device ID
- `device_name`: display name shown in Sleepy
- `token`: optional, sent as Authorization Bearer token
- `secret`: optional, included as `secret` field in request JSON
- `prefer_app_id`: if `true`, app_id is preferred over title for status text

## Payload

The plugin sends requests to: `POST {server_url}/api/device/set`

Example payload:

```json
{
  "id": "my-linux",
  "show_name": "Niri Desktop",
  "using": true,
  "status": "VS Code",
  "secret": "optional-secret"
}
```

`secret` is only included when configured and non-empty.

## Notes

- Network failures are logged as warnings
- If your server requires auth, configure `token` or `secret`
- Empty window title falls back to `app_id`, then `Unknown`
