use std::collections::BTreeMap;

pub type NetworkIndex = u64;
pub type Bytes32 = [u8; 32];

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlockPtr {
    pub number: u64,
    pub hash: Bytes32,
}

impl BlockPtr {
    pub fn new(number: u64, hash: Bytes32) -> Self {
        BlockPtr { number, hash }
    }
}

impl std::fmt::Debug for BlockPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockPtr")
            .field("number", &self.number)
            .field("hash", &format!("0x{}", hex::encode(&self.hash)))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    // TODO: Consider specifying epoch number here?
    SetBlockNumbersForNextEpoch(BTreeMap<String, BlockPtr>),
    RegisterNetworks {
        // Remove is by index
        remove: Vec<NetworkIndex>,
        // Add is by name
        add: Vec<String>,
    },
    CorrectEpochs {
        // TODO: include hash, count, and (if count is nonzero) merkle root
        data_by_network_id: BTreeMap<NetworkIndex, EpochDetails>,
    },
    UpdateVersion {
        version_number: u64,
    },
    ChangeOwnership {
        new_owner_address: [u8; 20],
    },
    Reset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompressedMessage {
    SetBlockNumbersForNextEpoch(CompressedSetBlockNumbersForNextEpoch),
    CorrectEpochs {
        data_by_network_id: BTreeMap<NetworkIndex, EpochDetails>,
    },
    RegisterNetworks {
        remove: Vec<u64>,
        add: Vec<String>,
    },
    UpdateVersion {
        version_number: u64,
    },
    ChangeOwnership {
        new_owner_address: [u8; 20],
    },
    Reset,
}

impl CompressedMessage {
    pub fn as_non_empty_block_numbers(&self) -> Option<(&[i64], Bytes32)> {
        match self {
            CompressedMessage::SetBlockNumbersForNextEpoch(
                CompressedSetBlockNumbersForNextEpoch::NonEmpty {
                    accelerations,
                    root,
                },
            ) => Some((accelerations, *root)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompressedSetBlockNumbersForNextEpoch {
    Empty {
        count: u64,
    },
    NonEmpty {
        accelerations: Vec<i64>,
        root: Bytes32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpochDetails {
    tx_hash: Bytes32,
    merkle_root: Bytes32,
}
