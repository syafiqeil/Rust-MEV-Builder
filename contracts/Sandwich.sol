// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IERC20 {
    function approve(address spender, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
}

interface IUniswapV2Router {
    function swapExactETHForTokens(uint amountOutMin, address[] calldata path, address to, uint deadline) external payable returns (uint[] memory amounts);
    function swapExactTokensForETH(uint amountIn, uint amountOutMin, address[] calldata path, address to, uint deadline) external returns (uint[] memory amounts);
}

contract SimpleSandwich {
    address public immutable owner;
    address public immutable router;
    address public immutable weth;

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    constructor(address _router, address _weth) {
        owner = msg.sender;
        router = _router;
        weth = _weth;
    }

    receive() external payable {}

    // --- STEP 1: FRONTRUN (BUY) ---
    // Dipanggil di awal bundle.
    // Tugas: Mengubah ETH menjadi Token untuk menaikkan harga.
    function buy(address token, uint amountIn) external payable onlyOwner {
        address[] memory path = new address[](2);
        path[0] = weth;
        path[1] = token;

        // Kirim ETH sejumlah amountIn (diambil dari saldo kontrak atau msg.value)
        // PENTING: amountOutMin = 0 karena kita ingin beli berapapun harganya (kita yang memanipulasi)
        IUniswapV2Router(router).swapExactETHForTokens{value: amountIn}(
            0, 
            path,
            address(this),
            block.timestamp
        );
    }

    // --- STEP 2: VICTIM TX (Terjadi di luar kontrak ini) ---

    // --- STEP 3: BACKRUN (SELL) ---
    // Dipanggil di akhir bundle.
    // Tugas: Menjual semua token yang didapat untuk mengambil profit.
    function sell(address token) external onlyOwner {
        uint tokenBal = IERC20(token).balanceOf(address(this));
        require(tokenBal > 0, "No tokens to sell");

        // Approve Router (Wajib)
        IERC20(token).approve(router, tokenBal);

        address[] memory path = new address[](2);
        path[0] = token;
        path[1] = weth;

        // Jual semua token kembali ke ETH
        IUniswapV2Router(router).swapExactTokensForETH(
            tokenBal,
            0, 
            path,
            address(this),
            block.timestamp
        );
        
        // Catatan: Di versi ini, kita tidak cek profit di sini.
        // Simulator Rust akan menjamin profit. Jika Rust menghitung rugi,
        // Rust tidak akan pernah mengirim transaksi ini.
    }

    // Tarik dana (ETH) ke dompet owner
    function withdraw() external onlyOwner {
        (bool success, ) = owner.call{value: address(this).balance}("");
        require(success, "Transfer failed");
    }

    // Tarik token ERC20 jika ada yang nyangkut
    function recoverToken(address token) external onlyOwner {
        uint bal = IERC20(token).balanceOf(address(this));
        IERC20(token).transfer(owner, bal);
    }
}