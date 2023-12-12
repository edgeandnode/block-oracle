use crate::{
    contracts::Contracts,
    hex_string,
    jrpc_utils::{get_latest_block, get_latest_blocks, JrpcExpBackoff},
    metrics::METRICS,
    subgraph::{query_subgraph, SubgraphState},
    Caip2ChainId, Config, Error, JrpcProviderForChain,
};
use epoch_encoding::{BlockPtr, Encoder, Message, CURRENT_ENCODING_VERSION};
use std::{cmp::Ordering, collections::BTreeMap};
use tracing::{debug, error, info, warn};
use web3::{transports::Http, Web3};

/// The main application in-memory state.
pub struct Oracle {
    config: Config,
    protocol_chain: JrpcProviderForChain<JrpcExpBackoff>,
    indexed_chains: Vec<JrpcProviderForChain<JrpcExpBackoff>>,
    contracts: Contracts<Http>,
}

impl Oracle {
    pub fn new(config: Config) -> Self {
        let protocol_chain = protocol_chain(&config);
        let indexed_chains = indexed_chains(&config);
        let contracts = Contracts::new(
            contracts_web3(&config),
            config.data_edge_address,
            config.epoch_manager_address,
            config.transaction_monitoring_options,
        )
        .expect("Failed to initialize Block Oracle's required contracts");

        Self {
            config,
            protocol_chain,
            indexed_chains,
            contracts,
        }
    }

    /// Runs a new polling iteration and submits new messages to the subgraph,
    /// if necessary.
    pub async fn run(&mut self) -> Result<(), Error> {
        info!("New polling iteration.");

        self.query_owner_eth_balance().await?;

        // Before anything else, we must get the latest subgraph state
        debug!("Querying the subgraph state...");
        let subgraph_state =
            query_subgraph(&self.config.subgraph_url, &self.config.bearer_token).await?;

        if self.detect_new_epoch(&subgraph_state).await? {
            self.handle_new_epoch(&subgraph_state).await?;
        } else {
            debug!("No epoch change detected.");
        }
        Ok(())
    }

    /// Checks if the Subgraph should consider that the Subgraph is at a previous epoch compared to
    /// the Epoch Manager.
    async fn detect_new_epoch(&self, subgraph_state: &SubgraphState) -> Result<bool, Error> {
        // Then we check if there is a new epoch by looking at the current Subgraph state.
        let last_block_number_indexed_by_subgraph = match self.is_new_epoch(subgraph_state).await {
            // The Subgraph is at the same epoch as the Epoch Manager.
            Ok(NewEpochCheck::SameEpoch) => return Ok(false),

            // The Subgraph is at a previous epoch than the Epoch Manager, but we still need to
            // check if the former is fresh.
            Ok(NewEpochCheck::PreviousEpoch {
                subgraph_latest_indexed_block,
            }) => subgraph_latest_indexed_block,

            // The Subgraph was recently initialized and needs to receive its first
            // SetBlockNumbersForNextEpoch message.
            Ok(NewEpochCheck::RecentlyInitialized) => return Ok(true),

            Err(other) => return Err(other),
        };

        let protocol_chain_current_block = get_latest_block(self.protocol_chain.web3.clone())
            .await
            .map_err(Error::BadJrpcProtocolChain)?;
        debug!(
            number = protocol_chain_current_block.number,
            hash = hex::encode(protocol_chain_current_block.hash).as_str(),
            "Got the latest block from the protocol chain."
        );

        let is_fresh = freshness::subgraph_is_fresh(
            last_block_number_indexed_by_subgraph.into(),
            protocol_chain_current_block.number.into(),
            self.protocol_chain.clone(),
            self.config.owner_address,
            self.config.data_edge_address,
            self.config.freshness_threshold,
        )
        .await
        .map_err(Error::BadJrpcProtocolChain)?;
        if !is_fresh {
            error!("Subgraph is not fresh");
            Err(Error::SubgraphNotFresh)
        } else {
            Ok(true)
        }
    }

