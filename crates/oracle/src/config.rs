use crate::models::Caip2ChainId;
use clap::Parser;
use secp256k1::SecretKey;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::read_to_string,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};
use thiserror::Error;
use tracing_subscriber::filter::LevelFilter;
use url::Url;
use web3::types::H160;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Error deserializing config file")]
    Toml(#[from] toml::de::Error),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub log_level: LevelFilter,
    pub owner_address: H160,
    pub owner_private_key: SecretKey,
    pub contract_address: H160,
    pub subgraph_url: Url,
    pub epoch_duration: u64,
    pub indexed_chains: Vec<IndexedChain>,
    pub protocol_chain: ProtocolChain,
    pub retry_strategy_max_wait_time: Duration,
}

#[derive(Clone, Debug)]
pub struct IndexedChain {
    pub id: Caip2ChainId,
    pub jrpc_url: Url,
}

#[derive(Clone, Debug)]
pub struct ProtocolChain {
    pub id: Caip2ChainId,
    pub jrpc_url: Url,
    pub polling_interval: Duration,
}

impl Config {
    /// Loads all configuration options from CLI arguments, the TOML
    /// configuration file, and environment variables.
    ///
    /// # Panics
    ///
    /// Will panic if any configuration value can't be read for any reason.
    pub fn parse() -> Self {
        let clap = Clap::parse();
        let config_file =
            ConfigFile::from_file(&clap.config_file).expect("Failed to read config file.");

        let retry_strategy_max_wait_time =
            Duration::from_secs(config_file.web3_transport_retry_max_wait_time_in_seconds);

        Self {
            log_level: clap.log_level,
            owner_address: config_file.owner_address.parse().unwrap(),
            owner_private_key: SecretKey::from_str(clap.owner_private_key.as_str()).unwrap(),
            contract_address: config_file.contract_address.parse().unwrap(),
            subgraph_url: clap.subgraph_url,
            epoch_duration: config_file.epoch_duration,
            retry_strategy_max_wait_time,
            indexed_chains: config_file
                .indexed_chains
                .into_iter()
                .map(|(chain_id, url)| IndexedChain {
                    id: chain_id,
                    jrpc_url: url,
                })
                .collect(),
            protocol_chain: ProtocolChain {
                id: config_file.protocol_chain.name,
                jrpc_url: config_file.protocol_chain.jrpc,
                polling_interval: Duration::from_secs(
                    config_file.protocol_chain_polling_interval_in_seconds,
                ),
            },
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[clap(name = "block-oracle")]
#[clap(bin_name = "block-oracle")]
#[clap(author, version, about, long_about = None)]
struct Clap {
    /// The private key for the oracle owner account.
    #[clap(long)]
    owner_private_key: String,
    /// Only show log messages at or above this level. `INFO` by default.
    #[clap(short, long, default_value = "info")]
    log_level: LevelFilter,
    /// The subgraph endpoint.
    #[clap(long)]
    subgraph_url: Url,
    /// The filepath of the TOML JSON-RPC configuration file.
    #[clap(long, default_value = "config.toml", parse(from_os_str))]
    config_file: PathBuf,
}

/// Represents the TOML config file
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct ConfigFile {
    owner_address: String,
    contract_address: String,
    indexed_chains: HashMap<Caip2ChainId, Url>,
    protocol_chain: SerdeProtocolChain,
    #[serde(default = "serde_defaults::epoch_duration")]
    epoch_duration: u64,
    #[serde(default = "serde_defaults::protocol_chain_polling_interval_in_seconds")]
    protocol_chain_polling_interval_in_seconds: u64,
    #[serde(default = "serde_defaults::web3_transport_retry_max_wait_time_in_seconds")]
    web3_transport_retry_max_wait_time_in_seconds: u64,
    #[serde(default = "serde_defaults::transaction_confirmation_poll_interval_in_seconds")]
    transaction_confirmation_poll_interval_in_seconds: u64,
    #[serde(default = "serde_defaults::transaction_confirmation_count")]
    transaction_confirmation_count: usize,
}

impl ConfigFile {
    /// Tries to Create a [`ConfigFile`] from a TOML file.
    fn from_file(file_path: &Path) -> Result<Self, ConfigError> {
        let string = read_to_string(file_path)?;
        toml::from_str(&string).map_err(ConfigError::Toml)
    }
}

/// These should be expressed as constants once
/// https://github.com/serde-rs/serde/issues/368 is fixed.
mod serde_defaults {
    pub fn epoch_duration() -> u64 {
        6_646
    }

    pub fn protocol_chain_polling_interval_in_seconds() -> u64 {
        120
    }

    pub fn web3_transport_retry_max_wait_time_in_seconds() -> u64 {
        60
    }

    pub fn transaction_confirmation_poll_interval_in_seconds() -> u64 {
        5
    }

    pub fn transaction_confirmation_count() -> usize {
        0
    }
}

#[derive(Deserialize, Debug)]
struct SerdeProtocolChain {
    name: Caip2ChainId,
    jrpc: Url,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONFIG: &str = include_str!("../config/dev/config.toml");

    #[test]
    fn deserialize_protocol_chain() {
        toml::de::from_str::<ConfigFile>(SAMPLE_CONFIG).unwrap();
    }
}
