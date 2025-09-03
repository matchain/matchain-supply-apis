use crate::{ERC20, StakingPool, utils};
use ethers::contract::Multicall;
use ethers::providers::Middleware;
use ethers::types::{Address, U256};
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

pub fn calculate_pool_vesting(
    initial: U256,
    tge_percentage: U256,
    cliff: U256,
    vesting: U256,
    ratio_precision: U256,
    current_ts: U256,
    tge_ts: U256,
    vesting_type: &str,
) -> PoolCalculation {
    // Bounds checking
    if initial > U256::from(10u128.pow(27)) || cliff > U256::from(2190) || vesting > U256::from(2190) || ratio_precision < U256::from(1000) || ratio_precision > U256::from(10u128.pow(16)) {
        eprintln!("Invalid vesting parameters: initial={}, cliff={}, vesting={}, ratio_precision={}", initial, cliff, vesting, ratio_precision);
        return PoolCalculation {
            initial,
            ratio_precision,
            locked_amount: U256::zero(),
            days_passed: U256::zero(),
            days_until_lock_ends: U256::zero(),
            days_until_vesting_ends: U256::zero(),
            unlocked_fraction: U256::zero(),
        };
    }

    let seconds_passed = current_ts.checked_sub(tge_ts).unwrap_or(U256::zero());
    let days_passed = seconds_passed / U256::from(86400u64);

    let days_until_lock_ends = cliff.checked_sub(days_passed).unwrap_or(U256::zero());

    let total_vesting_period = cliff.checked_add(vesting).unwrap_or(cliff);
    let days_until_vesting_ends = total_vesting_period.checked_sub(days_passed).unwrap_or(U256::zero());

    let unlocked_fraction = if vesting_type == "stepped" {
        if days_passed < cliff {
            tge_percentage * (ratio_precision / U256::from(100u64))
        } else {
            let periods_passed = (days_passed.checked_sub(cliff).unwrap_or(U256::zero())) / U256::from(90);
            let periods = cmp::min(periods_passed, U256::from(6));
            let step_percentage = U256::from(166700);
            let tge_scaled = tge_percentage * (ratio_precision / U256::from(100u64));
            let mut remaining = ratio_precision.checked_sub(tge_scaled).unwrap_or(U256::zero());
            let mut unlocked = tge_scaled;
            for _ in 0..periods.as_u64() {
                let release = (remaining * step_percentage) / ratio_precision;
                unlocked = unlocked.checked_add(release).unwrap_or(unlocked);
                remaining = remaining.checked_sub(release).unwrap_or(remaining);
            }
            cmp::min(unlocked, ratio_precision)
        }
    } else {
        // Linear vesting
        if days_passed < cliff {
            tge_percentage * (ratio_precision / U256::from(100u64))
        } else if vesting > U256::zero() {
            let vesting_progress = days_passed.checked_sub(cliff).unwrap_or(U256::zero());
            let frac = (vesting_progress.checked_mul(ratio_precision).unwrap_or(U256::zero()) / vesting)
                .checked_add(tge_percentage * (ratio_precision / U256::from(100u64)))
                .unwrap_or(tge_percentage * (ratio_precision / U256::from(100u64)));
            cmp::min(frac, ratio_precision)
        } else {
            tge_percentage * (ratio_precision / U256::from(100u64))
        }
    };

    let unlocked = if ratio_precision > U256::zero() {
        (initial * unlocked_fraction) / ratio_precision
    } else {
        U256::zero()
    };

    let locked = initial.checked_sub(unlocked).unwrap_or(U256::zero());

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

pub async fn get_total_supply(
    matchain_contract: &ERC20<impl Middleware + Clone + 'static>,
    bsc_contract: &ERC20<impl Middleware + Clone + 'static>,
    decimals: u8,
) -> Result<String, anyhow::Error> {
    let mut matchain_multicall = Multicall::new(matchain_contract.client(), Some("0xcA11bde05977b3631167028862bE2a173976CA11".parse::<Address>().unwrap())).await?;
    matchain_multicall.add_call(matchain_contract.total_supply(), false);
    matchain_multicall.add_call(matchain_contract.balance_of(Address::zero()), false);
    let matchain_results: Vec<U256> = matchain_multicall.call_array().await?;

    let mut bsc_multicall = Multicall::new(bsc_contract.client(), Some("0xcA11bde05977b3631167028862bE2a173976CA11".parse::<Address>().unwrap())).await?;
    bsc_multicall.add_call(bsc_contract.total_supply(), false);
    bsc_multicall.add_call(bsc_contract.balance_of(Address::zero()), false);
    let bsc_results: Vec<U256> = bsc_multicall.call_array().await?;

    let total_m = matchain_results[0];
    let burn_m = matchain_results[1];
    let total_b = bsc_results[0];
    let burn_b = bsc_results[1];

    let value = (total_m.checked_sub(burn_m).unwrap_or(U256::zero())) + (total_b.checked_sub(burn_b).unwrap_or(U256::zero()));
    eprintln!("Total Supply: Matchain = {}, BSC = {}, Burned = {}, Value = {}", total_m, total_b, burn_m + burn_b, value);

    Ok(utils::u256_to_human(value, decimals))
}

pub async fn get_circulating_supply(
    matchain_contract: &ERC20<impl Middleware + Clone + 'static>,
    bsc_contract: &ERC20<impl Middleware + Clone + 'static>,
    excluded_addresses: &[(Address, String)],
    pool_data: &[(Vec<(Address, String)>, U256, U256, U256, String, U256)],
    onchain_pool_addresses: &[Address],
    tge_timestamp: U256,
    decimals: u8,
) -> Result<String, anyhow::Error> {
    let current_block = matchain_contract.client().get_block_number().await?;
    let current_ts = matchain_contract.client().get_block(current_block).await?.map(|block| block.timestamp).unwrap_or(U256::zero());
    eprintln!("Current Block: {}, Current TS: {}", current_block, current_ts);

    let pool_addresses: Vec<Address> = pool_data
        .iter()
        .flat_map(|(addrs, _, _, _, _, _)| addrs.iter().map(|(addr, _)| *addr))
        .collect();

    let mut matchain_multicall = Multicall::new(matchain_contract.client(), Some("0xcA11bde05977b3631167028862bE2a173976CA11".parse::<Address>().unwrap())).await?;
    matchain_multicall.add_call(matchain_contract.total_supply(), false);
    matchain_multicall.add_call(matchain_contract.balance_of(Address::zero()), false);

    for (addr, chain) in excluded_addresses {
        if chain == "Matchain" {
            matchain_multicall.add_call(matchain_contract.balance_of(*addr), false);
        }
    }

    for (i, &addr) in onchain_pool_addresses.iter().enumerate() {
        let pool = StakingPool::new(addr, matchain_contract.client().clone());
        matchain_multicall.add_call(pool.initial_self_stake_amount(), false);
        matchain_multicall.add_call(pool.initial_lock_period(), false);
        matchain_multicall.add_call(pool.vesting_duration(), false);
        matchain_multicall.add_call(pool.ratio_precision(), false);
        eprintln!("Adding Pool call for address {}: {}", i, addr);
    }

    let matchain_results: Vec<U256> = matchain_multicall.call_array().await?;
    eprintln!("Matchain Results (length={}): {:?}", matchain_results.len(), matchain_results);

    let mut bsc_multicall = Multicall::new(bsc_contract.client(), Some("0xcA11bde05977b3631167028862bE2a173976CA11".parse::<Address>().unwrap())).await?;
    bsc_multicall.add_call(bsc_contract.total_supply(), false);
    bsc_multicall.add_call(bsc_contract.balance_of(Address::zero()), false);

    for (addr, chain) in excluded_addresses {
        if chain == "BSC" {
            bsc_multicall.add_call(bsc_contract.balance_of(*addr), false);
        }
    }

    let bsc_results: Vec<U256> = bsc_multicall.call_array().await?;
    eprintln!("BSC Results (length={}): {:?}", bsc_results.len(), bsc_results);

    let mut m_iter = matchain_results.into_iter();
    let total_m = m_iter.next().ok_or_else(|| anyhow::anyhow!("Missing Matchain total supply"))?;
    let burn_m = m_iter.next().ok_or_else(|| anyhow::anyhow!("Missing Matchain burn balance"))?;
    let matchain_excluded_balances: Vec<U256> = excluded_addresses
        .iter()
        .filter(|(_, chain)| chain == "Matchain")
        .map(|_| m_iter.next().ok_or_else(|| anyhow::anyhow!("Missing excluded balance")).unwrap())
        .collect();

    let onchain_pool_data: Vec<(U256, U256, U256, U256)> = (0..onchain_pool_addresses.len())
        .map(|i| {
            let initial = m_iter.next().ok_or_else(|| anyhow::anyhow!("Missing initial stake for pool {}", i)).unwrap();
            let lock_blocks = m_iter.next().ok_or_else(|| anyhow::anyhow!("Missing lock period for pool {}", i)).unwrap();
            let vesting_blocks = m_iter.next().ok_or_else(|| anyhow::anyhow!("Missing vesting duration for pool {}", i)).unwrap();
            let ratio_precision = m_iter.next().ok_or_else(|| anyhow::anyhow!("Missing ratio precision for pool {}", i)).unwrap();
            let lock_days = lock_blocks / U256::from(172800u64); // 0.5s/block
            let vesting_days = vesting_blocks / U256::from(172800u64);
            // Bounds checking
            if initial > U256::from(10u128.pow(27)) || lock_days > U256::from(2190) || vesting_days > U256::from(2190) || ratio_precision < U256::from(1000) || ratio_precision > U256::from(10u128.pow(16)) {
                eprintln!("Invalid Pool data for address {}: initial={}, lock_days={}, vesting_days={}, ratio_precision={}", onchain_pool_addresses[i], initial, lock_days, vesting_days, ratio_precision);
                (U256::zero(), U256::zero(), U256::zero(), U256::from(1_000_000))
            } else {
                (initial, lock_days, vesting_days, ratio_precision)
            }
        })
        .collect();
    eprintln!("Pool Data: {:?}", onchain_pool_data);

    let mut b_iter = bsc_results.into_iter();
    let total_b = b_iter.next().ok_or_else(|| anyhow::anyhow!("Missing BSC total supply"))?;
    let burn_b = b_iter.next().ok_or_else(|| anyhow::anyhow!("Missing BSC burn balance"))?;
    let bsc_excluded_balances: Vec<U256> = excluded_addresses
        .iter()
        .filter(|(_, chain)| chain == "BSC")
        .map(|_| b_iter.next().ok_or_else(|| anyhow::anyhow!("Missing excluded balance")).unwrap())
        .collect();

    let total_supply = (total_m.checked_sub(burn_m).unwrap_or(U256::zero())) + (total_b.checked_sub(burn_b).unwrap_or(U256::zero()));
    let excluded_balance = matchain_excluded_balances.iter().chain(bsc_excluded_balances.iter()).fold(U256::zero(), |acc, &b| acc + b);

    let ratio_precision = U256::from(1_000_000u64);
    let mut locked_balance = U256::zero();
    let mut wallet_details = Vec::new();
    let mut pool_details = Vec::new();
    let mut excluded_details = Vec::new();

    for (addrs, tge_percentage, cliff, vesting, vesting_type, balance_at_tge) in pool_data {
        let initial = *balance_at_tge;
        let calc = calculate_pool_vesting(
            initial,
            *tge_percentage,
            *cliff,
            *vesting,
            ratio_precision,
            current_ts,
            tge_timestamp,
            vesting_type,
        );
        locked_balance = locked_balance.checked_add(calc.locked_amount).unwrap_or(locked_balance);
        let unlocked_percent = (calc.unlocked_fraction * U256::from(100)) / ratio_precision;
        let initial_tokens = utils::u256_to_human(initial, decimals);
        let locked_tokens = utils::u256_to_human(calc.locked_amount, decimals);
        wallet_details.push((addrs.clone(), initial_tokens, locked_tokens, unlocked_percent, tge_percentage, cliff, vesting, vesting_type.clone()));
    }

    for (i, (initial, lock_days, vesting_days, ratio_precision)) in onchain_pool_data.iter().enumerate() {
        let calc = calculate_pool_vesting(
            *initial,
            U256::zero(),
            *lock_days,
            *vesting_days,
            *ratio_precision,
            current_ts,
            tge_timestamp,
            "linear",
        );
        locked_balance = locked_balance.checked_add(calc.locked_amount).unwrap_or(locked_balance);
        let unlocked_percent = (calc.unlocked_fraction * U256::from(100)) / *ratio_precision;
        let initial_tokens = utils::u256_to_human(*initial, decimals);
        let locked_tokens = utils::u256_to_human(calc.locked_amount, decimals);
        pool_details.push((onchain_pool_addresses[i], initial_tokens, locked_tokens, unlocked_percent, *lock_days, *vesting_days));
    }

    for (i, (addr, chain)) in excluded_addresses.iter().enumerate() {
        let balance = if chain == "Matchain" {
            matchain_excluded_balances.get(i).copied().unwrap_or(U256::zero())
        } else {
            bsc_excluded_balances.get(i - matchain_excluded_balances.len()).copied().unwrap_or(U256::zero())
        };
        let balance_tokens = utils::u256_to_human(balance, decimals);
        excluded_details.push((addr, chain, balance_tokens));
    }

    let total_supply_tokens = utils::u256_to_human(total_supply, decimals);
    let excluded_balance_tokens = utils::u256_to_human(excluded_balance, decimals);
    let locked_balance_tokens = utils::u256_to_human(locked_balance, decimals);
    let circulating_supply = total_supply.checked_sub(excluded_balance).unwrap_or(U256::zero()).checked_sub(locked_balance).unwrap_or(U256::zero());
    let circulating_supply_tokens = utils::u256_to_human(circulating_supply, decimals);

    // Formatted terminal output
    eprintln!("\n=============================================================");
    eprintln!("           Token Supply Overview (Block {})", current_block);
    eprintln!("=============================================================");
    eprintln!("Total Supply       : {} tokens", total_supply_tokens);
    eprintln!("Excluded Balance   : {} tokens", excluded_balance_tokens);
    eprintln!("Locked Balance     : {} tokens", locked_balance_tokens);
    eprintln!("Circulating Supply : {} tokens", circulating_supply_tokens);
    eprintln!("\nCalculation Breakdown:");
    eprintln!("- Total Supply = Matchain Total Supply + BSC Total Supply - Burned Tokens");
    eprintln!("- Circulating Supply = Total Supply - Excluded Balance - Locked Balance");
    eprintln!("- Excluded Balance = Sum of MAT balances from excluded addresses");
    eprintln!("- Locked Balance = Sum of locked tokens from vesting wallets and pools");
    eprintln!("\nExcluded Addresses:");
    eprintln!("{:-<60}", "");
    for (addr, chain, balance) in excluded_details {
        eprintln!("Address : {}\nChain   : {}\nBalance : {} tokens\n{:-<60}", addr, chain, balance, "");
    }
    eprintln!("\nWallet Vesting Details:");
    eprintln!("{:-<60}", "");
    for (addrs, initial, locked, unlocked_percent, tge_percentage, cliff, vesting, vesting_type) in wallet_details {
        let addrs_str = addrs.iter().map(|(addr, chain)| format!("{} ({})", addr, chain)).collect::<Vec<_>>().join(", ");
        eprintln!(
            "Addresses        : {}\nInitial Balance  : {} tokens\nLocked           : {} tokens\nUnlocked         : {}%\nSchedule         : TGE = {}%, Cliff = {} days, Vesting = {} days, Type = {}\n{:-<60}",
            addrs_str, initial, locked, unlocked_percent, tge_percentage, cliff, vesting, vesting_type, ""
        );
    }
    eprintln!("\nPool Vesting Details:");
    eprintln!("{:-<60}", "");
    for (addr, initial, locked, unlocked_percent, lock_days, vesting_days) in pool_details {
        eprintln!(
            "Address          : {}\nInitial Balance  : {} tokens\nLocked           : {} tokens\nUnlocked         : {}%\nSchedule         : Lock = {} days, Vesting = {} days\n{:-<60}",
            addr, initial, locked, unlocked_percent, lock_days, vesting_days, ""
        );
    }

    // Pie chart
    let total_supply_f64 = total_supply_tokens.parse::<f64>().unwrap_or(0.0);
    let excluded_f64 = excluded_balance_tokens.parse::<f64>().unwrap_or(0.0);
    let locked_f64 = locked_balance_tokens.parse::<f64>().unwrap_or(0.0);
    let circulating_f64 = circulating_supply_tokens.parse::<f64>().unwrap_or(0.0);
    let max_value = total_supply_f64;
    let excluded_percent = if max_value > 0.0 { (excluded_f64 / max_value) * 100.0 } else { 0.0 };
    let locked_percent = if max_value > 0.0 { (locked_f64 / max_value) * 100.0 } else { 0.0 };
    let circulating_percent = if max_value > 0.0 { (circulating_f64 / max_value) * 100.0 } else { 0.0 };
    let max_bar_length = 50;
    let excluded_bar = ((excluded_percent / 100.0) * max_bar_length as f64) as usize;
    let locked_bar = ((locked_percent / 100.0) * max_bar_length as f64) as usize;
    let circulating_bar = ((circulating_percent / 100.0) * max_bar_length as f64) as usize;

    eprintln!("\nSupply Distribution Pie Chart:");
    eprintln!(
        "Excluded ({:.1}%): [{}]{:.2}M",
        excluded_percent,
        "█".repeat(excluded_bar),
        excluded_f64 / 1e6
    );
    eprintln!(
        "Locked ({:.1}%): [{}]{:.2}M",
        locked_percent,
        "█".repeat(locked_bar),
        locked_f64 / 1e6
    );
    eprintln!(
        "Circulating ({:.1}%): [{}]{:.2}M",
        circulating_percent,
        "█".repeat(circulating_bar),
        circulating_f64 / 1e6
    );
    eprintln!("=====================================\n");

    Ok(circulating_supply_tokens)
}