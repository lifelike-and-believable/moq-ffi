// MoQ FFI - C API wrapper for moq-transport
//
// This library provides a C-compatible FFI interface to the Rust moq-transport
// implementation, enabling integration with C++ projects and game engines.

#[cfg(feature = "with_moq")]
mod backend_moq;

#[cfg(not(feature = "with_moq"))]
mod backend_stub;

// Re-export the appropriate backend
#[cfg(feature = "with_moq")]
pub use backend_moq::*;

#[cfg(not(feature = "with_moq"))]
pub use backend_stub::*;
