import { BigInt, Bytes } from "@graphprotocol/graph-ts";
import {
  GlobalState,
  Epoch,
  NetworkEpochBlockNumber,
  Network
} from "../generated/schema";
import { StoreCache } from "./store-cache";
import { BIGINT_ONE } from "./constants";

export enum MessageTag {
  SetBlockNumbersForEpochMessage = 0,
  CorrectEpochsMessage,
  UpdateVersionsMessage,
  RegisterNetworksMessage,
  ChangeOwnershipMessage
}

export namespace MessageTag {
  export function toString(tag: MessageTag): string {
    return [
      "SetBlockNumbersForEpochMessage",
      "CorrectEpochsMessage",
      "UpdateVersionsMessage",
      "RegisterNetworksMessage",
      "ChangeOwnershipMessage"
    ][tag]
  }
}

export function nextEpochId(state: GlobalState): BigInt {
  if (state.latestValidEpoch == null) {
    return BIGINT_ONE;
  } else {
    return BigInt.fromString(state.latestValidEpoch!) + BIGINT_ONE;
  }
}

export function createOrUpdateNetworkEpochBlockNumber(
  networkId: string,
  epochId: BigInt,
  acceleration: BigInt,
  cache: StoreCache
): NetworkEpochBlockNumber {
  let id = epochBlockNumberId(epochId, networkId);
  let previousId = epochBlockNumberId(epochId - BIGINT_ONE, networkId);

  let networkEpochBlockNumber = cache.getNetworkEpochBlockNumber(id)
  networkEpochBlockNumber.network = networkId;
  networkEpochBlockNumber.epoch = epochId.toString();
  networkEpochBlockNumber.acceleration = acceleration;

  if (cache.hasNetworkEpochBlockNumber(previousId)) {
    let previousNetworkEpochBlockNumber = cache.getNetworkEpochBlockNumber(previousId)
    networkEpochBlockNumber.delta = previousNetworkEpochBlockNumber.delta.plus(
      acceleration
    );
    networkEpochBlockNumber.blockNumber = previousNetworkEpochBlockNumber.blockNumber.plus(
      networkEpochBlockNumber.delta
    );
  } else {
    // If there's no previous entity then we consider the previous delta 0
    // There might be an edge case if the previous entity isn't 1 epoch behind
    // in case where a network is removed and then re-added
    // (^ Should we retain the progress of the network if it's removed?)
    networkEpochBlockNumber.delta = acceleration;
    networkEpochBlockNumber.blockNumber = networkEpochBlockNumber.delta;
  }

  return networkEpochBlockNumber;
}

export function getActiveNetworks(cache: StoreCache): Array<Network> {
  let state = cache.getGlobalState()
  let networks = new Array<Network>();
  let nextId = state.networkArrayHead;

  while (nextId != null) {
    let network = cache.getNetwork(nextId!);
    let isActive = network.removedAt == null;
    if (isActive) {
      networks.push(network);
    }
    nextId = network.nextArrayElement;
  }

  assert(
    networks.length == state.activeNetworkCount,
    `Found ${networks.length} active networks but ${state.activeNetworkCount} were expected. This is a bug!`,
  );
  return networks;
}

export function swapAndPop(index: u32, networks: Array<Network>): Network {
  assert(
    index < (networks.length as u32),
    `Tried to pop network at index ${index.toString()} but ` +
    `there are only ${networks.length.toString()} active networks. This is a bug!`
  );

  let tail = networks[networks.length - 1];
  let elementToRemove = networks[index];

  networks[index] = tail;
  networks[networks.length - 1] = elementToRemove;

  return networks.pop();
}

export function commitNetworkChanges(
  removedNetworks: Array<Network>,
  newNetworksList: Array<Network>,
  state: GlobalState
): void {
  for (let i = 0; i < removedNetworks.length; i++) {
    removedNetworks[i].state = null;
    removedNetworks[i].nextArrayElement = null;
    removedNetworks[i].arrayIndex = null;
  }

  for (let i = 0; i < newNetworksList.length; i++) {
    newNetworksList[i].state = state.id;
    newNetworksList[i].nextArrayElement =
      i < newNetworksList.length - 1 ? newNetworksList[i + 1].id : null;
    newNetworksList[i].arrayIndex = i;
  }

  if (newNetworksList.length > 0) {
    state.networkArrayHead = newNetworksList[0].id;
  } else {
    state.networkArrayHead = null;
  }
  state.activeNetworkCount = newNetworksList.length;
}

function epochBlockNumberId(epochId: BigInt, networkId: string): string {
  return [epochId.toString(), networkId].join("-");
}

export function parseCalldata(calldata: Bytes): Bytes {
  // hardcoded values to decode only the crossChainEpochOracle calldata
  // on the local development EventfulDataEdge contract
  let length = BigInt.fromUnsignedBytes(changetype<Bytes>(calldata.slice(36, 68).reverse()))
  return changetype<Bytes>(calldata.slice(68, 68+length.toI32()))
}
