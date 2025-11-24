//! Integration tests for network operations.

use polar_core::{NetworkStatus};
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};
use anyhow::Result;

#[tokio::test]
async fn test_docker_connectivity() -> Result<()> {
    println!("Testing Docker connectivity...");

    let manager = ContainerManager::new()?;
    manager.ping().await?;

    println!("✓ Docker is available and responding");
    Ok(())
}

#[tokio::test]
async fn test_create_bitcoin_container() -> Result<()> {
    println!("\nTesting Bitcoin Core container creation...");

    let manager = ContainerManager::new()?;
    let mut btc_node = BitcoinNode::new("test-bitcoin");

    println!("  - Creating Bitcoin Core container...");
    btc_node.start(&manager).await?;

    assert!(btc_node.node.container_id.is_some(), "Container ID should be set");
    println!("  ✓ Bitcoin Core container created: {:?}", btc_node.node.container_id);

    // Give it a moment to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("  - Stopping Bitcoin Core container...");
    btc_node.stop(&manager).await?;

    assert!(btc_node.node.container_id.is_none(), "Container ID should be cleared");
    println!("  ✓ Bitcoin Core container stopped and removed");

    Ok(())
}

#[tokio::test]
async fn test_create_lnd_container() -> Result<()> {
    println!("\nTesting LND container with Bitcoin backend...");

    let manager = ContainerManager::new()?;

    // First start Bitcoin
    let mut btc_node = BitcoinNode::new("test-bitcoin-2");
    println!("  - Starting Bitcoin Core...");
    btc_node.start(&manager).await?;
    let btc_id = btc_node.node.id.to_string();

    // Wait for Bitcoin to be ready
    println!("  - Waiting for Bitcoin Core to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Now start LND
    let mut lnd_node = LndNode::new("test-lnd", btc_id.clone());
    println!("  - Starting LND node...");
    lnd_node.start(&manager).await?;

    assert!(lnd_node.node.container_id.is_some(), "LND container ID should be set");
    println!("  ✓ LND container created: {:?}", lnd_node.node.container_id);

    // Wait a bit for LND to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Cleanup
    println!("  - Stopping LND...");
    lnd_node.stop(&manager).await?;
    println!("  - Stopping Bitcoin Core...");
    btc_node.stop(&manager).await?;

    println!("  ✓ All containers stopped and removed");

    Ok(())
}

#[tokio::test]
async fn test_network_creation_and_lifecycle() -> Result<()> {
    println!("\nTesting full network lifecycle...");

    use polar_core::{Network, Node, NodeKind};

    let manager = ContainerManager::new()?;

    // Create a test network structure
    let mut network = Network::new("test-network");
    let btc_node = Node::new("bitcoin-1", NodeKind::BitcoinCore);
    let lnd1 = Node::new("lnd-1", NodeKind::Lnd);
    let lnd2 = Node::new("lnd-2", NodeKind::Lnd);

    network.add_node(btc_node);
    network.add_node(lnd1);
    network.add_node(lnd2);

    println!("  - Network created with {} nodes", network.nodes.len());
    assert_eq!(network.status, NetworkStatus::Stopped);

    // Test that we can start nodes manually
    network.status = NetworkStatus::Starting;
    println!("  - Network status: Starting");

    // Start Bitcoin node
    for node in &mut network.nodes {
        if node.kind == NodeKind::BitcoinCore {
            let mut btc = BitcoinNode::new(node.name.clone());
            btc.node.id = node.id;
            println!("  - Starting Bitcoin Core node '{}'...", node.name);
            btc.start(&manager).await?;
            node.container_id = btc.node.container_id;
            println!("    ✓ Container ID: {:?}", node.container_id);
        }
    }

    // Wait for Bitcoin
    println!("  - Waiting for Bitcoin to be ready...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Get Bitcoin node ID for LND nodes
    let btc_id = network.nodes
        .iter()
        .find(|n| n.kind == NodeKind::BitcoinCore)
        .map(|n| n.id.to_string())
        .expect("Bitcoin node should exist");

    // Start LND nodes
    for node in &mut network.nodes {
        if node.kind == NodeKind::Lnd {
            let mut lnd = LndNode::new(node.name.clone(), btc_id.clone());
            lnd.node.id = node.id;
            println!("  - Starting LND node '{}'...", node.name);
            lnd.start(&manager).await?;
            node.container_id = lnd.node.container_id;
            println!("    ✓ Container ID: {:?}", node.container_id);
        }
    }

    network.status = NetworkStatus::Running;
    println!("  ✓ Network is running with {} nodes", network.nodes.len());

    // Verify all nodes have container IDs
    for node in &network.nodes {
        assert!(node.container_id.is_some(), "Node '{}' should have container ID", node.name);
    }

    // Wait a bit to let things stabilize
    println!("  - Letting network stabilize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Now stop everything
    network.status = NetworkStatus::Stopping;
    println!("  - Stopping network...");

    // Stop LND nodes first
    for node in &mut network.nodes {
        if node.kind == NodeKind::Lnd {
            if let Some(container_id) = &node.container_id {
                println!("  - Stopping LND node '{}'...", node.name);
                manager.stop_container(container_id).await?;
                manager.remove_container(container_id).await?;
                node.container_id = None;
            }
        }
    }

    // Then stop Bitcoin
    for node in &mut network.nodes {
        if node.kind == NodeKind::BitcoinCore {
            if let Some(container_id) = &node.container_id {
                println!("  - Stopping Bitcoin node '{}'...", node.name);
                manager.stop_container(container_id).await?;
                manager.remove_container(container_id).await?;
                node.container_id = None;
            }
        }
    }

    network.status = NetworkStatus::Stopped;
    println!("  ✓ Network stopped successfully");

    // Verify all containers are removed
    for node in &network.nodes {
        assert!(node.container_id.is_none(), "Node '{}' container should be removed", node.name);
    }

    Ok(())
}

#[tokio::test]
async fn test_docker_network_creation() -> Result<()> {
    println!("\nTesting Docker network creation...");

    let manager = ContainerManager::new()?;

    let network_name = "polar-test-network";
    println!("  - Creating Docker network '{}'...", network_name);

    let network_id = manager.create_network(network_name).await?;
    println!("  ✓ Network created with ID: {}", network_id);

    println!("  - Removing Docker network...");
    manager.remove_network(network_name).await?;
    println!("  ✓ Network removed successfully");

    Ok(())
}
