// src/supply.rs
use crate::{ERC20, StakingPool, utils};
use ethers::contract::Multicall;
use ethers::providers::Middleware;
use ethers::types::{Address, U256, U64};
use std::cmp;

#[derive(Debug, Clone)]
pub struct PoolCalculation {
    pub initial: U256,
    pub ratio_precision: U256,
    pub locked_amount: U256,
    pub days_passed: U256,
    pub days_until_lock_ends: U256,
    pub days_until_vesting_ends: U256,
    pub unlocked_fraction: U256,
}

#[derive(Debug, Clone)]
struct MulticallResults {
    total_supply: U256,
    burn_balance: U256,
    excluded_balances: Vec<U256>,
    pool_data: Vec<PoolData>,
}

#[derive(Debug, Clone)]
struct PoolData {
    initial: U256,
    pool_creation: U256,
    blocks_per_day: U256,
    lock_days: U256,
    vesting_days: U256,
    ratio_precision: U256,
}

pub fn calculate_pool_vesting(
    initial: U256,
    pool_creation: U256,
    blocks_per_day: U256,
    lock_days: U256,
    vesting_days: U256,
    ratio_precision: U256,
    current_block: U64,
) -> PoolCalculation {
    // Safe arithmetic operations with overflow checks
    let current_block_u256 = U256::from(current_block.as_u64());
    
    // Check if current_block is greater than pool_creation to avoid underflow
    let blocks_passed = if current_block_u256 > pool_creation {
        current_block_u256 - pool_creation
    } else {
        U256::zero()
    };
    
    // Safe division with zero check
    let days_passed = if blocks_per_day > U256::zero() {
        blocks_passed / blocks_per_day
    } else {
        U256::zero()
    };
    
    let lock_days_converted = if blocks_per_day > U256::zero() {
        lock_days / blocks_per_day
    } else {
        U256::zero()
    };
    
    let vesting_days_converted = if blocks_per_day > U256::zero() {
        vesting_days / blocks_per_day
    } else {
        U256::zero()
    };
    
    let days_until_lock_ends = if days_passed < lock_days_converted {
        lock_days_converted - days_passed
    } else {
        U256::zero()
    };
    
    let total_vesting_period = lock_days_converted + vesting_days_converted;
    let days_until_vesting_ends = if days_passed < total_vesting_period {
        total_vesting_period - days_passed
    } else {
        U256::zero()
    };
    
    let unlocked_fraction = if days_passed <= lock_days_converted {
        U256::zero()
    } else if vesting_days_converted > U256::zero() {
        let vesting_progress = days_passed - lock_days_converted;
        let frac = (vesting_progress * ratio_precision) / vesting_days_converted;
        cmp::min(frac, ratio_precision)
    } else {
        U256::zero()
    };
    
    // Safe multiplication and division
    let unlocked = if ratio_precision > U256::zero() {
        (initial * unlocked_fraction) / ratio_precision
    } else {
        U256::zero()
    };
    
    let locked = if unlocked < initial {
        initial - unlocked
    } else {
        U256::zero()
    };
    
    PoolCalculation {
        initial,
        ratio_precision,
        locked_amount: locked,
        days_passed,
        days_until_lock_ends,
        days_until_vesting_ends,
        unlocked_fraction,
    }
}

async fn build_multicall(
    contract: &ERC20<impl Middleware + 'static>,
    excluded_addresses: &[Address],
    pool_addresses: &[Address],
) -> Multicall<impl Middleware + 'static> {
    let multicall_addr: Address = "0xcA11bde05977b3631167028862bE2a173976CA11".parse().expect("Invalid multicall address");
    let mut multicall = Multicall::new(contract.client().clone(), Some(multicall_addr)).await.expect("Multicall creation failed");

    multicall.add_call(contract.total_supply(), false);
    multicall.add_call(contract.balance_of(Address::zero()), false);

    for addr in excluded_addresses {
        multicall.add_call(contract.balance_of(*addr), false);
    }

    for addr in pool_addresses {
        let pool = StakingPool::new(*addr, contract.client().clone());
        multicall.add_call(pool.initial_self_stake_amount(), false);
        multicall.add_call(pool.pool_creation(), false);
        multicall.add_call(pool.blocks_per_day(), false);
        multicall.add_call(pool.initial_lock_period(), false);
        multicall.add_call(pool.vesting_duration(), false);
        multicall.add_call(pool.ratio_precision(), false);
    }

    multicall
}

