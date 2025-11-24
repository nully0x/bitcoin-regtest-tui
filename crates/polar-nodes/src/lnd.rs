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

    /// Get a new on-chain Bitcoin address for depositing funds.
    pub async fn get_new_address(&self, manager: &ContainerManager) -> Result<String> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("LND node not running".to_string()))?;

        let output = manager
            .exec_command(
                container_id,
                vec![
                    "lncli",
                    "--network=regtest",
                    "--tlscertpath=/home/lnd/.lnd/tls.cert",
                    "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                    "newaddress",
                    "p2wkh",
                ],
            )
            .await?;

        let json: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| polar_core::Error::Config(format!("Failed to parse address: {}", e)))?;

        let address = json["address"]
            .as_str()
            .ok_or_else(|| polar_core::Error::Config("No address in response".to_string()))?
            .to_string();

        Ok(address)
    }

    /// Get the identity public key of the LND node.
    pub async fn get_pubkey(&self, manager: &ContainerManager) -> Result<String> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("LND node not running".to_string()))?;

        let output = manager
            .exec_command(
                container_id,
                vec![
                    "lncli",
                    "--network=regtest",
                    "--tlscertpath=/home/lnd/.lnd/tls.cert",
                    "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                    "getinfo",
                ],
            )
            .await?;

        let json: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| polar_core::Error::Config(format!("Failed to parse getinfo: {}", e)))?;

        let pubkey = json["identity_pubkey"]
            .as_str()
            .ok_or_else(|| polar_core::Error::Config("No pubkey in response".to_string()))?
            .to_string();

        Ok(pubkey)
    }

    /// Connect to another LND node as a peer.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `peer_pubkey` - Public key of the peer node
    /// * `peer_host` - Host address of the peer (format: "host:port")
    pub async fn connect_peer(
        &self,
        manager: &ContainerManager,
        peer_pubkey: &str,
        peer_host: &str,
    ) -> Result<()> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("LND node not running".to_string()))?;

        let peer_address = format!("{}@{}", peer_pubkey, peer_host);

        manager
            .exec_command(
                container_id,
                vec![
                    "lncli",
                    "--network=regtest",
                    "--tlscertpath=/home/lnd/.lnd/tls.cert",
                    "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                    "connect",
                    &peer_address,
                ],
            )
            .await?;

        Ok(())
    }

    /// Open a Lightning channel to another node.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `peer_pubkey` - Public key of the peer to open channel with
    /// * `amount` - Channel capacity in satoshis
    /// * `push_amount` - Amount to push to peer in satoshis (optional)
    pub async fn open_channel(
        &self,
        manager: &ContainerManager,
        peer_pubkey: &str,
        amount: u64,
        push_amount: Option<u64>,
    ) -> Result<String> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("LND node not running".to_string()))?;

        let amount_str = amount.to_string();
        let push_str = push_amount.map(|p| p.to_string());

        let mut args = vec![
            "lncli",
            "--network=regtest",
            "--tlscertpath=/home/lnd/.lnd/tls.cert",
            "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
            "openchannel",
            peer_pubkey,
            &amount_str,
        ];

        if let Some(ref push) = push_str {
            args.push(push);
        }

        let output = manager.exec_command(container_id, args).await?;

        // Parse the funding txid from the output
        let json: serde_json::Value = serde_json::from_str(&output).map_err(|e| {
            polar_core::Error::Config(format!(
                "Failed to parse channel open response: {}. Output was: {}",
                e, output
            ))
        })?;

        let funding_txid = json["funding_txid"]
            .as_str()
            .ok_or_else(|| {
                polar_core::Error::Config(format!(
                    "No funding_txid in response. Full response: {}",
                    output
                ))
            })?
            .to_string();

        Ok(funding_txid)
    }

    /// Create an invoice for receiving payment.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `amount` - Amount in satoshis
    /// * `memo` - Optional description for the invoice
    pub async fn create_invoice(
        &self,
        manager: &ContainerManager,
        amount: u64,
        memo: Option<&str>,
    ) -> Result<String> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("LND node not running".to_string()))?;

        let amount_str = amount.to_string();
        let memo_str = memo.map(|m| m.to_string());

        let mut args = vec![
            "lncli",
            "--network=regtest",
            "--tlscertpath=/home/lnd/.lnd/tls.cert",
            "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
            "addinvoice",
            "--json", // Add JSON flag for parseable output
            "--amt",
            &amount_str,
        ];

        if let Some(ref m) = memo_str {
            args.push("--memo");
            args.push(m);
        }

        let output = manager.exec_command(container_id, args).await?;

        let json: serde_json::Value = serde_json::from_str(&output).map_err(|e| {
            polar_core::Error::Config(format!(
                "Failed to parse invoice: {}. Output was: {}",
                e, output
            ))
        })?;

        let payment_request = json["payment_request"]
            .as_str()
            .ok_or_else(|| {
                polar_core::Error::Config(format!(
                    "No payment_request in response. Full response: {}",
                    output
                ))
            })?
            .to_string();

        Ok(payment_request)
    }

    /// Pay a Lightning invoice.
    ///
    /// # Arguments
    /// * `manager` - Docker container manager
    /// * `payment_request` - The bolt11 invoice string
    pub async fn pay_invoice(
        &self,
        manager: &ContainerManager,
        payment_request: &str,
    ) -> Result<String> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("LND node not running".to_string()))?;

        let output = manager
            .exec_command(
                container_id,
                vec![
                    "lncli",
                    "--network=regtest",
                    "--tlscertpath=/home/lnd/.lnd/tls.cert",
                    "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                    "payinvoice",
                    "--json", // Add JSON flag for parseable output
                    "--force",
                    payment_request,
                ],
            )
            .await?;

        let json: serde_json::Value = serde_json::from_str(&output).map_err(|e| {
            polar_core::Error::Config(format!(
                "Failed to parse payment response: {}. Output was: {}",
                e, output
            ))
        })?;

        let payment_hash = json["payment_hash"]
            .as_str()
            .ok_or_else(|| {
                polar_core::Error::Config(format!(
                    "No payment_hash in response. Full response: {}",
                    output
                ))
            })?
            .to_string();

        Ok(payment_hash)
    }

    /// List all channels for this node.
    pub async fn list_channels(&self, manager: &ContainerManager) -> Result<serde_json::Value> {
        let container_id = self
            .node
            .container_id
            .as_ref()
            .ok_or_else(|| polar_core::Error::Config("LND node not running".to_string()))?;

        let output = manager
            .exec_command(
                container_id,
                vec![
                    "lncli",
                    "--network=regtest",
                    "--tlscertpath=/home/lnd/.lnd/tls.cert",
                    "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                    "listchannels",
                ],
            )
            .await?;

        let json: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| polar_core::Error::Config(format!("Failed to parse channels: {}", e)))?;

        Ok(json)
    }
}
