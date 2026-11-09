//! Cross Platform, Performant and High Quality screen recordings

#[cfg(any(not(target_os = "macos"), feature = "macos"))]
pub mod capturer;
pub mod frame;
#[cfg(any(not(target_os = "macos"), feature = "macos"))]
mod targets;
#[cfg(any(not(target_os = "macos"), feature = "macos"))]
mod utils;

// Helper Methods
#[cfg(any(not(target_os = "macos"), feature = "macos"))]
pub use targets::{Display, Target, Window, get_all_targets};
#[cfg(any(not(target_os = "macos"), feature = "macos"))]
pub use utils::{has_permission, is_supported, request_permission};

#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod engine {
    pub use crate::capturer::engine::mac;
}

#[cfg(all(target_os = "macos", not(feature = "macos")))]
compile_error!("macOS screen capture requires the `macos` feature");
