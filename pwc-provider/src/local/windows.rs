//! The endpoint on Windows: the named pipe `\\.\pipe\pwc-<sid>-<provider_id>` (LOCAL §3.3, §3.4).

use std::ffi::{c_void, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, PipeMode, ServerOptions};
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER};
use windows_sys::Win32::System::Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::{connection, EndpointError};
use crate::prompt::PeerInfo;
use crate::provider::Provider;

/// Longest pipe name Windows accepts.
const PIPE_NAME_MAX: usize = 256;
/// Wait before retrying to create a pipe instance that could not be created.
const RETRY: Duration = Duration::from_millis(100);

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// A Windows handle closed when dropped.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by this module and is closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// The string form of the user SID of a process's token.
fn process_sid(process: HANDLE) -> io::Result<String> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: `token` receives a handle that `Owned` closes.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Owned(token);
    let mut size = 0u32;
    // SAFETY: a size query with a null buffer.
    unsafe { GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut size) };
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: the buffer holds `size` bytes, aligned for TOKEN_USER.
    if unsafe { GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), size, &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: GetTokenInformation filled the buffer with a TOKEN_USER.
    let user = unsafe { &*(buf.as_ptr() as *const TOKEN_USER) };
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: the SID is valid while `buf` lives; the string is freed with LocalFree below.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `text` is a NUL-terminated wide string allocated by the system.
    let sid = unsafe {
        let len = (0..).take_while(|&i| *text.add(i) != 0).count();
        let s = OsString::from_wide(std::slice::from_raw_parts(text, len));
        LocalFree(text.cast());
        s
    };
    Ok(sid.to_string_lossy().into_owned())
}

/// The user SID of the current process.
pub(crate) fn current_sid() -> io::Result<String> {
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing.
    process_sid(unsafe { GetCurrentProcess() })
}

/// The user SID and executable of process `pid`.
fn process_info(pid: u32) -> io::Result<(String, Option<PathBuf>)> {
    // SAFETY: the returned handle, if any, is closed by `Owned`.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let process = Owned(process);
    let sid = process_sid(process.0)?;
    let mut buf = vec![0u16; 32_768];
    let mut len = buf.len() as u32;
    // SAFETY: the buffer holds `len` wide characters.
    let executable = (unsafe { QueryFullProcessImageNameW(process.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) }
        != 0)
        .then(|| PathBuf::from(OsString::from_wide(&buf[..len as usize])));
    Ok((sid, executable))
}

/// The LOCAL §3.3 pipe name of `provider_id` for the current user.
pub fn pipe_name(provider_id: &str) -> io::Result<String> {
    Ok(name_for(&current_sid()?, provider_id))
}

fn name_for(sid: &str, provider_id: &str) -> String {
    format!(r"\\.\pipe\pwc-{sid}-{provider_id}")
}

/// A security descriptor whose protected DACL grants full access to one SID only.
struct Security {
    descriptor: *mut c_void,
}

// SAFETY: the descriptor is immutable after creation and only read by CreateNamedPipeW.
unsafe impl Send for Security {}
unsafe impl Sync for Security {}

impl Security {
    fn for_sid(sid: &str) -> io::Result<Security> {
        let sddl = wide(&format!("D:P(A;;GA;;;{sid})"));
        let mut descriptor: *mut c_void = std::ptr::null_mut();
        // SAFETY: `descriptor` receives a LocalAlloc'd descriptor freed in Drop.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Security { descriptor })
    }

    fn create(&self, name: &str, first: bool) -> io::Result<NamedPipeServer> {
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor,
            bInheritHandle: 0,
        };
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .pipe_mode(PipeMode::Byte);
        // SAFETY: `attributes` is a valid SECURITY_ATTRIBUTES for the duration of the call.
        unsafe {
            options.create_with_security_attributes_raw(name, (&mut attributes as *mut SECURITY_ATTRIBUTES).cast())
        }
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW, freed once.
        unsafe { LocalFree(self.descriptor) };
    }
}

/// A listening endpoint: the pending pipe instance and what is needed to create the next ones.
pub struct Endpoint {
    path: PathBuf,
    sid: String,
    security: Security,
    pending: NamedPipeServer,
}

impl Endpoint {
    /// The pipe name.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Whether the server of the existing pipe `name` runs under `sid`.
fn served_by(name: &str, sid: &str) -> bool {
    let Ok(client) = ClientOptions::new().open(name) else {
        return false;
    };
    let mut pid = 0u32;
    // SAFETY: the client handle is open for the duration of the call.
    if unsafe { GetNamedPipeServerProcessId(client.as_raw_handle() as HANDLE, &mut pid) } == 0 {
        return false;
    }
    process_info(pid).is_ok_and(|(server, _)| server == sid)
}

/// Creates the first instance of the pipe of `provider_id` (LOCAL §3.3). Must run inside a Tokio runtime.
pub fn bind_pipe(provider_id: &str) -> Result<Endpoint, EndpointError> {
    let sid = current_sid().map_err(|e| EndpointError::Io(PathBuf::from(provider_id), e))?;
    let name = name_for(&sid, provider_id);
    let path = PathBuf::from(&name);
    if name.len() > PIPE_NAME_MAX {
        return Err(EndpointError::PathTooLong(path));
    }
    let security = Security::for_sid(&sid).map_err(|e| EndpointError::Io(path.clone(), e))?;
    match security.create(&name, true) {
        Ok(pending) => Ok(Endpoint {
            path,
            sid,
            security,
            pending,
        }),
        Err(e) if [ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY].contains(&(e.raw_os_error().unwrap_or(0) as u32)) => {
            if served_by(&name, &sid) {
                Err(EndpointError::AlreadyRunning(path))
            } else {
                Err(EndpointError::NameTaken(path))
            }
        }
        Err(e) => Err(EndpointError::Io(path, e)),
    }
}

/// The peer of a connected instance if it runs under `sid` (LOCAL §3.4).
fn peer_info(pipe: &NamedPipeServer, sid: &str) -> Option<PeerInfo> {
    let mut pid = 0u32;
    // SAFETY: the pipe handle is open for the duration of the call.
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle() as HANDLE, &mut pid) } == 0 {
        return None;
    }
    let (peer, executable) = process_info(pid).ok()?;
    (peer == sid).then(|| PeerInfo {
        pid: i32::try_from(pid).ok(),
        executable,
    })
}

/// Accepts connections until the task is dropped. A new instance is created before each connection is handed
/// off, so the name is never left without a listening instance.
pub async fn serve(provider: Arc<Provider>, endpoint: Endpoint) {
    let Endpoint {
        path,
        sid,
        security,
        mut pending,
    } = endpoint;
    let name = path.to_string_lossy().into_owned();
    loop {
        let connected = pending.connect().await;
        let next = loop {
            match security.create(&name, false) {
                Ok(next) => break next,
                Err(_) => tokio::time::sleep(RETRY).await,
            }
        };
        let pipe = std::mem::replace(&mut pending, next);
        if connected.is_err() {
            continue;
        }
        let Some(peer) = peer_info(&pipe, &sid) else {
            continue; // LOCAL §3.4: closed without any message
        };
        let (rd, wr) = tokio::io::split(pipe);
        tokio::spawn(connection(provider.clone(), rd, wr, peer));
    }
}
