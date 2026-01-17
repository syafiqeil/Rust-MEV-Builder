// crates/searcher/src/main.rs

use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use anyhow::Result;
use futures_util::StreamExt;
use dotenv::dotenv;
use std::env;
use tracing::{info, warn}; 
use mev_strategy::UniswapStrategy;
use mev_executor::FlashbotsExecutor;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    info!("🚀 Starting MEV Bot (Searcher + Strategy + Simulator + Executor)...");

    // 1. Setup RPC & WSS
    let rpc_url = env::var("RPC_URL").expect("RPC_URL must be set");
    let wss_url = env::var("WSS_URL").expect("WSS_URL must be set");

    // 2. Setup Executor
    let attacker_key = env::var("ATTACKER_KEY").expect("ATTACKER_KEY missing");
    let auth_key = env::var("FLASHBOTS_AUTH_KEY").expect("FLASHBOTS_AUTH_KEY missing");
    
    info!("🔌 Initializing Flashbots Executor...");
    let executor = FlashbotsExecutor::new(attacker_key, auth_key)?;

    // 3. Setup Strategy
    info!("🧠 Initializing Strategy Engine...");
    let mut strategy = UniswapStrategy::new(rpc_url.clone(), executor)?;
    strategy.initialize(); 

    // 4. Connect to Mempool
    info!("👂 Listening for Pending Uniswap Transactions...");
    let ws = WsConnect::new(wss_url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?; 
    let sub = provider.subscribe_pending_transactions().await?;

    let mut stream = sub.into_stream();

    // Loop Stream
    while let Some(tx_hash) = stream.next().await {
        // tx_hash di sini adalah FixedBytes<32>, bukan Header

        // Fetch detail transaksi penuh berdasarkan Hash
        match provider.get_transaction_by_hash(tx_hash).await {
            Ok(Some(tx)) => {
                // Kirim ke Strategy untuk dianalisis (Async)
                strategy.process_tx(&tx).await;
            },
            Ok(None) => {
                // Transaksi mungkin sudah hilang/dropped dari mempool
            },
            Err(e) => {
                warn!("Failed to fetch tx {:?}: {}", tx_hash, e);
            }
        }
    }

    Ok(())
}