//! A utility to watch certain files and react to changes of those files (via event).
//! Only has a stub implementation on WASM which does not do anything but doesnt break existing code.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use native::{FileWatcher, events};

#[cfg(target_arch = "wasm32")]
#[allow(unused)]
pub use wasm::{FileWatcher, events};
