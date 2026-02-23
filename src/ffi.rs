/// C ABI bridge for platform shells (macOS Swift, Linux GTK).
/// This is the public API that native UIs consume.

use crate::core::{Terminal, VtParser};
use crate::pty::PtyManager;
use std::ffi::{c_char, c_int, c_uint, CStr};
use std::ptr;

/// Opaque handle to a terminal session.
pub struct TermSession {
    terminal: Terminal,
    parser: VtParser,
    pty: Option<PtyManager>,
    renderer: Option<GpuRenderer>,
    config: crate::config::Config,
    watcher: crate::watcher::ConfigWatcher,
    config_generation: u64,
}

/// GPU renderer state, initialized lazily when a Metal layer is provided.
struct GpuRenderer {
    render_state: crate::renderer::pipeline::RenderState,
    atlas: crate::renderer::atlas::GlyphAtlas,
}

#[no_mangle]
pub extern "C" fn term_session_new(cols: c_uint, rows: c_uint) -> *mut TermSession {
    let config = crate::config::Config::load();
    let theme = crate::theme::Theme::by_name(&config.colors.theme)
        .unwrap_or_else(crate::theme::Theme::default_dark);
    let mut terminal = Terminal::new(cols as usize, rows as usize);
    // Apply theme colors as terminal defaults
    terminal.set_default_colors(theme.fg, theme.bg);
    terminal.grid.set_scrollback_max(config.scrollback);
    let session = Box::new(TermSession {
        terminal,
        parser: VtParser::new(),
        pty: None,
        renderer: None,
        config,
        watcher: crate::watcher::ConfigWatcher::new(),
        config_generation: 0,
    });
    Box::into_raw(session)
}

#[no_mangle]
pub extern "C" fn term_session_free(session: *mut TermSession) {
    if !session.is_null() {
        unsafe { drop(Box::from_raw(session)); }
    }
}

#[no_mangle]
pub extern "C" fn term_session_spawn_shell(
    session: *mut TermSession,
    shell: *const c_char,
) -> c_int {
    let session = unsafe { &mut *session };
    let shell_str = if shell.is_null() {
        // Use config shell if no explicit shell given
        Some(session.config.shell.program.as_str())
    } else {
        unsafe { CStr::from_ptr(shell).to_str().ok() }
    };

    match PtyManager::spawn(shell_str) {
        Ok(pty) => {
            // Set non-blocking
            unsafe {
                let fd = pty.master_fd();
                let flags = nix::libc::fcntl(fd, nix::libc::F_GETFL);
                nix::libc::fcntl(fd, nix::libc::F_SETFL, flags | nix::libc::O_NONBLOCK);
            }
            session.pty = Some(pty);
            0
        }
        Err(_) => -1,
    }
}

/// Read from PTY and feed into terminal. Returns number of bytes processed.
#[no_mangle]
pub extern "C" fn term_session_read_pty(session: *mut TermSession) -> c_int {
    let session = unsafe { &mut *session };
    let Some(pty) = &session.pty else { return -1 };
    let mut buf = [0u8; 8192];
    let mut total = 0i32;
    loop {
        match pty.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                session.terminal.feed_bytes(&mut session.parser, &buf[..n]);
                total += n as i32;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => return -1,
        }
    }
    // Flush write-back (DSR responses) to PTY
    if !session.terminal.write_back.is_empty() {
        let wb: Vec<u8> = session.terminal.write_back.drain(..).collect();
        if let Some(pty) = &session.pty {
            let _ = pty.write(&wb);
        }
    }
    total
}

/// Write user input to PTY.
#[no_mangle]
pub extern "C" fn term_session_write_pty(
    session: *mut TermSession,
    data: *const u8,
    len: c_uint,
) -> c_int {
    let session = unsafe { &*session };
    let Some(pty) = &session.pty else { return -1 };
    let slice = unsafe { std::slice::from_raw_parts(data, len as usize) };
    match pty.write(slice) {
        Ok(n) => n as c_int,
        Err(_) => -1,
    }
}

