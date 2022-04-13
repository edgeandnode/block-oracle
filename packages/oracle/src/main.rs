mod config;
mod encoder;
mod event_source;
mod store;

use async_trait::async_trait;
use epoch_encoding::{self, Blockchain, Transaction};
use event_source::EventSource;
use lazy_static::lazy_static;
use std::collections::HashMap;
use store::models::Caip2ChainId;
use web3::transports::Http;
use web3::types::{Bytes, TransactionParameters, U256};

pub use encoder::Encoder;
pub use store::Store;

lazy_static! {
    pub static ref CONFIG: config::Config = config::Config::parse();
}

/// Responsible for receiving the encodede payload, constructing and signing the transactions to
/// Ethereum Mainnet.
type EthereumClient = ();

/// Tracks current Ethereum mainnet epoch.
type EpochTracker = ();

// -------------

type BlockChainState = ();

/// The main application in-memory state
struct Oracle {
    // -- components --
    store: Store,
    event_source: EventSource,
    encoder: Encoder,
    ethereum_client: EthereumClient,
    epoch_tracker: EpochTracker,

    // -- data --
    state_by_blockchain: HashMap<Caip2ChainId, BlockChainState>,
}

pub struct Web3JsonRpc {
    client: web3::Web3<Http>,
}

impl Web3JsonRpc {
    pub fn new(transport: web3::transports::Http) -> Self {
        let client = web3::Web3::new(transport);
        Self { client }
    }
}

#[async_trait]
impl Blockchain for Web3JsonRpc {
    type Err = String;

    async fn submit_oracle_messages(&mut self, transaction: Transaction) -> Result<(), Self::Err> {
        let tx_object = TransactionParameters {
            to: Some(CONFIG.contract_address.clone()),
            value: U256::zero(),
            nonce: Some(transaction.nonce.into()),
            data: Bytes::from(transaction.payload),
            ..Default::default()
        };
        let private_key = CONFIG.owner_private_key.clone();
        let signed = self
            .client
            .accounts()
            .sign_transaction(tx_object, &private_key)
            .await
            .unwrap();

        self.client
            .eth()
            .send_raw_transaction(signed.raw_transaction)
            .await
            .unwrap();

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Immediately dereference `CONFIG` to trigger `lazy_static` initialization.
    let _ = &*CONFIG;

    let store = Store::new(CONFIG.database_url.as_str()).await?;
    let networks = store.networks().await?;

    let json_rpc = Web3JsonRpc::new(Http::new("http://localhost:8545").unwrap());

    Ok(())
}
