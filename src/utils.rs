// src/utils.rs
use ethers::types::U256;

pub fn u256_to_human(value: U256, decimals: u8) -> String {
    if decimals == 0 {
        return value.to_string();
    }
    let divisor = U256::exp10(decimals as usize);
    let integer = value / divisor;
    let mut fraction = (value % divisor).to_string();
    let width = decimals as usize;
    while fraction.len() < width {
        fraction.insert(0, '0');
    }
    while fraction.ends_with('0') && !fraction.is_empty() {
        fraction.pop();
    }
    if fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{}.{}", integer, fraction)
    }
}