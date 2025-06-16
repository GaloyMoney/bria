#!/usr/bin/env bash
set -euo pipefail

# Change to repo directory
pushd repo

echo "--- Setting up Cachix ---"
cachix use "${CACHIX_CACHE_NAME}"

# cannot call the profile dev as it fails with a symlink error
# perhaps its a reserved keyword
echo "--- Setting up Nix development environment ---"
nix develop --profile dev-profile -c true
cachix push "${CACHIX_CACHE_NAME}" dev-profile

# --- Source Helpers Early ---
# Get REPO_ROOT early to source helpers
# export REPO_ROOT=$(git rev-parse --show-toplevel)
# if [[ -f "${REPO_ROOT}/bats/helpers.bash" ]]; then
#   echo "--- Sourcing helpers ---"
#   source "${REPO_ROOT}/bats/helpers.bash"
# else
#   echo "Error: helpers.bash not found at ${REPO_ROOT}/bats/helpers.bash"
#   exit 1
# fi

echo "--- Running bats tests in Nix environment ---"
nix -L develop --command sh -exc '

echo "--- Checking for Podman (via nix) ---"
command -v podman
echo "--- Podman check done ---"
command -v podman-compose
echo "--- Podman-compose check done ---"

echo "--- Testing Podman basic functionality ---"
podman info || echo "Warning: podman info failed."
echo "--- Podman info done ---"

echo "--- Starting Podman service ---"
# Ensure DOCKER_HOST points to the standard rootful socket location
export DOCKER_HOST=unix:///run/podman/podman.sock
podman system service --time=0 & # Start service in background
sleep 5 # Wait a bit for the socket to become active
echo "--- Podman service started (attempted) ---"

mkdir -p /etc/containers
echo "{\"default\": [{\"type\": \"insecureAcceptAnything\"}]}" > /etc/containers/policy.json
echo "unqualified-search-registries = [\"docker.io\"]" > /etc/containers/registries.conf

echo "--- Starting Dependencies with Podman Compose ---"
podman compose up -d integration-deps
echo "--- Podman-compose up done ---"

make setup-db

# --- Build Test Artifacts ---
echo "--- Building test artifacts---"
# nix build . -L
make build


# --- Run Bats Tests ---
echo "--- Running BATS tests ---"
bats -t bats

echo "--- e2e Tests done ---"

echo "--- Cleaning up dependencies ---"
podman compose down
echo "--- Cleanup done ---"

echo "--- All steps completed ---"
'
