// src/config.rs
use ethers::types::Address;
use serde_json;
use std::fs;

pub fn read_excluded_addresses() -> Vec<Address> {
    let content = fs::read_to_string("config/excluded_address_list.json")
        .expect("Failed to read excluded address list");
    serde_json::from_str(&content).expect("Failed to parse excluded address list")
}

pub fn read_pool_addresses() -> Vec<Address> {
    let content = fs::read_to_string("config/pool_address_list.json")
        .expect("Failed to read pool address list");
    serde_json::from_str(&content).expect("Failed to parse pool address list")
}

pub fn warn_about_duplicate_addresses() {
    let excluded_addresses = read_excluded_addresses();
    let pool_addresses = read_pool_addresses();
    
    let mut duplicates = Vec::new();
    
    for pool_addr in &pool_addresses {
        if excluded_addresses.contains(pool_addr) {
            duplicates.push(format!("0x{:x}", pool_addr));
        }
    }
    
    if !duplicates.is_empty() {
        eprintln!("\n⚠️  WARNING ⚠️");
        eprintln!("\n🔍 Found {} pool address(es) in the excluded addresses list:", duplicates.len());
        eprintln!();
        for addr in &duplicates {
            eprintln!("   • {}", addr);
        }
        eprintln!();
        eprintln!("📋 IMPACT:");
        eprintln!("   • These pools will be EXCLUDED from both exclusion and vesting calculations");
        eprintln!("   • Their balances will be subtracted from total supply");
        eprintln!("   • Their locked amounts will NOT be calculated separately");
        eprintln!("   • This prevents double counting but may affect accuracy");
        eprintln!();
        eprintln!("💡 RECOMMENDATION:");
        eprintln!("   • If you want vesting calculations for these pools, remove them from excluded list");
        eprintln!("   • If you want to exclude them entirely, this configuration is correct");
        eprintln!("   • Current behavior: Pools in excluded list are skipped from vesting calculations");
        eprintln!();
    }
}