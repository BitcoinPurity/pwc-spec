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
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW, ConvertStringSidToSidW,
    GetSecurityInfo, SDDL_REVISION_1, SE_KERNEL_OBJECT,
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
    let s = wide_to_string(text);
    // SAFETY: both were allocated by the system.
    unsafe {
        LocalFree(text.cast());
        LocalFree(descriptor);
    }
    s
}

fn wide_to_string(text: *const u16) -> String {
    // SAFETY: `text` is a NUL-terminated wide string allocated by the system.
    unsafe {
        let len = (0..).take_while(|&i| *text.add(i) != 0).count();
        String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
    }
}

/// An SDDL account, which may be an alias such as `SY` for LocalSystem, as an `S-1-...` string.
fn account_sid(sddl_account: &str) -> String {
    let wide: Vec<u16> = sddl_account.encode_utf16().chain([0]).collect();
    let mut sid = std::ptr::null_mut();
    // SAFETY: `wide` is NUL-terminated; the SID and the string are freed below.
    unsafe {
        assert_ne!(
            ConvertStringSidToSidW(wide.as_ptr(), &mut sid),
            0,
            "ConvertStringSidToSidW {sddl_account}"
        );
        let mut text = std::ptr::null_mut();
        assert_ne!(ConvertSidToStringSidW(sid, &mut text), 0, "ConvertSidToStringSidW");
        let s = wide_to_string(text);
        LocalFree(text.cast());
        LocalFree(sid);
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
    // The one entry's account, after the last `;` of `(A;;rights;;;account)`; SDDL writes well-known accounts as
    // aliases, such as `SY` when the tests run as LocalSystem.
    let account = sddl.trim_end_matches(')').rsplit(';').next().unwrap();
    assert_eq!(account_sid(account), sid, "{sddl}");
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
