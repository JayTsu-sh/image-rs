//! Application layer — use cases and the ports they depend on.
//!
//! Pure orchestration: no opencv, no axum. Wires the domain to ports
//! (traits) that infrastructure adapters implement.

pub mod diff_image;
pub mod get_image;
pub mod ports;
pub mod process_image;
