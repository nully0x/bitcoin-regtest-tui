//! Tests for channel information display in node info.

use anyhow::Result;
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};
use polar_tui::NetworkManager;

#[tokio::test]
async fn test_channel_list_in_node_info() -> Result<()> {
    println!("\nTesting channel list display in node info...");

    let manager = ContainerManager::new()?;
    let network_name = "polar-test-channel-info";

    // Create Docker network
    println!("  - Creating Docker network...");
    manager.create_network(network_name).await?;

    // Start Bitcoin node
    println!("  - Starting Bitcoin Core...");
    let mut btc_node = BitcoinNode::new("bitcoin-1");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Mine blocks
    println!("  - Mining 101 blocks...");
    btc_node.mine_blocks(&manager, 101, None).await?;

    // Start LND nodes
    println!("  - Starting LND nodes...");
    let mut lnd1 = LndNode::new("lnd-1", btc_id.clone());
    let mut lnd2 = LndNode::new("lnd-2", btc_id.clone());

    lnd1.start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    lnd2.start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Fund and open channel
    println!("  - Funding LND1 wallet...");
    let address = lnd1.get_new_address(&manager).await?;
    btc_node.send_to_address(&manager, &address, 1.0).await?;
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("  - Connecting nodes as peers...");
    let lnd2_pubkey = lnd2.get_pubkey(&manager).await?;
    let peer_host = format!("polar-lnd-{}:9735", lnd2.node.id);
    lnd1.connect_peer(&manager, &lnd2_pubkey, &peer_host)
        .await?;

    println!("  - Opening channel...");
    lnd1.open_channel(&manager, &lnd2_pubkey, 1000000, Some(0))
        .await?;
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Now test getting node info with channel list
    println!("  - Fetching node info with channel list...");
    let container_id = lnd1.node.container_id.as_ref().unwrap();
    let network_manager = NetworkManager::new()?;
    let node_info = network_manager.get_lnd_node_info(container_id).await?;

    println!("    ✓ Node: {}", node_info.alias);
    println!("    ✓ Active channels: {}", node_info.num_active_channels);
    println!("    ✓ Channel list size: {}", node_info.channels.len());

    if !node_info.channels.is_empty() {
        for (idx, channel) in node_info.channels.iter().enumerate() {
            println!("\n    Channel {}:", idx + 1);
            println!("      Point:    {}", channel.channel_point);
            println!("      Capacity: {} sats", channel.capacity);
            println!("      Local:    {} sats", channel.local_balance);
            println!("      Remote:   {} sats", channel.remote_balance);
            println!("      Active:   {}", channel.active);
        }
    }

    assert!(
        !node_info.channels.is_empty(),
        "Should have at least one channel"
    );
    assert_eq!(
        node_info.channels.len(),
        1,
        "Should have exactly one channel"
    );
    assert!(node_info.channels[0].active, "Channel should be active");

    println!("\n  ✓ Channel list fetched successfully!");

    // Cleanup
    println!("  - Cleaning up...");
    lnd1.stop(&manager).await?;
    lnd2.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    manager.remove_network(network_name).await?;

    Ok(())
}
