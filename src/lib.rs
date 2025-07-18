// src/lib.rs
#![allow(clippy::module_inception)]

use ethers::contract::abigen;

abigen!(ERC20, r#"[
    function totalSupply() external view returns (uint256)
    function balanceOf(address) external view returns (uint256)
    function decimals() external view returns (uint8)
]"#);

abigen!(StakingPool, "abi/staking_pool_abi.json");

pub mod config;
pub mod supply;
pub mod utils;