//! LND node implementation.

use polar_core::{Node, NodeKind, Result};
use polar_docker::{ContainerManager, PortMap};

/// Available LND versions.
pub const LND_VERSIONS: &[&str] = &[
    "polarlightning/lnd:0.18.5-beta",
    "polarlightning/lnd:0.18.3-beta",
    "polarlightning/lnd:0.17.5-beta",
    "polarlightning/lnd:0.16.4-beta",
];

/// LND Lightning node configuration and management.
pub struct LndNode {
    /// The underlying node data.
    pub node: Node,
    /// Docker image to use.
    pub image: String,
    /// Bitcoin backend node name.
    pub bitcoin_node: String,
    /// Node alias.
    pub alias: String,
}

impl LndNode {
    /// Default LND image.
    pub const DEFAULT_IMAGE: &'static str = "polarlightning/lnd:0.18.5-beta";

    /// Create a new LND node.
    pub fn new(name: impl Into<String>, bitcoin_node: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            node: Node::new(name.clone(), NodeKind::Lnd),
            image: Self::DEFAULT_IMAGE.to_string(),
            bitcoin_node: bitcoin_node.into(),
            alias: name, // Default alias is the node name
        }
    }

    /// Create a new LND node with custom alias.
    pub fn with_alias(
        name: impl Into<String>,
        bitcoin_node: impl Into<String>,
        alias: impl Into<String>,
    ) -> Self {
        Self {
            node: Node::new(name, NodeKind::Lnd),
            image: Self::DEFAULT_IMAGE.to_string(),
            bitcoin_node: bitcoin_node.into(),
            alias: alias.into(),
        }
    }

    /// Set a custom image version.
    pub fn with_image(mut self, image: impl Into<String>) -> Self {
        self.image = image.into();
        self
    }

    /// Start the LND container.
    pub async fn start(&mut self, manager: &ContainerManager) -> Result<()> {
        self.start_with_network(manager, None).await
    }

    /// Start the LND container on a specific Docker network.
    pub async fn start_with_network(
        &mut self,
        manager: &ContainerManager,
        network: Option<&str>,
    ) -> Result<()> {
        self.start_with_ports(manager, network, None).await
    }

    /// Start the LND container with custom port mappings.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `network` - Optional Docker network name
    /// * `ports` - Optional port configuration (rest, grpc, p2p)
    pub async fn start_with_ports(
        &mut self,
        manager: &ContainerManager,
        network: Option<&str>,
        ports: Option<(u16, u16, u16)>,
    ) -> Result<()> {
        // Ensure the image exists locally
        manager.ensure_image(&self.image).await?;

        let container_name = format!("polar-lnd-{}", self.node.id);

        let cmd = vec![
            "lnd".to_string(),
            "--noseedbackup".to_string(),
            "--trickledelay=5000".to_string(),
            format!("--alias={}", self.alias),
            "--debuglevel=info".to_string(),
            "--bitcoin.active".to_string(),
            "--bitcoin.regtest".to_string(),
            "--bitcoin.node=bitcoind".to_string(),
            format!("--bitcoind.rpchost=polar-btc-{}", self.bitcoin_node),
            "--bitcoind.rpcuser=polaruser".to_string(),
            "--bitcoind.rpcpass=polarpass".to_string(),
            format!(
                "--bitcoind.zmqpubrawblock=tcp://polar-btc-{}:28334",
                self.bitcoin_node
            ),
            format!(
                "--bitcoind.zmqpubrawtx=tcp://polar-btc-{}:28335",
                self.bitcoin_node
            ),
        ];

        // Configure port mappings if ports are provided
        let port_map = ports.map(|(rest_port, grpc_port, p2p_port)| {
            PortMap::from(vec![
                (8080, rest_port),  // REST API port
                (10009, grpc_port), // gRPC API port
                (9735, p2p_port),   // P2P/Peer port
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

    /// Stop the LND container.
    pub async fn stop(&mut self, manager: &ContainerManager) -> Result<()> {
        if let Some(container_id) = &self.node.container_id {
            manager.stop_container(container_id).await?;
            manager.remove_container(container_id).await?;
            self.node.container_id = None;
        }
        Ok(())
    }
}
