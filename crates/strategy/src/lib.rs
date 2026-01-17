// crates/strategy/src/lib.rs

use alloy::rpc::types::eth::Transaction;
use alloy::consensus::Transaction as TransactionTrait;
use alloy_primitives::{Address, Bytes, U256, I256};
use alloy::sol_types::{SolCall, SolConstructor}; 
use anyhow::Result;
use mev_simulator::EvmSimulator;
use mev_executor::FlashbotsExecutor;
use tracing::{info, warn};
use std::str::FromStr;
use std::env; 
use revm::context::TxEnv;
use revm::primitives::TxKind; 

use crate::uniswap::{IUniswapV2Router, UNISWAP_V2_ROUTER, WETH_ADDRESS};

pub mod uniswap;

const DEPLOYER_ADDRESS: &str = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"; 

alloy::sol! {
    interface ISandwich {
        constructor(address _router, address _weth);
        function buy(address token, uint amountIn) external payable;
        function sell(address token) external;
        function withdraw() external;
    }
}

pub struct UniswapStrategy {
    simulator: EvmSimulator,
    executor: FlashbotsExecutor,
    router_address: Address,
    weth_address: Address,
    contract_address: Option<Address>, 
}

impl UniswapStrategy {
    pub fn new(rpc_url: String, executor: FlashbotsExecutor) -> Result<Self> {
        dotenv::dotenv().ok();

        let simulator = EvmSimulator::new(rpc_url)?;
        let router_address = Address::from_str(UNISWAP_V2_ROUTER)?;
        let weth_address = Address::from_str(WETH_ADDRESS)?;

        Ok(Self {
            simulator,
            executor,
            router_address,
            weth_address,
            contract_address: None, 
        })
    }

    pub fn initialize(&mut self) {
        info!("Initializing Strategy: Deploying Sandwich Contract to Simulator...");
        match self.deploy_sandwich_contract() {
            Ok(addr) => {
                self.contract_address = Some(addr);
                info!("✅ Contract Deployed at: {:?}", addr);
            },
            Err(e) => {
                warn!("Failed to deploy contract: {}", e);
                warn!("Ensure SANDWICH_BYTECODE is set in .env");
            }
        }
    }

    pub async fn process_tx(&mut self, tx: &Transaction) {
        if self.contract_address.is_none() { return; }
        if tx.inner.to() != Some(self.router_address) { return; }

        let input_data = tx.inner.input();

        if let Ok(decoded) = IUniswapV2Router::swapExactETHForTokensCall::abi_decode(input_data) {
            info!("🎯 TARGET ACQUIRED: Swap ETH -> Tokens");

            self.execute_optimal_sandwich(
                tx.inner.signer(), 
                tx.inner.value(), 
                input_data.clone(), 
                decoded.path,
                tx 
            ).await;
        }
    }

    fn deploy_sandwich_contract(&mut self) -> Result<Address> {
        info!("Preparing Deployment...");
        
        // 1. Baca Bytecode
        let bytecode_hex = env::var("SANDWICH_BYTECODE")
            .map_err(|_| anyhow::anyhow!("SANDWICH_BYTECODE not found in .env"))?;
        
        let bytecode_clean = bytecode_hex.trim_start_matches("0x");
        let bytecode_raw = hex::decode(bytecode_clean)?;

        // 2. Encode Constructor
        let args = ISandwich::constructorCall {
            _router: self.router_address,
            _weth: self.weth_address,
        }.abi_encode();

        let mut deploy_data = bytecode_raw;
        deploy_data.extend(args);

        // 3. Setup Deployer
        let deployer = Address::from_str(DEPLOYER_ADDRESS)?;
        self.simulator.set_balance(deployer, U256::from(10) * U256::from(1_000_000_000_000_000_000u64)); 
        let nonce = self.simulator.get_nonce(deployer)?;

        // 4. Create Tx
        let mut tx = TxEnv::default();
        tx.caller = deployer;
        tx.kind = TxKind::Create; 
        tx.data = Bytes::from(deploy_data);
        tx.gas_limit = 10_000_000; 
        tx.gas_price = 20_000_000_000;
        tx.nonce = nonce;

        info!("Sending Deployment Transaction...");
        
        // 5. Execute & Capture Address
        let bundle_res = self.simulator.simulate_bundle(vec![tx])?;

        // VALIDASI DAN SUNTIK KODE (GOD MODE)
        match (bundle_res.created_address, bundle_res.created_code) {
            (Some(addr), Some(code)) => {
                info!("✅ EVM confirmed deployment at: {:?}", addr);
                info!("Injecting Runtime Code to DB (Persistence Fix)...");
                
                // INI KUNCINYA: Paksa DB menyimpan kode ini selamanya
                self.simulator.set_code(addr, code);
                
                Ok(addr)
            },
            _ => {
                Err(anyhow::anyhow!("Deployment failed: No address or code returned!"))
            }
        }
    }

