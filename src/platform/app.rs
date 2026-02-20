/// Window application: connects winit window, wgpu renderer, PTY, and terminal.

use crate::core::{Terminal, VtParser};
use crate::pty::PtyManager;
use crate::renderer::atlas::GlyphAtlas;
use crate::renderer::pipeline::RenderState;
use crate::renderer::cursor::Cursor;
use crate::renderer::selection::{Selection, SelectionMode};
use crate::renderer::scroll::SmoothScroll;
use crate::core::Color;

use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const FONT_DATA: &[u8] = include_bytes!("/System/Library/Fonts/Menlo.ttc");
const FONT_SIZE: f32 = 14.0;
const DEFAULT_COLS: usize = 80;
const DEFAULT_ROWS: usize = 24;

pub struct App {
    window: Option<Arc<Window>>,
    render: Option<RenderState>,
    atlas: Option<GlyphAtlas>,
    terminal: Terminal,
    parser: VtParser,
    pty: Option<PtyManager>,
    cursor: Cursor,
    selection: Selection,
    scroll: SmoothScroll,
}

impl App {
    pub fn new() -> Self {
        Self {
            window: None,
            render: None,
            atlas: None,
            terminal: Terminal::new(DEFAULT_COLS, DEFAULT_ROWS),
            parser: VtParser::new(),
            pty: None,
            cursor: Cursor::new(),
            selection: Selection::new(),
            scroll: SmoothScroll::new(),
        }
    }

