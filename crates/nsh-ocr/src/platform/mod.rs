//! The platform backends. Each target gets one `NativeBackend`; a target with none reports
//! that text recognition is not available.

#[cfg(target_os = "android")]
mod android;
#[cfg(target_vendor = "apple")]
mod apple;
// `self::` because the extern crate `ocrs` would otherwise be ambiguous with this module.
// `pub(crate)`: lib.rs re-exports the engine's public surface on these targets.
#[cfg(any(target_os = "linux", target_arch = "wasm32"))]
pub(crate) mod ocrs;
#[cfg(not(any(
    target_vendor = "apple",
    target_os = "android",
    target_os = "windows",
    target_os = "linux",
    target_arch = "wasm32"
)))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

use std::sync::Arc;

use crate::backend::Backend;

/// The backend of the target this build runs on.
pub(crate) fn default_backend() -> Arc<dyn Backend> {
    #[cfg(any(target_os = "linux", target_arch = "wasm32"))]
    let backend = self::ocrs::NativeBackend::new();
    #[cfg(target_vendor = "apple")]
    let backend = self::apple::NativeBackend;
    #[cfg(target_os = "android")]
    let backend = self::android::NativeBackend;
    #[cfg(target_os = "windows")]
    let backend = self::windows::NativeBackend;
    #[cfg(not(any(
        target_vendor = "apple",
        target_os = "android",
        target_os = "windows",
        target_os = "linux",
        target_arch = "wasm32"
    )))]
    let backend = self::unsupported::NativeBackend;
    Arc::new(backend)
}
