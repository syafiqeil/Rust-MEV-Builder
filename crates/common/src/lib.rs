// crates/common/src/lib.rs

use alloy_primitives::{Address, U256, Bytes};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTx {
    pub signer: Address,
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub gas_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevBundle {
    pub block_number: u64,
    pub txs: Vec<BundleTx>,
    pub timestamp: u64,
}