/// Get PTY master file descriptor (for polling).
#[no_mangle]
pub extern "C" fn term_session_pty_fd(session: *const TermSession) -> c_int {
    let session = unsafe { &*session };
    session.pty.as_ref().map(|p| p.master_fd()).unwrap_or(-1)
}

/// Resize terminal and PTY.
#[no_mangle]
pub extern "C" fn term_session_resize(
    session: *mut TermSession,
    cols: c_uint,
    rows: c_uint,
    pixel_width: c_uint,
    pixel_height: c_uint,
) {
    let session = unsafe { &mut *session };
    session.terminal.resize(cols as usize, rows as usize);
    if let Some(pty) = &session.pty {
        let ws = nix::pty::Winsize {
            ws_row: rows as u16,
            ws_col: cols as u16,
            ws_xpixel: pixel_width as u16,
            ws_ypixel: pixel_height as u16,
        };
        unsafe { nix::libc::ioctl(pty.master_fd(), nix::libc::TIOCSWINSZ, &ws); }
    }
}

/// Get cell character at (row, col). Returns Unicode codepoint.
#[no_mangle]
pub extern "C" fn term_session_cell_char(
    session: *const TermSession,
    row: c_uint,
    col: c_uint,
) -> u32 {
    let session = unsafe { &*session };
    session.terminal.grid.cell(row as usize, col as usize).ch as u32
}

/// Get cell foreground color. Returns packed RGB (0x00RRGGBB).
#[no_mangle]
pub extern "C" fn term_session_cell_fg(
    session: *const TermSession,
    row: c_uint,
    col: c_uint,
) -> u32 {
    let session = unsafe { &*session };
    let c = session.terminal.grid.cell(row as usize, col as usize).fg;
    (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
}

/// Get cell background color. Returns packed RGB.
#[no_mangle]
pub extern "C" fn term_session_cell_bg(
    session: *const TermSession,
    row: c_uint,
    col: c_uint,
) -> u32 {
    let session = unsafe { &*session };
    let c = session.terminal.grid.cell(row as usize, col as usize).bg;
    (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
}

/// Get cell attributes (bold, italic, etc). Returns bitfield.
#[no_mangle]
pub extern "C" fn term_session_cell_attr(
    session: *const TermSession,
    row: c_uint,
    col: c_uint,
) -> u8 {
    let session = unsafe { &*session };
    session.terminal.grid.cell(row as usize, col as usize).attr.bits()
}

/// Get cursor position. Writes to out_row and out_col.
#[no_mangle]
pub extern "C" fn term_session_cursor_pos(
    session: *const TermSession,
    out_row: *mut c_uint,
    out_col: *mut c_uint,
) {
    let session = unsafe { &*session };
    unsafe {
        *out_row = session.terminal.grid.cursor_row as c_uint;
        *out_col = session.terminal.grid.cursor_col as c_uint;
    }
}

/// Get terminal grid dimensions.
#[no_mangle]
pub extern "C" fn term_session_grid_size(
    session: *const TermSession,
    out_cols: *mut c_uint,
    out_rows: *mut c_uint,
) {
    let session = unsafe { &*session };
    unsafe {
        *out_cols = session.terminal.grid.cols() as c_uint;
        *out_rows = session.terminal.grid.rows() as c_uint;
    }
}

/// Get window title (set via OSC). Caller must free with term_string_free.
#[no_mangle]
pub extern "C" fn term_session_title(session: *const TermSession) -> *mut c_char {
    let session = unsafe { &*session };
    let c_str = std::ffi::CString::new(session.terminal.title.as_str()).unwrap_or_default();
    c_str.into_raw()
}

#[no_mangle]
pub extern "C" fn term_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(std::ffi::CString::from_raw(s)); }
    }
}

/// Returns 1 if cursor keys are in application mode.
#[no_mangle]
pub extern "C" fn term_session_cursor_keys_app(session: *const TermSession) -> c_int {
    let session = unsafe { &*session };
    session.terminal.cursor_keys_app as c_int
}

/// Returns 1 if cursor is visible.
#[no_mangle]
pub extern "C" fn term_session_cursor_visible(session: *const TermSession) -> c_int {
    let session = unsafe { &*session };
    session.terminal.cursor_visible as c_int
}

