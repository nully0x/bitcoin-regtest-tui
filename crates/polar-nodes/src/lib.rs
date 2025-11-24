//! Node implementations for Polar TUI.
//!
//! This crate provides Bitcoin Core and LND node management.

mod bitcoin;
mod lnd;

pub use bitcoin::{BITCOIN_VERSIONS, BitcoinNode};
pub use lnd::{LND_VERSIONS, LndNode};
