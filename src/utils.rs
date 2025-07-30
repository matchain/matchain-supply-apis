// src/utils.rs
use ethers::types::U256;

pub fn u256_to_human(value: U256, decimals: u8) -> String {
    if decimals == 0 {
        return value.to_string();
    }
    
    // Safe conversion to avoid overflow
    let divisor = U256::exp10(decimals as usize);
    
    // Check for zero divisor to avoid panic
    if divisor == U256::zero() {
        return "0".to_string();
    }
    
    let integer = value / divisor;
    let fraction_part = value % divisor;
    
    // Convert fraction to string and pad with zeros
    let mut fraction = fraction_part.to_string();
    let width = decimals as usize;
    
    // Pad with leading zeros to match decimal places
    while fraction.len() < width {
        fraction.insert(0, '0');
    }
    
    // Remove trailing zeros but keep at least one decimal place if there's a fraction
    while fraction.ends_with('0') && fraction.len() > 1 {
        fraction.pop();
    }
    
    if fraction == "0" || fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{}.{}", integer, fraction)
    }
}