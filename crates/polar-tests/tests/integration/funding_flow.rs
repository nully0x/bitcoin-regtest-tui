//! Integration tests for Lightning node funding flow.
//!
//! These tests verify the complete flow of funding an LND node's wallet
//! from a Bitcoin Core node, including address generation, transaction
//! sending, and confirmation.

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

/// Test the basic funding flow: Bitcoin node -> LND wallet
#[tokio::test]
async fn test_fund_lnd_wallet_basic() -> Result<()> {
    println!("\nTesting basic LND wallet funding flow...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-funding-1";
    println!("  - Creating Docker network...");
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup: Start Bitcoin Core on the network
    let mut btc_node = BitcoinNode::new("test-btc-funding-1");
    println!("  - Starting Bitcoin Core...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    // Wait for Bitcoin to initialize
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Setup: Start LND node on the same network
    let mut lnd_node = LndNode::new("test-lnd-funding-1", btc_id);
    println!("  - Starting LND node...");
    lnd_node
        .start_with_network(&manager, Some(network_name))
        .await?;

    // Wait for LND to initialize
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Step 1: Mine blocks to Bitcoin Core wallet to get funds
    println!("  - Mining 101 blocks to get mature coinbase rewards...");
    let blocks = btc_node.mine_blocks(&manager, 101, None).await?;
    assert_eq!(blocks.len(), 101, "Should mine 101 blocks");
    println!("    ✓ Mined {} blocks", blocks.len());

    // Step 2: Verify Bitcoin node has balance
    let btc_balance = btc_node.get_balance(&manager).await?;
    println!("  - Bitcoin node balance: {} BTC", btc_balance);
    assert!(
        btc_balance > 0.0,
        "Bitcoin node should have balance after mining"
    );

    // Step 3: Get new address from LND wallet
    println!("  - Getting new address from LND wallet...");
    let lnd_address = lnd_node.get_new_address(&manager).await?;
    println!("    ✓ LND address: {}", lnd_address);
    assert!(!lnd_address.is_empty(), "LND address should not be empty");
    assert!(
        lnd_address.starts_with("bcrt1"),
        "Address should be regtest bech32 (p2wkh)"
    );

    // Step 4: Send funds from Bitcoin node to LND address
    let fund_amount = 1.0;
    println!(
        "  - Sending {} BTC from Bitcoin node to LND address...",
        fund_amount
    );
    let txid = btc_node
        .send_to_address(&manager, &lnd_address, fund_amount)
        .await?;
    println!("    ✓ Transaction ID: {}", txid);
    assert_eq!(txid.len(), 64, "TXID should be 64 characters (hex)");

    // Step 5: Mine blocks to confirm the transaction
    println!("  - Mining 6 blocks to confirm transaction...");
    let confirm_blocks = btc_node.mine_blocks(&manager, 6, None).await?;
    assert_eq!(confirm_blocks.len(), 6, "Should mine 6 confirmation blocks");
    println!("    ✓ Transaction confirmed");

    // Step 6: Wait for LND to sync with the chain
    println!("  - Waiting for LND to sync...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Step 7: Verify LND wallet balance (this requires lncli walletbalance)
    // We'll use the container manager to execute the command directly
    let container_id = lnd_node
        .node
        .container_id
        .as_ref()
        .expect("LND should have container ID");

    let wallet_balance_output = manager
        .exec_command(
            container_id,
            vec![
                "lncli",
                "--network=regtest",
                "--tlscertpath=/home/lnd/.lnd/tls.cert",
                "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                "walletbalance",
            ],
        )
        .await?;

    println!("  - LND wallet balance response: {}", wallet_balance_output);

    let balance_json: serde_json::Value = serde_json::from_str(&wallet_balance_output)?;
    let confirmed_balance = balance_json["confirmed_balance"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    println!("  - LND confirmed balance: {} sats", confirmed_balance);
    assert!(
        confirmed_balance > 0,
        "LND wallet should have confirmed balance after funding and mining"
    );

    // The balance should be approximately 1 BTC (100,000,000 sats) minus fees
    let expected_sats = (fund_amount * 100_000_000.0) as i64;
    assert!(
        confirmed_balance > expected_sats - 10_000,
        "Balance should be close to {} sats (accounting for fees)",
        expected_sats
    );

    println!("  ✓ LND wallet successfully funded!");

    // Cleanup
    println!("  - Cleaning up...");
    lnd_node.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    println!("  ✓ Test completed successfully");
    // Network cleanup handled by RAII guard

    Ok(())
}

/// Test funding with insufficient Bitcoin balance
#[tokio::test]
async fn test_fund_lnd_wallet_insufficient_balance() -> Result<()> {
    println!("\nTesting funding with insufficient Bitcoin balance...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-funding-2";
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup: Start Bitcoin Core
    let mut btc_node = BitcoinNode::new("test-btc-funding-2");
    println!("  - Starting Bitcoin Core...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Setup: Start LND node
    let mut lnd_node = LndNode::new("test-lnd-funding-2", btc_id);
    println!("  - Starting LND node...");
    lnd_node
        .start_with_network(&manager, Some(network_name))
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Don't mine any blocks - Bitcoin wallet should be empty
    let btc_balance = btc_node.get_balance(&manager).await?;
    println!("  - Bitcoin node balance: {} BTC", btc_balance);
    assert_eq!(btc_balance, 0.0, "Bitcoin node should have zero balance");

    // Try to send funds - this should fail
    println!("  - Attempting to send 1.0 BTC with zero balance...");
    let lnd_address = lnd_node.get_new_address(&manager).await?;

    let result = btc_node.send_to_address(&manager, &lnd_address, 1.0).await;

    assert!(
        result.is_err(),
        "Sending funds with insufficient balance should fail"
    );
    println!("  ✓ Correctly failed with insufficient balance");

    // Cleanup
    lnd_node.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    // Network cleanup handled by RAII guard

    Ok(())
}

/// Test multiple funding transactions
#[tokio::test]
async fn test_fund_lnd_wallet_multiple_times() -> Result<()> {
    println!("\nTesting multiple funding transactions...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-funding-3";
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup
    let mut btc_node = BitcoinNode::new("test-btc-funding-3");
    println!("  - Starting Bitcoin Core...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let mut lnd_node = LndNode::new("test-lnd-funding-3", btc_id);
    println!("  - Starting LND node...");
    lnd_node
        .start_with_network(&manager, Some(network_name))
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Mine blocks to get funds
    println!("  - Mining 101 blocks...");
    btc_node.mine_blocks(&manager, 101, None).await?;

    // Fund LND wallet multiple times
    let num_fundings = 3;
    let amount_per_funding = 0.5;

    for i in 1..=num_fundings {
        println!("  - Funding transaction {} of {}...", i, num_fundings);

        let lnd_address = lnd_node.get_new_address(&manager).await?;
        let txid = btc_node
            .send_to_address(&manager, &lnd_address, amount_per_funding)
            .await?;
        println!("    ✓ TXID: {}", txid);

        // Mine blocks to confirm
        btc_node.mine_blocks(&manager, 1, None).await?;
    }

    // Mine additional blocks for full confirmation
    println!("  - Mining 5 more blocks for full confirmation...");
    btc_node.mine_blocks(&manager, 5, None).await?;

    // Wait for LND to sync
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Verify total balance
    let container_id = lnd_node
        .node
        .container_id
        .as_ref()
        .expect("LND should have container ID");

    let wallet_balance_output = manager
        .exec_command(
            container_id,
            vec![
                "lncli",
                "--network=regtest",
                "--tlscertpath=/home/lnd/.lnd/tls.cert",
                "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                "walletbalance",
            ],
        )
        .await?;

    let balance_json: serde_json::Value = serde_json::from_str(&wallet_balance_output)?;
    let confirmed_balance = balance_json["confirmed_balance"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    println!("  - LND confirmed balance: {} sats", confirmed_balance);

    // Should have approximately num_fundings * amount_per_funding BTC
    let expected_min_sats =
        ((num_fundings as f64 * amount_per_funding * 100_000_000.0) - 50_000.0) as i64;
    assert!(
        confirmed_balance > expected_min_sats,
        "Balance should reflect all funding transactions"
    );

    println!("  ✓ Multiple fundings successful!");

    // Cleanup
    lnd_node.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    // Network cleanup handled by RAII guard

    Ok(())
}

/// Test funding two different LND nodes from same Bitcoin node
#[tokio::test]
async fn test_fund_multiple_lnd_wallets() -> Result<()> {
    println!("\nTesting funding multiple LND wallets from one Bitcoin node...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-funding-4";
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup Bitcoin Core
    let mut btc_node = BitcoinNode::new("test-btc-funding-4");
    println!("  - Starting Bitcoin Core...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Setup two LND nodes
    let mut lnd_node_1 = LndNode::new("test-lnd-funding-4a", btc_id.clone());
    let mut lnd_node_2 = LndNode::new("test-lnd-funding-4b", btc_id.clone());

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

    // Fund first LND node
    println!("  - Funding LND node 1...");
    let addr1 = lnd_node_1.get_new_address(&manager).await?;
    let txid1 = btc_node.send_to_address(&manager, &addr1, 1.0).await?;
    println!("    ✓ TXID: {}", txid1);

    // Fund second LND node
    println!("  - Funding LND node 2...");
    let addr2 = lnd_node_2.get_new_address(&manager).await?;
    let txid2 = btc_node.send_to_address(&manager, &addr2, 2.0).await?;
    println!("    ✓ TXID: {}", txid2);

    // Confirm both transactions
    println!("  - Mining 6 blocks to confirm...");
    btc_node.mine_blocks(&manager, 6, None).await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Verify both nodes received funds
    for (i, node) in [&lnd_node_1, &lnd_node_2].iter().enumerate() {
        let container_id = node
            .node
            .container_id
            .as_ref()
            .expect("LND should have container ID");

        let wallet_balance_output = manager
            .exec_command(
                container_id,
                vec![
                    "lncli",
                    "--network=regtest",
                    "--tlscertpath=/home/lnd/.lnd/tls.cert",
                    "--macaroonpath=/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon",
                    "walletbalance",
                ],
            )
            .await?;

        let balance_json: serde_json::Value = serde_json::from_str(&wallet_balance_output)?;
        let confirmed_balance = balance_json["confirmed_balance"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        println!(
            "  - LND node {} confirmed balance: {} sats",
            i + 1,
            confirmed_balance
        );
        assert!(confirmed_balance > 0, "Node {} should have balance", i + 1);
    }

    println!("  ✓ Both LND nodes funded successfully!");

    // Cleanup
    lnd_node_1.stop(&manager).await?;
    lnd_node_2.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    // Network cleanup handled by RAII guard

    Ok(())
}

/// Test getting new addresses generates unique addresses
#[tokio::test]
async fn test_lnd_address_generation_uniqueness() -> Result<()> {
    println!("\nTesting LND address generation uniqueness...");

    let manager = ContainerManager::new()?;

    // Create a Docker network for the test
    let network_name = "polar-test-addr";
    manager.create_network(network_name).await?;
    let _cleanup = NetworkCleanup::new(&manager, network_name.to_string());

    // Setup
    let mut btc_node = BitcoinNode::new("test-btc-addr");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let mut lnd_node = LndNode::new("test-lnd-addr", btc_id);
    lnd_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Generate multiple addresses
    println!("  - Generating 5 addresses...");
    let mut addresses = Vec::new();
    for i in 1..=5 {
        let addr = lnd_node.get_new_address(&manager).await?;
        println!("    Address {}: {}", i, addr);
        addresses.push(addr);
    }

    // Verify all addresses are unique
    let unique_count = addresses
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(
        unique_count,
        addresses.len(),
        "All generated addresses should be unique"
    );

    println!("  ✓ All addresses are unique!");

    // Cleanup
    lnd_node.stop(&manager).await?;
    btc_node.stop(&manager).await?;
    // Network cleanup handled by RAII guard

    Ok(())
}
