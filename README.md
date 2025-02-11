# Epoch Block Oracle

[![GitHub](https://img.shields.io/github/license/graphprotocol/block-oracle)](https://github.com/graphprotocol/block-oracle/blob/main/LICENSE.txt)
![GitHub branch checks state](https://img.shields.io/github/checks-status/graphprotocol/block-oracle/main)

## Overview

This repository contains an implementation of the [Epoch Block Oracle (EBO)](https://forum.thegraph.com/t/gip-0038-epoch-block-oracle/3323), a monitoring tool that observes multiple blockchains to track their most recently produced blocks. It then encodes this data and updates it on the Ethereum blockchain.
This process serves as the first stage for updating the Epoch Subgraph, a timing mechanism that records the block heights of these monitored blockchains at the onset of a new epoch within The Graph Protocol.
The Epoch Block Oracle and Epoch Subgraph utilize a shared encoding schema, which is calculated based on the rate of block height acceleration for each monitored blockchain.
At the beginning of a new epoch, the EBO broadcasts a transaction containing encoded data that reflects the progress made by each monitored blockchain since the last epoch. The referential block numbers are sourced from the Epoch Subgraph, highlighting their cyclic relationship.

### Authors

- [@tilacog](https://github.com/tilacog): EBO
- [@neysofu](https://github.com/neysofu): EBO
- [@juanmardefago](https://github.com/juanmardefago): Epoch Subgraph

## Terminology

- **Indexed Chain:** Refers to a blockchain that is actively monitored by the EBO to capture its latest block height.
- **Protocol Chain:** This is the blockchain that receives transactions from the EBO. It hosts The Graph Protocol contracts, hence the name.
- **Block Delta:** The distance, measured in blocks, between the latest block height for a given indexed chain in the Epoch Subgraph and the latest block obtained by querying the indexed chain's JSON RPC.
- **Subgraph Freshness:** The Epoch Subgraph is deemed "fresh" if it has processed all pertinent transactions aimed at the DataEdge contract. This is verified by the Oracle fetching the latest block from the Protocol Chain and comparing its number with the subgraph's current block, performing additional scans if the block numbers are within a specific range. That range is referenced as the "freshness threshold."
- **Message:** Specific content transmitted from the EBO to the Data Edge contract. It indicates different types of state changes that the Epoch Subgraph should process.
- **DataEdge:** This Ethereum contract is designed for gas efficiency and to facilitate data transfer to subgraphs. For a complete reference, consult [GIP-0025: DataEdge](https://forum.thegraph.com/t/gip-0025-dataedge/3161).
- **EventfulDataEdge:** The Eventful DataEdge operates similarly to the standard DataEdge. However, its fallback method triggers an event containing the literal payload. This adaptation was necessary due to Hardhat not supporting traces, but it is also used in Arbitrum chains as those don't support traces either.

## Repository structure

This repository contains the codebases for:

- the Epoch Block Oracle,
- the Epoch Subgraph, and
- the [Block Oracle encoder](https://graphprotocol.github.io/block-oracle/), An online tool to encode EBO Messages in JSON format. Stakeholders use it to verify that a specific Message produces the anticipated byte sequence upon encoding, thus ensuring confidence when signing transactions to the Data Edge.
There is a Cargo workspace with four Rust crates and three TypeScript packages:

### Rust Crates

#### `encoding`

Types for encoding EBO messages into bytes to be fed into the transaction calldata.

#### `json-oracle-encoder`

Support types used by the Block Oracle encoder utility.

#### `oracle`

The EBO itself.

#### `xtask`

Project-specific workflows, adhering to the [xtask](https://github.com/matklad/cargo-xtask/) standard.

### Typescript Packages

#### `subgraph`

Code for the Epoch Subgraph.

#### `contracts`

Data Edge implementation.

#### `json-encoder-web`

Package for the Block Oracle encoder website.

## Main Operation

The EBO operates within a continuous loop, pausing for a set duration between each cycle. During each iteration, it performs the following actions:

1. Checks its own wallet balance.
2. Queries the Epoch Subgraph.
3. Check the Epoch Manager contract to see if a new epoch has started.
Upon detecting a new epoch, the EBO will:
4. Retrieve the latest blocks for all indexed chains.
5. Prepare a transaction for the DataEdge contract that includes the encoded block distance.
6. Broadcast the transaction and await its receipt.
If an error occurs at any point, the EBO will log an error message and restart the main loop after the configured sleep interval.

### Running the program

Here are the steps for running the EBO:

1. First, build the program:
    
    ```bash
    $ cargo build --release
    ```
    
2. Then run the compiled program using the `run` subcommand and passing the TOML configuration file as a parameter:
    
    ```bash
    $ block-oracle run config.toml
    ```
    

## Configuration

The EBO is set up via a TOML file. Entries can be literal values or strings prefixed with a dollar sign, denoting environment variables read upon program launch.
Here is an example of a functional configuration file:

```toml
owner_address = "0x5f49491e965895deD343Af13389eE45EF60ED793"
owner_private_key = "$ARBITRUM_MAINNET_PRIVATE_KEY"
epoch_manager_address = "0x5a843145c43d328b9bb7a4401d94918f131bb281"
data_edge_address = "0x633bb9790d7c4c59991cebd377c0ed6501a35ebe"
log_level = "trace"
subgraph_url = "[https://api.thegraph.com/subgraphs/name/graphprotocol/arbitrum-epoch-block-oracle](https://api.thegraph.com/subgraphs/name/graphprotocol/arbitrum-epoch-block-oracle)"
bearer_token = "TODO"
freshness_threshold = 500
[protocol_chain]
name = "eip155:42161"
jrpc = "$ARBITRUM_JSON_RPC_ENDPOINT"
polling_interval_in_seconds = 30

[transaction_monitoring]
gas_limit = 10_000_000

[indexed_chains]
"eip155:1"         = "$ETHEREUM_JSON_RPC_ENDPOINT"
"eip155:100"       = "$GNOSIS_JSON_RPC_ENDPOINT"
"eip155:42161"     = "$ARBITRUM_JSON_RPC_ENDPOINT"
"eip155:43114"     = "$AVALANCHE_JSON_RPC_ENDPOINT"
"eip155:42161"     = "$ARBITRUM_JSON_RPC_ENDPOINT"
"eip155:42220"     = "$CELO_JSON_RPC_ENDPOINT"
"eip155:137"       = "$POLYGON_JSON_RPC_ENDPOINT"
"eip155:250"       = "$FANTOM_JSON_RPC_ENDPOINT"
```

The `protocol_chain` section contains settings for dealing with the blockchain to which the EBO will send its transactions, like Ethereum or Arbitrum One.
The `indexed_chains` table has keys for each supported indexed chain */(in CAIP2-ID format)*, mapped to the URL of a JSON RPC endpoint for that network. The EBO does not validate the network ID for any indexed chain.
All possible configuration entries, as well as their description, can be found in the `/crates/oracle/src/config.rs` file.
Note that although the `bearer_token` can be configured, it is not currently utilized by the EBO. This feature was intended for querying the Epoch Subgraph on the Network, but as of now, the Subgraph isn't deployed there and is directly queried on the Hosted Service.

## Maintenance

### Adding a new indexed chain

We have a document for that: [adding-a-new-indexed-chain.org](./crates/oracle/docs/adding-a-new-indexed-chain.org)

### Removing an indexed chain

Chains are added by name but are removed using their index. To remove an indexed chain, follow these steps:

1. Query the Epoch Subgraph to obtain a list of all indexed chains.
2. Broadcast a `RegisterNetwork` message specifying the 0-based index of the corresponding chains to remove.
Mind that removing networks will reorder the supported network list.

## Error Handling

### If the EBO becomes unresponsive/frozen

We've observed that a malfunctioning JSON RPC provider can sometimes cause the EBO to become unresponsive. To pinpoint the problematic RPC provider, you have two options:

1. Review the logs to determine the specific point at which the EBO ceased to log activities. Identify whether the issue occurred during the collection of the latest block numbers (indexed chains) or interaction with the protocol chain.
2. Manually interact with each provider, interacting with providers as typically performed by the EBO.

Common operations to examine in both approaches include:

- Checking the wallet balance.
- Querying for the latest block.
- Querying the EpochManager contract for the current epoch.
- If none of the above worked, try broadcasting a transaction.

## Metrics

📊 We have a Grafana dashboard for all EBO-relevant metrics:
[https://thegraph.grafana.net/d/z20RKM24z/epoch-block-oracle-all-networks?orgId=1](https://thegraph.grafana.net/d/z20RKM24z/epoch-block-oracle-all-networks?orgId=1)

(This is private to The Graph core devs - if you think you need access to this, please get in touch.)


## Testing

Both the EBO and the Epoch Subgraph have unit tests.
The project includes a Docker Compose-based development environment that provides all necessary dependencies for running the EBO and indexing the Epoch Subgraph.

### Epoch Subgraph - Unit Tests

To run Epoch Subgraph unit tests, visit the `/packages/subgraph` directory and run:

```bash
$ yarn test
```

### Epoch Block Oracle - Unit tests

To run the EBO unit tests, visit the `/crates` directory and run

```bash
$ cargo test
```

### Epoch Block Oracle - Development Environment

To start the development environment, visit the `/k8s/compose` directory and use this command to start the container orchestration:

```bash
$ docker-compose up
```

## Interpreting Logs

### Regular Operation - No epoch change

It runs in cycles; most are short because there's no work to be done.

Like this log section shows:

```
*(TRACE entries were omitted for brevity)*

**INFO** New polling iteration.
**INFO** Owner ETH Balance is 3061726497312866000 gwei
DEBUG Querying the subgraph state...
**INFO** Fetching latest subgraph state
DEBUG Latest Epoch Subgraph payload at block #53810939 is valid
DEBUG Subgraph is at epoch 3819
DEBUG Epoch Manager is at epoch 3819
DEBUG No epoch change detected.
**INFO** Going to sleep before next polling iteration. seconds=30
```

🔮 Since both `Epoch Subgraph` and `Epoch Manager` contract pointed to the same current epoch, the EBO wasn’t triggered.

Mind the first and last lines. They delineate the whole cycle.

```
**INFO** New polling iteration.
...
**INFO** Going to sleep before next polling iteration. seconds=30
```

### Regular Operation - Epoch change

Here is a log section that describes a successful cycle when there’s an epoch change:

```
**INFO** New polling iteration.
TRACE Sending JRPC call id=15122 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBalance", params: Array([String("0x1e7f07543b873f4c43c03e44b6ded4674079fecb"), String("latest")]), id: Num(15122) }) network=eip155:421613
 INFO Owner ETH Balance is 3061780334912866000 gwei
DEBUG Querying the subgraph state...
 INFO Fetching latest subgraph state
DEBUG Latest Epoch Subgraph payload at block #53324044 is valid
DEBUG Subgraph is at epoch 3797
TRACE Querying the Epoch Manager for the current epoch
TRACE Sending JRPC call id=15123 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_call", params: Array([Object({"data": String("0x76671808"), "to": String("0x8ecedc7631f4616d7f4074f9fc9d0368674794be")}), String("latest")]), id: Num(15123) }) network=eip155:421613
DEBUG Epoch Manager is at epoch 3798
TRACE Sending JRPC call id=15124 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(15124) }) network=eip155:421613
DEBUG Got the latest block from the protocol chain. number=53345397 hash="1af6ed205781e0a743858d5ff492eb9e9872ee0070f440a54013816af46fce2d"
TRACE Sending JRPC call id=15125 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc5e"), Bool(true)]), id: Num(15125) }) network=eip155:421613
TRACE Sending JRPC call id=15126 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc5f"), Bool(true)]), id: Num(15126) }) network=eip155:421613
TRACE Sending JRPC call id=15127 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc60"), Bool(true)]), id: Num(15127) }) network=eip155:421613
TRACE Sending JRPC call id=15128 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc61"), Bool(true)]), id: Num(15128) }) network=eip155:421613
TRACE Sending JRPC call id=15129 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc62"), Bool(true)]), id: Num(15129) }) network=eip155:421613
TRACE Sending JRPC call id=15130 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc63"), Bool(true)]), id: Num(15130) }) network=eip155:421613
TRACE Sending JRPC call id=15131 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc64"), Bool(true)]), id: Num(15131) }) network=eip155:421613
TRACE Sending JRPC call id=15132 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc65"), Bool(true)]), id: Num(15132) }) network=eip155:421613
TRACE Sending JRPC call id=15133 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc66"), Bool(true)]), id: Num(15133) }) network=eip155:421613
TRACE Sending JRPC call id=15134 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc67"), Bool(true)]), id: Num(15134) }) network=eip155:421613
TRACE Sending JRPC call id=15135 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc68"), Bool(true)]), id: Num(15135) }) network=eip155:421613
TRACE Sending JRPC call id=15136 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc69"), Bool(true)]), id: Num(15136) }) network=eip155:421613
TRACE Sending JRPC call id=15137 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6a"), Bool(true)]), id: Num(15137) }) network=eip155:421613
TRACE Sending JRPC call id=15138 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6b"), Bool(true)]), id: Num(15138) }) network=eip155:421613
TRACE Sending JRPC call id=15139 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6c"), Bool(true)]), id: Num(15139) }) network=eip155:421613
TRACE Sending JRPC call id=15140 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6d"), Bool(true)]), id: Num(15140) }) network=eip155:421613
TRACE Sending JRPC call id=15141 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6e"), Bool(true)]), id: Num(15141) }) network=eip155:421613
TRACE Sending JRPC call id=15142 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6f"), Bool(true)]), id: Num(15142) }) network=eip155:421613
TRACE Sending JRPC call id=15143 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc70"), Bool(true)]), id: Num(15143) }) network=eip155:421613
TRACE Sending JRPC call id=15144 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc71"), Bool(true)]), id: Num(15144) }) network=eip155:421613
TRACE Sending JRPC call id=15145 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc72"), Bool(true)]), id: Num(15145) }) network=eip155:421613
TRACE Sending JRPC call id=15146 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc73"), Bool(true)]), id: Num(15146) }) network=eip155:421613
TRACE Sending JRPC call id=15147 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc74"), Bool(true)]), id: Num(15147) }) network=eip155:421613
TRACE Sending JRPC call id=15148 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc75"), Bool(true)]), id: Num(15148) }) network=eip155:421613
TRACE Epoch Subgraph is fresh. Found no calls between last synced block and the protocol chain's head subgraph_latest_block=53345374 current_block=53345397
 INFO Entering a new epoch.
 INFO Collecting latest block information from all indexed chains.
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:1
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:421614
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:11155111
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:5
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:100
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:421613
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:250
 WARN Multiple networks present in the configuration file are not registered ignored_networks=[Caip2ChainId { chain_id: "eip155:11155111" }, Caip2ChainId { chain_id: "eip155:421614" }]
DEBUG Compressing 'SetBlockNumbersForNextEpoch' SetBlockNumbersForNextEpoch({"eip155:1": BlockPtr { number: 18510960, hash: "0x74b6d6432f4dc838c3a5119693561a9c101fed163dce7bad6abaf263b9b470fc" }, "eip155:100": BlockPtr { number: 30815953, hash: "0x20aa4fed306d9261564ac1d80cff875c7e031797991f29208ee4bb8181fd2137" }, "eip155:250": BlockPtr { number: 70249225, hash: "0x0003c1fa000004e70f8bbec3190271a1755ae367f719d0bd117b165a7d9c5423" }, "eip155:421613": BlockPtr { number: 53345398, hash: "0xc4e934c2ec0d96da4c12aea6b00ff0e782c873e2a92d822ca9e97226579f1625" }, "eip155:5": BlockPtr { number: 9994590, hash: "0x64efdb32be53208dc22bcc58e49e885dc64289a2f8be63235778e063df943fba" }}) networks=[("eip155:1", Network { block_number: 18510274, block_delta: 657, array_index: 0 }), ("eip155:5", Network { block_number: 9994037, block_delta: 555, array_index: 1 }), ("eip155:100", Network { block_number: 30814421, block_delta: 1461, array_index: 2 }), ("eip155:421613", Network { block_number: 53324042, block_delta: 21710, array_index: 3 }), ("eip155:250", Network { block_number: 70245210, block_delta: 3410, array_index: 4 })] networks_count=5
DEBUG Successfully compressed 'SetBlockNumbersForNextEpoch' compressed=[SetBlockNumbersForNextEpoch(NonEmpty { accelerations: [29, -2, 71, -354, 605], root: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] })] networks=[("eip155:1", Network { block_number: 18510960, block_delta: 686, array_index: 0 }), ("eip155:5", Network { block_number: 9994590, block_delta: 553, array_index: 1 }), ("eip155:100", Network { block_number: 30815953, block_delta: 1532, array_index: 2 }), ("eip155:421613", Network { block_number: 53345398, block_delta: 21356, array_index: 3 }), ("eip155:250", Network { block_number: 70249225, block_delta: 4015, array_index: 4 })]
DEBUG Successfully encoded 'SetBlockNumbersForNextEpoch' encoded="0x00000000000000000000000000000000000000000000000000000000000000000075073a020e0bea12"
 INFO Sending transaction to DataEdge
TRACE Starting Transaction Monitor options=TransactionMonitoringOptions { confirmation_timeout_in_seconds: 120, max_retries: 10, gas_percentual_increase: 50, poll_interval_in_seconds: 5, confirmations: 2, gas_limit: 10000000, max_fee_per_gas: None, max_priority_fee_per_gas: None }
TRACE Sending JRPC call id=15149 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getTransactionCount", params: Array([String("0x1e7f07543b873f4c43c03e44b6ded4674079fecb"), String("latest")]), id: Num(15149) }) network=eip155:421613
TRACE Sending JRPC call id=15150 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_gasPrice", params: Array([]), id: Num(15150) }) network=eip155:421613
DEBUG Fetched current nonce and gas price from provider nonce=1549 gas_price=100000000
TRACE Sending JRPC call id=15151 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_chainId", params: Array([]), id: Num(15151) }) network=eip155:421613
TRACE Broadcasting transaction with timeout gas=10000000 hash=0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb timeout=120
TRACE Sending JRPC call id=15152 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_sendRawTransaction", params: Array([String("0xf8ee82060d8405f5e10083989680941b3e91512c8b41a677730996e84201986810e7af80b884a1dce3320000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002900000000000000000000000000000000000000000000000000000000000000000075073a020e0bea120000000000000000000000000000000000000000000000830cddfda00e0aac12dcb4e4b1c6b3d5e4a319b6437b96752f01e94f87a1d852dc0661b693a049c7f6683c1a8fed8e2064ae6e8a8eac16235441db32ce2c42c4d69ea494b359")]), id: Num(15152) }) network=eip155:421613
TRACE Sending JRPC call id=15153 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_newBlockFilter", params: Array([]), id: Num(15153) }) network=eip155:421613
TRACE Sending JRPC call id=15154 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getFilterChanges", params: Array([String("0x97d82a1967fbff3d24dbd6a87224a7cc")]), id: Num(15154) }) network=eip155:421613
TRACE Sending JRPC call id=15155 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getTransactionReceipt", params: Array([String("0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb")]), id: Num(15155) }) network=eip155:421613
TRACE Sending JRPC call id=15156 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_blockNumber", params: Array([]), id: Num(15156) }) network=eip155:421613
TRACE Sending JRPC call id=15157 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getTransactionReceipt", params: Array([String("0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb")]), id: Num(15157) }) network=eip155:421613
 INFO Contract call submitted successfully. tx_hash=0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb
 **INFO** Going to sleep before next polling iteration. seconds=30
```

There’s a lot to unpack, so let’s go over each group of messages:

#### 1. Initial checks

```
**INFO** New polling iteration.
TRACE Sending JRPC call id=15122 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBalance", params: Array([String("0x1e7f07543b873f4c43c03e44b6ded4674079fecb"), String("latest")]), id: Num(15122) }) network=eip155:421613
 INFO Owner ETH Balance is 3061780334912866000 gwei
DEBUG Querying the subgraph state...
 INFO Fetching latest subgraph state
DEBUG Latest Epoch Subgraph payload at block #53324044 is valid
DEBUG Subgraph is at epoch 3797
TRACE Querying the Epoch Manager for the current epoch
TRACE Sending JRPC call id=15123 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_call", params: Array([Object({"data": String("0x76671808"), "to": String("0x8ecedc7631f4616d7f4074f9fc9d0368674794be")}), String("latest")]), id: Num(15123) }) network=eip155:421613
DEBUG Epoch Manager is at epoch 3798
```

The EBO wakes to a new cycle (also called `polling iteration`) and checks

1. Its wallet balance
2. the Epoch Subgraph state: subgraph health and current epoch
3. The Epoch Manager contract's current epoch.

#### 2. Epoch Subgraph Freshness Check

```
TRACE Sending JRPC call id=15124 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(15124) }) network=eip155:421613
**DEBUG** Got the latest block from the protocol chain. number=53345397 hash="1af6ed205781e0a743858d5ff492eb9e9872ee0070f440a54013816af46fce2d"
TRACE Sending JRPC call id=15125 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc5e"), Bool(true)]), id: Num(15125) }) network=eip155:421613
TRACE Sending JRPC call id=15126 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc5f"), Bool(true)]), id: Num(15126) }) network=eip155:421613
TRACE Sending JRPC call id=15127 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc60"), Bool(true)]), id: Num(15127) }) network=eip155:421613
TRACE Sending JRPC call id=15128 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc61"), Bool(true)]), id: Num(15128) }) network=eip155:421613
TRACE Sending JRPC call id=15129 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc62"), Bool(true)]), id: Num(15129) }) network=eip155:421613
TRACE Sending JRPC call id=15130 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc63"), Bool(true)]), id: Num(15130) }) network=eip155:421613
TRACE Sending JRPC call id=15131 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc64"), Bool(true)]), id: Num(15131) }) network=eip155:421613
TRACE Sending JRPC call id=15132 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc65"), Bool(true)]), id: Num(15132) }) network=eip155:421613
TRACE Sending JRPC call id=15133 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc66"), Bool(true)]), id: Num(15133) }) network=eip155:421613
TRACE Sending JRPC call id=15134 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc67"), Bool(true)]), id: Num(15134) }) network=eip155:421613
TRACE Sending JRPC call id=15135 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc68"), Bool(true)]), id: Num(15135) }) network=eip155:421613
TRACE Sending JRPC call id=15136 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc69"), Bool(true)]), id: Num(15136) }) network=eip155:421613
TRACE Sending JRPC call id=15137 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6a"), Bool(true)]), id: Num(15137) }) network=eip155:421613
TRACE Sending JRPC call id=15138 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6b"), Bool(true)]), id: Num(15138) }) network=eip155:421613
TRACE Sending JRPC call id=15139 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6c"), Bool(true)]), id: Num(15139) }) network=eip155:421613
TRACE Sending JRPC call id=15140 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6d"), Bool(true)]), id: Num(15140) }) network=eip155:421613
TRACE Sending JRPC call id=15141 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6e"), Bool(true)]), id: Num(15141) }) network=eip155:421613
TRACE Sending JRPC call id=15142 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc6f"), Bool(true)]), id: Num(15142) }) network=eip155:421613
TRACE Sending JRPC call id=15143 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc70"), Bool(true)]), id: Num(15143) }) network=eip155:421613
TRACE Sending JRPC call id=15144 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc71"), Bool(true)]), id: Num(15144) }) network=eip155:421613
TRACE Sending JRPC call id=15145 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc72"), Bool(true)]), id: Num(15145) }) network=eip155:421613
TRACE Sending JRPC call id=15146 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc73"), Bool(true)]), id: Num(15146) }) network=eip155:421613
TRACE Sending JRPC call id=15147 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc74"), Bool(true)]), id: Num(15147) }) network=eip155:421613
TRACE Sending JRPC call id=15148 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("0x32dfc75"), Bool(true)]), id: Num(15148) }) network=eip155:421613
**TRACE** Epoch Subgraph is fresh. Found no calls between last synced block and the protocol chain's head subgraph_latest_block=53345374 current_block=53345397
 **INFO Entering a new epoch.**
```

To ensure the Epoch Subgraph is fresh, the EBO scans every block between the latest block indexed by the subgraph and the chain head, looking if there’s any transaction to the DataEdge contract that still needs to be picked up by the Epoch Subgraph.  
That’s why we see all those `eth_getBlockByNumber` calls.

Once it confirms that the subgraph is fresh, it determines there's a new epoch that needs processing.

#### 3. Fetch the latest blocks for all Indexed Chains

```
 **INFO** Collecting latest block information from all indexed chains.
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:1
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:421614
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:11155111
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:5
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:100
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:421613
TRACE Sending JRPC call id=25 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getBlockByNumber", params: Array([String("latest"), Bool(false)]), id: Num(25) }) network=eip155:250
 **WARN** Multiple networks present in the configuration file are not registered ignored_networks=[Caip2ChainId { chain_id: "eip155:11155111" }, Caip2ChainId { chain_id: "eip155:421614" }]

```

At the beginning of each new epoch, the EBO sends an `eth_getBlockByNumber(latest)` request to the RPCs of each indexed chain.

Look at the `WARN` message at the end. It tells us that this EBO is set up with more indexed chains than what the Council added to the Epoch Subgraph. But, this isn't a problem for the EBO as it will just ignore those networks from now on. When those networks get added, the EBO will stop showing this warning.

#### 4. Preparing the DataEdge Transaction Payload

```
DEBUG Compressing 'SetBlockNumbersForNextEpoch' SetBlockNumbersForNextEpoch({"eip155:1": BlockPtr { number: 18510960, hash: "0x74b6d6432f4dc838c3a5119693561a9c101fed163dce7bad6abaf263b9b470fc" }, "eip155:100": BlockPtr { number: 30815953, hash: "0x20aa4fed306d9261564ac1d80cff875c7e031797991f29208ee4bb8181fd2137" }, "eip155:250": BlockPtr { number: 70249225, hash: "0x0003c1fa000004e70f8bbec3190271a1755ae367f719d0bd117b165a7d9c5423" }, "eip155:421613": BlockPtr { number: 53345398, hash: "0xc4e934c2ec0d96da4c12aea6b00ff0e782c873e2a92d822ca9e97226579f1625" }, "eip155:5": BlockPtr { number: 9994590, hash: "0x64efdb32be53208dc22bcc58e49e885dc64289a2f8be63235778e063df943fba" }}) networks=[("eip155:1", Network { block_number: 18510274, block_delta: 657, array_index: 0 }), ("eip155:5", Network { block_number: 9994037, block_delta: 555, array_index: 1 }), ("eip155:100", Network { block_number: 30814421, block_delta: 1461, array_index: 2 }), ("eip155:421613", Network { block_number: 53324042, block_delta: 21710, array_index: 3 }), ("eip155:250", Network { block_number: 70245210, block_delta: 3410, array_index: 4 })] networks_count=5
DEBUG Successfully compressed 'SetBlockNumbersForNextEpoch' compressed=[SetBlockNumbersForNextEpoch(NonEmpty { accelerations: [29, -2, 71, -354, 605], root: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] })] networks=[("eip155:1", Network { block_number: 18510960, block_delta: 686, array_index: 0 }), ("eip155:5", Network { block_number: 9994590, block_delta: 553, array_index: 1 }), ("eip155:100", Network { block_number: 30815953, block_delta: 1532, array_index: 2 }), ("eip155:421613", Network { block_number: 53345398, block_delta: 21356, array_index: 3 }), ("eip155:250", Network { block_number: 70249225, block_delta: 4015, array_index: 4 })]
DEBUG Successfully encoded 'SetBlockNumbersForNextEpoch' encoded="0x00000000000000000000000000000000000000000000000000000000000000000075073a020e0bea12"
```

This demonstrates the EBO's successful compression and encoding of the latest blocks from all Indexed Chains, to be the payload for the DataEdge contract transaction. This is the content the Epoch Subgraph is expected to decode.

#### 5. Broadcast the Transaction

```
 **INFO** Sending transaction to DataEdge
TRACE Starting Transaction Monitor options=TransactionMonitoringOptions { confirmation_timeout_in_seconds: 120, max_retries: 10, gas_percentual_increase: 50, poll_interval_in_seconds: 5, confirmations: 2, gas_limit: 10000000, max_fee_per_gas: None, max_priority_fee_per_gas: None }
TRACE Sending JRPC call id=15149 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getTransactionCount", params: Array([String("0x1e7f07543b873f4c43c03e44b6ded4674079fecb"), String("latest")]), id: Num(15149) }) network=eip155:421613
TRACE Sending JRPC call id=15150 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_gasPrice", params: Array([]), id: Num(15150) }) network=eip155:421613
DEBUG Fetched current nonce and gas price from provider nonce=1549 gas_price=100000000
TRACE Sending JRPC call id=15151 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_chainId", params: Array([]), id: Num(15151) }) network=eip155:421613
TRACE Broadcasting transaction with timeout gas=10000000 hash=0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb timeout=120
TRACE Sending JRPC call id=15152 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_sendRawTransaction", params: Array([String("0xf8ee82060d8405f5e10083989680941b3e91512c8b41a677730996e84201986810e7af80b884a1dce3320000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002900000000000000000000000000000000000000000000000000000000000000000075073a020e0bea120000000000000000000000000000000000000000000000830cddfda00e0aac12dcb4e4b1c6b3d5e4a319b6437b96752f01e94f87a1d852dc0661b693a049c7f6683c1a8fed8e2064ae6e8a8eac16235441db32ce2c42c4d69ea494b359")]), id: Num(15152) }) network=eip155:421613
TRACE Sending JRPC call id=15153 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_newBlockFilter", params: Array([]), id: Num(15153) }) network=eip155:421613
TRACE Sending JRPC call id=15154 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getFilterChanges", params: Array([String("0x97d82a1967fbff3d24dbd6a87224a7cc")]), id: Num(15154) }) network=eip155:421613
TRACE Sending JRPC call id=15155 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getTransactionReceipt", params: Array([String("0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb")]), id: Num(15155) }) network=eip155:421613
TRACE Sending JRPC call id=15156 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_blockNumber", params: Array([]), id: Num(15156) }) network=eip155:421613
TRACE Sending JRPC call id=15157 request=MethodCall(MethodCall { jsonrpc: Some(V2), method: "eth_getTransactionReceipt", params: Array([String("0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb")]), id: Num(15157) }) network=eip155:421613
 **INFO** Contract call submitted successfully. tx_hash=0xa663f6f1d9b55f9db991389ca10bdf49f30f6ffa1770adee79499d6e413cfbfb
 **INFO** Going to sleep before next polling iteration. seconds=30
```

The last step is to transmit the transaction to the DataEdge contract. The EBO uses the [transaction_monitoring] configuration group to manage the construction and monitoring of the transaction. Here, we can see how the EBO interfaces with the JSON RPC provider until it gets a transaction receipt back, indicating the operation’s success.

This concludes a healthy EBO cycle.

## Legal

The source code of this project is made available under the terms of both the MIT license and the Apache License 2.0.
