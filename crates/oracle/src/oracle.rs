use crate::{
    hex_string,
    jrpc_utils::{get_latest_block, get_latest_blocks},
    Caip2ChainId, Config, EpochTracker, Error, JrpcExpBackoff, JrpcProviderForChain, NetworksDiff,
    SubgraphQuery, SubgraphStateTracker,
};
use epoch_encoding::{self as ee, BlockPtr, Encoder, Message, CURRENT_ENCODING_VERSION};
use std::collections::{BTreeMap, HashSet};
use tracing::{debug, error, info, warn};
use web3::{
    contract::{Contract, Options},
    types::{Bytes, H256},
};

const CONTRACT_FUNCTION_NAME: &'static str = "crossChainEpochOracle";

/// The main application in-memory state.
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
            let transport = JrpcExpBackoff::http(
                config.protocol_chain.jrpc_url.clone(),
                config.protocol_chain.id.clone(),
                backoff_max,
            );
            JrpcProviderForChain::new(config.protocol_chain.id.clone(), transport)
        };
        let indexed_chains = config
            .indexed_chains
            .iter()
            .map(|chain| {
                let transport =
                    JrpcExpBackoff::http(chain.jrpc_url.clone(), chain.id.clone(), backoff_max);
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

    /// Runs a new polling iteration and submits new messages to the subgraph,
    /// if necessary.
    pub async fn run(&mut self) -> Result<(), Error> {
        info!("New polling iteration.");
        if !self.detect_new_epoch().await? {
            debug!("No epoch change detected.");
            return Ok(());
        }

        info!("Entering a new epoch.");
        self.handle_new_epoch().await?;

        Ok(())
    }

    async fn detect_new_epoch(&mut self) -> Result<bool, Error> {
        let block = get_latest_block(self.protocol_chain.web3.clone())
            .await
            .map_err(Error::BadJrpcProtocolChain)?;
        debug!(
            number = block.number,
            hash = hex::encode(block.hash).as_str(),
            "Got the latest block from the protocol chain."
        );

        debug!("Querying the subgraph state...");
        self.subgraph_state.refresh().await;
        if let Some(subgraph_error) = self.subgraph_state.error() {
            return Err(Error::Subgraph(subgraph_error));
        }

        let last_block_number_indexed_by_subgraph =
            if let Some(state) = self.subgraph_state.last_state() {
                state.0
            } else {
                warn!("The subgraph state is uninitialized");
                0
            };

        let is_fresh = freshness::subgraph_is_fresh(
            last_block_number_indexed_by_subgraph.into(),
            block.number.into(),
            self.protocol_chain.clone(),
            self.config.owner_address,
            self.config.contract_address,
            self.config.freshness_threshold,
        )
        .await
        .map_err(Error::BadJrpcProtocolChain)?;
        if !is_fresh {
            error!("Subgraph is not fresh");
            return Err(Error::SubgraphNotFresh);
        }

        Ok(self.epoch_tracker.is_new_epoch(block.number).await?)
    }

    async fn handle_new_epoch(&mut self) -> Result<(), Error> {
        info!("Collecting latest block information from all indexed chains.");

        let latest_blocks_res = get_latest_blocks(&self.indexed_chains).await;
        let latest_blocks = latest_blocks_res
            .iter()
            .filter_map(|(chain_id, res)| match res {
                Ok(block) => Some((chain_id.clone(), block.clone())),
                Err(e) => {
                    warn!(
                        chain_id = chain_id.as_str(),
                        error = e.to_string().as_str(),
                        "Failed to get latest block from chain. Skipping."
                    );
                    None
                }
            })
            .collect();

        let payload = self.produce_next_payload(latest_blocks)?;
        let tx_hash = submit_call(self.config, self.protocol_chain.clone(), payload)
            .await
            .map_err(Error::CantSubmitTx)?;
        info!(
            tx_hash = tx_hash.to_string().as_str(),
            "Contract call submitted successfully."
        );

        // TODO: After broadcasting a transaction to the protocol chain and getting a transaction
        // receipt, we should monitor it until it get enough confirmations. It's unclear which
        // component should do this task.

        Ok(())
    }

    fn produce_next_payload(
        &self,
        latest_blocks: BTreeMap<Caip2ChainId, BlockPtr>,
    ) -> Result<Vec<u8>, Error> {
        let registered_networks = registered_networks(&self.subgraph_state);

        let mut messages = vec![];

        // First, we need to make sure that there are no pending
        // `RegisterNetworks` messages.
        let networks_diff = {
            // `NetworksDiff::calculate` uses u32's but `registered_networks` has u64's
            let networks_and_block_numbers = registered_networks
                .iter()
                .map(|(chain_id, network)| (chain_id.clone(), network.block_number))
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
        debug!(
            encoded = hex_string(&encoded).as_str(),
            "Successfully encoded message(s)."
        );
        Ok(encoded)
    }
}

fn registered_networks(
    subgraph_state: &SubgraphStateTracker<SubgraphQuery>,
) -> Vec<(Caip2ChainId, ee::Network)> {
    if let Some(state) = subgraph_state.last_state() {
        state
            .1
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
            .collect()
    } else {
        // The subgraph is uninitialized, so there's no registered networks at all.
        vec![]
    }
}

fn latest_blocks_to_message(latest_blocks: BTreeMap<Caip2ChainId, BlockPtr>) -> ee::Message {
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
    use tracing::{debug, trace};
    use web3::types::{H160, U64};

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
    pub async fn subgraph_is_fresh<T>(
        subgraph_latest_block: U64,
        current_block: U64,
        protocol_chain: JrpcProviderForChain<T>,
        owner_address: H160,
        contract_address: H160,
        freshness_threshold: u64,
    ) -> web3::Result<bool>
    where
        T: web3::Transport,
    {
        // If this ever happens, then there must be a serious bug in the code
        if subgraph_latest_block > current_block {
            return Ok(true);
        }
        let block_distance = (current_block - subgraph_latest_block).as_u64();
        if block_distance == 0 {
            return Ok(true);
        } else if block_distance > freshness_threshold {
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
            subgraph_latest_block.as_u64()..=current_block.as_u64(),
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
