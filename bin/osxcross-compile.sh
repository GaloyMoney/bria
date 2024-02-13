#!/bin/bash

MACOS_TARGET="x86_64-apple-darwin"

echo "Building target for platform ${MACOS_TARGET}"
echo

# Make libz-sys (git2-rs -> libgit2-sys -> libz-sys) build as a statically linked lib
# This prevents the host zlib from being linked
export LIBZ_SYS_STATIC=1

# For cross compiling ring https://github.com/briansmith/ring/blob/main/BUILDING.md#cross-compiling
export TARGET_CC=/workspace/osxcross/target/bin/x86_64h-apple-darwin23-cc
export TARGET_AR=/workspace/osxcross/target/bin/x86_64h-apple-darwin23-ar

SQLX_OFFLINE=true cargo build --release --target "${MACOS_TARGET}" --all-features

echo
echo Done
