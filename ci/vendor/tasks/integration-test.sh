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

echo "--- Running integration tests in Nix environment ---"
nix -L develop --command sh -exc '
set -euo pipefail

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
ENGINE_DEFAULT=podman bin/docker-compose-up.sh integration-deps
echo "--- Podman-compose up done ---"

make setup-db

echo "--- Running Integration Tests ---"
cargo nextest run --verbose --locked
echo "--- Tests done ---"

echo "--- Cleaning up dependencies ---"
podman compose down
echo "--- Cleanup done ---"

echo "--- All steps completed ---"
'
