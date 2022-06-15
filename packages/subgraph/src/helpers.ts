import { BigInt } from "@graphprotocol/graph-ts";
import {
  GlobalState,
  Epoch,
  NetworkEpochBlockNumber,
  Network
} from "../generated/schema";
import { BIGINT_ONE, INITIAL_ENCODING_VERSION } from "./constants";

export enum MessageTag {
  SetBlockNumbersForEpochMessage = 0,
  CorrectEpochsMessage,
  UpdateVersionsMessage,
  RegisterNetworksMessage
}

export namespace MessageTag {
  export function toString(tag: MessageTag): string {
    return [
      "SetBlockNumbersForEpochMessage",
      "CorrectEpochsMessage",
      "UpdateVersionsMessage",
      "RegisterNetworksMessage"
    ][tag]
  }
}

export function getGlobalState(): GlobalState {
  let id = "0"
  let state = GlobalState.load(id);
  if (state == null) {
    state = new GlobalState(id);
    state.networkCount = 0;
    state.activeNetworkCount = 0;
    state.encodingVersion = INITIAL_ENCODING_VERSION;
    state.networks = [];
  }
  return state;
}

export function nextEpochId(globalState: GlobalState): BigInt {
  if (globalState.latestValidEpoch == null) {
    return BIGINT_ONE;
  } else {
    return BigInt.fromString(globalState.latestValidEpoch!).plus(BIGINT_ONE);
  }
}

export function getOrCreateEpoch(epochId: BigInt): Epoch {
  let epoch = Epoch.load(epochId.toString());
  if (epoch == null) {
    epoch = new Epoch(epochId.toString());
    epoch.epochNumber = epochId;
    epoch.save();
  }
  return epoch;
}

export function createOrUpdateNetworkEpochBlockNumber(
  networkId: string,
  epochId: BigInt,
  acceleration: BigInt
): NetworkEpochBlockNumber {
  let id = epochBlockNumberId(epochId, networkId);
  let previousId = epochBlockNumberId(epochId.minus(BIGINT_ONE), networkId);

  let blockNum = NetworkEpochBlockNumber.load(id);
  if (blockNum == null) {
    blockNum = new NetworkEpochBlockNumber(id);
    blockNum.network = networkId;
    blockNum.epoch = epochId.toString();
  }
  blockNum.acceleration = acceleration;

  let previous = NetworkEpochBlockNumber.load(previousId);
  if (previous != null) {
    blockNum.delta = previous.delta.plus(acceleration);
    blockNum.blockNumber = previous.blockNumber.plus(blockNum.delta);
  } else {
    // If there's no previous entity then we consider the previous delta 0
    // There might be an edge case if the previous entity isn't 1 epoch behind
    // in case where a network is removed and then re-added
    // (^ Should we retain the progress of the network if it's removed?)
    blockNum.delta = acceleration;
    blockNum.blockNumber = blockNum.delta;
  }

  return blockNum;
}

export function getActiveNetworks(state: GlobalState): Array<Network> {
  let networks = new Array<Network>();
  let nextId = state.networkArrayHead;

  while (nextId != null) {
    let network = Network.load(nextId!)!;
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
    removedNetworks[i].save();
  }

  for (let i = 0; i < newNetworksList.length; i++) {
    newNetworksList[i].state = state.id;
    newNetworksList[i].nextArrayElement =
      i < newNetworksList.length - 1 ? newNetworksList[i + 1].id : null;
    newNetworksList[i].arrayIndex = i;
    newNetworksList[i].save();
  }

  if (newNetworksList.length > 0) {
    state.networkArrayHead = newNetworksList[0].id;
  } else {
    state.networkArrayHead = null;
  }
  state.activeNetworkCount = newNetworksList.length;
  state.save();
}

function epochBlockNumberId(epochId: BigInt, networkId: string): string {
  return [epochId.toString(), networkId].join("-");
}