/// Returns 1 if bracketed paste mode is on.
#[no_mangle]
pub extern "C" fn term_session_bracketed_paste(session: *const TermSession) -> c_int {
    let session = unsafe { &*session };
    session.terminal.bracketed_paste as c_int
}

/// Get configured font size.
#[no_mangle]
pub extern "C" fn term_session_font_size(session: *const TermSession) -> f32 {
    let session = unsafe { &*session };
    session.config.font.size
}

/// Get configured font family. Caller must free with term_string_free.
#[no_mangle]
pub extern "C" fn term_session_font_family(session: *const TermSession) -> *mut c_char {
    let session = unsafe { &*session };
    let c_str = std::ffi::CString::new(session.config.font.family.as_str()).unwrap_or_default();
    c_str.into_raw()
}

/// Get configured window width.
#[no_mangle]
pub extern "C" fn term_session_window_width(session: *const TermSession) -> u32 {
    unsafe { &*session }.config.window.width
}

/// Get configured window height.
#[no_mangle]
pub extern "C" fn term_session_window_height(session: *const TermSession) -> u32 {
    unsafe { &*session }.config.window.height
}

/// Get theme background color as packed RGB.
#[no_mangle]
pub extern "C" fn term_session_theme_bg(session: *const TermSession) -> u32 {
    let session = unsafe { &*session };
    let theme = crate::theme::Theme::by_name(&session.config.colors.theme)
        .unwrap_or_else(crate::theme::Theme::default_dark);
    (theme.bg.r as u32) << 16 | (theme.bg.g as u32) << 8 | theme.bg.b as u32
}

/// Get theme foreground color as packed RGB.
#[no_mangle]
pub extern "C" fn term_session_theme_fg(session: *const TermSession) -> u32 {
    let session = unsafe { &*session };
    let theme = crate::theme::Theme::by_name(&session.config.colors.theme)
        .unwrap_or_else(crate::theme::Theme::default_dark);
    (theme.fg.r as u32) << 16 | (theme.fg.g as u32) << 8 | theme.fg.b as u32
}

/// Get scrollback line count.
#[no_mangle]
pub extern "C" fn term_session_scrollback_len(session: *const TermSession) -> c_uint {
    let session = unsafe { &*session };
    session.terminal.grid.scrollback().len() as c_uint
}

/// Get cell char from scrollback. `sb_row` 0 = oldest line.
#[no_mangle]
pub extern "C" fn term_session_scrollback_cell_char(
    session: *const TermSession, sb_row: c_uint, col: c_uint,
) -> u32 {
    let session = unsafe { &*session };
    let sb = session.terminal.grid.scrollback();
    let row = sb_row as usize;
    let col = col as usize;
    if row < sb.len() && col < sb[row].len() {
        sb[row][col].ch as u32
    } else { 0 }
}

