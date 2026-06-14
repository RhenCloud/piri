use std::os::unix::io::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use log::debug;
use std::collections::HashMap;

use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_output, wl_region, wl_registry, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use zwlr_layer_shell_v1::ZwlrLayerShellV1;
use zwlr_layer_surface_v1::{Anchor, ZwlrLayerSurfaceV1};

use super::gradient::{GradientBuilder, GradientDirection, GradientStops, RgbColor};

#[derive(Debug, Clone)]
pub struct NativeRenderRequest {
    pub show_left: bool,
    pub show_right: bool,
    pub width: i32,
    pub height: i32,
    pub left_start: RgbColor,
    pub left_end: RgbColor,
    pub right_start: RgbColor,
    pub right_end: RgbColor,
    pub base_alpha: f64,
    pub target_output: Option<String>,
    // Animation fields
    pub animation_t: f64,         // Animation progress 0.0-1.0, 0 = no animation
    pub animation_style: String,  // "pulse" | "fade"
    pub animation_amplitude: f64, // 0.0-1.0
}

struct EdgeSurface {
    surface: ZwlrLayerSurfaceV1,
    wl_surface: wl_surface::WlSurface,
    buffer: Option<wl_buffer::WlBuffer>,
    _shm_pool: Option<wl_shm_pool::WlShmPool>,
    _fd: Option<OwnedFd>,
    data: Option<Arc<Mutex<Vec<u8>>>>,
    width: i32,
    height: i32,
    configured: bool,
}

pub struct NativeSurfaceRenderer {
    tx: Option<mpsc::Sender<NativeRenderRequest>>,
    notify_fd: Option<OwnedFd>,
    thread_error: Arc<Mutex<Option<String>>>,
}

struct AppState {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layer_shell: Option<ZwlrLayerShellV1>,
    outputs: HashMap<String, wl_output::WlOutput>,
    left: Option<EdgeSurface>,
    right: Option<EdgeSurface>,
    pending_left: Option<NativeRenderRequest>,
    pending_right: Option<NativeRenderRequest>,
    // Animation state
    animation_active: bool,
    animation_timerfd: Option<OwnedFd>,
    animation_start: Option<std::time::Instant>,
    animation_duration_ms: f64,
    animation_repeat_count: u32,
    animation_repeat_max: u32,
    animation_request: Option<NativeRenderRequest>,
}