    /// Checks if the Subgraph epoch is behind the Epoch Manager's current epoch.
    ///
    /// Returns a pair of values indicating: 1) if there is a new epoch; and 2) the latest block
    /// number indexed by the subgraph. Returns `None` if the Subgraph is not initialized.
    async fn is_new_epoch(&self, subgraph_state: &SubgraphState) -> Result<NewEpochCheck, Error> {
        use NewEpochCheck::*;
        let (subgraph_latest_indexed_block, subgraph_latest_epoch) = {
            match subgraph_state.latest_epoch_number() {
                Some(epoch_num) => (subgraph_state.last_indexed_block_number, epoch_num),
                None => {
                    // If the Epoch Subgraph has registered networks, then this means it was
                    // recently initialized but never received its first SetBlockNumbersForNextEpoch
                    // message.
                    if subgraph_state.has_registered_networks() {
                        return Ok(NewEpochCheck::RecentlyInitialized);
                    }
                    return Err(Error::SubgraphNotInitialized);
                }
            }
        };

        debug!("Subgraph is at epoch {subgraph_latest_epoch}");
        METRICS.set_current_epoch("subgraph", subgraph_latest_epoch as i64);
        let manager_current_epoch = self.contracts.query_current_epoch().await?;
        match subgraph_latest_epoch.cmp(&manager_current_epoch) {
            Ordering::Less => Ok(PreviousEpoch {
                subgraph_latest_indexed_block,
            }),
            Ordering::Equal => Ok(SameEpoch),
            Ordering::Greater => Err(Error::EpochManagerBehindSubgraph {
                manager: manager_current_epoch,
                subgraph: subgraph_latest_epoch,
            }),
        }
    }

    async fn handle_new_epoch(&mut self, subgraph_state: &SubgraphState) -> Result<(), Error> {
        info!("Entering a new epoch.");
        info!("Collecting latest block information from all indexed chains.");

        let latest_blocks_res = get_latest_blocks(&self.indexed_chains).await;
        let latest_blocks = latest_blocks_res
            .iter()
            .filter_map(|(chain_id, res)| -> Option<(Caip2ChainId, BlockPtr)> {
                match res {
                    Ok(block) => {
                        METRICS.set_latest_block_number(
                            chain_id.as_str(),
                            "jrpc",
                            block.number as i64,
                        );
                        Some((chain_id.clone(), *block))
                    }
                    Err(e) => {
                        warn!(
                            chain_id = chain_id.as_str(),
                            error = e.to_string().as_str(),
                            "Failed to get latest block from chain. Skipping."
                        );
                        None
                    }
                }
            })
            .collect();

        let payload = set_block_numbers_for_next_epoch(subgraph_state, latest_blocks);
        let transaction_receipt = self
            .contracts
            .submit_call(payload, &self.config.owner_private_key)
            .await
            .map_err(Error::ContractError)?;
        METRICS.set_last_sent_message();
        info!(
            tx_hash = ?transaction_receipt.transaction_hash,
            "Contract call submitted successfully."
        );

        // TODO: After broadcasting a transaction to the protocol chain and getting a transaction
        // receipt, we should monitor it until it get enough confirmations. It's unclear which
        // component should do this task.

        Ok(())
    }

    /// Queries the Protocol Chain for the current balance of the Owner's account.
    ///
    /// Used for monitoring and logging.
    async fn query_owner_eth_balance(&self) -> Result<(), Error> {
        let balance = self
            .protocol_chain
            .web3
            .eth()
            .balance(self.config.owner_address, None)
            .await
            .map_err(Error::BadJrpcProtocolChain)?;

        info!("Owner ETH Balance is {} gwei", balance);

        // overflow check
        METRICS.set_wallet_balance(i64::try_from(balance).unwrap_or(i64::MAX));

        Ok(())
    }
}

