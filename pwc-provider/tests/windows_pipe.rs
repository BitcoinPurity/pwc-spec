//! The named pipe endpoint on Windows (LOCAL §3.3, §3.4, L-24). The protocol tests of the other files also run
//! over the pipe on Windows.
#![cfg(windows)]

mod common;

use std::os::windows::io::AsRawHandle;

use common::Harness;
use pwc_provider::local::{self, EndpointError};
use pwc_provider::provider::Config;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1, SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

/// The DACL of the pipe behind a client handle, in SDDL.
fn dacl(handle: std::os::windows::io::RawHandle) -> String {
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: the handle is open; the descriptor is freed below.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    assert_eq!(status, 0, "GetSecurityInfo");
    let mut text = std::ptr::null_mut();
    // SAFETY: the descriptor is valid; the string is freed below.
    let ok = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(ok, 0, "ConvertSecurityDescriptorToStringSecurityDescriptorW");
    // SAFETY: `text` is a NUL-terminated wide string allocated by the system.
    unsafe {
        let len = (0..).take_while(|&i| *text.add(i) != 0).count();
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        LocalFree(text.cast());
        LocalFree(descriptor);
        s
    }
}

#[tokio::test]
async fn pipe_name_and_dacl_grant_the_user_only() {
    let h = Harness::start(Config::default()).await;
    let name = h.path.to_string_lossy().into_owned();
    let prefix = local::pipe_name("x").unwrap();
    let prefix = &prefix[..prefix.len() - 1];
    assert!(prefix.starts_with(r"\\.\pipe\pwc-S-1-"), "{prefix}");
    assert!(name.starts_with(prefix), "{name}");
    let sid = &prefix[r"\\.\pipe\pwc-".len()..prefix.len() - 1];
    let client = common::open_pipe(&h.path).await;
    let sddl = dacl(client.as_raw_handle());
    assert!(sddl.starts_with("D:P"), "protected DACL: {sddl}");
    assert_eq!(sddl.matches("(A;").count(), 1, "one allow entry: {sddl}");
    assert!(sddl.contains(sid), "{sddl}");
}

#[tokio::test]
async fn second_instance_already_running_and_name_reusable() {
    let id = format!("pwc-test-dup-{}", std::process::id());
    let first = local::bind_pipe(&id).unwrap();
    assert!(matches!(local::bind_pipe(&id), Err(EndpointError::AlreadyRunning(_))));
    drop(first);
    local::bind_pipe(&id).expect("the name is free once the first endpoint is gone");
}

#[tokio::test]
async fn overlong_name_refused() {
    assert!(matches!(
        local::bind_pipe(&"p".repeat(300)),
        Err(EndpointError::PathTooLong(_))
    ));
}