    fn init_renderer(&mut self, window: Arc<Window>) {
        let mut atlas = GlyphAtlas::new(FONT_DATA, FONT_SIZE);

        // Pre-rasterize ASCII for fast startup
        for ch in ' '..='~' {
            atlas.get_glyph(ch);
        }

        let size = window.inner_size();
        let max_cells = DEFAULT_COLS * DEFAULT_ROWS * 2; // headroom

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("No GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("terminal-device"),
                ..Default::default()
            },
            None,
        ))
        .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let render = RenderState::new_with_surface(
            device,
            queue,
            surface,
            config,
            &atlas,
            format,
            max_cells,
        );

        self.render = Some(render);
        self.atlas = Some(atlas);
        self.window = Some(window);
    }

    fn read_pty(&mut self) {
        let Some(pty) = &self.pty else { return };
        let mut buf = [0u8; 8192];
        loop {
            match pty.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    self.terminal.feed_bytes(&mut self.parser, &buf[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
    }

    fn render_frame(&mut self) {
        let Some(render) = &self.render else { return };
        let Some(atlas) = &mut self.atlas else { return };
        let Some(window) = &self.window else { return };

        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        render.update_atlas(atlas);

        let (mut vertices, mut indices) = render.build_vertices(
            &self.terminal.grid,
            atlas,
            size.width as f32,
            size.height as f32,
        );

        // Add selection highlight
        let (sel_v, sel_i) = self.selection.build_vertices(
            &self.terminal.grid,
            atlas.cell_width, atlas.cell_height,
            size.width as f32, size.height as f32,
        );
        let base = vertices.len() as u32;
        vertices.extend_from_slice(&sel_v);
        indices.extend(sel_i.iter().map(|i| i + base));

        // Add cursor
        let cursor_verts = self.cursor.build_vertices(
            self.terminal.grid.cursor_row,
            self.terminal.grid.cursor_col,
            atlas.cell_width, atlas.cell_height,
            size.width as f32, size.height as f32,
            Color { r: 200, g: 200, b: 200 },
        );
        if cursor_verts.len() == 4 {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&cursor_verts);
            indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
        }

        if vertices.is_empty() {
            return;
        }

        render.queue.write_buffer(
            &render.vertex_buffer,
            0,
            bytemuck::cast_slice(&vertices),
        );
        render.queue.write_buffer(
            &render.index_buffer,
            0,
            bytemuck::cast_slice(&indices),
        );

        let surface = render.surface.as_ref().unwrap();
        let output = match surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                if let Some(config) = &render.config {
                    surface.configure(&render.device, config);
                }
                return;
            }
            Err(_) => return,
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = render.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame-encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cell-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0, g: 0.0, b: 0.0, a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            pass.set_pipeline(&render.pipeline);
            pass.set_bind_group(0, &render.atlas_bind_group, &[]);
            pass.set_vertex_buffer(0, render.vertex_buffer.slice(..));
            pass.set_index_buffer(render.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        render.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn handle_key_input(&mut self, event: &winit::event::KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        self.cursor.reset_blink();
        self.scroll.reset(); // snap to bottom on keypress
        let Some(pty) = &self.pty else { return };

        let bytes: Option<Vec<u8>> = match &event.logical_key {
            Key::Named(NamedKey::Enter) => Some(vec![0x0d]),
            Key::Named(NamedKey::Backspace) => Some(vec![0x7f]),
            Key::Named(NamedKey::Tab) => Some(vec![0x09]),
            Key::Named(NamedKey::Escape) => Some(vec![0x1b]),
            Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
            Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
            Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
            Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
            Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
            Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
            Key::Named(NamedKey::PageUp) => Some(b"\x1b[5~".to_vec()),
            Key::Named(NamedKey::PageDown) => Some(b"\x1b[6~".to_vec()),
            Key::Named(NamedKey::Delete) => Some(b"\x1b[3~".to_vec()),
            Key::Character(s) => {
                Some(s.as_str().as_bytes().to_vec())
            }
            _ => None,
        };

        if let Some(data) = bytes {
            let _ = pty.write(&data);
        }
    }

    fn update_terminal_size(&mut self) {
        let Some(window) = &self.window else { return };
        let Some(atlas) = &self.atlas else { return };

        let size = window.inner_size();
        let cols = (size.width as f32 / atlas.cell_width).floor() as usize;
        let rows = (size.height as f32 / atlas.cell_height).floor() as usize;

        if cols > 0 && rows > 0 {
            self.terminal.resize(cols, rows);
            // Also resize PTY
            if let Some(pty) = &self.pty {
                let ws = nix::pty::Winsize {
                    ws_row: rows as u16,
                    ws_col: cols as u16,
                    ws_xpixel: size.width as u16,
                    ws_ypixel: size.height as u16,
                };
                unsafe {
                    nix::libc::ioctl(pty.master_fd(), nix::libc::TIOCSWINSZ, &ws);
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Terminal")
            .with_inner_size(PhysicalSize::new(800, 600));

        let window = Arc::new(event_loop.create_window(attrs).expect("Failed to create window"));
        self.init_renderer(window);

        // Spawn PTY
        let pty = PtyManager::spawn(None).expect("Failed to spawn PTY");
        // Set non-blocking
        unsafe {
            let flags = nix::libc::fcntl(pty.master_fd(), nix::libc::F_GETFL);
            nix::libc::fcntl(pty.master_fd(), nix::libc::F_SETFL, flags | nix::libc::O_NONBLOCK);
        }
        self.pty = Some(pty);

        self.update_terminal_size();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let Some(render) = &self.render {
                    if size.width > 0 && size.height > 0 {
                        if let Some(config) = &mut render.config.clone() {
                            config.width = size.width;
                            config.height = size.height;
                            render.surface.as_ref().unwrap().configure(&render.device, config);
                        }
                    }
                }
                self.update_terminal_size();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_input(&event);
            }

            WindowEvent::MouseInput { state, button: winit::event::MouseButton::Left, .. } => {
                if let Some(atlas) = &self.atlas {
                    // TODO: track mouse position via CursorMoved for accurate coords
                    match state {
                        ElementState::Pressed => {
                            self.selection.clear();
                        }
                        ElementState::Released => {
                            // Selection finalized
                        }
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(atlas) = &self.atlas {
                    let lines = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => {
                            pos.y as f32 / atlas.cell_height
                        }
                    };
                    let scrollback_len = 0; // TODO: expose scrollback len from grid
                    self.scroll.scroll(lines, atlas.cell_height, scrollback_len.max(1));
                }
            }

            WindowEvent::RedrawRequested => {
                self.read_pty();
                self.scroll.update();
                self.render_frame();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
