use crate::{config::Config, models::JrpcProviderForChain};
use secp256k1::SecretKey;
use thiserror::Error;
use tracing::{error, info};
use web3::{
    contract::{Contract, Options},
    types::Bytes,
};

const FUNCTION_NAME: &'static str = "crossChainEpochOracle";

#[derive(Debug, Error)]
pub enum EmitterError {
    #[error("Failed to broadcast the signed transaction")]
    BroadcastTransaction(#[from] web3::Error),
}

impl crate::MainLoopFlow for EmitterError {
    fn instruction(&self) -> crate::OracleControlFlow {
        use std::ops::ControlFlow::*;
        use EmitterError::*;
        match self {
            error @ BroadcastTransaction(json_rpc_error) => {
                error!(%json_rpc_error, "{error}");
                Continue(None)
            }
        }
    }
}

/// Responsible for receiving the encoded payload, constructing and signing the
/// transactions to Ethereum Mainnet.
pub struct Emitter<T>
where
    T: web3::Transport,
{
    contract: Contract<T>,
    owner_private_key: SecretKey,
}

impl<T> Emitter<T>
where
    T: web3::Transport,
{
    pub fn new(config: &Config, chain: JrpcProviderForChain<T>) -> Self {
        let contract = Contract::from_json(
            chain.web3.eth(),
            config.contract_address,
            include_bytes!("abi/data_edge.json"),
        )
        .expect("Can't read the ABI JSON file");

        Self {
            contract,
            owner_private_key: config.owner_private_key,
        }
    }

    pub async fn submit_oracle_messages(
        &mut self,
        data: Vec<u8>,
    ) -> Result<web3::types::H256, EmitterError> {
        let payload = Bytes::from(data);
        let tx = self
            .contract
            .signed_call(
                FUNCTION_NAME,
                (payload,),
                Options::default(),
                &self.owner_private_key,
            )
            .await?;
        info!(transaction_hash = ?tx, "Sent transaction");
        Ok(tx)
    }
}
