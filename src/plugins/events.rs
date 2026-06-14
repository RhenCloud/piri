use std::collections::{HashMap, HashSet};

use niri_ipc::Event;

/// Normalized piri event — fine-grained sub-events split from coarse niri events.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PiriEvent {
    // --- WindowOpenedOrChanged 拆分出的子事件 ---
    /// 新窗口首次出现
    WindowOpened {
        window: niri_ipc::Window,
    },
    /// 已有窗口属性变化（title、app_id、layout 等），但浮动/平铺状态未变
    WindowChanged {
        window: niri_ipc::Window,
    },
    /// 窗口在浮动和平铺之间切换
    WindowToggleFloating {
        window: niri_ipc::Window,
    },

    // --- 其他 niri 事件的透传 ---
    WorkspacesChanged {
        workspaces: Vec<niri_ipc::Workspace>,
    },
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    WorkspaceActiveWindowChanged {
        workspace_id: u64,
        active_window_id: Option<u64>,
    },
    WindowsChanged {
        windows: Vec<niri_ipc::Window>,
    },
    WindowClosed {
        id: u64,
    },
    WindowFocusChanged {
        id: Option<u64>,
    },
    WindowLayoutsChanged {
        changes: Vec<(u64, niri_ipc::WindowLayout)>,
    },
    WindowFocusTimestampChanged {
        id: u64,
        focus_timestamp: Option<niri_ipc::Timestamp>,
    },
    WindowUrgencyChanged {
        id: u64,
        urgent: bool,
    },
    KeyboardLayoutsChanged {
        keyboard_layouts: niri_ipc::KeyboardLayouts,
    },
    KeyboardLayoutSwitched {
        idx: u8,
    },
    ConfigLoaded {
        failed: bool,
    },
}

/// Tracks previous window state to classify `WindowOpenedOrChanged` into sub-events.
#[derive(Default)]
pub struct EventNormalizer {
    seen_windows: HashSet<u64>,
    window_floating_state: HashMap<u64, bool>,
}

impl EventNormalizer {
    pub fn new() -> Self {
        Self {
            seen_windows: HashSet::new(),
            window_floating_state: HashMap::new(),
        }
    }

    /// Convert a raw niri event into zero or more `PiriEvent`s.
    pub fn normalize_event(&mut self, event: &Event) -> Vec<PiriEvent> {
        match event {
            Event::WindowOpenedOrChanged { window } => {
                vec![self.classify_window_event(window)]
            }
            Event::WindowClosed { id } => {
                self.on_window_closed(*id);
                vec![PiriEvent::WindowClosed { id: *id }]
            }
            Event::WindowsChanged { windows } => {
                self.rebuild_from_niri_windows(windows);
                vec![PiriEvent::WindowsChanged {
                    windows: windows.clone(),
                }]
            }
            Event::WorkspacesChanged { workspaces } => vec![PiriEvent::WorkspacesChanged {
                workspaces: workspaces.clone(),
            }],
            Event::WorkspaceActivated { id, focused } => vec![PiriEvent::WorkspaceActivated {
                id: *id,
                focused: *focused,
            }],
            Event::WorkspaceActiveWindowChanged {
                workspace_id,
                active_window_id,
            } => vec![PiriEvent::WorkspaceActiveWindowChanged {
                workspace_id: *workspace_id,
                active_window_id: *active_window_id,
            }],
            Event::WindowFocusChanged { id } => vec![PiriEvent::WindowFocusChanged { id: *id }],
            Event::WindowLayoutsChanged { changes } => vec![PiriEvent::WindowLayoutsChanged {
                changes: changes.clone(),
            }],
            Event::WindowFocusTimestampChanged {
                id,
                focus_timestamp,
            } => vec![PiriEvent::WindowFocusTimestampChanged {
                id: *id,
                focus_timestamp: *focus_timestamp,
            }],
            Event::WindowUrgencyChanged { id, urgent } => vec![PiriEvent::WindowUrgencyChanged {
                id: *id,
                urgent: *urgent,
            }],
            Event::KeyboardLayoutsChanged { keyboard_layouts } => {
                vec![PiriEvent::KeyboardLayoutsChanged {
                    keyboard_layouts: keyboard_layouts.clone(),
                }]
            }
            Event::KeyboardLayoutSwitched { idx } => {
                vec![PiriEvent::KeyboardLayoutSwitched { idx: *idx }]
            }
            Event::ConfigLoaded { failed } => vec![PiriEvent::ConfigLoaded { failed: *failed }],
            // Events not used by any plugin — skip
            Event::WorkspaceUrgencyChanged { .. }
            | Event::OverviewOpenedOrClosed { .. }
            | Event::ScreenshotCaptured { .. }
            | Event::CastsChanged { .. }
            | Event::CastStartedOrChanged { .. }
            | Event::CastStopped { .. } => vec![],
        }
    }

    fn classify_window_event(&mut self, window: &niri_ipc::Window) -> PiriEvent {
        if !self.seen_windows.contains(&window.id) {
            // 新窗口
            self.seen_windows.insert(window.id);
            self.window_floating_state.insert(window.id, window.is_floating);
            return PiriEvent::WindowOpened {
                window: window.clone(),
            };
        }

        let prev_floating = self.window_floating_state.get(&window.id).copied().unwrap_or(false);

        if prev_floating != window.is_floating {
            // 浮动/平铺状态切换
            self.window_floating_state.insert(window.id, window.is_floating);
            return PiriEvent::WindowToggleFloating {
                window: window.clone(),
            };
        }

        // 其他属性变化
        PiriEvent::WindowChanged {
            window: window.clone(),
        }
    }

    fn on_window_closed(&mut self, id: u64) {
        self.seen_windows.remove(&id);
        self.window_floating_state.remove(&id);
    }

    fn rebuild_from_niri_windows(&mut self, windows: &[niri_ipc::Window]) {
        self.seen_windows.clear();
        self.window_floating_state.clear();
        for w in windows {
            self.seen_windows.insert(w.id);
            self.window_floating_state.insert(w.id, w.is_floating);
        }
    }

    pub fn rebuild_from_piri_windows(&mut self, windows: &[crate::niri::Window]) {
        self.seen_windows.clear();
        self.window_floating_state.clear();
        for w in windows {
            self.seen_windows.insert(w.id);
            self.window_floating_state.insert(w.id, w.floating);
        }
    }
}
