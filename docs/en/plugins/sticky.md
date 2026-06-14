# Sticky Plugin

The Sticky plugin pins **one floating window** as a follower: when your focused workspace changes, that window is moved to the current focused workspace automatically.

It is useful for persistent utility windows (dictionary, translator, logs, media control, etc.).

## Configuration

Enable the plugin:

```toml
[piri.plugins]
sticky = true
```

Sticky state is runtime memory only and is not written to config files. It is cleared when the daemon restarts.

## Command Line

```bash
# Set currently focused window as sticky (follow within the same monitor only)
piri sticky add

# Set currently focused window as sticky (allow cross-monitor following)
piri sticky add --cross

# Clear sticky binding
piri sticky delete
```

## Behavior

1. `add` only accepts the **currently focused floating window**. If the focused window is not floating, the command fails.
2. Only one sticky window is tracked at a time. Running `add` again replaces the previous sticky target.
3. `--cross` controls monitor behavior:
   - Without `--cross`: follow only within the same monitor; no cross-monitor move.
   - With `--cross`: follow to the current focused monitor + workspace.
4. `delete` only removes the sticky binding; it does not close the window.

## Common Patterns

- Bind a key to focus a utility floating window, then run `piri sticky add`.
- Use `piri sticky add --cross` for a “carry-along” window across monitors.
- Run `piri sticky delete` when you no longer want follow behavior.
