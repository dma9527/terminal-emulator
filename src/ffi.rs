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
}

#[no_mangle]
pub extern "C" fn term_session_new(cols: c_uint, rows: c_uint) -> *mut TermSession {
    let session = Box::new(TermSession {
        terminal: Terminal::new(cols as usize, rows as usize),
        parser: VtParser::new(),
        pty: None,
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
        None
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
