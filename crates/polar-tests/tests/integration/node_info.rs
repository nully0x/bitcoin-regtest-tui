//! Integration tests for node info retrieval.

use anyhow::Result;
use polar_docker::ContainerManager;
use polar_nodes::{BitcoinNode, LndNode};

#[tokio::test]
async fn test_bitcoin_exec_command() -> Result<()> {
    println!("Testing Bitcoin CLI exec command...");

    let manager = ContainerManager::new()?;
    let mut btc_node = BitcoinNode::new("test-bitcoin-exec");

    println!("  - Starting Bitcoin Core container...");
    btc_node.start(&manager).await?;

    let container_id = btc_node
        .node
        .container_id
        .as_ref()
        .expect("Container should be running");

    // Wait for Bitcoin to be ready
    println!("  - Waiting for Bitcoin Core to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Try to execute a command
    println!("  - Executing bitcoin-cli getblockchaininfo...");
    match manager
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
        .await
    {
        Ok(output) => {
            println!("  ✓ Command executed successfully");
            println!("  Output length: {} bytes", output.len());
            println!(
                "  First 200 chars: {}",
                &output.chars().take(200).collect::<String>()
            );

            // Try to parse as JSON
            match serde_json::from_str::<serde_json::Value>(&output) {
                Ok(json) => {
                    println!("  ✓ Output is valid JSON");
                    println!("  Chain: {}", json["chain"].as_str().unwrap_or("unknown"));
                    println!("  Blocks: {}", json["blocks"].as_u64().unwrap_or(0));
                }
                Err(e) => {
                    println!("  ✗ Failed to parse JSON: {}", e);
                    println!("  Raw output: {}", output);
                }
            }
        }
        Err(e) => {
            println!("  ✗ Command failed: {}", e);
        }
    }

    println!("  - Stopping Bitcoin Core container...");
    btc_node.stop(&manager).await?;

    Ok(())
}

#[tokio::test]
async fn test_lnd_exec_command() -> Result<()> {
    println!("Testing LND CLI exec command...");

    let manager = ContainerManager::new()?;

    // Create a Docker network
    let network_name = "polar-test-lnd-network";
    println!("  - Creating Docker network...");
    manager.create_network(network_name).await?;

    // Start Bitcoin first ON THE NETWORK
    let mut btc_node = BitcoinNode::new("test-bitcoin-lnd");
    println!("  - Starting Bitcoin Core container...");
    btc_node
        .start_with_network(&manager, Some(network_name))
        .await?;
    let btc_id = btc_node.node.id.to_string();

    // Wait for Bitcoin to be ready
    println!("  - Waiting for Bitcoin Core to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Now start LND ON THE SAME NETWORK
    let mut lnd_node = LndNode::new("test-lnd", btc_id);
    println!("  - Starting LND container...");
    lnd_node
        .start_with_network(&manager, Some(network_name))
        .await?;

    let container_id = lnd_node
        .node
        .container_id
        .as_ref()
        .expect("Container should be running");

    // Wait for LND to be ready (needs more time than Bitcoin)
    println!("  - Waiting for LND to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Check if container is still running
    match manager.inspect_container(container_id).await {
        Ok(info) => {
            if let Some(state) = info.state {
                println!(
                    "  - Container state: running={:?}, status={:?}",
                    state.running, state.status
                );
            }
        }
        Err(e) => {
            println!("  ✗ Failed to inspect container: {}", e);
        }
    }

    // Try to execute a command
    println!("  - Executing lncli getinfo...");
    match manager
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
        .await
    {
        Ok(output) => {
            println!("  ✓ Command executed successfully");
            println!("  Output length: {} bytes", output.len());
            println!(
                "  First 300 chars: {}",
                &output.chars().take(300).collect::<String>()
            );

            // Try to parse as JSON
            match serde_json::from_str::<serde_json::Value>(&output) {
                Ok(json) => {
                    println!("  ✓ Output is valid JSON");
                    println!("  Alias: {}", json["alias"].as_str().unwrap_or("unknown"));
                    println!(
                        "  Version: {}",
                        json["version"].as_str().unwrap_or("unknown")
                    );
                    println!(
                        "  Synced to chain: {}",
                        json["synced_to_chain"].as_bool().unwrap_or(false)
                    );
                }
                Err(e) => {
                    println!("  ✗ Failed to parse JSON: {}", e);
                    println!("  Raw output: {}", output);
                }
            }
        }
        Err(e) => {
            println!("  ✗ Command failed: {}", e);
        }
    }

    println!("  - Stopping LND container...");
    lnd_node.stop(&manager).await?;
    println!("  - Stopping Bitcoin Core container...");
    btc_node.stop(&manager).await?;
    println!("  - Removing Docker network...");
    manager.remove_network(network_name).await?;

    Ok(())
}