    async fn execute_optimal_sandwich(
        &mut self, 
        victim: Address, 
        victim_value: U256, 
        victim_input: Bytes, 
        path: Vec<Address>,
        _original_tx: &Transaction
    ) {
        info!("Calculating Sweet Spot (Contract Mode)...");

        let one_ether = U256::from(1_000_000_000_000_000_000u64);

        // Konfigurasi Binary Search
        let mut low = one_ether / U256::from(100);
        let mut high =  one_ether * U256::from(10);

        // Variabel untuk menyimpan hasil terbaik
        let mut best_profit = I256::ZERO;
        let mut best_amount = U256::ZERO;
        
        let contract_addr = self.contract_address.expect("Contract not deployed");
        let weth = self.weth_address;

        let estimated_gas_cost = I256::try_from(5_000_000_000_000_000u64).unwrap();
        
        // BINARY SEARCH LOOP 
        for i in 0..8 {
            // Ambil nilai tengah
            let mid = (low + high) / U256::from(2);

            // Clone simulator state
            let mut sim_sandbox = self.simulator.clone();
            sim_sandbox.set_balance(contract_addr, U256::from(1_000_000) * one_ether);

            // Simulasikan!
            let gross_revenue = Self::try_atomic_sandwich(
                &mut sim_sandbox, 
                contract_addr, 
                weth,
                mid, 
                victim, 
                victim_value, 
                victim_input.clone(), 
                path.clone()
            );

            // Cek apakah revert 
            let is_revert = gross_revenue == I256::try_from(-999i64).unwrap();
            let input_eth_display = wei_to_eth(mid);

            if is_revert {
                info!("Iter #{} | ⚠️ Input: {:.4} ETH -> REVERT (Too High)", i, input_eth_display);
                high = mid; 
            } else {
                let net_profit = gross_revenue - estimated_gas_cost;

                let revenue_eth = i256_to_eth_f64(gross_revenue);
                let net_eth = i256_to_eth_f64(net_profit);

                if net_profit > I256::ZERO {
                    info!("Iter #{} | 🟢 Input: {:.4} ETH -> Rev: {:.5} ETH | Net: {:.5} ETH", i, input_eth_display, revenue_eth, net_eth);
                } else {
                    info!("Iter #{} | 🔴 Input: {:.4} ETH -> Rev: {:.5} ETH | Net: {:.5} ETH (Gas Loss)", i, input_eth_display, revenue_eth, net_eth);
                }

                if net_profit > best_profit {
                    best_profit = net_profit;
                    best_amount = mid;
                }
                
                low = mid;
            }
        }

        // --- Final Execution Decision
        // Minimum profit 0.005 ETH (untuk cover gas mainnet)
        let min_profit = I256::try_from(5_000_000_000_000u64).unwrap();

        if best_profit > min_profit {
            let profit_eth = i256_to_eth_f64(best_profit);
            info!("SWEET SPOT FOUND: {:.4} ETH -> Est. Profit: {:.4} ETH", wei_to_eth(best_amount), profit_eth);
            info!("PREPARING FLASHBOTS BUNDLE...");

            // Konstruksi Transaksi Nyata
            let token_out = path[1];

            // A. Buy
            let buy_calldata = ISandwich::buyCall {
                token: token_out,
                amountIn: best_amount,
            }.abi_encode();

            // B. Sell
            let sell_calldata = ISandwich::sellCall {
                token: token_out,
            }.abi_encode();

            // C. Sign Config (TODO: Fetch dynamic nonce in Prod)
            let nonce = 10;
            let chain_id = 1;

            // Kalkulasi priority fee dinamis
            let priority_fee = 2_000_000_000;

            let txs_to_sign = vec![
                (contract_addr, Bytes::from(buy_calldata), best_amount, 350_000, nonce, priority_fee),
                (contract_addr, Bytes::from(sell_calldata), U256::ZERO, 350_000, nonce + 1, priority_fee),
            ];

            match self.executor.sign_bundle_txs(txs_to_sign, chain_id).await {
                Ok(my_signed_txs) => {
                    let mut final_bundle = Vec::new();
                    final_bundle.push(my_signed_txs[0].clone()); // Buy
                    // final_bundle.push(victim_raw_bytes);      // Victim
                    final_bundle.push(my_signed_txs[1].clone());

                    let target_block =  20_000_000;
                    match self.executor.send_bundle(final_bundle, target_block).await {
                        Ok(_) => info!("Bundle Sent Successfully!"),
                        Err(e) => warn!("Bundle Failed: {}", e),
                    }
                }
                Err(e) => warn!("❌ Signing Failed: {}", e),
            }
        } else {
            // Opsional: Log jika profit kecil agar tahu bot bekerja
            if best_profit > I256::ZERO {
                let profit_eth = i256_to_eth_f64(best_profit);
                info!("⚠️ Opportunity ignored. Profit too low: {:.5} ETH (Best Input: {:.4} ETH)", profit_eth, wei_to_eth(best_amount));
            }
        }
    }

