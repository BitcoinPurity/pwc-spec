//! Native methods of `MemoryBackend`.

use std::sync::Arc;

use jni::objects::JClass;
use jni::sys::jlong;
use jni::JNIEnv;
use pwc_provider::memory::MemoryBackend;

use crate::native;
use crate::provider::{free, into_ptr, MemoryHandle};

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryDemo<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
) -> jlong {
    native(&mut env, 0, |_| Ok(into_ptr::<MemoryHandle>(Arc::new(MemoryBackend::demo()))))
}

#[no_mangle]
pub extern "system" fn Java_org_bitcoinpurity_pwc_provider_Native_memoryFree<'l>(
    mut env: JNIEnv<'l>,
    _: JClass<'l>,
    m: jlong,
) {
    // SAFETY: called once by the Java handle's cleaner.
    native(&mut env, (), |_| Ok(unsafe { free::<MemoryHandle>(m) }))
}
