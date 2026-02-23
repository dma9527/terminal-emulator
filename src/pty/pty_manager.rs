use nix::pty::{openpty, OpenptyResult};
use nix::unistd::{close, dup2, execvp, fork, setsid, ForkResult};
use std::ffi::CString;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

pub struct PtyManager {
    master: OwnedFd,
    child_pid: nix::unistd::Pid,
}

impl PtyManager {
    /// Spawn a new PTY with the user's default shell.
    pub fn spawn(shell: Option<&str>) -> io::Result<Self> {
        Self::spawn_with_integration(shell, true)
    }

    pub fn spawn_with_integration(shell: Option<&str>, integrate: bool) -> io::Result<Self> {
        let OpenptyResult { master, slave } =
            openpty(None, None).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let shell_path = shell
            .map(String::from)
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/zsh".into());

        let integration_dir = if integrate {
            Some(crate::shell_scripts::write_integration_scripts())
        } else { None };

        match unsafe { fork() }.map_err(|e| io::Error::new(io::ErrorKind::Other, e))? {
            ForkResult::Child => {
                drop(master);
                setsid().ok();
                dup2(slave.as_raw_fd(), 0).ok();
                dup2(slave.as_raw_fd(), 1).ok();
                dup2(slave.as_raw_fd(), 2).ok();
                if slave.as_raw_fd() > 2 {
                    close(slave.as_raw_fd()).ok();
                }

                // Inject shell integration via env vars
                if let Some(dir) = &integration_dir {
                    if shell_path.contains("zsh") {
                        std::env::set_var("ZDOTDIR", dir);
                    } else if shell_path.contains("bash") {
                        let bashrc = dir.join(".bashrc");
                        std::env::set_var("ENV", &bashrc);
                        // bash --rcfile for non-login shells
                    }
                    std::env::set_var("TERM_PROGRAM", "term");
                    std::env::set_var("TERM_PROGRAM_VERSION", "0.1.0");
                }

                let c_shell = CString::new(shell_path).unwrap();
                execvp(&c_shell, &[&c_shell]).ok();
                std::process::exit(1);
            }
            ForkResult::Parent { child } => {
                drop(slave);
                Ok(Self {
                    master,
                    child_pid: child,
                })
            }
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut file = unsafe { std::fs::File::from_raw_fd(self.master.as_raw_fd()) };
        let n = file.read(buf);
        std::mem::forget(file); // don't close the fd
        n
    }

    pub fn write(&self, data: &[u8]) -> io::Result<usize> {
        let mut file = unsafe { std::fs::File::from_raw_fd(self.master.as_raw_fd()) };
        let n = file.write(data);
        std::mem::forget(file);
        n
    }

    pub fn master_fd(&self) -> i32 {
        self.master.as_raw_fd()
    }

    pub fn child_pid(&self) -> i32 {
        self.child_pid.as_raw()
    }
}
