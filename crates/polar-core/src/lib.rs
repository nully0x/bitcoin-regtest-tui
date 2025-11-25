//! Core types and configuration for Polar TUI.
//!
//! This crate provides shared data structures, configuration management,
//! and error types used across the polar workspace.

mod config;
mod error;
mod network;
mod node_info;

pub use config::Config;
pub use error::{Error, Result};
pub use network::{LightningImpl, Network, NetworkStatus, Node, NodeKind, NodePorts, PortConfig};
pub use node_info::{BitcoinNodeInfo, ChannelInfo, LndNodeInfo, NodeInfo};
