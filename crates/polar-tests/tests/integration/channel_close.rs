//! Tests for Lightning Network channel closing operations.

use anyhow::Result;
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};

#[tokio::test]
async fn test_cooperative_channel_close() -> Result<()> {
    println!("\nTesting cooperative channel close...");

    let manager = ContainerManager::new()?;
    let network_name = "polar-test-channel-close";

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

    // Fund LND1 wallet
    println!("  - Funding LND1 wallet...");
    let lnd1_address = lnd1.get_new_address(&manager).await?;
    btc_node
        .send_to_address(&manager, &lnd1_address, 1.0)
        .await?;

    // Mine blocks to confirm funding
    println!("  - Mining 6 blocks to confirm funding...");
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Connect nodes as peers
    println!("  - Connecting nodes as peers...");
    let lnd2_pubkey = lnd2.get_pubkey(&manager).await?;
    let peer_host = format!("polar-lnd-{}:9735", lnd2.node.id);
    lnd1.connect_peer(&manager, &lnd2_pubkey, &peer_host)
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Open channel
    println!("  - Opening channel...");
    let channel_capacity = 1_000_000;
    let funding_txid = lnd1
        .open_channel(&manager, &lnd2_pubkey, channel_capacity, None)
        .await?;
    println!("    ✓ Channel opened with funding txid: {}", funding_txid);

    // Mine blocks to confirm channel
    println!("  - Mining 6 blocks to confirm channel...");
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Get channel point from list channels
    let channels = lnd1.list_channels(&manager).await?;
    let channel_point = channels["channels"][0]["channel_point"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("No channel_point found"))?;
    println!("    ✓ Channel point: {}", channel_point);

    // Close channel cooperatively
    println!("  - Closing channel cooperatively...");
    let closing_txid = lnd1.close_channel(&manager, channel_point, false).await?;
    println!("    ✓ Channel closing initiated. Txid: {}", closing_txid);

    // Mine blocks to confirm close
    println!("  - Mining 6 blocks to confirm close...");
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify no channels remain
    let channels_after = lnd1.list_channels(&manager).await?;
    let channel_count = channels_after["channels"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);

    println!("    ✓ Channel count after close: {}", channel_count);
    assert_eq!(channel_count, 0, "All channels should be closed");

    println!("  ✓ Channel closed successfully!");

    // Cleanup
    println!("  - Cleaning up...");
    lnd1.stop(&manager).await?;
    lnd2.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    manager.remove_network(network_name).await?;
    println!("  - Network removed");

    Ok(())
}
