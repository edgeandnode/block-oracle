use crate::{
    jrpc_utils::{get_latest_block, get_latest_blocks},
    Caip2ChainId, Config, EpochTracker, Error, JrpcExpBackoff, JrpcProviderForChain, NetworksDiff,
    SubgraphQuery, SubgraphStateTracker,
};
use epoch_encoding::{self as ee, BlockPtr, Encoder, Message, CURRENT_ENCODING_VERSION};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};
use web3::{
    contract::{Contract, Options},
    types::{Bytes, H256},
};

const CONTRACT_FUNCTION_NAME: &'static str = "crossChainEpochOracle";

/// The main application in-memory state
pub struct Oracle {
    config: &'static Config,
    epoch_tracker: EpochTracker,
    protocol_chain: JrpcProviderForChain<JrpcExpBackoff>,
    indexed_chains: Vec<JrpcProviderForChain<JrpcExpBackoff>>,
    subgraph_state: SubgraphStateTracker<SubgraphQuery>,
}

impl Oracle {
    pub fn new(config: &'static Config) -> Self {
        let subgraph_api = SubgraphQuery::new(config.subgraph_url.clone());
        let subgraph_state = SubgraphStateTracker::new(subgraph_api);
        let backoff_max = config.retry_strategy_max_wait_time;
        let epoch_tracker = EpochTracker::new(config);
        let protocol_chain = {
            let transport =
                JrpcExpBackoff::http(config.protocol_chain.jrpc_url.clone(), backoff_max);
            JrpcProviderForChain::new(config.protocol_chain.id.clone(), transport)
        };
        let indexed_chains = config
            .indexed_chains
            .iter()
            .map(|chain| {
                let transport = JrpcExpBackoff::http(chain.jrpc_url.clone(), backoff_max);
                JrpcProviderForChain::new(chain.id.clone(), transport)
            })
            .collect();

        Self {
            config,
            protocol_chain,
            indexed_chains,
            epoch_tracker,
            subgraph_state,
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        self.subgraph_state.refresh().await;
        if self.subgraph_state.error().is_some() {
            return Ok(());
        }

        let block = get_latest_block(self.protocol_chain.web3.clone())
            .await
            .map_err(Error::BadJrpcProtocolChain)?;
        debug!(
            block = block.number,
            hash = hex::encode(block.hash).as_str(),
            "Got the latest block from the protocol chain."
        );

        let is_new_epoch = self.epoch_tracker.is_new_epoch(block.number).await?;
        if !is_new_epoch {
            return Ok(());
        }

        info!("Entering a new epoch.");
        info!("Collecting latest block information from all indexed chains.");
        // Get indexed chains' latest blocks.
        let latest_blocks = get_latest_blocks(&self.indexed_chains).await?;
        let payload = self.produce_next_payload(latest_blocks)?;
        submit_call(self.config, self.protocol_chain.clone(), payload)
            .await
            .map_err(Error::CantSubmitTx)?;

        // TODO: After broadcasting a transaction to the protocol chain and getting a transaction
        // receipt, we should monitor it until it get enough confirmations. It's unclear which
        // component should do this task.

        Ok(())
    }

    fn produce_next_payload(
        &self,
        latest_blocks: HashMap<Caip2ChainId, BlockPtr>,
    ) -> Result<Vec<u8>, Error> {
        info!("A new epoch started in the protocol chain");
        let registered_networks = self.registered_networks()?;

        let mut messages = vec![];

        // First, we need to make sure that there are no pending
        // `RegisterNetworks` messages.
        let networks_diff = {
            // `NetworksDiff::calculate` uses u32's but `registered_networks` has u64's
            let networks_and_block_numbers = registered_networks
                .iter()
                .map(|(chain_id, network)| {
                    let block_number = u32::try_from(network.block_number).expect(&format!(
                        "expected a block number that would fit a u32, but found {}",
                        network.block_number
                    ));
                    (chain_id.clone(), block_number)
                })
                .collect();
            NetworksDiff::calculate(networks_and_block_numbers, self.config)
        };
        info!(
            created = networks_diff.insertions.len(),
            deleted = networks_diff.deletions.len(),
            "Performed indexed chain diffing."
        );
        if let Some(msg) = networks_diff_to_message(&networks_diff) {
            messages.push(msg);
        }

        messages.push(latest_blocks_to_message(latest_blocks));

        let available_networks: Vec<(String, epoch_encoding::Network)> = {
            // intersect networks from config and subgraph
            let config_chain_ids: HashSet<&Caip2ChainId> = self
                .config
                .indexed_chains
                .iter()
                .map(|chain| &chain.id)
                .collect();
            registered_networks
                .into_iter()
                .filter_map(|(chain_id, network)| {
                    if config_chain_ids.contains(&chain_id) {
                        Some((chain_id.as_str().to_owned(), network))
                    } else {
                        None
                    }
                })
                .collect()
        };

        debug!(
            messages = ?messages,
            messages_count = messages.len(),
            networks_count = available_networks.len(),
            "Compressing message(s)."
        );

        let mut compression_engine = Encoder::new(CURRENT_ENCODING_VERSION, available_networks)
            .expect(format!("Can't prepare for encoding because something went wrong",).as_str());
        let encoded = compression_engine
            .encode(&messages[..])
            .expect(format!("Encoding failed: {:?}", messages).as_str());
        debug!(encoded = ?encoded, "Successfully encoded message(s).");
        Ok(encoded)
    }

    fn registered_networks(&self) -> Result<Vec<(Caip2ChainId, epoch_encoding::Network)>, Error> {
        if self.subgraph_state.is_failed() {
            todo!("Handle this as an error")
        }
        if self.subgraph_state.is_uninitialized() {
            info!("Epoch Subgraph contains no initial state");
            return Ok(Default::default());
        };
        info!("subgraph data is {:?}", self.subgraph_state.last_state());
        Ok(self
            .subgraph_state
            .last_state()
            .expect("expected data from a valid subgraph state, but found none")
            .networks
            .iter()
            .map(|network| {
                (
                    network.id.clone(),
                    epoch_encoding::Network {
                        block_number: network.latest_block_number,
                        block_delta: network.delta,
                    },
                )
            })
            .collect())
    }
}

fn latest_blocks_to_message(latest_blocks: HashMap<Caip2ChainId, BlockPtr>) -> ee::Message {
    Message::SetBlockNumbersForNextEpoch(
        latest_blocks
            .into_iter()
            .map(|(chain_id, block_ptr)| (chain_id.as_str().to_owned(), block_ptr))
            .collect(),
    )
}

fn networks_diff_to_message(diff: &NetworksDiff) -> Option<ee::Message> {
    if diff.deletions.is_empty() && diff.insertions.is_empty() {
        None
    } else {
        Some(ee::Message::RegisterNetworks {
            remove: diff.deletions.iter().map(|x| *x.1 as u64).collect(),
            add: diff
                .insertions
                .iter()
                .map(|x| x.0.as_str().to_string())
                .collect(),
        })
    }
}

async fn submit_call<T>(
    config: &Config,
    protocol_chain: JrpcProviderForChain<T>,
    payload: Vec<u8>,
) -> web3::Result<H256>
where
    T: web3::Transport,
{
    let contract = Contract::from_json(
        protocol_chain.web3.eth(),
        config.contract_address,
        include_bytes!("abi/data_edge.json"),
    )
    .expect("Can't read the ABI JSON file");

    let payload = Bytes::from(payload);
    let tx = contract
        .signed_call(
            CONTRACT_FUNCTION_NAME,
            (payload,),
            Options::default(),
            &config.owner_private_key,
        )
        .await?;
    info!(transaction_hash = ?tx, "Sent transaction");
    Ok(tx)
}

mod freshness {
    use crate::{jrpc_utils::calls_in_block_range, models::JrpcProviderForChain};
    use thiserror::Error;
    use tracing::{debug, error, trace};
    use web3::types::{H160, U64};

