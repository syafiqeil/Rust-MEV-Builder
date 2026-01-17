// crates/executor/src/lib.rs

use alloy::network::{EthereumWallet, TransactionBuilder};
use alloy::primitives::{Address, Bytes, U256};
use alloy::rpc::types::eth::TransactionRequest; 
use alloy::signers::Signer;
use alloy::signers::local::PrivateKeySigner;
use alloy::eips::eip2718::Encodable2718;
use anyhow::Result;
use reqwest::Client;
use serde::Serialize;
use std::str::FromStr;
use tracing::{info, error};
use url::Url;

// URL Relay Flashbots (Mainnet)
const FLASHBOTS_RELAY_URL: &str = "https://relay.flashbots.net"; 

pub struct FlashbotsExecutor {
    client: Client,
    signer: PrivateKeySigner,       // Wallet Attacker
    auth_signer: PrivateKeySigner,  // Wallet Reputasi
    relay_url: Url,
}

#[derive(Serialize)]
struct FlashbotsBundleRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<BundleParams>,
}

#[derive(Serialize)]
struct BundleParams {
    txs: Vec<String>,       // List Hex String dari Signed Txs
    #[serde(rename = "blockNumber")]
    block_number: String,    // Target Block (Hex)
}

impl FlashbotsExecutor {
    pub fn new(attacker_key: String, auth_key: String) -> Result<Self> {
        let signer = PrivateKeySigner::from_str(&attacker_key)?;
        let auth_signer = PrivateKeySigner::from_str(&auth_key)?;
        let relay_url = Url::parse(FLASHBOTS_RELAY_URL)?;

        Ok(Self {
            client: Client::new(),
            signer,
            auth_signer,
            relay_url,
        })
    }

    /// Menandatangani daftar transaksi mentah (Raw Txs)
    pub async fn sign_bundle_txs(
        &self, 
        txs: Vec<(Address, Bytes, U256, u64, u64, u128)>, // (to, data, value, gas, nonce, priority_fee)
        chain_id: u64
    ) -> Result<Vec<String>> {
        let wallet = EthereumWallet::from(self.signer.clone());
        let mut signed_txs = Vec::new();

        for (to, data, value, gas, nonce, priority_fee) in txs {
            let tx_req = TransactionRequest::default()
                .with_to(to)
                .with_value(value)
                .with_input(data)
                .with_nonce(nonce)
                .with_gas_limit(gas)
                .with_max_priority_fee_per_gas(priority_fee)
                .with_max_fee_per_gas(priority_fee + 20_000_000_000) // Base + Tip
                .with_chain_id(chain_id); 

            let tx_envelope = tx_req.build(&wallet).await?;
            
            // Encode ke RLP Bytes
            let rlp_signed = tx_envelope.encoded_2718(); 
            signed_txs.push(hex::encode(rlp_signed));
        }

        Ok(signed_txs)
    }

    /// Mengirim Bundle ke Flashbots
    pub async fn send_bundle(
        &self, 
        signed_txs: Vec<String>, 
        target_block: u64
    ) -> Result<()> {
        info!("🚀 Sending Bundle to Flashbots (Target Block: {})", target_block);

        let params = BundleParams {
            txs: signed_txs,
            block_number: format!("0x{:x}", target_block),
        };

        let request_body = FlashbotsBundleRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "eth_sendBundle".to_string(),
            params: vec![params],
        };

        let body_string = serde_json::to_string(&request_body)?;

        // Buat Signature Header
        let msg_hash = alloy::primitives::keccak256(body_string.as_bytes());
        let signature = self.auth_signer.sign_hash(&msg_hash).await?;
        let auth_address = self.auth_signer.address();
        
        // Format signature header
        let header_val = format!("{:?}:0x{}", auth_address, signature.as_hex_string());

        // Kirim HTTP Request
        let res = self.client.post(self.relay_url.clone())
            .header("Content-Type", "application/json")
            .header("X-Flashbots-Signature", header_val)
            .body(body_string)
            .send()
            .await?;

        let status = res.status();
        let resp_text = res.text().await?;
        
        if status.is_success() {
            info!("✅ Flashbots Response: {}", resp_text);
        } else {
            error!("❌ Flashbots Error ({}): {}", status, resp_text);
        }

        Ok(())
    }
}

// Extension Trait Helper
trait SigHex {
    fn as_hex_string(&self) -> String;
}

impl SigHex for alloy::signers::Signature {
    fn as_hex_string(&self) -> String {
        format!("{}", self)
    }
}