/// Get cell fg from scrollback. Returns packed RGB.
#[no_mangle]
pub extern "C" fn term_session_scrollback_cell_fg(
    session: *const TermSession, sb_row: c_uint, col: c_uint,
) -> u32 {
    let session = unsafe { &*session };
    let sb = session.terminal.grid.scrollback();
    let (row, col) = (sb_row as usize, col as usize);
    if row < sb.len() && col < sb[row].len() {
        let c = sb[row][col].fg;
        (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
    } else { 0 }
}

/// Get cell bg from scrollback. Returns packed RGB.
#[no_mangle]
pub extern "C" fn term_session_scrollback_cell_bg(
    session: *const TermSession, sb_row: c_uint, col: c_uint,
) -> u32 {
    let session = unsafe { &*session };
    let sb = session.terminal.grid.scrollback();
    let (row, col) = (sb_row as usize, col as usize);
    if row < sb.len() && col < sb[row].len() {
        let c = sb[row][col].bg;
        (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
    } else { 0 }
}

/// Get last command exit code (-1 if none).
#[no_mangle]
pub extern "C" fn term_session_last_exit_code(session: *const TermSession) -> c_int {
    let session = unsafe { &*session };
    session.terminal.shell.last_exit_code().unwrap_or(-1)
}

/// Get command count in shell integration history.
#[no_mangle]
pub extern "C" fn term_session_command_count(session: *const TermSession) -> c_uint {
    let session = unsafe { &*session };
    session.terminal.shell.history().len() as c_uint
}

/// Get command info by index. Returns prompt_row, or -1 if invalid.
#[no_mangle]
pub extern "C" fn term_session_command_prompt_row(session: *const TermSession, idx: c_uint) -> c_int {
    let session = unsafe { &*session };
    session.terminal.shell.history().get(idx as usize)
        .map(|c| c.prompt_row as c_int).unwrap_or(-1)
}

/// Get command exit code by index. Returns -1 if invalid/unknown.
#[no_mangle]
pub extern "C" fn term_session_command_exit_code(session: *const TermSession, idx: c_uint) -> c_int {
    let session = unsafe { &*session };
    session.terminal.shell.history().get(idx as usize)
        .and_then(|c| c.exit_code).unwrap_or(-1) as c_int
}

/// Get command duration in milliseconds by index. Returns 0 if unknown.
#[no_mangle]
pub extern "C" fn term_session_command_duration_ms(session: *const TermSession, idx: c_uint) -> u64 {
    let session = unsafe { &*session };
    session.terminal.shell.history().get(idx as usize)
        .and_then(|c| c.duration).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Get working directory. Caller must free with term_string_free.
#[no_mangle]
pub extern "C" fn term_session_working_dir(session: *const TermSession) -> *mut c_char {
    let session = unsafe { &*session };
    let dir = &session.terminal.shell.working_dir;
    std::ffi::CString::new(dir.as_str()).unwrap_or_default().into_raw()
}

/// Get previous prompt row from current position. Returns -1 if none.
#[no_mangle]
pub extern "C" fn term_session_prev_prompt(session: *const TermSession, current_row: c_uint) -> c_int {
    let session = unsafe { &*session };
    session.terminal.shell.prev_prompt(current_row as usize).map(|r| r as c_int).unwrap_or(-1)
}

/// Get next prompt row from current position. Returns -1 if none.
#[no_mangle]
pub extern "C" fn term_session_next_prompt(session: *const TermSession, current_row: c_uint) -> c_int {
    let session = unsafe { &*session };
    session.terminal.shell.next_prompt(current_row as usize).map(|r| r as c_int).unwrap_or(-1)
}

/// Get URL at grid position. Returns null if none. Caller must free.
#[no_mangle]
pub extern "C" fn term_session_url_at(
    session: *const TermSession, row: c_uint, col: c_uint,
) -> *mut c_char {
    let session = unsafe { &*session };
    match crate::url_detect::url_at(&session.terminal.grid, row as usize, col as usize) {
        Some(url) => std::ffi::CString::new(url).unwrap_or_default().into_raw(),
        None => std::ptr::null_mut(),
    }
}

/// Poll for config changes. Returns new generation number if config changed, 0 if not.
#[no_mangle]
pub extern "C" fn term_session_poll_config(session: *mut TermSession) -> u64 {
    let session = unsafe { &mut *session };
    if let Some(new_config) = session.watcher.poll() {
        let theme = crate::theme::Theme::by_name(&new_config.colors.theme)
            .unwrap_or_else(crate::theme::Theme::default_dark);
        session.terminal.set_default_colors(theme.fg, theme.bg);
        session.terminal.grid.set_scrollback_max(new_config.scrollback);
        session.config = new_config;
        session.config_generation += 1;
        session.config_generation
    } else {
        0
    }
}

/// Extract text from grid between two positions (for selection copy).
#[no_mangle]
pub extern "C" fn term_session_extract_text(
    session: *const TermSession,
    start_row: c_uint, start_col: c_uint,
    end_row: c_uint, end_col: c_uint,
) -> *mut c_char {
    let session = unsafe { &*session };
    let grid = &session.terminal.grid;
    let mut text = String::new();

    let (sr, sc) = (start_row as usize, start_col as usize);
    let (er, ec) = (end_row as usize, end_col as usize);

    for row in sr..=er.min(grid.rows() - 1) {
        let col_start = if row == sr { sc } else { 0 };
        let col_end = if row == er { ec } else { grid.cols() };
        for col in col_start..col_end.min(grid.cols()) {
            let ch = grid.cell(row, col).ch;
            if ch != '\0' { text.push(ch); }
        }
        if row != er { text = text.trim_end().to_string(); text.push('\n'); }
    }
    let text = text.trim_end().to_string();
    std::ffi::CString::new(text).unwrap_or_default().into_raw()
}

/// Initialize GPU renderer with a CAMetalLayer pointer (macOS).
/// Returns 0 on success, -1 on failure.
#[no_mangle]
pub extern "C" fn term_session_init_gpu(
    session: *mut TermSession,
    metal_layer: *mut std::ffi::c_void,
    width: u32,
    height: u32,
) -> c_int {
    let session = unsafe { &mut *session };
    if metal_layer.is_null() { return -1; }

    let result = pollster::block_on(async {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });

        let surface = unsafe {
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer)
            )
        };
        let surface = match surface {
            Ok(s) => s,
            Err(_) => return -1,
        };

        let adapter = match instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await {
            Some(a) => a,
            None => return -1,
        };

        let (device, queue) = match adapter.request_device(
            &wgpu::DeviceDescriptor { label: Some("term-gpu"), ..Default::default() },
            None,
        ).await {
            Ok(dq) => dq,
            Err(_) => return -1,
        };

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let font_data = include_bytes!("/System/Library/Fonts/Menlo.ttc");
        let atlas = crate::renderer::atlas::GlyphAtlas::new(font_data, 14.0);
        let max_cells = (width / 8) as usize * (height / 16) as usize + 256;
        let render_state = crate::renderer::pipeline::RenderState::new_with_surface(
            device, queue, surface, config, &atlas, format, max_cells,
        );

        session.renderer = Some(GpuRenderer { render_state, atlas });
        0
    });
    result
}