fn set_block_numbers_for_next_epoch(
    subgraph_state: &SubgraphState,
    mut latest_blocks: BTreeMap<Caip2ChainId, BlockPtr>,
) -> Vec<u8> {
    let registered_networks = subgraph_state
        .global_state
        .as_ref()
        .map(|gs| gs.networks.clone())
        // In case the subgraph is uninitialized, there's effectively no registered networks at all.
        .unwrap_or_default();

    // We're not interested in unregistered networks. So we isolate them into a separate
    // collection, log them, and finally discard them.
    let mut ignored_networks = Vec::new();
    for chain_id in latest_blocks.keys().cloned() {
        if !registered_networks
            .iter()
            .any(|network| network.id == chain_id)
        {
            ignored_networks.push(chain_id);
        }
    }
    if !ignored_networks.is_empty() {
        warn!(
            ignored_networks = ?ignored_networks,
            "Multiple networks present in the configuration file are not registered"
        );
    }
    for chain_id in ignored_networks {
        latest_blocks.remove(&chain_id);
    }

    let message = Message::SetBlockNumbersForNextEpoch(
        latest_blocks
            .into_iter()
            .map(|(chain_id, block_ptr)| (chain_id.as_str().to_owned(), block_ptr))
            .collect(),
    );
    let available_networks: Vec<(String, epoch_encoding::Network)> = {
        registered_networks
            .into_iter()
            .map(|network| (network.id.as_str().to_owned(), network.into()))
            .collect()
    };

    debug!(
        message = ?message,
        networks = ?available_networks,
        networks_count = available_networks.len(),
        "Compressing 'SetBlockNumbersForNextEpoch'"
    );

    let mut compression_engine = Encoder::new(CURRENT_ENCODING_VERSION, available_networks)
        .expect("Can't prepare for encoding because something went wrong.");
    let compression_engine_initially = compression_engine.clone();

    let compressed = compression_engine
        .compress(&[message])
        .unwrap_or_else(|error| panic!("Encoding failed. Error: {error}"));
    debug!(
        compressed = ?compressed,
        networks = ?compression_engine.network_deltas(),
        "Successfully compressed 'SetBlockNumbersForNextEpoch'"
    );
    let encoded = compression_engine.encode(&compressed);
    debug!(
        encoded = hex_string(&encoded).as_str(),
        "Successfully encoded 'SetBlockNumbersForNextEpoch'"
    );

    assert_ne!(
        compression_engine, compression_engine_initially,
        "The encoder has identical internal state compared to what \
            it had before these new messages. This is a bug!"
    );

    encoded
}

fn protocol_chain(config: &Config) -> JrpcProviderForChain<JrpcExpBackoff> {
    let transport = JrpcExpBackoff::http(
        config.protocol_chain.jrpc_url.clone(),
        config.protocol_chain.id.clone(),
        config.retry_strategy_max_wait_time,
    );
    JrpcProviderForChain::new(config.protocol_chain.id.clone(), transport)
}

fn contracts_web3(config: &Config) -> Web3<Http> {
    // Unwrap/Expect: the URL string value we pass to Http::new will be parsed into an URL again.
    // Since that value already comes from a parsed (and thus, valid) URL, we can assume it won't panic.
    let transport = Http::new(config.protocol_chain.jrpc_url.as_str()).expect("URL to be valid");
    Web3::new(transport)
}

fn indexed_chains(config: &Config) -> Vec<JrpcProviderForChain<JrpcExpBackoff>> {
    config
        .indexed_chains
        .iter()
        .map(|chain| {
            let transport = JrpcExpBackoff::http(
                chain.jrpc_url.clone(),
                chain.id.clone(),
                config.retry_strategy_max_wait_time,
            );
            JrpcProviderForChain::new(chain.id.clone(), transport)
        })
        .collect()
}

mod freshness {
    use crate::models::JrpcProviderForChain;
    use crate::runner::jrpc_utils::calls_in_block_range;
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

/// Used inside the 'Oracle::is_new_epoch' method to return information about the Epoch Subgraph
/// current state.
enum NewEpochCheck {
    /// The Epoch Subgraph was recently initialized but never received a SetBlockNumbersForNextEpoch
    /// message.
    RecentlyInitialized,
    /// The Epoch Subgraph is at a previous epoch than the Epoch Manager.
    PreviousEpoch { subgraph_latest_indexed_block: u64 },
    /// The Epoch Subgraph is at the same epoch as the Epoch Manager.
    SameEpoch,
}
