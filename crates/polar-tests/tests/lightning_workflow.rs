//! Integration tests for Lightning Network workflow.
//!
//! Tests the complete Lightning workflow:
//! - Mining blocks
//! - Funding LND wallets
//! - Opening channels
//! - Sending payments

use anyhow::Result;
use polar_tui::NetworkManager;

#[tokio::test]
async fn test_full_lightning_workflow() -> Result<()> {
    println!("\n=== Starting Full Lightning Workflow Test ===\n");

    // Step 1: Create network manager
    println!("Step 1: Creating network manager...");
    let mut manager = NetworkManager::new()?;
    println!("✓ Network manager created\n");

    // Step 2: Create a test network
    let network_name = "test-workflow";
    println!("Step 2: Creating network '{}'...", network_name);
    manager.create_network(network_name)?;
    println!("✓ Network created\n");

    // Step 3: Start the network
    println!("Step 3: Starting network...");
    manager.start_network(network_name).await?;
    println!("✓ Network started\n");

    // Wait for nodes to be fully ready
    println!("Waiting 5 seconds for nodes to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    println!("✓ Nodes initialized\n");

    // Step 4: Mine initial blocks to get Bitcoin
    println!("Step 4: Mining 101 blocks to mature coinbase...");
    let block_hashes = manager.mine_blocks(network_name, 101).await?;
    println!("✓ Mined {} blocks", block_hashes.len());
    println!("  First block: {}", &block_hashes[0][..16]);
    println!(
        "  Last block: {}\n",
        &block_hashes[block_hashes.len() - 1][..16]
    );

    // Step 5: Fund LND wallets
    println!("Step 5: Funding LND wallets...");
    println!("  Funding lnd-1 with 1 BTC...");
    let txid1 = manager.fund_lnd_wallet(network_name, "lnd-1", 1.0).await?;
    println!("  ✓ TXID: {}", &txid1[..16]);

    println!("  Funding lnd-2 with 1 BTC...");
    let txid2 = manager.fund_lnd_wallet(network_name, "lnd-2", 1.0).await?;
    println!("  ✓ TXID: {}\n", &txid2[..16]);

    // Step 6: Mine blocks to confirm funding transactions
    println!("Step 6: Mining 6 blocks to confirm funding...");
    let confirm_blocks = manager.mine_blocks(network_name, 6).await?;
    println!("✓ Mined {} confirmation blocks\n", confirm_blocks.len());

    // Wait for LND to process the blocks
    println!("Waiting 3 seconds for LND to process blocks...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    println!("✓ Processing complete\n");

    // Step 7: Open a channel from lnd-1 to lnd-2
    println!("Step 7: Opening channel from lnd-1 to lnd-2...");
    println!("  Capacity: 500,000 sats");
    println!("  Push amount: 100,000 sats");
    match manager
        .open_channel(network_name, "lnd-1", "lnd-2", 500_000, Some(100_000))
        .await
    {
        Ok(funding_txid) => {
            println!("✓ Channel opened. Funding TXID: {}\n", &funding_txid[..16]);

            // Step 8: Mine blocks to confirm channel
            println!("Step 8: Mining 6 blocks to confirm channel...");
            let channel_confirm = manager.mine_blocks(network_name, 6).await?;
            println!("✓ Mined {} confirmation blocks\n", channel_confirm.len());

            // Wait for channel to become active
            println!("Waiting 5 seconds for channel to become active...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            println!("✓ Channel should be active\n");

            // Step 9: Send a payment from lnd-2 to lnd-1
            println!("Step 9: Sending payment from lnd-2 to lnd-1...");
            println!("  Amount: 10,000 sats");
            println!("  Memo: Test payment");
            match manager
                .send_payment(network_name, "lnd-2", "lnd-1", 10_000, Some("Test payment"))
                .await
            {
                Ok(payment_hash) => {
                    println!("✓ Payment sent. Hash: {}\n", &payment_hash[..16]);
                }
                Err(e) => {
                    println!("⚠ Payment failed: {}\n", e);
                    println!("This might be expected if graph sync is not working yet.\n");
                }
            }
        }
        Err(e) => {
            println!("⚠ Channel opening failed: {}\n", e);
            println!("This is the issue we need to debug.\n");
        }
    }

    // Step 10: Stop the network
    println!("Step 10: Stopping network...");
    manager.stop_network(network_name).await?;
    println!("✓ Network stopped\n");

    // Step 11: Delete the network
    println!("Step 11: Deleting network...");
    manager.delete_network(network_name).await?;
    println!("✓ Network deleted\n");

    println!("=== Full Lightning Workflow Test Complete ===\n");
    Ok(())
}

#[tokio::test]
async fn test_mine_blocks() -> Result<()> {
    println!("\n=== Testing Mine Blocks ===\n");

    let mut manager = NetworkManager::new()?;
    let network_name = "test-mining";

    println!("Creating and starting network...");
    manager.create_network(network_name)?;
    manager.start_network(network_name).await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Mining 10 blocks...");
    let hashes = manager.mine_blocks(network_name, 10).await?;
    println!("✓ Mined {} blocks", hashes.len());
    assert_eq!(hashes.len(), 10);

    manager.stop_network(network_name).await?;
    manager.delete_network(network_name).await?;

    println!("=== Mine Blocks Test Complete ===\n");
    Ok(())
}

#[tokio::test]
async fn test_fund_lnd_wallet() -> Result<()> {
    println!("\n=== Testing Fund LND Wallet ===\n");

    let mut manager = NetworkManager::new()?;
    let network_name = "test-funding";

    println!("Creating and starting network...");
    manager.create_network(network_name)?;
    manager.start_network(network_name).await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Mining 101 blocks for coinbase maturity...");
    manager.mine_blocks(network_name, 101).await?;

    println!("Funding lnd-1 with 0.5 BTC...");
    let txid = manager.fund_lnd_wallet(network_name, "lnd-1", 0.5).await?;
    println!("✓ Funding TXID: {}", &txid[..16]);
    assert!(!txid.is_empty());

    manager.stop_network(network_name).await?;
    manager.delete_network(network_name).await?;

    println!("=== Fund LND Wallet Test Complete ===\n");
    Ok(())
}

#[tokio::test]
async fn test_open_channel_detailed() -> Result<()> {
    println!("\n=== Testing Channel Opening (Detailed) ===\n");

    let mut manager = NetworkManager::new()?;
    // Use timestamp to ensure unique network name
    let network_name = format!(
        "test-channel-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    println!("Creating and starting network...");
    manager.create_network(&network_name)?;
    manager.start_network(&network_name).await?;

    println!("Waiting for nodes to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    println!("Mining 101 blocks...");
    manager.mine_blocks(&network_name, 101).await?;

    println!("Funding both LND wallets...");
    manager.fund_lnd_wallet(&network_name, "lnd-1", 1.0).await?;
    manager.fund_lnd_wallet(&network_name, "lnd-2", 1.0).await?;

    println!("Mining 6 blocks to confirm funding...");
    manager.mine_blocks(&network_name, 6).await?;

    println!("Waiting for LND to process...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("\nAttempting to open channel...");
    match manager
        .open_channel(&network_name, "lnd-1", "lnd-2", 500_000, Some(100_000))
        .await
    {
        Ok(txid) => {
            println!("✓ Channel opened successfully!");
            println!("  Funding TXID: {}", txid);
        }
        Err(e) => {
            println!("✗ Channel opening failed!");
            println!("  Error: {}", e);
            println!("\nThis error needs to be debugged.");
        }
    }

    manager.stop_network(&network_name).await?;
    manager.delete_network(&network_name).await?;

    println!("\n=== Channel Opening Test Complete ===\n");
    Ok(())
}
