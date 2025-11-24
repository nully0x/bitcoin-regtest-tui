//! Bitcoin Core node implementation.

use polar_core::{Node, NodeKind, Result};
use polar_docker::{ContainerManager, PortMap};

/// Available Bitcoin Core versions.
pub const BITCOIN_VERSIONS: &[&str] = &[
    "polarlightning/bitcoind:28.0",
    "polarlightning/bitcoind:27.0",
    "polarlightning/bitcoind:26.0",
];

/// Bitcoin Core node configuration and management.
pub struct BitcoinNode {
    /// The underlying node data.
    pub node: Node,
    /// Docker image to use.
    pub image: String,
}

impl BitcoinNode {
    /// Default Bitcoin Core image.
    pub const DEFAULT_IMAGE: &'static str = "polarlightning/bitcoind:28.0";

    /// Create a new Bitcoin Core node.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            node: Node::new(name, NodeKind::BitcoinCore),
            image: Self::DEFAULT_IMAGE.to_string(),
        }
    }

    /// Start the Bitcoin Core container.
    pub async fn start(&mut self, manager: &ContainerManager) -> Result<()> {
        self.start_with_network(manager, None).await
    }

    /// Start the Bitcoin Core container on a specific Docker network.
    pub async fn start_with_network(
        &mut self,
        manager: &ContainerManager,
        network: Option<&str>,
    ) -> Result<()> {
        self.start_with_ports(manager, network, None).await
    }

    /// Start the Bitcoin Core container with custom port mappings.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `network` - Optional Docker network name
    /// * `ports` - Optional port configuration (rpc, p2p, zmq_block, zmq_tx)
    pub async fn start_with_ports(
        &mut self,
        manager: &ContainerManager,
        network: Option<&str>,
        ports: Option<(u16, u16, u16, u16)>,
    ) -> Result<()> {
        // Ensure the image exists locally
        manager.ensure_image(&self.image).await?;

        let container_name = format!("polar-btc-{}", self.node.id);

        let cmd = vec![
            "bitcoind".to_string(),
            "-regtest".to_string(),
            "-server".to_string(),
            "-rpcuser=polaruser".to_string(),
            "-rpcpassword=polarpass".to_string(),
            "-rpcallowip=0.0.0.0/0".to_string(),
            "-rpcbind=0.0.0.0".to_string(),
            "-zmqpubrawblock=tcp://0.0.0.0:28334".to_string(),
            "-zmqpubrawtx=tcp://0.0.0.0:28335".to_string(),
        ];

        // Configure port mappings if ports are provided
        let port_map = ports.map(|(rpc_port, p2p_port, zmq_block_port, zmq_tx_port)| {
            PortMap::from(vec![
                (18443, rpc_port),       // RPC port
                (18444, p2p_port),       // P2P port
                (28334, zmq_block_port), // ZMQ block port
                (28335, zmq_tx_port),    // ZMQ tx port
            ])
        });

        let container_id = manager
            .create_container_with_config(
                &container_name,
                &self.image,
                Some(cmd),
                port_map,
                network,
            )
            .await?;

        manager.start_container(&container_id).await?;
        self.node.container_id = Some(container_id);

        Ok(())
    }

    /// Stop the Bitcoin Core container.
    pub async fn stop(&mut self, manager: &ContainerManager) -> Result<()> {
        if let Some(container_id) = &self.node.container_id {
            manager.stop_container(container_id).await?;
            manager.remove_container(container_id).await?;
            self.node.container_id = None;
        }
        Ok(())
    }
}
