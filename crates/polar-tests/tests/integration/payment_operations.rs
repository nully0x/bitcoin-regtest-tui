//! Tests for Lightning Network payment operations between nodes.

use anyhow::Result;
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};

#[tokio::test]
async fn test_payment_between_two_nodes_with_direct_channel() -> Result<()> {
    println!("\nTesting Lightning payment between two nodes...");

    let manager = ContainerManager::new()?;
    let network_name = "polar-test-payment-direct";

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

    // Mine initial blocks to activate the blockchain
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

    // Fund LND1 wallet (will open channel)
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

    // Open channel from lnd1 to lnd2 with push amount
    println!("  - Opening channel with push amount...");
    let channel_capacity = 1_000_000; // 1M sats
    let push_amount = 500_000; // Push 500k sats to lnd2

    lnd1.open_channel(&manager, &lnd2_pubkey, channel_capacity, Some(push_amount))
        .await?;

    // Mine blocks to confirm channel
    println!("  - Mining 6 blocks to confirm channel...");
    btc_node.mine_blocks(&manager, 6, None).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Verify both nodes have the channel
    println!("  - Verifying channel exists...");
    let lnd1_channels = lnd1.list_channels(&manager).await?;
    let lnd2_channels = lnd2.list_channels(&manager).await?;

    let lnd1_channel_count = lnd1_channels["channels"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);
    let lnd2_channel_count = lnd2_channels["channels"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(0);

    println!("    ✓ LND1 has {} channel(s)", lnd1_channel_count);
    println!("    ✓ LND2 has {} channel(s)", lnd2_channel_count);
    assert!(lnd1_channel_count > 0, "LND1 should have a channel");
    assert!(lnd2_channel_count > 0, "LND2 should have a channel");

    // Test payment from lnd1 to lnd2
    println!("  - Creating invoice on LND2...");
    let payment_amount = 10_000; // 10k sats
    let invoice = lnd2
        .create_invoice(&manager, payment_amount, Some("test payment"))
        .await?;
    println!("    ✓ Created invoice");

    println!("  - Paying invoice from LND1...");
    let payment_hash = lnd1.pay_invoice(&manager, &invoice).await?;
    println!("    ✓ Payment successful! Hash: {}", payment_hash);

    // Test payment in reverse direction (lnd2 to lnd1)
    println!("  - Creating reverse invoice on LND1...");
    let reverse_amount = 5_000; // 5k sats
    let reverse_invoice = lnd1
        .create_invoice(&manager, reverse_amount, Some("reverse payment"))
        .await?;
    println!("    ✓ Created reverse invoice");

    println!("  - Paying reverse invoice from LND2...");
    let reverse_payment_hash = lnd2.pay_invoice(&manager, &reverse_invoice).await?;
    println!(
        "    ✓ Reverse payment successful! Hash: {}",
        reverse_payment_hash
    );

    println!("  ✓ All payments completed successfully!");

    // Cleanup
    println!("  - Cleaning up...");
    lnd1.stop(&manager).await?;
    lnd2.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    manager.remove_network(network_name).await?;
    println!("  - Network removed");

    Ok(())
}

#[tokio::test]
async fn test_payment_fails_without_channel() -> Result<()> {
    println!("\nTesting that payment fails without a channel...");

    let manager = ContainerManager::new()?;
    let network_name = "polar-test-payment-nochannel";

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
    btc_node.mine_blocks(&manager, 101, None).await?;

    // Create LND nodes but don't open channel
    let mut lnd1 = LndNode::new("lnd-1", btc_id.clone());
    let mut lnd2 = LndNode::new("lnd-2", btc_id.clone());

    println!("  - Starting LND nodes (no channel)...");
    lnd1.start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    lnd2.start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Try to make payment without channel - should fail
    println!("  - Attempting payment without channel...");
    let payment_amount = 10_000;
    let invoice = lnd2
        .create_invoice(&manager, payment_amount, Some("should fail"))
        .await?;

    let result = lnd1.pay_invoice(&manager, &invoice).await;
    assert!(result.is_err(), "Payment should fail without a channel");
    println!("    ✓ Payment correctly failed: {:?}", result.unwrap_err());

    // Cleanup
    println!("  - Cleaning up...");
    lnd1.stop(&manager).await?;
    lnd2.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    manager.remove_network(network_name).await?;
    println!("  - Network removed");

    Ok(())
}