    fn try_atomic_sandwich(
        sim: &mut EvmSimulator, 
        contract: Address, 
        _weth: Address, 
        amount_in: U256, 
        victim: Address, 
        victim_value: U256, 
        victim_input: Bytes, 
        path: Vec<Address>
    ) -> I256 {
        let token_out = path[1]; 
        let deployer = Address::from_str(DEPLOYER_ADDRESS).unwrap(); 

        // 1. TX 1: Contract.BUY()
        let buy_calldata = ISandwich::buyCall {
            token: token_out,
            amountIn: amount_in,
        }.abi_encode();

        let mut tx_buy = TxEnv::default();
        tx_buy.caller = deployer; 
        tx_buy.kind = TxKind::Call(contract);
        tx_buy.data = Bytes::from(buy_calldata);
        tx_buy.value = U256::ZERO; 
        tx_buy.gas_limit = 500_000;
        tx_buy.gas_price = 30_000_000_000;
        tx_buy.nonce = sim.get_nonce(deployer).unwrap_or(0);

        // 2. TX 2: Victim Swap
        let router_addr = Address::from_str(UNISWAP_V2_ROUTER).unwrap();
        let mut tx_victim = TxEnv::default();
        tx_victim.caller = victim;
        tx_victim.kind = TxKind::Call(router_addr);
        tx_victim.value = victim_value;
        tx_victim.data = victim_input;
        tx_victim.gas_limit = 500_000;
        tx_victim.gas_price = 25_000_000_000;
        tx_victim.nonce = sim.get_nonce(victim).unwrap_or(0);

        // 3. TX 3: Contract.SELL()
        let sell_calldata = ISandwich::sellCall {
            token: token_out,
        }.abi_encode();

        let mut tx_sell = TxEnv::default();
        tx_sell.caller = deployer;
        tx_sell.kind = TxKind::Call(contract);
        tx_sell.data = Bytes::from(sell_calldata);
        tx_sell.value = U256::ZERO;
        tx_sell.gas_limit = 500_000;
        tx_sell.gas_price = 20_000_000_000;
        tx_sell.nonce = tx_buy.nonce + 1; 

        // EXECUTE BUNDLE
        let bundle = vec![tx_buy, tx_victim, tx_sell];
        let bal_before = sim.get_balance(contract).unwrap_or(U256::ZERO);

        match sim.simulate_bundle(bundle) {
            Ok(res) => {
                if res.victim_reverted {
                    return I256::try_from(-999i64).unwrap(); // Kode Revert
                }
                
                let bal_after = sim.get_balance(contract).unwrap_or(U256::ZERO);
                let revenue = I256::from_raw(bal_after) - I256::from_raw(bal_before);
                
                revenue
            },
            Err(_) => I256::ZERO,
        }
    }
}

// --- Helper ---
fn i256_to_eth_f64(val: I256) -> f64 {
    let s = val.to_string();
    let f: f64 = s.parse().unwrap_or(0.0);
    f / 1e18
}

fn wei_to_eth(val: U256) -> f64 {
    let s = val.to_string();
    let f: f64 = s.parse().unwrap_or(0.0);
    f / 1e18
}