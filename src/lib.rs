pub mod core;
pub mod pty;
pub mod renderer;
pub mod platform;

#[no_mangle]
pub extern "C" fn libterm_version() -> *const std::ffi::c_char {
    c"0.1.0".as_ptr()
}
