# Fine-grained Event Splitting

Piri automatically splits Niri's coarse-grained events into fine-grained sub-events at the event distribution layer, so plugins receive semantically specific events without tracking window state themselves.

## Difference from Niri Native

Niri's `WindowOpenedOrChanged` event is coarse-grained — the same event covers new window opening, floating/tiled state toggling, and window property changes (title, app_id, etc.). Each plugin that needs to distinguish these cases must maintain its own `seen_windows`, `window_floating_state` and other state to determine what happened.

Piri's approach: automatically split it into three independent sub-events at the event distribution layer, so plugins don't need to track state themselves.

## PiriEvent Enum

Piri defines its own event enum `PiriEvent`, splitting Niri's raw events into fine-grained sub-events:

### Sub-events (split from WindowOpenedOrChanged)

| Event | Description |
|-------|-------------|
| `WindowOpened` | New window appears for the first time |
| `WindowChanged` | Existing window property changes (title, app_id, layout, etc.), but floating/tiled state unchanged |
| `WindowToggleFloating` | Window toggles between floating and tiled |

### Passthrough Events

The following events are passed through from Niri with unchanged semantics:

| Event | Description |
|-------|-------------|
| `WindowClosed` | Window closed |
| `WindowFocusChanged` | Window focus changed |
| `WindowLayoutsChanged` | Window layout changed (resize, etc.) |
| `WindowFocusTimestampChanged` | Window focus timestamp changed |
| `WindowUrgencyChanged` | Window urgency state changed |
| `WindowsChanged` | Full window list update |
| `WorkspaceActivated` | Workspace activated |
| `WorkspaceActiveWindowChanged` | Workspace active window changed |
| `WorkspacesChanged` | Workspace configuration changed |
| `KeyboardLayoutsChanged` | Keyboard layout configuration changed |
| `KeyboardLayoutSwitched` | Keyboard layout switched |
| `ConfigLoaded` | Configuration loaded |

## Classification Logic

`EventNormalizer` internally maintains window state tracking:

```
When WindowOpenedOrChanged { window } arrives:

1. window.id is NOT in the known window list
   → emit WindowOpened
   → record window.id and is_floating state

2. window.id IS in the known window list, and is_floating differs from recorded value
   → emit WindowToggleFloating
   → update is_floating record

3. Other cases
   → emit WindowChanged
```

State cleanup:
- `WindowClosed` removes the window record
- `WindowsChanged` rebuilds state from the full window list
- On startup, seeds from the initial window list to avoid triggering `WindowOpened` for existing windows

## Impact on Plugins

All plugins now receive `PiriEvent` instead of `niri_ipc::Event`. Plugins only need to declare the sub-events they care about in `is_interested_in_event`, without tracking window state themselves.

**Before migration** (had to track state manually):
```rust
// Plugin had to maintain seen_windows, window_floating_state, etc.
fn handle_event(&mut self, event: &niri_ipc::Event) {
    match event {
        Event::WindowOpenedOrChanged { window } => {
            let is_new = !self.seen_windows.contains(&window.id);
            let floating_changed = ...; // Had to compare manually
            // Complex classification logic
        }
    }
}
```

**After migration** (events already normalized):
```rust
fn handle_event(&mut self, event: &PiriEvent) {
    match event {
        PiriEvent::WindowOpened { window } => { /* new window */ }
        PiriEvent::WindowToggleFloating { window } => { /* floating toggle */ }
        PiriEvent::WindowChanged { window } => { /* property change */ }
        _ => {}
    }
}
```
