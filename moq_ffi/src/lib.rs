// MoQ FFI - C API wrapper for moq-transport
//
// This library provides a C-compatible FFI interface to the Rust moq-transport
// implementation, enabling integration with C++ projects and game engines.

#[cfg(any(feature = "with_moq", feature = "with_moq_draft07"))]
mod backend_moq;

#[cfg(not(any(feature = "with_moq", feature = "with_moq_draft07")))]
mod backend_stub;

// Re-export the appropriate backend
#[cfg(any(feature = "with_moq", feature = "with_moq_draft07"))]
pub use backend_moq::*;

#[cfg(not(any(feature = "with_moq", feature = "with_moq_draft07")))]
pub use backend_stub::*;
