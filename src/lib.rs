pub mod core;
pub mod pty;
pub mod renderer;
pub mod platform;
pub mod ffi;
pub mod config;
pub mod theme;
pub mod clipboard;
pub mod watcher;
pub mod search;
pub mod url_detect;
pub mod dirty;
pub mod session;

#[no_mangle]
pub extern "C" fn libterm_version() -> *const std::ffi::c_char {
    c"0.1.0".as_ptr()
}
