//! Network lifecycle management.

use polar_core::{
    BitcoinNodeInfo, Config, Error, LightningImpl, LndNodeInfo, Network, NetworkStatus, Node,
    NodeInfo, NodeKind, NodePorts, Result,
};
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Manages network lifecycle and operations.
pub struct NetworkManager {
    /// Docker container manager.
    container_manager: ContainerManager,
    /// Active networks.
    networks: HashMap<String, Network>,
    /// Configuration.
    config: Config,
    /// Log channel sender (optional).
    log_tx: Option<mpsc::UnboundedSender<String>>,
}

impl NetworkManager {
    /// Create a new network manager.
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        let mut manager = Self {
            container_manager: ContainerManager::new()?,
            networks: HashMap::new(),
            config,
            log_tx: None,
        };

        // Load existing networks from disk
        if let Err(e) = manager.load_networks() {
            // Can't log here yet since logger isn't set up
            eprintln!("Warning: Failed to load networks: {}", e);
        }

        Ok(manager)
    }

    /// Set the log channel sender.
    pub fn set_logger(&mut self, log_tx: mpsc::UnboundedSender<String>) {
        self.log_tx = Some(log_tx);
    }

    /// Log a message.
    fn log(&self, message: impl Into<String>) {
        if let Some(tx) = &self.log_tx {
            let _ = tx.send(message.into());
        }
    }

    /// Get the networks directory path.
    fn networks_dir(&self) -> PathBuf {
        self.config.data_dir.join("networks")
    }

    /// Get the path to a network file.
    fn network_file_path(&self, network_id: &str) -> PathBuf {
        self.networks_dir().join(format!("{}.json", network_id))
    }

    /// Save all networks to disk.
    fn save_networks(&self) -> Result<()> {
        let networks_dir = self.networks_dir();
        std::fs::create_dir_all(&networks_dir)?;

        for network in self.networks.values() {
            self.save_network(network)?;
        }

        Ok(())
    }

    /// Save a single network to disk.
    fn save_network(&self, network: &Network) -> Result<()> {
        let networks_dir = self.networks_dir();
        std::fs::create_dir_all(&networks_dir)?;

        let file_path = self.network_file_path(&network.id.to_string());
        let content = serde_json::to_string_pretty(network)?;
        std::fs::write(&file_path, content)?;

        Ok(())
    }

    /// Load all networks from disk.
    fn load_networks(&mut self) -> Result<()> {
        let networks_dir = self.networks_dir();

        if !networks_dir.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&networks_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_network(&path) {
                    Ok(network) => {
                        self.networks.insert(network.name.clone(), network);
                    }
                    Err(e) => {
                        self.log(format!(
                            "Warning: Failed to load network from {:?}: {}",
                            path, e
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single network from a file.
    fn load_network(&self, path: &PathBuf) -> Result<Network> {
        let content = std::fs::read_to_string(path)?;
        let network: Network = serde_json::from_str(&content)?;
        Ok(network)
    }

    /// Delete a network from disk.
    fn delete_network_file(&self, network_id: &str) -> Result<()> {
        let file_path = self.network_file_path(network_id);

        if file_path.exists() {
            std::fs::remove_file(&file_path)?;
        }

        Ok(())
    }

    /// Create a new network with default nodes.
    pub fn create_network(&mut self, name: impl Into<String>) -> Result<()> {
        self.create_network_with_config(
            name,
            2,
            "polar-node",
            polar_nodes::LndNode::DEFAULT_IMAGE,
            polar_nodes::BitcoinNode::DEFAULT_IMAGE,
        )
    }

    /// Create a new network with custom configuration.
    pub fn create_network_with_config(
        &mut self,
        name: impl Into<String>,
        lnd_count: usize,
        alias_prefix: &str,
        lnd_version: &str,
        btc_version: &str,
    ) -> Result<()> {
        let name = name.into();

        if self.networks.contains_key(&name) {
            return Err(Error::Config(format!("Network '{}' already exists", name)));
        }

        let mut network = Network::new(name.clone());

        // Store versions and alias
        network.lnd_version = Some(lnd_version.to_string());
        network.btc_version = Some(btc_version.to_string());
        network.alias_prefix = Some(alias_prefix.to_string());

        // Add a Bitcoin Core node
        let btc_node = Node::new("bitcoin-1", NodeKind::BitcoinCore);
        network.add_node(btc_node);

        // Add LND nodes
        for i in 1..=lnd_count {
            let lnd_node = Node::new(format!("lnd-{}", i), NodeKind::Lnd);
            network.add_node(lnd_node);
        }

        self.networks.insert(name.clone(), network.clone());

        // Persist the network to disk
        self.save_network(&network)?;

        Ok(())
    }

    /// Start a network.
    pub async fn start_network(&mut self, name: &str) -> Result<()> {
        let network = self
            .networks
            .get_mut(name)
            .ok_or_else(|| Error::NetworkNotFound(name.to_string()))?;

        if network.status == NetworkStatus::Running {
            return Ok(());
        }

        network.status = NetworkStatus::Starting;

        // Create a Docker network for this polar network
        let docker_network_name = format!("polar-{}", network.id);
        self.container_manager
            .create_network(&docker_network_name)
            .await?;

        // Get stored versions and alias
        let btc_version = network
            .btc_version
            .clone()
            .unwrap_or_else(|| BitcoinNode::DEFAULT_IMAGE.to_string());
        let lnd_version = network
            .lnd_version
            .clone()
            .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string());
        let alias_prefix = network
            .alias_prefix
            .clone()
            .unwrap_or_else(|| "polar-node".to_string());

        // Allocate ports for all nodes that don't have them yet
        let nodes_needing_ports: Vec<_> = network
            .nodes
            .iter()
            .filter(|n| !network.port_mappings.contains_key(&n.id))
            .map(|n| (n.id, n.kind))
            .collect();

        for (node_id, node_kind) in nodes_needing_ports {
            network.allocate_ports(node_id, node_kind);
        }

        // Start Bitcoin Core nodes first
        for node in &mut network.nodes {
            if node.kind == NodeKind::BitcoinCore {
                let mut btc_node = BitcoinNode::new(node.name.clone());
                btc_node.node.id = node.id;
                btc_node.image = btc_version.clone();

                // Get the allocated port configuration
                let port_config = network.port_mappings.get(&node.id).unwrap().clone();

                // Extract Bitcoin Core ports
                let ports = match &port_config.ports {
                    NodePorts::BitcoinCore {
                        rpc,
                        p2p,
                        zmq_block,
                        zmq_tx,
                    } => Some((*rpc, *p2p, *zmq_block, *zmq_tx)),
                    _ => None,
                };

                match btc_node
                    .start_with_ports(&self.container_manager, Some(&docker_network_name), ports)
                    .await
                {
                    Ok(_) => {
                        node.container_id = btc_node.node.container_id;
                    }
                    Err(e) => {
                        network.status = NetworkStatus::Error;
                        return Err(e);
                    }
                }
            }
        }

        // Wait a bit for Bitcoin Core to be ready
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Find the Bitcoin node ID first
        let btc_node_id = network
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::BitcoinCore)
            .map(|n| n.id.to_string())
            .ok_or_else(|| Error::Config("No Bitcoin node found in network".to_string()))?;

        // Then start LND nodes with custom aliases
        let mut lnd_counter = 1;
        for node in &mut network.nodes {
            if node.kind == NodeKind::Lnd {
                let node_alias = format!("{}-{}", alias_prefix, lnd_counter);
                let mut lnd_node =
                    LndNode::with_alias(node.name.clone(), btc_node_id.clone(), node_alias);
                lnd_node.node.id = node.id;
                lnd_node.image = lnd_version.clone();

                // Get the allocated port configuration
                let port_config = network.port_mappings.get(&node.id).unwrap().clone();

                // Extract LND ports
                let ports = match &port_config.ports {
                    NodePorts::Lnd { rest, grpc, p2p } => Some((*rest, *grpc, *p2p)),
                    _ => None,
                };

                match lnd_node
                    .start_with_ports(&self.container_manager, Some(&docker_network_name), ports)
                    .await
                {
                    Ok(_) => {
                        node.container_id = lnd_node.node.container_id;
                    }
                    Err(e) => {
                        network.status = NetworkStatus::Error;
                        return Err(e);
                    }
                }
                lnd_counter += 1;
            }
        }

        network.status = NetworkStatus::Running;

        // Clone network for persistence to avoid borrow issues
        let network_clone = network.clone();
        self.save_network(&network_clone)?;

        Ok(())
    }

    /// Stop a network.
    pub async fn stop_network(&mut self, name: &str) -> Result<()> {
        let network = self
            .networks
            .get_mut(name)
            .ok_or_else(|| Error::NetworkNotFound(name.to_string()))?;

        if network.status == NetworkStatus::Stopped {
            return Ok(());
        }

        network.status = NetworkStatus::Stopping;

        // Stop LND nodes first
        for node in &mut network.nodes {
            if node.kind == NodeKind::Lnd {
                if let Some(container_id) = &node.container_id {
                    self.container_manager.stop_container(container_id).await?;
                    self.container_manager
                        .remove_container(container_id)
                        .await?;
                    node.container_id = None;
                }
            }
        }

        // Then stop Bitcoin Core nodes
        for node in &mut network.nodes {
            if node.kind == NodeKind::BitcoinCore {
                if let Some(container_id) = &node.container_id {
                    self.container_manager.stop_container(container_id).await?;
                    self.container_manager
                        .remove_container(container_id)
                        .await?;
                    node.container_id = None;
                }
            }
        }

        network.status = NetworkStatus::Stopped;

        // Clone network for persistence to avoid borrow issues
        let network_clone = network.clone();

        // Remove the Docker network
        let docker_network_name = format!("polar-{}", network_clone.id);
        if let Err(e) = self
            .container_manager
            .remove_network(&docker_network_name)
            .await
        {
            // Log but don't fail - network might not exist
            self.log(format!(
                "Warning: Failed to remove network {}: {}",
                docker_network_name, e
            ));
        }

        self.save_network(&network_clone)?;

        Ok(())
    }

    /// Get all networks.
    pub fn networks(&self) -> &HashMap<String, Network> {
        &self.networks
    }

    /// Get a network by name.
    pub fn get_network(&self, name: &str) -> Option<&Network> {
        self.networks.get(name)
    }

    /// Get a mutable reference to a network by name.
    pub fn get_network_mut(&mut self, name: &str) -> Option<&mut Network> {
        self.networks.get_mut(name)
    }

    /// Delete a network.
    pub async fn delete_network(&mut self, name: &str) -> Result<()> {
        // Check if network exists and get its status and ID
        let (should_stop, network_id) = if let Some(network) = self.networks.get(name) {
            (
                network.status == NetworkStatus::Running,
                network.id.to_string(),
            )
        } else {
            return Ok(());
        };

        // Stop the network first if it's running
        if should_stop {
            self.stop_network(name).await?;
        }

        // Remove from in-memory map
        self.networks.remove(name);

        // Delete the network file from disk
        self.delete_network_file(&network_id)?;

        Ok(())
    }

    /// Get information about a Bitcoin Core node.
    pub async fn get_bitcoin_node_info(&self, container_id: &str) -> Result<BitcoinNodeInfo> {
        // Execute bitcoin-cli getblockchaininfo
        let blockchain_info = self
            .container_manager
            .exec_command(
                container_id,
                vec![
                    "bitcoin-cli",
                    "-regtest",
                    "-rpcuser=polaruser",
                    "-rpcpassword=polarpass",
                    "getblockchaininfo",
                ],
            )
            .await?;

        // Execute bitcoin-cli getnetworkinfo
        let network_info = self
            .container_manager
            .exec_command(
                container_id,
                vec![
                    "bitcoin-cli",
                    "-regtest",
                    "-rpcuser=polaruser",
                    "-rpcpassword=polarpass",
                    "getnetworkinfo",
                ],
            )
            .await?;

        // Execute bitcoin-cli getbalance
        let balance_info = self
            .container_manager
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

        // Parse JSON responses
        let blockchain_json: serde_json::Value = serde_json::from_str(&blockchain_info)
            .map_err(|e| Error::Config(format!("Failed to parse blockchain info: {}", e)))?;

        let network_json: serde_json::Value = serde_json::from_str(&network_info)
            .map_err(|e| Error::Config(format!("Failed to parse network info: {}", e)))?;

        // Get container info for ports
        let container_info = self
            .container_manager
            .inspect_container(container_id)
            .await?;

        let ports = container_info
            .network_settings
            .as_ref()
            .and_then(|ns| ns.ports.as_ref())
            .cloned()
            .unwrap_or_default();

        // Extract RPC port (18443 for regtest)
        let rpc_host = ports
            .get("18443/tcp")
            .and_then(|bindings| bindings.as_ref())
            .and_then(|b| b.first())
            .map(|binding| {
                format!(
                    "{}:{}",
                    binding.host_ip.as_deref().unwrap_or("0.0.0.0"),
                    binding.host_port.as_deref().unwrap_or("18443")
                )
            })
            .unwrap_or_else(|| "18443".to_string());

        // Extract P2P port (18444 for regtest)
        let p2p_host = ports
            .get("18444/tcp")
            .and_then(|bindings| bindings.as_ref())
            .and_then(|b| b.first())
            .map(|binding| {
                format!(
                    "{}:{}",
                    binding.host_ip.as_deref().unwrap_or("0.0.0.0"),
                    binding.host_port.as_deref().unwrap_or("18444")
                )
            })
            .unwrap_or_else(|| "18444".to_string());

        // Parse balance
        let balance: f64 = balance_info.trim().parse().unwrap_or(0.0);

        Ok(BitcoinNodeInfo {
            version: network_json["subversion"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            blocks: blockchain_json["blocks"].as_u64().unwrap_or(0),
            chain: blockchain_json["chain"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            connections: network_json["connections"].as_u64().unwrap_or(0) as u32,
            difficulty: blockchain_json["difficulty"].as_f64().unwrap_or(0.0),
            ibd_complete: !blockchain_json["initialblockdownload"]
                .as_bool()
                .unwrap_or(true),
            balance,
            rpc_host,
            p2p_host,
        })
    }

    /// Get information about an LND node.
    pub async fn get_lnd_node_info(&self, container_id: &str) -> Result<LndNodeInfo> {
        // LND commands with proper network flag and TLS cert path
        let lncli_args = vec![
            "lncli",
            "--network=regtest",
            "--tlscertpath=/home/lnd/.lnd/tls.cert",
            "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
        ];

        // Execute lncli getinfo
        let mut getinfo_cmd = lncli_args.clone();
        getinfo_cmd.push("getinfo");
        let getinfo = self
            .container_manager
            .exec_command(container_id, getinfo_cmd)
            .await?;

        // Execute lncli walletbalance
        let mut wallet_cmd = lncli_args.clone();
        wallet_cmd.push("walletbalance");
        let wallet_balance = self
            .container_manager
            .exec_command(container_id, wallet_cmd)
            .await?;

        // Execute lncli channelbalance
        let mut channel_cmd = lncli_args.clone();
        channel_cmd.push("channelbalance");
        let channel_balance = self
            .container_manager
            .exec_command(container_id, channel_cmd)
            .await?;

        // Execute lncli listchannels
        let mut list_channels_cmd = lncli_args.clone();
        list_channels_cmd.push("listchannels");
        let list_channels = self
            .container_manager
            .exec_command(container_id, list_channels_cmd)
            .await?;

        // Parse JSON responses
        let info_json: serde_json::Value = serde_json::from_str(&getinfo)
            .map_err(|e| Error::Config(format!("Failed to parse getinfo: {}", e)))?;

        let wallet_json: serde_json::Value = serde_json::from_str(&wallet_balance)
            .map_err(|e| Error::Config(format!("Failed to parse wallet balance: {}", e)))?;

        let channel_json: serde_json::Value = serde_json::from_str(&channel_balance)
            .map_err(|e| Error::Config(format!("Failed to parse channel balance: {}", e)))?;

        let channels_json: serde_json::Value = serde_json::from_str(&list_channels)
            .map_err(|e| Error::Config(format!("Failed to parse channels list: {}", e)))?;

        // Parse channel list
        let channels = channels_json["channels"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|ch| polar_core::ChannelInfo {
                        channel_point: ch["channel_point"]
                            .as_str()
                            .unwrap_or("unknown")
                            .to_string(),
                        remote_pubkey: ch["remote_pubkey"]
                            .as_str()
                            .unwrap_or("unknown")
                            .to_string(),
                        capacity: ch["capacity"]
                            .as_str()
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(0),
                        local_balance: ch["local_balance"]
                            .as_str()
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(0),
                        remote_balance: ch["remote_balance"]
                            .as_str()
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(0),
                        active: ch["active"].as_bool().unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Get container info for ports
        let container_info = self
            .container_manager
            .inspect_container(container_id)
            .await?;

        let ports = container_info
            .network_settings
            .as_ref()
            .and_then(|ns| ns.ports.as_ref())
            .cloned()
            .unwrap_or_default();

        // Extract REST port (8080)
        let rest_host = ports
            .get("8080/tcp")
            .and_then(|bindings| bindings.as_ref())
            .and_then(|b| b.first())
            .map(|binding| {
                format!(
                    "{}:{}",
                    binding.host_ip.as_deref().unwrap_or("0.0.0.0"),
                    binding.host_port.as_deref().unwrap_or("8080")
                )
            })
            .unwrap_or_else(|| "8080".to_string());

        // Extract gRPC port (10009)
        let grpc_host = ports
            .get("10009/tcp")
            .and_then(|bindings| bindings.as_ref())
            .and_then(|b| b.first())
            .map(|binding| {
                format!(
                    "{}:{}",
                    binding.host_ip.as_deref().unwrap_or("0.0.0.0"),
                    binding.host_port.as_deref().unwrap_or("10009")
                )
            })
            .unwrap_or_else(|| "10009".to_string());

        Ok(LndNodeInfo {
            alias: info_json["alias"].as_str().unwrap_or("unknown").to_string(),
            version: info_json["version"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            identity_pubkey: info_json["identity_pubkey"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            num_active_channels: info_json["num_active_channels"].as_u64().unwrap_or(0) as u32,
            num_pending_channels: info_json["num_pending_channels"].as_u64().unwrap_or(0) as u32,
            num_peers: info_json["num_peers"].as_u64().unwrap_or(0) as u32,
            synced_to_chain: info_json["synced_to_chain"].as_bool().unwrap_or(false),
            synced_to_graph: info_json["synced_to_graph"].as_bool().unwrap_or(false),
            block_height: info_json["block_height"].as_u64().unwrap_or(0) as u32,
            block_hash: info_json["block_hash"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            wallet_balance: wallet_json["confirmed_balance"]
                .as_str()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0),
            channel_balance: channel_json["balance"]
                .as_str()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0),
            rest_host,
            grpc_host,
            channels,
        })
    }

    /// Get node information for any node type.
    pub async fn get_node_info(&self, network_name: &str, node_name: &str) -> Result<NodeInfo> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        let node = network
            .nodes
            .iter()
            .find(|n| n.name == node_name)
            .ok_or_else(|| Error::Config(format!("Node '{}' not found", node_name)))?;

        let container_id = node
            .container_id
            .as_ref()
            .ok_or_else(|| Error::Config("Node is not running".to_string()))?;

        match node.kind {
            NodeKind::BitcoinCore => {
                let info = self.get_bitcoin_node_info(container_id).await?;
                Ok(NodeInfo::Bitcoin(info))
            }
            NodeKind::Lnd => {
                let info = self.get_lnd_node_info(container_id).await?;
                Ok(NodeInfo::Lnd(info))
            }
        }
    }

    /// Add a new Lightning node to an existing network.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network to add the node to
    /// * `implementation` - Lightning implementation type (LND, Core Lightning, etc.)
    ///
    /// # Returns
    /// The name of the newly created node
    pub async fn add_lightning_node(
        &mut self,
        network_name: &str,
        implementation: LightningImpl,
    ) -> Result<String> {
        let network = self
            .networks
            .get_mut(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Determine the NodeKind based on implementation
        let node_kind = match implementation {
            LightningImpl::Lnd => NodeKind::Lnd,
            // Future implementations will be added here
        };

        // Count existing nodes of this implementation to generate unique name and alias
        let impl_count = network.nodes.iter().filter(|n| n.kind == node_kind).count();
        let next_number = impl_count + 1;

        // Create new Lightning node with implementation-specific naming
        let node_name = format!("{}-{}", implementation.short_name(), next_number);
        let lightning_node = Node::new(node_name.clone(), node_kind);
        network.add_node(lightning_node);

        // Check if network is running and get needed data
        let is_running = network.status == NetworkStatus::Running;
        let network_id = network.id;
        let alias_prefix = network
            .alias_prefix
            .clone()
            .unwrap_or_else(|| network_name.to_string());
        let lnd_version = network
            .lnd_version
            .clone()
            .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string());

        // If network is running, start the new node automatically
        if is_running {
            // Find the Bitcoin node ID
            let btc_node_id = network
                .nodes
                .iter()
                .find(|n| n.kind == NodeKind::BitcoinCore)
                .map(|n| n.id.to_string())
                .ok_or_else(|| Error::Config("No Bitcoin node found in network".to_string()))?;

            // Find the newly added node
            let new_node = network
                .nodes
                .iter_mut()
                .find(|n| n.name == node_name && n.kind == node_kind)
                .ok_or_else(|| Error::Config("Failed to find newly created node".to_string()))?;

            // Start the new Lightning node based on implementation
            match implementation {
                LightningImpl::Lnd => {
                    let node_alias = format!("{}-{}", alias_prefix, next_number);
                    let mut lnd_node =
                        LndNode::with_alias(node_name.clone(), btc_node_id, node_alias);
                    lnd_node.node.id = new_node.id;
                    lnd_node.image = lnd_version;

                    let docker_network_name = format!("polar-{}", network_id);
                    lnd_node
                        .start_with_network(&self.container_manager, Some(&docker_network_name))
                        .await?;

                    new_node.container_id = lnd_node.node.container_id;
                } // Future implementations will be added here
            }
        }

        // Save the updated network state (once at the end)
        let network_clone = network.clone();
        self.save_network(&network_clone)?;

        Ok(node_name)
    }

    /// Remove a Lightning node from a network.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    /// * `node_name` - Name of the node to remove
    ///
    /// # Returns
    /// Success or error
    pub async fn delete_lightning_node(
        &mut self,
        network_name: &str,
        node_name: &str,
    ) -> Result<()> {
        let network = self
            .networks
            .get_mut(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Find the node
        let node = network
            .nodes
            .iter()
            .find(|n| n.name == node_name)
            .ok_or_else(|| Error::Config(format!("Node '{}' not found", node_name)))?;

        // Don't allow deleting Bitcoin node
        if node.kind == NodeKind::BitcoinCore {
            return Err(Error::Config(
                "Cannot delete Bitcoin node. Delete the entire network instead.".to_string(),
            ));
        }

        // If node is running, stop it first
        if node.container_id.is_some() {
            let node_clone = node.clone();
            let node_kind = node.kind;

            match node_kind {
                NodeKind::Lnd => {
                    let mut lnd_node = LndNode {
                        node: node_clone,
                        image: network
                            .lnd_version
                            .clone()
                            .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
                        bitcoin_node: String::new(),
                        alias: String::new(),
                    };
                    lnd_node.stop(&self.container_manager).await?;
                }
                NodeKind::BitcoinCore => {
                    // Already checked above, but included for completeness
                    return Err(Error::Config("Cannot delete Bitcoin node".to_string()));
                }
            }
        }

        // Remove the node from the network
        network.nodes.retain(|n| n.name != node_name);

        // Save the updated network state
        let network_clone = network.clone();
        self.save_network(&network_clone)?;

        Ok(())
    }

    /// Check if Docker is available.
    pub async fn check_docker(&self) -> Result<()> {
        self.container_manager.ping().await
    }

    /// Mine blocks on the Bitcoin node in a network.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    /// * `num_blocks` - Number of blocks to mine (default: 100)
    pub async fn mine_blocks(&self, network_name: &str, num_blocks: u32) -> Result<Vec<String>> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Find the Bitcoin node
        let btc_node = network
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::BitcoinCore)
            .ok_or_else(|| Error::Config("No Bitcoin node found in network".to_string()))?;

        if btc_node.container_id.is_none() {
            return Err(Error::Config(
                "Bitcoin node is not running. Please start the network first.".to_string(),
            ));
        }

        let btc_node_obj = BitcoinNode {
            node: btc_node.clone(),
            image: network
                .btc_version
                .clone()
                .unwrap_or_else(|| BitcoinNode::DEFAULT_IMAGE.to_string()),
        };

        btc_node_obj
            .mine_blocks(&self.container_manager, num_blocks, None)
            .await
    }

    /// Fund an LND node's wallet from the Bitcoin node.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    /// * `lnd_node_name` - Name of the LND node to fund
    /// * `amount` - Amount in BTC
    /// * `auto_mine` - Whether to automatically mine blocks to confirm the transaction (default: true)
    ///
    /// # Returns
    /// The transaction ID of the funding transaction
    pub async fn fund_lnd_wallet(
        &self,
        network_name: &str,
        lnd_node_name: &str,
        amount: f64,
    ) -> Result<String> {
        self.fund_lnd_wallet_with_options(network_name, lnd_node_name, amount, true)
            .await
    }

    /// Fund an LND node's wallet from the Bitcoin node with custom options.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    /// * `lnd_node_name` - Name of the LND node to fund
    /// * `amount` - Amount in BTC
    /// * `auto_mine` - Whether to automatically mine blocks to confirm the transaction
    ///
    /// # Returns
    /// The transaction ID of the funding transaction
    pub async fn fund_lnd_wallet_with_options(
        &self,
        network_name: &str,
        lnd_node_name: &str,
        amount: f64,
        auto_mine: bool,
    ) -> Result<String> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Find the Bitcoin node
        let btc_node = network
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::BitcoinCore)
            .ok_or_else(|| Error::Config("No Bitcoin node found in network".to_string()))?;

        // Find the LND node
        let lnd_node = network
            .nodes
            .iter()
            .find(|n| n.name == lnd_node_name && n.kind == NodeKind::Lnd)
            .ok_or_else(|| Error::Config(format!("LND node '{}' not found", lnd_node_name)))?;

        let btc_node_obj = BitcoinNode {
            node: btc_node.clone(),
            image: network
                .btc_version
                .clone()
                .unwrap_or_else(|| BitcoinNode::DEFAULT_IMAGE.to_string()),
        };

        let lnd_node_obj = LndNode {
            node: lnd_node.clone(),
            image: network
                .lnd_version
                .clone()
                .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
            bitcoin_node: btc_node.id.to_string(),
            alias: lnd_node.name.clone(),
        };

        // Check Bitcoin node balance before attempting to send
        let btc_balance = btc_node_obj.get_balance(&self.container_manager).await?;
        if btc_balance < amount {
            return Err(Error::Config(format!(
                "Insufficient balance in Bitcoin node. Have: {} BTC, Need: {} BTC. Try mining blocks first.",
                btc_balance, amount
            )));
        }

        // Get a new address from the LND node
        let address = lnd_node_obj
            .get_new_address(&self.container_manager)
            .await?;

        // Send funds from Bitcoin node to LND address
        let txid = btc_node_obj
            .send_to_address(&self.container_manager, &address, amount)
            .await?;

        // Mine blocks to confirm the transaction if auto_mine is enabled
        if auto_mine {
            self.log("Auto-mining 6 blocks to confirm funding transaction");
            btc_node_obj
                .mine_blocks(&self.container_manager, 6, None)
                .await?;

            // Give LND a moment to detect the confirmed transaction
            self.log("Waiting for LND to sync with confirmed blocks");
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        Ok(txid)
    }

    /// Open a Lightning channel between two LND nodes.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    /// * `from_node` - Name of the node opening the channel
    /// * `to_node` - Name of the node to open channel to
    /// * `capacity` - Channel capacity in satoshis
    /// * `push_amount` - Amount to push to peer (optional)
    pub async fn open_channel(
        &self,
        network_name: &str,
        from_node: &str,
        to_node: &str,
        capacity: u64,
        push_amount: Option<u64>,
    ) -> Result<String> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Find both nodes
        let from = network
            .nodes
            .iter()
            .find(|n| n.name == from_node && n.kind == NodeKind::Lnd)
            .ok_or_else(|| Error::Config(format!("LND node '{}' not found", from_node)))?;

        let to = network
            .nodes
            .iter()
            .find(|n| n.name == to_node && n.kind == NodeKind::Lnd)
            .ok_or_else(|| Error::Config(format!("LND node '{}' not found", to_node)))?;

        let from_lnd = LndNode {
            node: from.clone(),
            image: network
                .lnd_version
                .clone()
                .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
            bitcoin_node: String::new(), // Not needed for this operation
            alias: from.name.clone(),
        };

        let to_lnd = LndNode {
            node: to.clone(),
            image: network
                .lnd_version
                .clone()
                .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
            bitcoin_node: String::new(),
            alias: to.name.clone(),
        };

        // Get the target node's pubkey
        let to_pubkey = to_lnd.get_pubkey(&self.container_manager).await?;

        // Note: We connect via Docker network using container names, not host ports

        // Connect as peers using the container name (within Docker network)
        let peer_host = format!("polar-lnd-{}:9735", to.id);
        from_lnd
            .connect_peer(&self.container_manager, &to_pubkey, &peer_host)
            .await?;

        // Open the channel
        let funding_txid = from_lnd
            .open_channel(&self.container_manager, &to_pubkey, capacity, push_amount)
            .await?;

        Ok(funding_txid)
    }

    /// Close a Lightning channel.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    /// * `node_name` - Name of the node that owns the channel
    /// * `channel_point` - Channel point in format "funding_txid:output_index"
    /// * `force` - Whether to force close the channel
    pub async fn close_channel(
        &self,
        network_name: &str,
        node_name: &str,
        channel_point: &str,
        force: bool,
    ) -> Result<String> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        let node = network
            .nodes
            .iter()
            .find(|n| n.name == node_name && n.kind == NodeKind::Lnd)
            .ok_or_else(|| Error::Config(format!("LND node '{}' not found", node_name)))?;

        let lnd = LndNode {
            node: node.clone(),
            image: network
                .lnd_version
                .clone()
                .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
            bitcoin_node: String::new(),
            alias: node.name.clone(),
        };

        let closing_txid = lnd
            .close_channel(&self.container_manager, channel_point, force)
            .await?;

        Ok(closing_txid)
    }

    /// Send a Lightning payment from one node to another.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    /// * `from_node` - Name of the paying node
    /// * `to_node` - Name of the receiving node
    /// * `amount` - Amount in satoshis
    /// * `memo` - Optional payment memo
    pub async fn send_payment(
        &self,
        network_name: &str,
        from_node: &str,
        to_node: &str,
        amount: u64,
        memo: Option<&str>,
    ) -> Result<String> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Find both nodes
        let from = network
            .nodes
            .iter()
            .find(|n| n.name == from_node && n.kind == NodeKind::Lnd)
            .ok_or_else(|| Error::Config(format!("LND node '{}' not found", from_node)))?;

        let to = network
            .nodes
            .iter()
            .find(|n| n.name == to_node && n.kind == NodeKind::Lnd)
            .ok_or_else(|| Error::Config(format!("LND node '{}' not found", to_node)))?;

        let from_lnd = LndNode {
            node: from.clone(),
            image: network
                .lnd_version
                .clone()
                .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
            bitcoin_node: String::new(),
            alias: from.name.clone(),
        };

        let to_lnd = LndNode {
            node: to.clone(),
            image: network
                .lnd_version
                .clone()
                .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
            bitcoin_node: String::new(),
            alias: to.name.clone(),
        };

        // Create invoice on receiving node
        let invoice = to_lnd
            .create_invoice(&self.container_manager, amount, memo)
            .await?;

        // Pay invoice from sending node
        let payment_hash = from_lnd
            .pay_invoice(&self.container_manager, &invoice)
            .await?;

        Ok(payment_hash)
    }

    /// Synchronize the Lightning Network graph across all LND nodes.
    /// This connects all LND nodes to each other as peers so they can discover
    /// channels and route payments.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    ///
    /// # Returns
    /// Number of LND nodes synchronized
    pub async fn sync_graph(&self, network_name: &str) -> Result<usize> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Get all LND nodes
        let lnd_nodes: Vec<_> = network
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Lnd)
            .collect();

        if lnd_nodes.len() < 2 {
            return Ok(0); // Nothing to sync with less than 2 nodes
        }

        // Connect each LND node to all other LND nodes
        for (i, from_node) in lnd_nodes.iter().enumerate() {
            for to_node in lnd_nodes.iter().skip(i + 1) {
                let from_lnd = LndNode {
                    node: (*from_node).clone(),
                    image: network
                        .lnd_version
                        .clone()
                        .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
                    bitcoin_node: String::new(),
                    alias: from_node.name.clone(),
                };

                let to_lnd = LndNode {
                    node: (*to_node).clone(),
                    image: network
                        .lnd_version
                        .clone()
                        .unwrap_or_else(|| LndNode::DEFAULT_IMAGE.to_string()),
                    bitcoin_node: String::new(),
                    alias: to_node.name.clone(),
                };

                // Get the target node's pubkey
                let to_pubkey = to_lnd.get_pubkey(&self.container_manager).await?;

                // Connect as peers using the container name (within Docker network)
                let peer_host = format!("polar-lnd-{}:9735", to_node.id);

                // Try to connect, but don't fail if already connected
                let _ = from_lnd
                    .connect_peer(&self.container_manager, &to_pubkey, &peer_host)
                    .await;
            }
        }

        Ok(lnd_nodes.len())
    }

    /// Synchronize LND nodes with the Bitcoin blockchain.
    /// This waits for all LND nodes to be synced to the chain.
    ///
    /// # Arguments
    /// * `network_name` - Name of the network
    ///
    /// # Returns
    /// Number of LND nodes synchronized
    pub async fn sync_chain(&self, network_name: &str) -> Result<usize> {
        let network = self
            .get_network(network_name)
            .ok_or_else(|| Error::NetworkNotFound(network_name.to_string()))?;

        // Get all LND nodes
        let lnd_nodes: Vec<_> = network
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Lnd)
            .collect();

        if lnd_nodes.is_empty() {
            return Ok(0);
        }

        // Wait for each LND node to sync with the chain
        // We'll check if synced_to_chain is true for each node
        let mut synced_count = 0;
        for node in &lnd_nodes {
            if let Some(container_id) = &node.container_id {
                // Use getinfo to check sync status
                let output = self.container_manager
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
                    .await;

                if let Ok(info) = output {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&info) {
                        if json["synced_to_chain"].as_bool().unwrap_or(false) {
                            synced_count += 1;
                        }
                    }
                }
            }
        }

        Ok(synced_count)
    }
}
