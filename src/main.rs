// src/main.rs
use axum::{extract::State, response::Json, routing::get, Router};
use dotenvy::dotenv;
use ethers::providers::{Http, Provider};
use ethers::types::Address;
use matchain_supply_apis::{config, supply, ERC20};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use anyhow::Result as AnyhowResult;

#[derive(Clone)]
struct AppState {
    contract: Arc<ERC20<Provider<Http>>>,
    excluded_addresses: Vec<Address>,
    pool_addresses: Vec<Address>,
    decimals: u8,
}

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    dotenv().ok();

    let rpc_url = env::var("RPC_URL")?;
    let provider = Arc::new(Provider::<Http>::try_from(rpc_url)?);

    let token_address = env::var("TOKEN_ADDRESS")?.parse::<Address>()?;

    let contract = Arc::new(ERC20::new(token_address, provider.clone()));

    let decimals = contract.decimals().call().await?;

    let excluded_addresses = config::read_excluded_addresses();
    let pool_addresses = config::read_pool_addresses();

    let state = Arc::new(AppState {
        contract,
        excluded_addresses,
        pool_addresses,
        decimals,
    });

    let app = Router::new()
        .route("/total-supply", get(total_supply))
        .route("/circulating-supply", get(circulating_supply))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn total_supply(State(state): State<Arc<AppState>>) -> Json<String> {
    match supply::get_total_supply(&state.contract, state.decimals).await {
        Ok(value) => Json(value),
        Err(_) => Json("Error calculating total supply".to_string()),
    }
}

async fn circulating_supply(State(state): State<Arc<AppState>>) -> Json<String> {
    match supply::get_circulating_supply(
        &state.contract,
        &state.excluded_addresses,
        &state.pool_addresses,
        state.decimals,
    ).await {
        Ok(value) => Json(value),
        Err(_) => Json("Error calculating circulating supply".to_string()),
    }
}