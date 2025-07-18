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