//! The endpoint on macOS and Linux: an `AF_UNIX` socket in the endpoint directory (LOCAL §3.2, §3.4).

use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};

use super::{connection, EndpointError};
use crate::prompt::PeerInfo;
use crate::provider::Provider;

/// A listening endpoint. Dropping it removes the socket file.
pub struct Endpoint {
    listener: UnixListener,
    path: PathBuf,
}

impl Endpoint {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn euid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// The endpoint directory of LOCAL §3.2 for the current user.
pub fn default_directory() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/PWC/run"));
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    match runtime {
        Some(r) if r.is_absolute() && std::fs::metadata(&r).is_ok_and(|m| m.uid() == euid()) => Some(r.join("pwc")),
        _ => home.map(|h| h.join(".pwc/run")),
    }
}

const SUN_PATH_MAX: usize = if cfg!(target_os = "macos") { 104 } else { 108 };

/// Creates or checks the endpoint directory and listens on `<dir>/<provider_id>.sock`.
pub fn bind(dir: &Path, provider_id: &str) -> Result<Endpoint, EndpointError> {
    let io = |e| EndpointError::Io(dir.to_path_buf(), e);
    match std::fs::metadata(dir) {
        Ok(m) => {
            if !m.is_dir() || m.uid() != euid() || m.mode() & 0o077 != 0 {
                return Err(EndpointError::UnsafeDirectory(dir.to_path_buf()));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(io)?;
        }
        Err(e) => return Err(io(e)),
    }
    let path = dir.join(format!("{provider_id}.sock"));
    if path.as_os_str().len() + 1 > SUN_PATH_MAX {
        return Err(EndpointError::PathTooLong(path));
    }
    if std::fs::symlink_metadata(&path).is_ok() {
        match std::os::unix::net::UnixStream::connect(&path) {
            Ok(_) => return Err(EndpointError::AlreadyRunning(path)),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(&path).map_err(io)?;
            }
            Err(e) => return Err(EndpointError::Io(path, e)),
        }
    }
    let listener = UnixListener::bind(&path).map_err(|e| EndpointError::Io(path.clone(), e))?;
    let endpoint = Endpoint { listener, path };
    std::fs::set_permissions(&endpoint.path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| EndpointError::Io(endpoint.path.clone(), e))?;
    Ok(endpoint)
}

/// Accepts connections until the task is dropped.
pub async fn serve(provider: Arc<Provider>, endpoint: Endpoint) {
    loop {
        if let Ok((stream, _)) = endpoint.listener.accept().await {
            let Some(peer) = peer_info(&stream) else {
                continue; // LOCAL §3.4: closed without any message
            };
            let (rd, wr) = stream.into_split();
            tokio::spawn(connection(provider.clone(), rd, wr, peer));
        }
    }
}

fn peer_info(stream: &UnixStream) -> Option<PeerInfo> {
    let cred = stream.peer_cred().ok()?;
    if cred.uid() != euid() {
        return None;
    }
    let pid = cred.pid();
    let executable = pid.and_then(executable);
    Some(PeerInfo { pid, executable })
}

#[cfg(target_os = "macos")]
fn executable(pid: i32) -> Option<PathBuf> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: the buffer is valid for the length passed.
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    buf.truncate(n as usize);
    Some(PathBuf::from(
        <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(buf),
    ))
}

#[cfg(not(target_os = "macos"))]
fn executable(pid: i32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).ok()
}