fn parse_multicall_results(
    results: Vec<U256>,
    excluded_len: usize,
    pool_len: usize,
) -> MulticallResults {
    let mut iter = results.into_iter();

    let total_supply = iter.next().expect("Missing total supply");
    let burn_balance = iter.next().expect("Missing burn balance");

    let excluded_balances: Vec<U256> = (0..excluded_len).map(|_| iter.next().expect("Missing excluded balance")).collect();

    let pool_data: Vec<PoolData> = (0..pool_len).map(|_| PoolData {
        initial: iter.next().expect("Missing initial"),
        pool_creation: iter.next().expect("Missing pool creation"),
        blocks_per_day: iter.next().expect("Missing blocks per day"),
        lock_days: iter.next().expect("Missing lock days"),
        vesting_days: iter.next().expect("Missing vesting days"),
        ratio_precision: iter.next().expect("Missing ratio precision"),
    }).collect();

    MulticallResults {
        total_supply,
        burn_balance,
        excluded_balances,
        pool_data,
    }
}

pub async fn get_total_supply(contract: &ERC20<impl Middleware + 'static>, decimals: u8) -> Result<String, anyhow::Error> {
    let multicall = build_multicall(contract, &[], &[]).await;
    let results: Vec<U256> = multicall.call_array().await?;
    let parsed = parse_multicall_results(results, 0, 0);
    
    // Safe subtraction to prevent underflow
    let value = if parsed.burn_balance < parsed.total_supply {
        parsed.total_supply - parsed.burn_balance
    } else {
        U256::zero()
    };
    
    Ok(utils::u256_to_human(value, decimals))
}

pub async fn get_circulating_supply(
    contract: &ERC20<impl Middleware + 'static>,
    excluded_addresses: &[Address],
    pool_addresses: &[Address],
    decimals: u8,
) -> Result<String, anyhow::Error> {
    let multicall = build_multicall(contract, excluded_addresses, pool_addresses).await;
    let results: Vec<U256> = multicall.call_array().await?;
    let parsed = parse_multicall_results(results, excluded_addresses.len(), pool_addresses.len());

    let excluded_balance = parsed.excluded_balances.iter().fold(U256::zero(), |acc, &b| acc + b);

    let current_block = contract.client().get_block_number().await?;

    // Filter out pools that are in the excluded list to avoid double counting
    let mut locked_balance = U256::zero();
    for (i, pool_addr) in pool_addresses.iter().enumerate() {
        // Skip this pool if it's in the excluded addresses list
        if excluded_addresses.contains(pool_addr) {
            continue;
        }
        
        // Calculate locked amount for this pool
        if i < parsed.pool_data.len() {
            let data = &parsed.pool_data[i];
            let calc = calculate_pool_vesting(
                data.initial,
                data.pool_creation,
                data.blocks_per_day,
                data.lock_days,
                data.vesting_days,
                data.ratio_precision,
                current_block,
            );
            locked_balance = locked_balance + calc.locked_amount;
        }
    }

    // Safe arithmetic operations to prevent overflow
    let mut value = parsed.total_supply;
    
    // Subtract excluded balance safely
    if excluded_balance < value {
        value = value - excluded_balance;
    } else {
        value = U256::zero();
    }
    
    // Subtract locked balance safely
    if locked_balance < value {
        value = value - locked_balance;
    } else {
        value = U256::zero();
    }
    
    // Subtract burn balance safely
    if parsed.burn_balance < value {
        value = value - parsed.burn_balance;
    } else {
        value = U256::zero();
    }
    
    Ok(utils::u256_to_human(value, decimals))
}