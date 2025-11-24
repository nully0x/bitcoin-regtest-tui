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
            "-fallbackfee=0.00001".to_string(), // Enable fallback fee for regtest
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
        self.node.container_id = Some(container_id.clone());

        // Wait a bit for bitcoind to fully start
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Create a default wallet (required for Bitcoin Core 28.0+)
        // This will fail if wallet already exists, which is fine - we'll ignore that error
        let _ = manager
            .exec_command(
                &container_id,
                vec![
                    "bitcoin-cli",
                    "-regtest",
                    "-rpcuser=polaruser",
                    "-rpcpassword=polarpass",
                    "createwallet",
                    "default",
                ],
            )
            .await;

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

    /// Mine blocks to a specific address.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `blocks` - Number of blocks to mine
    /// * `address` - Optional Bitcoin address (will generate one if not provided)
    pub async fn mine_blocks(
        &self,
        manager: &ContainerManager,
        blocks: u32,
        address: Option<&str>,
    ) -> Result<Vec<String>> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("Bitcoin node not running".to_string()))?;

        // Get or create an address to mine to
        let mining_address = if let Some(addr) = address {
            eprintln!(
                "[DEBUG] BitcoinNode::mine_blocks: Using provided address: {}",
                addr
            );
            addr.to_string()
        } else {
            // Generate a new address
            eprintln!("[DEBUG] BitcoinNode::mine_blocks: Generating new address");
            let output = manager
                .exec_command(
                    container_id,
                    vec![
                        "bitcoin-cli",
                        "-regtest",
                        "-rpcuser=polaruser",
                        "-rpcpassword=polarpass",
                        "getnewaddress",
                    ],
                )
                .await
                .map_err(|e| {
                    if e.to_string().contains("No wallet is loaded") {
                        polar_core::Error::Config(
                            "No wallet loaded. Try restarting the network.".to_string(),
                        )
                    } else {
                        e
                    }
                })?;
            let addr = output.trim().to_string();
            eprintln!(
                "[DEBUG] BitcoinNode::mine_blocks: Generated address: {}",
                addr
            );
            addr
        };

        // Mine the blocks
        eprintln!(
            "[DEBUG] BitcoinNode::mine_blocks: Mining {} blocks to address {}",
            blocks, mining_address
        );
        let output = manager
            .exec_command(
                container_id,
                vec![
                    "bitcoin-cli",
                    "-regtest",
                    "-rpcuser=polaruser",
                    "-rpcpassword=polarpass",
                    "generatetoaddress",
                    &blocks.to_string(),
                    &mining_address,
                ],
            )
            .await?;

        eprintln!("[DEBUG] BitcoinNode::mine_blocks: Raw output: {}", output);

        // Parse the block hashes from the output
        let block_hashes: Vec<String> = serde_json::from_str(&output).map_err(|e| {
            eprintln!(
                "[ERROR] BitcoinNode::mine_blocks: Failed to parse output as JSON: {}",
                e
            );
            polar_core::Error::Config(format!(
                "Failed to parse block hashes: {}. Output was: {}",
                e, output
            ))
        })?;

        eprintln!(
            "[DEBUG] BitcoinNode::mine_blocks: Successfully mined {} blocks",
            block_hashes.len()
        );
        Ok(block_hashes)
    }

    /// Get a new Bitcoin address from the node's wallet.
    pub async fn get_new_address(&self, manager: &ContainerManager) -> Result<String> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("Bitcoin node not running".to_string()))?;

        let output = manager
            .exec_command(
                container_id,
                vec![
                    "bitcoin-cli",
                    "-regtest",
                    "-rpcuser=polaruser",
                    "-rpcpassword=polarpass",
                    "getnewaddress",
                ],
            )
            .await?;

        Ok(output.trim().to_string())
    }

    /// Send Bitcoin to an address.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `address` - Destination address
    /// * `amount` - Amount in BTC
    pub async fn send_to_address(
        &self,
        manager: &ContainerManager,
        address: &str,
        amount: f64,
    ) -> Result<String> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("Bitcoin node not running".to_string()))?;

        let output = manager
            .exec_command(
                container_id,
                vec![
                    "bitcoin-cli",
                    "-regtest",
                    "-rpcuser=polaruser",
                    "-rpcpassword=polarpass",
                    "sendtoaddress",
                    address,
                    &amount.to_string(),
                ],
            )
            .await?;

        Ok(output.trim().to_string())
    }

    /// Get the wallet balance.
    pub async fn get_balance(&self, manager: &ContainerManager) -> Result<f64> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("Bitcoin node not running".to_string()))?;

        let output = manager
            .exec_command(
                container_id,
                vec![
                    "bitcoin-cli",
                    "-regtest",
                    "-rpcuser=polaruser",
                    "-rpcpassword=polarpass",
                    "getbalance",
                ],
            )
            .await?;

        let balance: f64 = output
            .trim()
            .parse()
            .map_err(|e| polar_core::Error::Config(format!("Failed to parse balance: {}", e)))?;

        Ok(balance)
    }
}
