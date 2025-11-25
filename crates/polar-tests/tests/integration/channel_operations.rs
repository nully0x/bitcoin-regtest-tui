//! Integration tests for Lightning channel operations.
//!
//! These tests verify the complete flow of opening, managing, and using
//! Lightning channels between LND nodes.

use anyhow::Result;
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};

/// RAII guard to ensure Docker network cleanup
struct NetworkCleanup<'a> {
    manager: &'a ContainerManager,
    network_name: String,
}

impl<'a> NetworkCleanup<'a> {
    fn new(manager: &'a ContainerManager, network_name: String) -> Self {
        Self {
            manager,
            network_name,
        }
    }
}

impl<'a> Drop for NetworkCleanup<'a> {
    fn drop(&mut self) {
        // Best effort cleanup - ignore errors
        let _ = futures::executor::block_on(self.manager.remove_network(&self.network_name));
    }
}

/// Test opening a basic channel between two LND nodes
#[tokio::test]
async fn test_open_channel_basic() -> Result<()> {
    println!("\nTesting basic channel opening between two LND nodes...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-channel-1";
    println!("  - Creating Docker network...");
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup: Start Bitcoin Core
    let mut btc_node = BitcoinNode::new("test-btc-channel-1");
    println!("  - Starting Bitcoin Core...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Setup: Start two LND nodes
    let mut lnd_node_1 = LndNode::new("test-lnd-channel-1a", btc_id.clone());
    let mut lnd_node_2 = LndNode::new("test-lnd-channel-1b", btc_id.clone());

    println!("  - Starting LND node 1...");
    lnd_node_1
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("  - Starting LND node 2...");
    lnd_node_2
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Mine blocks to get funds
    println!("  - Mining 101 blocks...");
    btc_node.mine_blocks(&manager, 101, None).await?;

    // Fund LND node 1 (the one that will open the channel)
    println!("  - Funding LND node 1...");
    let addr1 = lnd_node_1.get_new_address(&manager).await?;
    let txid = btc_node.send_to_address(&manager, &addr1, 1.0).await?;
    println!("    ✓ Funding TXID: {}", txid);

    // Mine blocks to confirm funding
    println!("  - Mining 6 blocks to confirm funding...");
    btc_node.mine_blocks(&manager, 6, None).await?;

    // Wait for LND to sync
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Get node 2's pubkey
    println!("  - Getting node 2 pubkey...");
    let node2_pubkey = lnd_node_2.get_pubkey(&manager).await?;
    println!("    ✓ Node 2 pubkey: {}", &node2_pubkey[..16]);

    // Connect as peers
    println!("  - Connecting nodes as peers...");
    let peer_host = format!("polar-lnd-{}:9735", lnd_node_2.node.id);
    lnd_node_1
        .connect_peer(&manager, &node2_pubkey, &peer_host)
        .await?;
    println!("    ✓ Nodes connected as peers");

    // Open channel
    let channel_capacity = 1_000_000; // 1 million sats
    println!(
        "  - Opening channel with capacity {} sats...",
        channel_capacity
    );
    let funding_txid = lnd_node_1
        .open_channel(&manager, &node2_pubkey, channel_capacity, None)
        .await?;
    println!("    ✓ Channel funding TXID: {}", funding_txid);
    assert_eq!(
        funding_txid.len(),
        64,
        "Funding TXID should be 64 characters"
    );

    // Mine blocks to confirm channel
    println!("  - Mining 6 blocks to confirm channel...");
    btc_node.mine_blocks(&manager, 6, None).await?;

    // Wait for channel to be fully confirmed
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Verify channel exists
    println!("  - Verifying channel on node 1...");
    let channels = lnd_node_1.list_channels(&manager).await?;
    let channel_count = channels["channels"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);
    println!("    ✓ Node 1 has {} channel(s)", channel_count);
    assert!(channel_count > 0, "Node 1 should have at least one channel");

    println!("  ✓ Channel opened successfully!");

    // Cleanup
    println!("  - Cleaning up...");
    lnd_node_1.stop(&manager).await?;
    lnd_node_2.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    // Network cleanup handled by RAII guard

    Ok(())
}

/// Test opening a channel with push amount
#[tokio::test]
async fn test_open_channel_with_push() -> Result<()> {
    println!("\nTesting channel opening with push amount...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-channel-2";
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup infrastructure
    let mut btc_node = BitcoinNode::new("test-btc-channel-2");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let mut lnd_node_1 = LndNode::new("test-lnd-channel-2a", btc_id.clone());
    let mut lnd_node_2 = LndNode::new("test-lnd-channel-2b", btc_id.clone());

    lnd_node_1
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    lnd_node_2
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Mine and fund
    println!("  - Mining and funding nodes...");
    btc_node.mine_blocks(&manager, 101, None).await?;

    let addr1 = lnd_node_1.get_new_address(&manager).await?;
    btc_node.send_to_address(&manager, &addr1, 1.0).await?;
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Get pubkey and connect
    let node2_pubkey = lnd_node_2.get_pubkey(&manager).await?;
    let peer_host = format!("polar-lnd-{}:9735", lnd_node_2.node.id);
    lnd_node_1
        .connect_peer(&manager, &node2_pubkey, &peer_host)
        .await?;

    // Open channel with push amount
    let channel_capacity = 1_000_000;
    let push_amount = 500_000; // Push 500k sats to the other side
    println!(
        "  - Opening channel with {} sats capacity, pushing {} sats...",
        channel_capacity, push_amount
    );

    let funding_txid = lnd_node_1
        .open_channel(&manager, &node2_pubkey, channel_capacity, Some(push_amount))
        .await?;
    println!("    ✓ Channel funding TXID: {}", funding_txid);

    // Confirm channel
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Verify channel exists on both nodes
    let channels_1 = lnd_node_1.list_channels(&manager).await?;
    let channels_2 = lnd_node_2.list_channels(&manager).await?;

    let count_1 = channels_1["channels"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);
    let count_2 = channels_2["channels"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);

    println!("    ✓ Node 1 has {} channel(s)", count_1);
    println!("    ✓ Node 2 has {} channel(s)", count_2);

    assert_eq!(count_1, 1, "Node 1 should have exactly one channel");
    assert_eq!(count_2, 1, "Node 2 should have exactly one channel");

    println!("  ✓ Channel with push amount opened successfully!");

    // Cleanup
    lnd_node_1.stop(&manager).await?;
    lnd_node_2.stop(&manager).await?;
    btc_node.stop(&manager).await?;

    Ok(())
}

/// Test opening multiple channels
#[tokio::test]
async fn test_open_multiple_channels() -> Result<()> {
    println!("\nTesting opening multiple channels...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-channel-3";
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup infrastructure
    let mut btc_node = BitcoinNode::new("test-btc-channel-3");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Start 3 LND nodes
    let mut lnd_node_1 = LndNode::new("test-lnd-channel-3a", btc_id.clone());
    let mut lnd_node_2 = LndNode::new("test-lnd-channel-3b", btc_id.clone());
    let mut lnd_node_3 = LndNode::new("test-lnd-channel-3c", btc_id.clone());

    println!("  - Starting 3 LND nodes...");
    lnd_node_1
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    lnd_node_2
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    lnd_node_3
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Mine and fund node 1
    println!("  - Mining and funding node 1...");
    btc_node.mine_blocks(&manager, 101, None).await?;

    let addr1 = lnd_node_1.get_new_address(&manager).await?;
    btc_node.send_to_address(&manager, &addr1, 2.0).await?;
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Get pubkeys
    let node2_pubkey = lnd_node_2.get_pubkey(&manager).await?;
    let node3_pubkey = lnd_node_3.get_pubkey(&manager).await?;

    // Open channel from node 1 to node 2
    println!("  - Opening channel: node 1 -> node 2...");
    let peer_host_2 = format!("polar-lnd-{}:9735", lnd_node_2.node.id);
    lnd_node_1
        .connect_peer(&manager, &node2_pubkey, &peer_host_2)
        .await?;

    let funding_txid_1 = lnd_node_1
        .open_channel(&manager, &node2_pubkey, 500_000, None)
        .await?;
    println!("    ✓ Channel 1 funding TXID: {}", &funding_txid_1[..16]);

    // Open channel from node 1 to node 3
    println!("  - Opening channel: node 1 -> node 3...");
    let peer_host_3 = format!("polar-lnd-{}:9735", lnd_node_3.node.id);
    lnd_node_1
        .connect_peer(&manager, &node3_pubkey, &peer_host_3)
        .await?;

    let funding_txid_2 = lnd_node_1
        .open_channel(&manager, &node3_pubkey, 500_000, None)
        .await?;
    println!("    ✓ Channel 2 funding TXID: {}", &funding_txid_2[..16]);

    // Confirm both channels
    println!("  - Mining 6 blocks to confirm channels...");
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Verify node 1 has 2 channels
    let channels = lnd_node_1.list_channels(&manager).await?;
    let channel_count = channels["channels"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);

    println!("    ✓ Node 1 has {} channel(s)", channel_count);
    assert_eq!(channel_count, 2, "Node 1 should have exactly 2 channels");

    println!("  ✓ Multiple channels opened successfully!");

    // Cleanup
    lnd_node_1.stop(&manager).await?;
    lnd_node_2.stop(&manager).await?;
    lnd_node_3.stop(&manager).await?;
    btc_node.stop(&manager).await?;

    Ok(())
}

/// Test that opening channel fails without sufficient funds
#[tokio::test]
async fn test_open_channel_insufficient_funds() -> Result<()> {
    println!("\nTesting channel opening with insufficient funds...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-channel-4";
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup infrastructure
    let mut btc_node = BitcoinNode::new("test-btc-channel-4");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let mut lnd_node_1 = LndNode::new("test-lnd-channel-4a", btc_id.clone());
    let mut lnd_node_2 = LndNode::new("test-lnd-channel-4b", btc_id.clone());

    lnd_node_1
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    lnd_node_2
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Don't fund node 1 - just mine some blocks for the network
    println!("  - Mining blocks (but not funding nodes)...");
    btc_node.mine_blocks(&manager, 101, None).await?;

    // Get pubkey and try to connect
    let node2_pubkey = lnd_node_2.get_pubkey(&manager).await?;
    let peer_host = format!("polar-lnd-{}:9735", lnd_node_2.node.id);
    lnd_node_1
        .connect_peer(&manager, &node2_pubkey, &peer_host)
        .await?;

    // Try to open channel without funds - should fail
    println!("  - Attempting to open channel without funds...");
    let result = lnd_node_1
        .open_channel(&manager, &node2_pubkey, 1_000_000, None)
        .await;

    assert!(result.is_err(), "Opening channel without funds should fail");
    println!("    ✓ Correctly failed to open channel without funds");

    // Cleanup
    lnd_node_1.stop(&manager).await?;
    lnd_node_2.stop(&manager).await?;
    btc_node.stop(&manager).await?;

    Ok(())
}
