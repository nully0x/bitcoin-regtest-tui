//! Docker management for Polar TUI.
//!
//! This crate handles Docker container lifecycle and log streaming
//! for Lightning Network nodes.

mod container;
mod logs;
mod ports;

pub use container::ContainerManager;
pub use logs::LogStream;
pub use ports::PortMap;
