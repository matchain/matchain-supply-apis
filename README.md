# Matchain Supply APIs

Rust API server for calculating total and circulating supply of tokens on Matchain.

## Setup

1. Clone repo: `git clone <repo-url>`
2. Install Rust: <https://rustup.rs>
3. Copy `.env.example` to `.env` and fill values (RPC_URL, TOKEN_ADDRESS).
4. Build: `cargo build --release`
5. Run: `cargo run --release`

## Endpoints

- `GET /total-supply`: Total supply (human-readable).
- `GET /circulating-supply`: Circulating supply (human-readable).

## Config

- `config/excluded_address_list.json`: Array of excluded addresses.
- `config/pool_address_list.json`: Array of pool addresses.
- `abi/staking_pool_abi.json`: Staking pool ABI.

## Dependencies

- axum
- ethers
- dotenvy
- serde_json
- anyhow

See `Cargo.toml` for versions.

## License

MIT
