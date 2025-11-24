//! Network and node types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A Lightning Network development environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Network status.
    pub status: NetworkStatus,
    /// Nodes in this network.
    pub nodes: Vec<Node>,
    /// LND Docker image version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lnd_version: Option<String>,
    /// Bitcoin Core Docker image version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub btc_version: Option<String>,
    /// Node alias prefix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias_prefix: Option<String>,
    /// Port mappings for nodes (node_id -> PortConfig)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub port_mappings: HashMap<Uuid, PortConfig>,
}

/// Port configuration for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    /// Ports specific to this node type
    pub ports: NodePorts,
}

/// Port mappings for different node types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NodePorts {
    /// Bitcoin Core ports
    BitcoinCore {
        /// RPC port (host -> container 18443)
        rpc: u16,
        /// P2P port (host -> container 18444)
        p2p: u16,
        /// ZMQ raw block port (host -> container 28334)
        zmq_block: u16,
        /// ZMQ raw tx port (host -> container 28335)
        zmq_tx: u16,
    },
    /// LND ports
    Lnd {
        /// REST API port (host -> container 8080)
        rest: u16,
        /// gRPC API port (host -> container 10009)
        grpc: u16,
        /// P2P/Peer port (host -> container 9735)
        p2p: u16,
    },
}

impl Network {
    /// Create a new network with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            status: NetworkStatus::Stopped,
            nodes: Vec::new(),
            lnd_version: None,
            btc_version: None,
            alias_prefix: None,
            port_mappings: HashMap::new(),
        }
    }

    /// Add a node to this network.
    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Allocate ports for a new node, avoiding conflicts with existing nodes.
    pub fn allocate_ports(&mut self, node_id: Uuid, kind: NodeKind) -> PortConfig {
        let base_port = self.find_next_available_base_port();

        let ports = match kind {
            NodeKind::BitcoinCore => NodePorts::BitcoinCore {
                rpc: base_port,
                p2p: base_port + 1,
                zmq_block: base_port + 2,
                zmq_tx: base_port + 3,
            },
            NodeKind::Lnd => NodePorts::Lnd {
                rest: base_port,
                grpc: base_port + 1,
                p2p: base_port + 2,
            },
        };

        let config = PortConfig { ports };
        self.port_mappings.insert(node_id, config.clone());
        config
    }

    /// Find the next available base port by checking all allocated ports.
    fn find_next_available_base_port(&self) -> u16 {
        const PORT_RANGE_START: u16 = 20000;
        const PORT_INCREMENT: u16 = 10; // Reserve 10 ports per node

        let max_port = self
            .port_mappings
            .values()
            .flat_map(|config| config.get_all_ports())
            .max()
            .unwrap_or(PORT_RANGE_START - PORT_INCREMENT);

        // Round up to next increment
        ((max_port / PORT_INCREMENT) + 1) * PORT_INCREMENT
    }
}

impl PortConfig {
    /// Get all ports allocated to this node.
    pub fn get_all_ports(&self) -> Vec<u16> {
        match &self.ports {
            NodePorts::BitcoinCore {
                rpc,
                p2p,
                zmq_block,
                zmq_tx,
            } => {
                vec![*rpc, *p2p, *zmq_block, *zmq_tx]
            }
            NodePorts::Lnd { rest, grpc, p2p } => {
                vec![*rest, *grpc, *p2p]
            }
        }
    }
}

/// Status of a network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkStatus {
    /// Network is stopped.
    Stopped,
    /// Network is starting.
    Starting,
    /// Network is running.
    Running,
    /// Network is stopping.
    Stopping,
    /// Network encountered an error.
    Error,
}

/// A node in a Lightning Network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier.
    pub id: Uuid,
    /// Node name.
    pub name: String,
    /// Node type.
    pub kind: NodeKind,
    /// Docker container ID (if running).
    pub container_id: Option<String>,
}

impl Node {
    /// Create a new node.
    pub fn new(name: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            container_id: None,
        }
    }
}

/// Lightning implementation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightningImpl {
    /// LND (Lightning Network Daemon).
    Lnd,
    // Future: CoreLightning, Eclair, etc.
}

impl LightningImpl {
    /// Get all available Lightning implementations.
    pub fn all() -> &'static [LightningImpl] {
        &[LightningImpl::Lnd]
    }

    /// Get the short name for this implementation.
    pub fn short_name(&self) -> &'static str {
        match self {
            LightningImpl::Lnd => "lnd",
        }
    }
}

impl std::fmt::Display for LightningImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightningImpl::Lnd => write!(f, "LND"),
        }
    }
}

/// Type of node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Bitcoin Core node.
    BitcoinCore,
    /// LND Lightning node.
    Lnd,
}

impl NodeKind {
    /// Check if this node is a Lightning implementation.
    pub fn is_lightning(&self) -> bool {
        matches!(self, NodeKind::Lnd)
    }
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeKind::BitcoinCore => write!(f, "Bitcoin Core"),
            NodeKind::Lnd => write!(f, "LND"),
        }
    }
}
