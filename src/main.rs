use anyhow::Result as AnyhowResult;
use axum::{Router, extract::State, routing::get};
use dotenvy::dotenv;
use ethers::providers::{Http, Provider};
use ethers::types::{Address, U256};
use matchain_supply_apis::{ERC20, config, supply};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    matchain_contract: Arc<ERC20<Provider<Http>>>,
    bsc_contract: Arc<ERC20<Provider<Http>>>,
    excluded_addresses: Vec<(Address, String)>,
    pool_data: Vec<(Vec<(Address, String)>, U256, U256, U256, String, U256)>,
    onchain_pool_addresses: Vec<Address>,
    tge_timestamp: U256,
    decimals: u8,
}

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    dotenv().ok();

    if let Err(e) = config::validate_address_lists() {
        eprintln!("Configuration Error: {}", e);
        std::process::exit(1);
    }

    let rpc_url = env::var("RPC_URL")?;
    let bnb_rpc_url = env::var("BNB_RPC_URL")?;
    let token_address = env::var("TOKEN_ADDRESS")?.parse::<Address>()?;
    let mat_bnb_token_address = env::var("MAT_BNB_TOKEN_ADDRESS")?.parse::<Address>()?;
    let tge_timestamp = U256::from(env::var("TGE_TIMESTAMP")?.parse::<u64>()?);

    let matchain_provider = Provider::<Http>::try_from(rpc_url)?;
    let bsc_provider = Provider::<Http>::try_from(bnb_rpc_url)?;

    let matchain_contract = Arc::new(ERC20::new(token_address, Arc::new(matchain_provider)));
    let bsc_contract = Arc::new(ERC20::new(mat_bnb_token_address, Arc::new(bsc_provider)));

    let decimals = matchain_contract.decimals().call().await?;

    let excluded_addresses = config::read_excluded_addresses();
    let pool_data = config::read_pool_data();
    let onchain_pool_addresses = config::read_onchain_pool_addresses();

    let state = Arc::new(AppState {
        matchain_contract,
        bsc_contract,
        excluded_addresses,
        pool_data,
        onchain_pool_addresses,
        tge_timestamp,
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

async fn total_supply(State(state): State<Arc<AppState>>) -> String {
    match supply::get_total_supply(&state.matchain_contract, state.decimals).await {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error calculating total supply: {:?}", e);
            "0".to_string()
        }
    }
}

async fn circulating_supply(State(state): State<Arc<AppState>>) -> String {
    match supply::get_circulating_supply(
        &state.matchain_contract,
        &state.excluded_addresses,
        &state.pool_data,
        &state.onchain_pool_addresses,
        state.tge_timestamp,
        state.decimals,
    )
    .await
    {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error calculating circulating supply: {:?}", e);
            "0".to_string()
        }
    }
}