impl Default for NativeSurfaceRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeSurfaceRenderer {
    pub fn new() -> Self {
        Self {
            tx: None,
            notify_fd: None,
            thread_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn render(&mut self, request: NativeRenderRequest) -> Result<()> {
        self.ensure_started();

        // Check if the renderer thread reported an error
        if let Some(err) = self.thread_error.lock().unwrap().as_ref() {
            self.tx = None;
            anyhow::bail!("Native Wayland renderer failed: {}", err);
        }

        let tx = self.tx.as_ref().context("Native renderer channel unavailable")?;
        tx.send(request)
            .map_err(|_| anyhow::anyhow!("Native Wayland renderer thread exited unexpectedly"))?;

        // Wake up the event loop via eventfd
        if let Some(fd) = self.notify_fd.as_ref() {
            let buf: [u8; 8] = 1u64.to_ne_bytes();
            unsafe {
                libc::write(fd.as_raw_fd(), buf.as_ptr() as *const _, 8);
            }
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        self.tx = None;
        if let Some(fd) = self.notify_fd.take() {
            let buf: [u8; 8] = 1u64.to_ne_bytes();
            unsafe {
                libc::write(fd.as_raw_fd(), buf.as_ptr() as *const _, 8);
            }
        }
    }

    fn ensure_started(&mut self) {
        if self.tx.is_some() {
            return;
        }

        // Check environment variables first
        if std::env::var("WAYLAND_DISPLAY").is_err() {
            log::warn!("WAYLAND_DISPLAY not set, native Wayland renderer unavailable");
            return;
        }

        *self.thread_error.lock().unwrap() = None;
        let error_sink = Arc::clone(&self.thread_error);

        // Create eventfd for waking up the event loop
        let eventfd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
        if eventfd < 0 {
            log::error!(
                "Failed to create eventfd: {}",
                std::io::Error::last_os_error()
            );
            return;
        }
        let read_fd = unsafe { libc::dup(eventfd) };
        if read_fd < 0 {
            unsafe { libc::close(eventfd) };
            log::error!("Failed to dup eventfd: {}", std::io::Error::last_os_error());
            return;
        }

        let (tx, rx) = mpsc::channel::<NativeRenderRequest>();
        let notify_fd = unsafe { OwnedFd::from_raw_fd(eventfd) };
        let eventfd_read = unsafe { OwnedFd::from_raw_fd(read_fd) };
        self.notify_fd = Some(notify_fd);

        // Use a channel to wait for initialization to complete
        let (init_tx, init_rx) = mpsc::channel::<Result<()>>();
        thread::spawn(move || {
            match init_wayland() {
                Ok((conn, mut event_queue, state)) => {
                    // Signal successful initialization
                    let _ = init_tx.send(Ok(()));
                    debug!("Native Wayland renderer started");
                    // Run the event loop with eventfd read end
                    if let Err(e) = event_loop(&conn, &mut event_queue, state, rx, eventfd_read) {
                        let msg = format!("{:#}", e);
                        log::error!("Native Wayland renderer error: {}", msg);
                        *error_sink.lock().unwrap() = Some(msg);
                    }
                }
                Err(e) => {
                    let msg = format!("{:#}", e);
                    log::error!("Native Wayland renderer init error: {}", msg);
                    *error_sink.lock().unwrap() = Some(msg);
                    let _ = init_tx.send(Err(e));
                }
            }
        });

        // Wait for initialization to complete
        match init_rx.recv() {
            Ok(Ok(())) => {
                self.tx = Some(tx);
            }
            Ok(Err(e)) => {
                log::error!("Native renderer init failed: {:#}", e);
                self.notify_fd = None;
            }
            Err(_) => {
                log::error!("Native renderer thread died during init");
                self.notify_fd = None;
            }
        }
    }
}

impl Drop for NativeSurfaceRenderer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Create a timerfd for animation frame timing.
fn create_timerfd() -> Result<OwnedFd> {
    let fd = unsafe { libc::timerfd_create(libc::CLOCK_MONOTONIC, libc::TFD_CLOEXEC) };
    if fd < 0 {
        anyhow::bail!("timerfd_create failed: {}", std::io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Set timerfd to fire periodically.
/// `interval_ms` = interval between firings (milliseconds).
/// `initial_ms` = first firing delay (milliseconds, defaults to interval if 0).
fn set_timerfd(timerfd: &OwnedFd, interval_ms: f64, initial_ms: f64) {
    let interval_sec = (interval_ms / 1000.0) as i64;
    let interval_nsec = ((interval_ms % 1000.0) * 1_000_000.0) as i64;
    let initial_sec = if initial_ms > 0.0 {
        (initial_ms / 1000.0) as i64
    } else {
        interval_sec
    };
    let initial_nsec = if initial_ms > 0.0 {
        ((initial_ms % 1000.0) * 1_000_000.0) as i64
    } else {
        interval_nsec
    };

    let spec = libc::itimerspec {
        it_interval: libc::timespec {
            tv_sec: interval_sec,
            tv_nsec: interval_nsec,
        },
        it_value: libc::timespec {
            tv_sec: initial_sec,
            tv_nsec: initial_nsec,
        },
    };
    unsafe {
        libc::timerfd_settime(timerfd.as_raw_fd(), 0, &spec, std::ptr::null_mut());
    }
}

/// Clear timerfd by reading the counter.
fn clear_timerfd(timerfd: &OwnedFd) {
    let mut buf = [0u8; 8];
    unsafe {
        libc::read(timerfd.as_raw_fd(), buf.as_mut_ptr() as *mut _, 8);
    }
}

/// Disable timerfd (stop firing).
fn disable_timerfd(timerfd: &OwnedFd) {
    let spec = libc::itimerspec {
        it_interval: libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        },
        it_value: libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        },
    };
    unsafe {
        libc::timerfd_settime(timerfd.as_raw_fd(), 0, &spec, std::ptr::null_mut());
    }
}

fn init_wayland() -> Result<(Connection, wayland_client::EventQueue<AppState>, AppState)> {
    let conn = Connection::connect_to_env().context("Failed to connect to Wayland display")?;

    // Set the Wayland socket to non-blocking so guard.read() returns EAGAIN
    // instead of blocking, allowing us to check the mpsc channel promptly.
    {
        let fd = conn.as_fd().as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags >= 0 {
            unsafe {
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }
    }

    let display = conn.display();

    let mut event_queue = conn.new_event_queue::<AppState>();
    let qh = event_queue.handle();

    let _registry = display.get_registry(&qh, ());

    let mut state = AppState {
        compositor: None,
        shm: None,
        layer_shell: None,
        outputs: HashMap::new(),
        left: None,
        right: None,
        pending_left: None,
        pending_right: None,
        animation_active: false,
        animation_timerfd: None,
        animation_start: None,
        animation_duration_ms: 600.0,
        animation_repeat_count: 0,
        animation_repeat_max: 3,
        animation_request: None,
    };

    // Discover globals and collect output names
    event_queue.roundtrip(&mut state)?;
    // Second roundtrip to receive wl_output Name events
    event_queue.roundtrip(&mut state)?;

    debug!("Discovered {} output(s)", state.outputs.len());

    // Verify required globals are available
    state.compositor.as_ref().context("wl_compositor not available")?;
    state.shm.as_ref().context("wl_shm not available")?;
    state
        .layer_shell
        .as_ref()
        .context("zwlr_layer_shell_v1 not available (is this a wlroots compositor?)")?;

    Ok((conn, event_queue, state))
}

fn event_loop(
    conn: &Connection,
    event_queue: &mut wayland_client::EventQueue<AppState>,
    mut state: AppState,
    rx: mpsc::Receiver<NativeRenderRequest>,
    notify_fd: OwnedFd,
) -> Result<()> {
    let qh = event_queue.handle();

    // Clone globals from the initialized state
    let compositor = state.compositor.as_ref().unwrap().clone();
    let shm = state.shm.as_ref().unwrap().clone();
    let layer_shell = state.layer_shell.as_ref().unwrap().clone();

    let wl_fd = conn.as_fd().as_raw_fd();
    let event_fd = notify_fd.as_raw_fd();

    debug!("Entering native Wayland event loop");

    loop {
        // Drain all pending render requests (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(req) => {
                    if let Err(e) =
                        apply_request(&mut state, &compositor, &shm, &layer_shell, &qh, req)
                    {
                        log::error!("Native render error: {:#}", e);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }

        // Flush outgoing Wayland messages
        if let Err(e) = event_queue.flush() {
            log::error!("Wayland flush error: {:#}", e);
            return Err(e.into());
        }

        // Dispatch any pending Wayland events
        let dispatched = event_queue.dispatch_pending(&mut state)?;

        // Build poll fds: wayland + eventfd + optionally timerfd
        let mut fds_vec = vec![
            libc::pollfd {
                fd: wl_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: event_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let timerfd_idx = if state.animation_active {
            if let Some(ref tfd) = state.animation_timerfd {
                fds_vec.push(libc::pollfd {
                    fd: tfd.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                });
                Some(2)
            } else {
                None
            }
        } else {
            None
        };

        // If no pending events, block until an fd is ready
        if dispatched == 0 {
            let ret = unsafe { libc::poll(fds_vec.as_mut_ptr(), fds_vec.len() as u64, -1) };
            if ret < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err.into());
            }

            // timerfd triggered: animation frame tick
            if let Some(idx) = timerfd_idx {
                if fds_vec[idx as usize].revents & libc::POLLIN != 0 {
                    clear_timerfd(state.animation_timerfd.as_ref().unwrap());
                    handle_animation_frame(&mut state, &compositor, &shm, &qh)?;
                    continue;
                }
            }

            // eventfd triggered: clear it and continue to drain render requests
            if fds_vec[1].revents & libc::POLLIN != 0 {
                let mut buf = [0u8; 8];
                unsafe {
                    libc::read(event_fd, buf.as_mut_ptr() as *mut _, 8);
                }
                continue;
            }

            // Wayland fd ready: read from socket
            if fds_vec[0].revents & libc::POLLIN != 0 {
                if let Some(guard) = conn.prepare_read() {
                    if guard.read().is_err() {
                        continue;
                    }
                }
            }
        }
    }
}

/// Handle a timerfd tick: advance animation and re-render.
fn handle_animation_frame(
    state: &mut AppState,
    _compositor: &wl_compositor::WlCompositor,
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<AppState>,
) -> Result<()> {
    let Some(req) = state.animation_request.clone() else {
        return Ok(());
    };

    let elapsed = state.animation_start.map(|t| t.elapsed().as_millis() as f64).unwrap_or(0.0);

    let t = (elapsed / state.animation_duration_ms).clamp(0.0, 1.0);

    // Re-render with current animation progress
    let mut animated_req = req.clone();
    animated_req.animation_t = t;

    // Render left side
    if animated_req.show_left {
        if let Some(ref edge) = state.left {
            if edge.configured {
                render_to_surface(state, shm, qh, &animated_req, true)?;
            }
        }
    }

    // Render right side
    if animated_req.show_right {
        if let Some(ref edge) = state.right {
            if edge.configured {
                render_to_surface(state, shm, qh, &animated_req, false)?;
            }
        }
    }

    // Check if animation cycle completed
    if t >= 1.0 {
        state.animation_repeat_count += 1;
        if state.animation_repeat_max > 0
            && state.animation_repeat_count >= state.animation_repeat_max
        {
            // Stop animation, keep final state (static indicator)
            if let Some(ref tfd) = state.animation_timerfd {
                disable_timerfd(tfd);
            }
            state.animation_active = false;
            state.animation_start = None;
        } else {
            // Reset for next cycle
            state.animation_start = Some(std::time::Instant::now());
            if let Some(ref tfd) = state.animation_timerfd {
                set_timerfd(tfd, 16.667, 16.667); // ~60 FPS
            }
        }
    }

    Ok(())
}

fn apply_request(
    state: &mut AppState,
    compositor: &wl_compositor::WlCompositor,
    shm: &wl_shm::WlShm,
    layer_shell: &ZwlrLayerShellV1,
    qh: &QueueHandle<AppState>,
    req: NativeRenderRequest,
) -> Result<()> {
    let w = req.width.max(1);
    let h = req.height.max(1);

    // Handle animation request: update state and (re)start if needed
    if req.animation_t >= 0.0 {
        // Create timerfd if needed
        if state.animation_timerfd.is_none() {
            match create_timerfd() {
                Ok(tfd) => state.animation_timerfd = Some(tfd),
                Err(e) => log::error!("Failed to create animation timerfd: {}", e),
            }
        }

        if let Some(ref tfd) = state.animation_timerfd {
            // Always update the request (may include new sides like right when left was active)
            state.animation_request = Some(req.clone());

            // (Re)start animation if not active, or reset if already running
            if !state.animation_active {
                state.animation_active = true;
                state.animation_start = Some(std::time::Instant::now());
                state.animation_repeat_count = 0;
                set_timerfd(tfd, 16.667, 16.667);
            } else {
                // Reset animation to start for the new request
                state.animation_start = Some(std::time::Instant::now());
                state.animation_repeat_count = 0;
                set_timerfd(tfd, 16.667, 16.667);
            }
        }
    } else if req.animation_t < 0.0 {
        // Animation disabled for this request, stop any active animation
        if state.animation_active {
            if let Some(ref tfd) = state.animation_timerfd {
                disable_timerfd(tfd);
            }
            state.animation_active = false;
            state.animation_start = None;
        }
    }

    debug!(
        "apply_request: left={}, right={}, {}x{}, output={:?}, known_outputs={}, animation_t={}",
        req.show_left,
        req.show_right,
        w,
        h,
        req.target_output,
        state.outputs.keys().len(),
        req.animation_t
    );

    // Left edge
    if req.show_left {
        let needs_recreate =
            state.left.as_ref().map(|s| s.width != w || s.height != h).unwrap_or(true);
        if needs_recreate {
            let output =
                req.target_output.as_deref().and_then(|name| state.outputs.get(name).cloned());
            debug!(
                "Left edge: target={:?}, resolved={:?}",
                req.target_output,
                output.as_ref().map(|_| "found")
            );
            state.left = Some(create_edge_surface(
                compositor,
                layer_shell,
                qh,
                w,
                h,
                true,
                output,
            )?);
        }
        if let Some(ref edge) = state.left {
            if edge.configured {
                render_to_surface(state, shm, qh, &req, true)?;
            } else {
                // Surface not yet configured — queue render for after configure
                debug!("Deferring left render until initial configure");
                state.pending_left = Some(req.clone());
                // Trigger configure by committing the empty surface
                edge.wl_surface.commit();
            }
        }
    } else {
        hide_surface(&mut state.left);
    }

    // Right edge
    if req.show_right {
        let needs_recreate =
            state.right.as_ref().map(|s| s.width != w || s.height != h).unwrap_or(true);
        if needs_recreate {
            let output =
                req.target_output.as_deref().and_then(|name| state.outputs.get(name).cloned());
            state.right = Some(create_edge_surface(
                compositor,
                layer_shell,
                qh,
                w,
                h,
                false,
                output,
            )?);
        }
        if let Some(ref edge) = state.right {
            if edge.configured {
                render_to_surface(state, shm, qh, &req, false)?;
            } else {
                debug!("Deferring right render until initial configure");
                state.pending_right = Some(req);
                edge.wl_surface.commit();
            }
        }
    } else {
        hide_surface(&mut state.right);
    }

    Ok(())
}

fn create_edge_surface(
    compositor: &wl_compositor::WlCompositor,
    layer_shell: &ZwlrLayerShellV1,
    qh: &QueueHandle<AppState>,
    width: i32,
    height: i32,
    is_left: bool,
    output: Option<wl_output::WlOutput>,
) -> Result<EdgeSurface> {
    let wl_surface = compositor.create_surface(qh, ());

    let namespace = if is_left {
        "piri-render-left"
    } else {
        "piri-render-right"
    };

    let output_ref = output.as_ref();
    let layer_surface = layer_shell.get_layer_surface(
        &wl_surface,
        output_ref,
        zwlr_layer_shell_v1::Layer::Overlay,
        namespace.to_string(),
        qh,
        (),
    );

    let anchor = if is_left { Anchor::Left } else { Anchor::Right };
    layer_surface.set_anchor(anchor);
    layer_surface.set_exclusive_zone(0);
    layer_surface.set_size(width as u32, height as u32);
    layer_surface.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);

    // Create empty input region for mouse passthrough
    let region = compositor.create_region(qh, ());
    wl_surface.set_input_region(Some(&region));
    wl_surface.commit();
    region.destroy();

    debug!(
        "Created native edge surface: side={}, {}x{}",
        if is_left { "left" } else { "right" },
        width,
        height
    );

    Ok(EdgeSurface {
        surface: layer_surface,
        wl_surface,
        buffer: None,
        _shm_pool: None,
        _fd: None,
        data: None,
        width,
        height,
        configured: false,
    })
}

fn render_to_surface(
    state: &mut AppState,
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<AppState>,
    req: &NativeRenderRequest,
    is_left: bool,
) -> Result<()> {
    let w = req.width.max(1);
    let h = req.height.max(1);

    let edge = if is_left {
        state.left.as_mut().unwrap()
    } else {
        state.right.as_mut().unwrap()
    };

    // Create buffer on first render
    if edge.buffer.is_none() {
        let stride = w * 4;
        let size = stride * h;
        let fd = create_shm_file(size)?;
        let data = Arc::new(Mutex::new(vec![0u8; size as usize]));

        let fd_borrowed = unsafe { std::os::unix::io::BorrowedFd::borrow_raw(fd.as_raw_fd()) };
        let shm_pool = shm.create_pool(fd_borrowed, size, qh, ());
        let buffer = shm_pool.create_buffer(0, w, h, stride, wl_shm::Format::Argb8888, qh, ());

        edge.buffer = Some(buffer);
        edge._shm_pool = Some(shm_pool);
        edge._fd = Some(fd);
        edge.data = Some(data);
    }

    // Render gradient into pixel buffer
    {
        let data_arc = edge.data.as_ref().unwrap();
        let mut buf = data_arc.lock().unwrap();
        if is_left {
            render_gradient(
                &mut buf,
                w,
                h,
                req.left_start,
                req.left_end,
                req.base_alpha,
                true,
                req.animation_t,
                &req.animation_style,
                req.animation_amplitude,
            );
        } else {
            render_gradient(
                &mut buf,
                w,
                h,
                req.right_start,
                req.right_end,
                req.base_alpha,
                false,
                req.animation_t,
                &req.animation_style,
                req.animation_amplitude,
            );
        }
        write_buffer(edge._fd.as_ref().unwrap(), &buf);
    }

    edge.wl_surface.attach(Some(edge.buffer.as_ref().unwrap()), 0, 0);
    edge.wl_surface.damage(0, 0, w, h);
    edge.wl_surface.commit();

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_gradient(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    start: RgbColor,
    end: RgbColor,
    base_alpha: f64,
    is_left: bool,
    t: f64,
    style: &str,
    amplitude: f64,
) {
    // Clear all pixels to transparent
    pixels.fill(0);

    // Apply animation modulation to base_alpha
    let anim_alpha = if t > 0.0 {
        match style {
            "pulse" => {
                // Pulse: alpha oscillates around base_alpha using sine wave
                let modulation = (t * 2.0 * std::f64::consts::PI).sin();
                (base_alpha * (1.0 - amplitude * 0.5 + amplitude * 0.5 * modulation))
                    .clamp(0.0, 1.0)
            }
            "fade" => {
                // Fade: fade in during first 1/3 of animation, then stay at base_alpha
                if t < 1.0 / 3.0 {
                    base_alpha * (t * 3.0).min(1.0)
                } else {
                    base_alpha
                }
            }
            _ => base_alpha,
        }
    } else {
        base_alpha
    };

    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
        .expect("Failed to create cairo surface");

    {
        let cr = cairo::Context::new(&surface).expect("Failed to create cairo context");

        // Clear to fully transparent
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.paint().ok();

        cr.set_operator(cairo::Operator::Over);

        let total_width = (width as f64).max(1.0);

        let white = RgbColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        };

        // Vertical color gradient (top→bottom)
        let color_stops = GradientStops::new().add_stop(0.0, start, 1.0).add_stop(1.0, end, 1.0);
        let color_gradient =
            GradientBuilder::new(GradientDirection::Vertical, total_width, height as f64)
                .build(&color_stops);

        // Horizontal alpha: edge → center, guaranteed 0 at width boundary
        let core_max = anim_alpha * 0.72;
        let core_mid = anim_alpha * 0.28;

        let h_stops = if is_left {
            GradientStops::new()
                .add_stop(0.0, white, core_max)
                .add_stop(0.5, white, core_mid)
                .add_stop(0.8, white, core_mid * 0.3)
                .add_stop(1.0, white, 0.0)
        } else {
            GradientStops::new()
                .add_stop(0.0, white, 0.0)
                .add_stop(0.2, white, core_mid * 0.3)
                .add_stop(0.5, white, core_mid)
                .add_stop(1.0, white, core_max)
        };
        let h_grad =
            GradientBuilder::new(GradientDirection::Horizontal, total_width, height as f64)
                .build(&h_stops);

        // Vertical alpha: center → top/bottom (fast fade at edges)
        let v_stops = GradientStops::new()
            .add_stop(0.0, white, 0.0)
            .add_stop(0.2, white, 0.6)
            .add_stop(0.8, white, 0.6)
            .add_stop(1.0, white, 0.0);
        let v_grad = GradientBuilder::new(GradientDirection::Vertical, total_width, height as f64)
            .build(&v_stops);

        // Intermediate surface: color × horizontal alpha
        let tmp = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
            .expect("Failed to create tmp surface");
        {
            let tmp_cr = cairo::Context::new(&tmp).expect("Failed to create tmp context");
            tmp_cr.set_operator(cairo::Operator::Source);
            tmp_cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            tmp_cr.paint().ok();
            tmp_cr.set_operator(cairo::Operator::Over);
            tmp_cr.rectangle(0.0, 0.0, total_width, height as f64);
            tmp_cr.set_source(&color_gradient).ok();
            tmp_cr.mask(&h_grad).ok();
        }
        tmp.flush();

        // Composite onto main: apply vertical alpha via mask
        cr.rectangle(0.0, 0.0, total_width, height as f64);
        cr.set_source_surface(&tmp, 0.0, 0.0).ok();
        cr.mask(&v_grad).ok();
    }

    surface.flush();

    // Copy pixel data from cairo surface to the output buffer
    let src_stride = surface.stride() as usize;
    let dst_stride = (width as usize) * 4;
    let src_data = surface.data().expect("Failed to access cairo surface data");
    for y in 0..height as usize {
        let src_row = &src_data[y * src_stride..y * src_stride + dst_stride];
        let dst_row = &mut pixels[y * dst_stride..y * dst_stride + dst_stride];
        dst_row.copy_from_slice(src_row);
    }
}

fn write_buffer(fd: &OwnedFd, data: &[u8]) {
    let raw_fd = fd.as_raw_fd();
    unsafe {
        libc::lseek(raw_fd, 0, libc::SEEK_SET);
        let mut offset = 0;
        while offset < data.len() {
            let written = libc::write(
                raw_fd,
                data[offset..].as_ptr() as *const libc::c_void,
                data.len() - offset,
            );
            if written <= 0 {
                break;
            }
            offset += written as usize;
        }
    }
}

fn hide_surface(edge: &mut Option<EdgeSurface>) {
    if let Some(ref e) = *edge {
        if e.configured {
            e.wl_surface.attach(None, 0, 0);
            e.wl_surface.commit();
        }
    }
    *edge = None;
}

fn create_shm_file(size: i32) -> Result<OwnedFd> {
    let fd = unsafe { libc::memfd_create(c"piri-shm".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        anyhow::bail!(
            "Failed to create memfd: {}",
            std::io::Error::last_os_error()
        );
    }
    if unsafe { libc::ftruncate(fd, size as libc::off_t) } < 0 {
        unsafe {
            libc::close(fd);
        }
        anyhow::bail!(
            "Failed to truncate memfd: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

// ============ Wayland Dispatch Impls ============

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "wl_output" => {
                    let _output: wl_output::WlOutput = registry.bind(name, version.min(4), qh, ());
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_output::WlOutput, ()> for AppState {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            state.outputs.insert(name, output.clone());
        }
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, .. } = event {
            surface.ack_configure(serial);

            let shm = state.shm.as_ref().unwrap().clone();

            // Check left surface
            if let Some(ref left) = state.left {
                if &left.surface == surface {
                    if !left.configured {
                        // Mark configured
                        state.left.as_mut().unwrap().configured = true;
                        // Process pending render if any
                        if let Some(req) = state.pending_left.take() {
                            if let Err(e) = render_to_surface(state, &shm, qh, &req, true) {
                                log::error!("Deferred left render failed: {:#}", e);
                            }
                        }
                    } else if state.left.as_ref().unwrap().buffer.is_some() {
                        // Re-attach on resize configure
                        let left = state.left.as_ref().unwrap();
                        left.wl_surface.attach(Some(left.buffer.as_ref().unwrap()), 0, 0);
                        left.wl_surface.commit();
                    }
                    return;
                }
            }

            // Check right surface
            if let Some(ref right) = state.right {
                if &right.surface == surface {
                    if !right.configured {
                        state.right.as_mut().unwrap().configured = true;
                        if let Some(req) = state.pending_right.take() {
                            if let Err(e) = render_to_surface(state, &shm, qh, &req, false) {
                                log::error!("Deferred right render failed: {:#}", e);
                            }
                        }
                    } else if state.right.as_ref().unwrap().buffer.is_some() {
                        let right = state.right.as_ref().unwrap();
                        right.wl_surface.attach(Some(right.buffer.as_ref().unwrap()), 0, 0);
                        right.wl_surface.commit();
                    }
                }
            }
        }
    }
}

impl Dispatch<wl_region::WlRegion, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
