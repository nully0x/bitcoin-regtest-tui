//! Node information structures.

use serde::{Deserialize, Serialize};

/// Information about a Bitcoin Core node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinNodeInfo {
    /// Node version.
    pub version: String,
    /// Block height.
    pub blocks: u64,
    /// Chain (e.g., "regtest", "mainnet").
    pub chain: String,
    /// Number of connections.
    pub connections: u32,
    /// Network difficulty.
    pub difficulty: f64,
    /// Is initial block download complete.
    pub ibd_complete: bool,
    /// Wallet balance in BTC.
    pub balance: f64,
    /// RPC host:port.
    pub rpc_host: String,
    /// P2P host:port.
    pub p2p_host: String,
}

/// Information about a Lightning channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// Channel point (funding_txid:output_index).
    pub channel_point: String,
    /// Remote node public key.
    pub remote_pubkey: String,
    /// Channel capacity in satoshis.
    pub capacity: i64,
    /// Local balance in satoshis.
    pub local_balance: i64,
    /// Remote balance in satoshis.
    pub remote_balance: i64,
    /// Whether the channel is active.
    pub active: bool,
}

/// Information about an LND node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndNodeInfo {
    /// Node alias.
    pub alias: String,
    /// Node version.
    pub version: String,
    /// Public key / identity pubkey.
    pub identity_pubkey: String,
    /// Number of active channels.
    pub num_active_channels: u32,
    /// Number of pending channels.
    pub num_pending_channels: u32,
    /// Number of peers.
    pub num_peers: u32,
    /// Is synced to chain.
    pub synced_to_chain: bool,
    /// Is synced to graph.
    pub synced_to_graph: bool,
    /// Block height.
    pub block_height: u32,
    /// Block hash.
    pub block_hash: String,
    /// Wallet balance (satoshis).
    pub wallet_balance: i64,
    /// Channel balance (satoshis).
    pub channel_balance: i64,
    /// REST API host:port.
    pub rest_host: String,
    /// gRPC host:port.
    pub grpc_host: String,
    /// List of active channels.
    pub channels: Vec<ChannelInfo>,
}

/// Unified node information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeInfo {
    /// Bitcoin Core node information.
    Bitcoin(BitcoinNodeInfo),
    /// LND node information.
    Lnd(LndNodeInfo),
}
