// src/config.rs
use ethers::types::Address;
use serde_json;

pub fn read_excluded_addresses() -> Vec<Address> {
    let content = include_str!("../config/excluded_address_list.json");
    serde_json::from_str(&content).expect("Failed to parse excluded address list")
}

pub fn read_pool_addresses() -> Vec<Address> {
    let content = include_str!("../config/pool_address_list.json");
    serde_json::from_str(&content).expect("Failed to parse pool address list")
}

pub fn validate_address_lists() -> Result<(), String> {
    let excluded_addresses = read_excluded_addresses();
    let pool_addresses = read_pool_addresses();

    let mut duplicates = Vec::new();

    for pool_addr in &pool_addresses {
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
            💡 TIP: Pool addresses should only be in 'config/pool_address_list.json', \
            not in the excluded list since they are handled separately in the vesting calculations.\n",
            duplicates.join("\n")
        ));
    }

    Ok(())
}
