// crates/strategy/src/uniswap.rs

use alloy::sol;

// 1. Definisikan Interface Uniswap V2 Router
sol! {
    #[derive(Debug, PartialEq, Eq)]
    interface IUniswapV2Router {
        function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline) external payable returns (uint[] memory amounts);
        function swapExactTokensForETH(uint amountIn, uint amountOutMin, address[] calldata path, address to, uint deadline) external returns (uint[] memory amounts);
        function swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] calldata path, address to, uint deadline) external returns (uint[] memory amounts);
    }
}

// 2. Alamat Router Uniswap V2 di Mainnet Ethereum
pub const UNISWAP_V2_ROUTER: &str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
pub const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";