/// Render the terminal grid using GPU. Returns 0 on success.
#[no_mangle]
pub extern "C" fn term_session_render_gpu(
    session: *mut TermSession,
    width: u32,
    height: u32,
) -> c_int {
    let session = unsafe { &mut *session };
    let Some(renderer) = &mut session.renderer else { return -1 };

    let (vertices, indices) = renderer.render_state.build_vertices(
        &session.terminal.grid,
        &mut renderer.atlas,
        width as f32,
        height as f32,
    );

    if vertices.is_empty() { return 0; }

    renderer.render_state.update_atlas(&mut renderer.atlas);

    // Upload vertex/index data
    renderer.render_state.queue.write_buffer(
        &renderer.render_state.vertex_buffer,
        0,
        bytemuck::cast_slice(&vertices),
    );
    renderer.render_state.queue.write_buffer(
        &renderer.render_state.index_buffer,
        0,
        bytemuck::cast_slice(&indices),
    );

    // Render
    let surface = match &renderer.render_state.surface {
        Some(s) => s,
        None => return -1,
    };
    let frame = match surface.get_current_texture() {
        Ok(f) => f,
        Err(_) => return -1,
    };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = renderer.render_state.device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: Some("render") },
    );

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terminal"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&renderer.render_state.pipeline);
        pass.set_bind_group(0, &renderer.render_state.atlas_bind_group, &[]);
        pass.set_vertex_buffer(0, renderer.render_state.vertex_buffer.slice(..));
        pass.set_index_buffer(renderer.render_state.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }

    renderer.render_state.queue.submit(std::iter::once(encoder.finish()));
    frame.present();
    0
}

/// Resize the GPU surface.
#[no_mangle]
pub extern "C" fn term_session_resize_gpu(
    session: *mut TermSession,
    width: u32,
    height: u32,
) {
    let session = unsafe { &mut *session };
    let Some(renderer) = &mut session.renderer else { return };
    if let Some(config) = &mut renderer.render_state.config {
        config.width = width.max(1);
        config.height = height.max(1);
        if let Some(surface) = &renderer.render_state.surface {
            surface.configure(&renderer.render_state.device, config);
        }
    }
}
