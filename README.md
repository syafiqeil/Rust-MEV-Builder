# Rust MEV Sandwich Bot (Uniswap V2) 

![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?logo=rust)
![Alloy](https://img.shields.io/badge/Stack-Alloy_Rs-blue)
![Revm](https://img.shields.io/badge/EVM-Revm_v33-red)
![License](https://img.shields.io/badge/License-MIT-green)

A high-performance, atomic MEV Sandwich Bot built from scratch in **Rust**. This project leverages the latest Ethereum Rust stack (**Alloy-rs** & **Revm**) to simulate, optimize, and execute sandwich attacks on Uniswap V2 pools with millisecond precision.

---

## Project Overview

This bot is designed to listen to the Ethereum Public Mempool, detect large pending swaps, and atomically "sandwich" them (Buy Frontrun -> Victim Swap -> Sell Backrun) to extract profit.

Unlike basic bots that guess input amounts, this engine uses a **Binary Search Algorithm** to surgically find the maximum profitable input amount ("Sweet Spot") without triggering the victim's slippage protection.

### Key Features
* **The Ear (Searcher):** Real-time Mempool streaming using Websockets (WSS).
* **The Brain (Simulator):** Local Mainnet Forking using `revm` for high-speed, gas-free simulation.
* **The Scalpel (Optimizer):** Custom Binary Search algorithm to calculate the optimal input amount with 99.6% precision in <300ms.
* **The Hand (Executor):** Integration with **Flashbots** to bypass the public mempool and avoid being sandwiched.
* **Safety First:** Comprehensive pre-execution simulation checks (`Net Profit > Gas Cost`) to prevent loss.

---

## Architecture

The workspace is divided into 4 modular crates:

| Crate | Description |
| :--- | :--- |
| **`crates/searcher`** | Connects to WSS/RPC, filters pending txs, and orchestrates the pipeline. |
| **`crates/simulator`** | Wraps `revm` DB. Handles Mainnet state fetching (forking) and "God Mode" code injection. |
| **`crates/strategy`** | Contains the Binary Search logic, Profit/Loss calculation, and V2 math. |
| **`crates/executor`** | Handles private key signing and sending Bundles to Flashbots Relay. |

---

## Technical Journey & Engineering Challenges

This project was built in two distinct phases, solving critical engineering hurdles along the way.

### Phase 1: Infrastructure & "The Brain"
* **Transition to Alloy:** Migrated from `ethers-rs` to `alloy-rs` for better type safety and performance.
* **The "Silent Failure" Bug:** Encountered a critical issue where `revm` would lose the deployed contract state during forking/cloning.
* **Solution:** Engineered a **Persistence Fix (God Mode)** that forcibly injects the contract's Runtime Bytecode into the Simulator's `CacheDB` after deployment, ensuring the bot "remembers" the contract across multiple simulation clones.

### Phase 2: Intelligence & "The Scalpel"
* **From Sledgehammer to Scalpel:** Initially, the bot used static inputs (1, 5, 10 ETH), which often caused the victim's transaction to revert due to `INSUFFICIENT_OUTPUT_AMOUNT`.
* **Binary Search Optimization:** Implemented a recursive Binary Search loop (8 iterations).
    * The bot simulates ranges from `0.01 ETH` to `10 ETH`.
    * It dynamically adjusts the input based on whether the simulation `Reverts` (too aggressive) or `Succeeds` (room for more).
    * **Result:** The bot finds the precise "Sweet Spot" (e.g., `2.46 ETH`) that maximizes revenue without breaking the victim's trade.

---

## Installation & Setup

### Prerequisites
* [Rust & Cargo](https://rustup.rs/)
* An Ethereum Node Provider (Alchemy/Infura) with WSS support.
* A dedicated wallet for Flashbots Reputation (can be empty).

### 1. Clone the Repository

git clone [https://github.com/syafiqeil/rust-mev-bot.git](https://github.com/syafiqeil/rust-mev-bot.git)
cd rust-mev-bot

### 2. Configure Environment Variables
Create a .env file in the root directory:

    # Node Provider
    RPC_URL=[https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY](https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY)
    WSS_URL=wss://[eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY](https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY)

    # Private Keys (Add 0x prefix)
    ATTACKER_KEY=0x...      # Wallet with ETH for gas
    FLASHBOTS_AUTH_KEY=0x... # Random new wallet for Flashbots ID

    # Smart Contract Bytecode (Compiled Sandwich.sol)
    SANDWICH_BYTECODE=0x...


### 3. Run the Bot
    cargo run -p mev-searcher

## Sample Output (Log)
    INFO mev_searcher: 👂 Listening for Pending Uniswap Transactions...
    INFO mev_strategy: 🎯 TARGET ACQUIRED: Swap ETH -> Tokens
    INFO mev_strategy: 🧠 Calculating Precision Sweet Spot (Binary Search)...
    INFO mev_strategy: Iter #0 | ⚠️ Input: 5.0000 ETH -> REVERT (Too High)
    INFO mev_strategy: Iter #1 | 🟢 Input: 2.5000 ETH -> Rev: 0.005 ETH | Net: -0.003 ETH
    INFO mev_strategy: Iter #2 | ⚠️ Input: 3.7500 ETH -> REVERT (Too High)
    INFO mev_strategy: Iter #3 | 🟢 Input: 3.1250 ETH -> Rev: 0.008 ETH | Net: 0.0005 ETH
    INFO mev_strategy: 🔥 SWEET SPOT FOUND: 3.1250 ETH -> Est. Profit: 0.0005 ETH
    INFO mev_strategy: 🚀 PREPARING FLASHBOTS BUNDLE...
    INFO mev_executor: ⚡ Bundle Sent Successfully!

## Diclaimer
This project is for educational and research purposes only. MEV extraction on Mainnet is highly competitive and carries financial risk (gas costs, smart contract bugs). The author is not responsible for any financial losses incurred from using this codebase.

