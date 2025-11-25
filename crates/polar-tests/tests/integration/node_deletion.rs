//! Tests for Lightning node deletion operations.

use anyhow::Result;
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};
use polar_tui::NetworkManager;

#[tokio::test]
async fn test_delete_lightning_node_via_network_manager() -> Result<()> {
    println!("\nTesting deletion of Lightning node via NetworkManager...");

    let manager = ContainerManager::new()?;
    let network_name = "polar-test-node-del";

    println!("  - Creating Docker network...");
    manager.create_network(network_name).await?;

    // Create Bitcoin node
    let mut btc_node = BitcoinNode::new("bitcoin-1");
    println!("  - Starting Bitcoin Core...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Mine initial blocks
    println!("  - Mining 101 blocks...");
    btc_node.mine_blocks(&manager, 101, None).await?;

    // Create LND nodes
    let mut lnd1 = LndNode::new("lnd-1", btc_id.clone());
    let mut lnd2 = LndNode::new("lnd-2", btc_id.clone());

    println!("  - Starting LND nodes...");
    lnd1.start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    lnd2.start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Now we need to create a network in NetworkManager that matches our setup
    // This simulates what would happen if the user created the network via TUI
    println!("  - Creating NetworkManager instance...");
    let mut network_manager = NetworkManager::new()?;

    // Create a matching network configuration
    println!("  - Creating network configuration...");
    network_manager.create_network(network_name)?;

    // Delete the second LND node
    println!("  - Deleting lnd-2 node...");
    match network_manager
        .delete_lightning_node(network_name, "lnd-2")
        .await
    {
        Ok(()) => {
            println!("    ✓ Node deleted successfully");
        }
        Err(e) => {
            println!("    ✗ Failed to delete node: {}", e);
            lnd1.stop(&manager).await?;
            lnd2.stop(&manager).await?;
            btc_node.stop(&manager).await?;
            manager.remove_network(network_name).await?;
            return Err(e.into());
        }
    }

    // Verify node was deleted from network config
    let network = network_manager
        .get_network(network_name)
        .expect("Network should exist");

    let lnd_nodes: Vec<_> = network
        .nodes
        .iter()
        .filter(|n| n.kind == polar_core::NodeKind::Lnd)
        .collect();

    println!("    ✓ Remaining LND nodes in config: {}", lnd_nodes.len());
    assert_eq!(lnd_nodes.len(), 1, "Should have 1 LND node remaining");

    println!("  ✓ Node deletion test passed!");

    // Cleanup
    println!("  - Cleaning up...");
    lnd1.stop(&manager).await?;
    // lnd2 should already be stopped by delete_lightning_node
    btc_node.stop(&manager).await?;
    network_manager.delete_network(network_name).await?;

    Ok(())
}

#[tokio::test]
async fn test_cannot_delete_bitcoin_node() -> Result<()> {
    println!("\nTesting that Bitcoin node cannot be deleted...");

    let manager = ContainerManager::new()?;
    let network_name = "polar-test-btc-del";

    println!("  - Creating Docker network...");
    manager.create_network(network_name).await?;

    // Create Bitcoin node
    let mut btc_node = BitcoinNode::new("bitcoin-1");
    println!("  - Starting Bitcoin Core...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Create network manager
    println!("  - Creating NetworkManager instance...");
    let mut network_manager = NetworkManager::new()?;

    // Create network configuration
    println!("  - Creating network configuration...");
    network_manager.create_network(network_name)?;

    // Try to delete Bitcoin node (should fail)
    println!("  - Attempting to delete Bitcoin node...");
    match network_manager
        .delete_lightning_node(network_name, "bitcoin-1")
        .await
    {
        Ok(()) => {
            println!("    ✗ Bitcoin node deletion should have failed!");
            btc_node.stop(&manager).await?;
            network_manager.delete_network(network_name).await?;
            return Err(anyhow::anyhow!("Bitcoin node should not be deletable"));
        }
        Err(e) => {
            println!("    ✓ Bitcoin node deletion rejected as expected");
            println!("      Error: {}", e);
            assert!(e.to_string().contains("Cannot delete Bitcoin node"));
        }
    }

    println!("  ✓ Bitcoin node protection test passed!");

    // Cleanup
    println!("  - Cleaning up...");
    btc_node.stop(&manager).await?;
    network_manager.delete_network(network_name).await?;

    Ok(())
}