    #[derive(Debug, Error)]
    enum FreshnessCheckEror {
        #[error("Epoch Subgraph advanced beyond protocol chain's head")]
        SubgraphBeyondChain,
        #[error(transparent)]
        Web3(#[from] web3::Error),
    }

    /// Number of blocks that the Epoch Subgraph may be away from the protocol chain's head. If the
    /// block distance is lower than this, a `trace_filter` JSON RPC call will be used to infer if
    /// any relevant transaction happened within that treshold.
    ///
    /// This should be configurable.
    const FRESHNESS_THRESHOLD: u64 = 10;

    /// The Epoch Subgraph is considered fresh if it has processed all relevant transactions
    /// targeting the DataEdge contract.
    ///
    /// To assert that, the Block Oracle will need to get the latest block from a JSON RPC provider
    /// and compare its number with the subgraph’s current block.
    ///
    /// If they are way too different, then the subgraph is not fresh, and we should gracefully
    /// handle that error.
    ///
    /// Otherwise, if block numbers are under a certain threshold apart, we could scan the blocks
    /// in between and ensure they’re not relevant to the DataEdge contract.
    async fn subgaph_is_fresh<T>(
        subgraph_latest_block: U64,
        current_block: U64,
        protocol_chain: JrpcProviderForChain<T>,
        owner_address: H160,
        contract_address: H160,
    ) -> Result<bool, FreshnessCheckEror>
    where
        T: web3::Transport,
    {
        // If this ever happens, then there must be a serious bug in the code
        if subgraph_latest_block > current_block {
            let anomaly = FreshnessCheckEror::SubgraphBeyondChain;
            error!(%anomaly);
            return Err(anomaly);
        }
        let block_distance = (current_block - subgraph_latest_block).as_u64();
        if block_distance == 0 {
            return Ok(true);
        } else if block_distance > FRESHNESS_THRESHOLD {
            debug!(
                %subgraph_latest_block,
                %current_block,
                "Epoch Subgraph is not considered fresh because it is {} blocks behind \
                 protocol chain's head",
                block_distance
            );
            return Ok(false);
        }
        // Scan the blocks in betwenn for transactions from the Owner to the Data Edge contract
        let calls = calls_in_block_range(
            protocol_chain.web3,
            subgraph_latest_block,
            current_block,
            owner_address,
            contract_address,
        )
        .await?;

        if calls.is_empty() {
            trace!(
                %subgraph_latest_block,
                %current_block,
                "Epoch Subgraph is fresh. \
                 Found no calls between last synced block and the protocol chain's head",
            );
            Ok(true)
        } else {
            debug!(
                %subgraph_latest_block,
                %current_block,
                "Epoch Subgraph is not fresh. \
                 Found {} calls between the last synced block and the protocol chain's head",
                calls.len()
            );
            Ok(false)
        }
    }
}
