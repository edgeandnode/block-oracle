#!/usr/bin/env bash
set -eu

. ./prelude.sh

await "curl --silent --fail localhost:${HARDHAT_JRPC_PORT}" > /dev/null

github_clone graphprotocol/contracts dev
cd build/graphprotocol/contracts

yarn install && yarn deploy-localhost --skip-confirmation
