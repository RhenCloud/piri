# Sleepy Plugin

The Sleepy plugin reports your currently focused app/window to a Sleepy server whenever focus changes. It can optionally also report MPRIS media playback status to a separate device.

References:
- cloud-pyprland sleepy plugin behavior (focus-based reporting)
- Sleepy official API (`/api/device/set`)

## Features

- Listens to window focus change events
- Resolves focused window information
- Sends status to Sleepy via HTTP POST
- Built-in deduplication: skips unchanged statuses
- Optional MPRIS media playback reporting (separate device)

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

### Media playback reporting (optional)

To report MPRIS playback status to a separate device, add these fields:

```toml
[sleepy]
media_process_name = "spotify|vlc|firefox"
media_device_id = "my-linux-media"
media_device_name = "Media Player"
media_poll_interval = 5
```

- `media_process_name`: regex matched against the MPRIS player process/bus name (e.g. `spotify`, `vlc`, `firefox`). When the plugin finds a matching player that is playing, it reports the track to the media device. Required to enable media reporting.
- `media_device_id`: device ID used for media status
- `media_device_name`: display name used for media status
- `media_poll_interval`: polling interval in seconds (default `5`)

Media status format is `♪title - artist - album`, following the official Sleepy Windows client. When nothing is playing, the plugin reports `using: false` with status `没有媒体播放`.

## Field Notes

- `server_url`: Sleepy server base URL (without trailing `/` is recommended)
- `device_id`: unique device ID
- `device_name`: display name shown in Sleepy
- `token`: optional, sent as Authorization Bearer token
- `secret`: optional, included as `secret` field in request JSON
- `prefer_app_id`: if `true`, app_id is preferred over title for status text

## Payload

The plugin sends requests to: `POST {server_url}/api/device/set`

Example payload (focus status):

```json
{
  "id": "my-linux",
  "show_name": "Niri Desktop",
  "using": true,
  "status": "VS Code",
  "secret": "optional-secret"
}
```

Example payload (media status):

```json
{
  "id": "my-linux-media",
  "show_name": "Media Player",
  "using": true,
  "status": "♪Song - Artist - Album"
}
```

`secret` is only included when configured and non-empty.

## Notes

- Network failures are logged as warnings
- If your server requires auth, configure `token` or `secret`
- Empty window title falls back to `app_id`, then `Unknown`
- Media reporting requires D-Bus with MPRIS support (any MPRIS-compatible player, e.g. Spotify, VLC, Firefox)
