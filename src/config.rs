use ethers::types::{Address, U256};
use serde_json;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct PoolEntry {
    addresses: Vec<AddressInfo>,
    tge_percentage: u64,
    cliff: u64,
    vesting: u64,
    balance_at_tge: u64,
    #[serde(default = "default_vesting_type")]
    vesting_type: String,
}

fn default_vesting_type() -> String {
    "linear".to_string()
}

#[derive(Serialize, Deserialize)]
struct AddressInfo {
    address: Address,
    chain: String,
}

pub fn read_excluded_addresses() -> Vec<Address> {
    let content = include_str!("../config/excluded_address_list.json");
    let entries: Vec<PoolEntry> = serde_json::from_str(&content).expect("Failed to parse excluded address list");
    entries
        .into_iter()
        .flat_map(|entry| entry.addresses.into_iter().map(|info| info.address))
        .collect()
}

pub fn read_pool_data() -> Vec<(Vec<(Address, String)>, U256, U256, U256, String, U256)> {
    let content = include_str!("../config/excluded_address_list.json");
    let pool_entries: Vec<PoolEntry> = serde_json::from_str(&content).expect("Failed to parse pool address list");
    pool_entries
        .into_iter()
        .map(|entry| (
            entry.addresses.into_iter().map(|info| (info.address, info.chain)).collect(),
            U256::from(entry.tge_percentage),
            U256::from(entry.cliff),
            U256::from(entry.vesting),
            entry.vesting_type,
            U256::from(entry.balance_at_tge) * U256::from(10u64.pow(18)), // Convert to wei
        ))
        .collect()
}

pub fn read_onchain_pool_addresses() -> Vec<Address> {
    let content = include_str!("../config/pool_address_list.json");
    serde_json::from_str(&content).expect("Failed to parse onchain pool address list")
}

pub fn validate_address_lists() -> Result<(), String> {
    let excluded_addresses = read_excluded_addresses();
    let onchain_pool_addresses = read_onchain_pool_addresses();

    let mut duplicates = Vec::new();

    for pool_addr in &onchain_pool_addresses {
        if excluded_addresses.contains(pool_addr) {
            duplicates.push(format!("0x{:x}", pool_addr));
        }
    }

    if !duplicates.is_empty() {
        return Err(format!(
            "\n❌ CONFIGURATION ERROR ❌\n\n\
            🚫 Pool addresses found in excluded addresses list!\n\n\
            This would cause double counting in supply calculations.\n\n\
            🔧 TO FIX:\n\
            Remove these addresses from 'config/excluded_address_list.json':\n\n\
            {}\n\n\
            💡 TIP: Pool addresses should only be in 'config/pool_address_list.json' or vesting data, \
            not in the excluded list since they are handled separately in the vesting calculations.\n",
            duplicates.join("\n")
        ));
    }

    Ok(())
}