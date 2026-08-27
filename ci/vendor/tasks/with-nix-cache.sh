#!/bin/sh
set -eu

# Wrapper that adds a persistent local Nix binary cache.
# Concourse task caches persist /nix-cache between runs of the same task step.
# On the first run the cache is empty; on subsequent runs Nix finds most store
# paths locally and avoids downloading them again from remote substituters.

# Maximum cache size in bytes (25 GB). When exceeded after saving,
# the oldest binary-cache entries are removed until size drops below.
CACHE_MAX_BYTES="${CACHE_MAX_BYTES:-$((25 * 1024 * 1024 * 1024))}"
NIX_CACHE_DIR="${NIX_CACHE_DIR:-/nix-cache}"

# Deep consistency maintenance (malformed/broken/orphan scans) walks every
# .narinfo and payload in the cache. That is useful when the cache is over its
# size limit or occasionally to heal stale metadata, but doing it after every
# successful warm task adds tens of seconds of serial post-test time. Run the
# full maintenance at most daily while the cache is under the size limit. Set
# to 0 to restore the old "scan every run" behavior.
CACHE_MAINTENANCE_INTERVAL_SECONDS="${CACHE_MAINTENANCE_INTERVAL_SECONDS:-86400}"
CACHE_MAINTENANCE_STAMP="${NIX_CACHE_DIR}/.last-maintenance-epoch"

append_nix_config() {
  extra_config=$(cat)
  if [ -n "${NIX_CONFIG:-}" ]; then
    NIX_CONFIG=$(printf '%s\n%s' "$NIX_CONFIG" "$extra_config")
  else
    NIX_CONFIG="$extra_config"
  fi
  export NIX_CONFIG
}

setup_fetch_retries() {
  # Nix binary-cache substitutions use these NIX_CONFIG values. This covers
  # Remote binary-cache downloads and still lets Nix fall back to building
  # from source when a substituter is flaky.
  append_nix_config <<EOF
connect-timeout = 60
stalled-download-timeout = 600
download-attempts = 6
fallback = true
EOF

  # Nixpkgs fetchurl reads these impure env vars inside fixed-output
  # derivations. This covers crates.io and registry.npmjs.org tarballs.
  export NIX_CONNECT_TIMEOUT=60
  NIX_CURL_FLAGS="${NIX_CURL_FLAGS:+$NIX_CURL_FLAGS }--connect-timeout 60"
  export NIX_CURL_FLAGS="$NIX_CURL_FLAGS --retry 6"

  # pnpm inherits npm config from the environment. This covers direct pnpm
  # install steps in release helpers, outside Nix fixed-output fetchers.
  export npm_config_fetch_timeout=300000
  export npm_config_fetch_retry_mintimeout=20000
  export npm_config_fetch_retry_maxtimeout=120000
  export npm_config_fetch_retries=6

  echo "fetch-retries: nix connect-timeout=60s stalled-timeout=600s attempts=6"
  echo "fetch-retries: npm timeout=${npm_config_fetch_timeout}ms retries=${npm_config_fetch_retries}"
}

setup_nix_cache() {
  mkdir -p "$NIX_CACHE_DIR"
  if [ -f "$NIX_CACHE_DIR/nix-cache-info" ]; then
    cache_size=$(du -sh "$NIX_CACHE_DIR" 2>/dev/null | cut -f1)
    echo "nix-cache: local cache found (${cache_size}), restoring"
    # Modify nix.conf for tools that read it directly
    echo "extra-substituters = file://$NIX_CACHE_DIR" >> /etc/nix/nix.conf
    echo "require-sigs = false" >> /etc/nix/nix.conf

    # Export NIX_CONFIG so the running nix-daemon picks up changes.
    # The daemon loads nix.conf at startup (before this wrapper runs),
    # so modifying the file alone is not enough. NIX_CONFIG is read by
    # nix commands at invocation time and supplements the daemon config.
    append_nix_config <<EOF
extra-substituters = file://$NIX_CACHE_DIR
require-sigs = false
EOF
  else
    echo "nix-cache: no local cache found (cold start)"
  fi
}

nix_cache_size_bytes() {
  bytes=$(du -sb "$NIX_CACHE_DIR" 2>/dev/null | cut -f1 || true)
  case "$bytes" in
    "" | *[!0-9]*) bytes=0 ;;
  esac
  echo "$bytes"
}

nix_cache_size_mb() {
  bytes=${1:-0}
  case "$bytes" in
    *[!0-9]*) bytes=0 ;;
  esac
  echo "$((bytes / 1024 / 1024))"
}

cache_maintenance_due() {
  interval=$CACHE_MAINTENANCE_INTERVAL_SECONDS
  case "$interval" in
    "" | *[!0-9]*) interval=86400 ;;
  esac

  if [ "$interval" -eq 0 ]; then
    echo "nix-cache: maintenance due because interval is 0"
    return 0
  fi

  if [ ! -r "$CACHE_MAINTENANCE_STAMP" ]; then
    echo "nix-cache: maintenance due because no previous maintenance stamp exists"
    return 0
  fi

  IFS= read -r last_epoch < "$CACHE_MAINTENANCE_STAMP" || last_epoch=
  case "$last_epoch" in
    "" | *[!0-9]*)
      echo "nix-cache: maintenance due because previous maintenance stamp is invalid"
      return 0
      ;;
  esac

  now_epoch=$(date +%s 2>/dev/null || echo 0)
  case "$now_epoch" in
    "" | *[!0-9]* | 0)
      echo "nix-cache: maintenance due because current time is unavailable"
      return 0
      ;;
  esac

  age=$((now_epoch - last_epoch))
  if [ "$age" -lt 0 ]; then
    echo "nix-cache: maintenance due because previous maintenance stamp is in the future"
    return 0
  fi

  if [ "$age" -ge "$interval" ]; then
    echo "nix-cache: maintenance due; last full scan was ${age}s ago (interval ${interval}s)"
    return 0
  fi

  echo "nix-cache: maintenance not due; last full scan was ${age}s ago (interval ${interval}s)"
  return 1
}

mark_cache_maintenance() {
  date +%s > "$CACHE_MAINTENANCE_STAMP" 2>/dev/null || true
}

narinfo_field() {
  field=$1
  narinfo=$2

  if [ ! -r "$narinfo" ]; then
    return
  fi

  while IFS= read -r line; do
    case "$line" in
      "$field: "*)
        echo "${line#"$field: "}"
        return
        ;;
    esac
  done < "$narinfo"
}

narinfo_url() {
  narinfo_field URL "$1"
}

narinfo_store_path() {
  narinfo_field StorePath "$1"
}

write_narinfo_list() {
  ls -tr "$NIX_CACHE_DIR"/*.narinfo > "$1" 2>/dev/null || true
}

write_payload_nar_list() {
  : > "$1"

  if [ -d "$NIX_CACHE_DIR/nar" ]; then
    for nar in "$NIX_CACHE_DIR"/nar/*; do
      if [ -f "$nar" ]; then
        echo "$nar"
      fi
    done >> "$1"
  fi

  for nar in "$NIX_CACHE_DIR"/*.nar "$NIX_CACHE_DIR"/*.nar.*; do
    if [ -f "$nar" ]; then
      echo "$nar"
    fi
  done >> "$1"

  sort -u "$1" > "$1.sorted"
  mv "$1.sorted" "$1"
}

log_payload_summary() {
  payload_count=0
  payload_bytes=0

  if [ -d "$NIX_CACHE_DIR/nar" ]; then
    for nar in "$NIX_CACHE_DIR"/nar/*; do
      if [ -f "$nar" ]; then
        payload_count=$((payload_count + 1))
      fi
    done
    payload_bytes=$(du -sb "$NIX_CACHE_DIR/nar" 2>/dev/null | cut -f1 || true)
  fi

  case "$payload_bytes" in
    "" | *[!0-9]*) payload_bytes=0 ;;
  esac

  echo "nix-cache: nar directory payload files: $payload_count"
  echo "nix-cache: nar directory payload bytes: $payload_bytes ($(nix_cache_size_mb "$payload_bytes") MB)"
}

log_malformed_narinfo_sample() {
  narinfo=$1
  sample_count=$2
  if [ "$sample_count" -lt 10 ]; then
    size=unknown
    if [ -r "$narinfo" ]; then
      size=$(wc -c < "$narinfo" 2>/dev/null | tr -d ' ' || echo unknown)
    fi
    first_line=
    if [ -r "$narinfo" ]; then
      IFS= read -r first_line < "$narinfo" || true
    fi
    echo "nix-cache: malformed narinfo sample path=$narinfo size=${size:-unknown} first_line=${first_line:-empty}"
  fi
}

remove_narinfo_entry() {
  narinfo=$1
  url=$(narinfo_url "$narinfo")
  store_path=$(narinfo_store_path "$narinfo")

  echo "nix-cache: pruning entry narinfo=$narinfo store_path=${store_path:-unknown} url=${url:-unknown}"
  rm -f "$narinfo"

  if [ -n "$url" ]; then
    nar="$NIX_CACHE_DIR/$url"
    if [ -f "$nar" ]; then
      echo "nix-cache: removing $nar"
      rm -f "$nar"
    fi
  fi
}

remove_malformed_narinfos() {
  malformed_count=0
  sample_count=0

  write_narinfo_list /tmp/nix-cache-narinfos.txt
  while read -r narinfo; do
    url=$(narinfo_url "$narinfo")
    store_path=$(narinfo_store_path "$narinfo")
    if [ -z "$url" ] || [ -z "$store_path" ]; then
      echo "$narinfo"
    fi
  done < /tmp/nix-cache-narinfos.txt > /tmp/nix-cache-malformed-narinfos.txt

  if [ -s /tmp/nix-cache-malformed-narinfos.txt ]; then
    malformed_count=$(wc -l < /tmp/nix-cache-malformed-narinfos.txt | tr -d ' ')
    echo "nix-cache: removing $malformed_count malformed narinfo entries"
    while read -r narinfo; do
      log_malformed_narinfo_sample "$narinfo" "$sample_count"
      sample_count=$((sample_count + 1))
      remove_narinfo_entry "$narinfo"
    done < /tmp/nix-cache-malformed-narinfos.txt
  fi
  rm -f /tmp/nix-cache-narinfos.txt /tmp/nix-cache-malformed-narinfos.txt
  echo "nix-cache: malformed narinfo entries: $malformed_count"
}

remove_broken_narinfos() {
  broken_count=0

  write_narinfo_list /tmp/nix-cache-narinfos.txt
  while read -r narinfo; do
    url=$(narinfo_url "$narinfo")
    if [ -n "$url" ] && [ ! -f "$NIX_CACHE_DIR/$url" ]; then
      echo "$narinfo"
    fi
  done < /tmp/nix-cache-narinfos.txt > /tmp/nix-cache-broken-narinfos.txt

  if [ -s /tmp/nix-cache-broken-narinfos.txt ]; then
    broken_count=$(wc -l < /tmp/nix-cache-broken-narinfos.txt | tr -d ' ')
    echo "nix-cache: removing $broken_count broken narinfo entries"
    while read -r narinfo; do
      remove_narinfo_entry "$narinfo"
    done < /tmp/nix-cache-broken-narinfos.txt
  fi
  rm -f /tmp/nix-cache-narinfos.txt /tmp/nix-cache-broken-narinfos.txt
  echo "nix-cache: broken narinfo entries: $broken_count"
}

remove_orphan_nars() {
  orphan_count=0
  orphan_bytes=0
  sample_count=0

  log_payload_summary

  write_narinfo_list /tmp/nix-cache-narinfos.txt
  while read -r narinfo; do
    url=$(narinfo_url "$narinfo")
    store_path=$(narinfo_store_path "$narinfo")
    case "$url" in
      "" | /* | *../*) ;;
      *)
        if [ -n "$store_path" ]; then
          echo "$NIX_CACHE_DIR/$url"
        fi
        ;;
    esac
  done < /tmp/nix-cache-narinfos.txt | sort -u > /tmp/nix-cache-live-nars.txt

  write_payload_nar_list /tmp/nix-cache-payload-nars.txt

  comm -23 /tmp/nix-cache-payload-nars.txt /tmp/nix-cache-live-nars.txt \
    > /tmp/nix-cache-orphan-nars.txt

  if [ -s /tmp/nix-cache-orphan-nars.txt ]; then
    orphan_count=$(wc -l < /tmp/nix-cache-orphan-nars.txt | tr -d ' ')
    while read -r nar; do
      if [ -f "$nar" ]; then
        bytes=$(du -sb "$nar" 2>/dev/null | cut -f1 || true)
        case "$bytes" in
          "" | *[!0-9]*) bytes=0 ;;
        esac
        orphan_bytes=$((orphan_bytes + bytes))
        if [ "$sample_count" -lt 10 ]; then
          echo "nix-cache: orphan nar sample path=$nar size_mb=$(nix_cache_size_mb "$bytes")"
          sample_count=$((sample_count + 1))
        fi
        rm -f "$nar"
      fi
    done < /tmp/nix-cache-orphan-nars.txt
  fi

  rm -f \
    /tmp/nix-cache-narinfos.txt \
    /tmp/nix-cache-live-nars.txt \
    /tmp/nix-cache-payload-nars.txt \
    /tmp/nix-cache-orphan-nars.txt

  echo "nix-cache: orphan nar payloads: $orphan_count"
  echo "nix-cache: orphan nar payload bytes removed: $orphan_bytes ($(nix_cache_size_mb "$orphan_bytes") MB)"
}

# Remove oldest cache entries until cache is under CACHE_MAX_BYTES.
prune_nix_cache() {
  echo "nix-cache: calculating cache size..."
  current_bytes=$(nix_cache_size_bytes)
  echo "nix-cache: cache size before maintenance is $(nix_cache_size_mb "$current_bytes") MB"

  if [ "$current_bytes" -le "$CACHE_MAX_BYTES" ] && ! cache_maintenance_due; then
    echo "nix-cache: under limit ($(nix_cache_size_mb "$CACHE_MAX_BYTES") MB), skipping deep consistency scan"
    return
  fi

  if [ "$current_bytes" -le "$CACHE_MAX_BYTES" ]; then
    echo "nix-cache: under limit, running periodic consistency scan"
  else
    echo "nix-cache: over limit, running consistency scan before pruning"
  fi

  remove_malformed_narinfos
  remove_broken_narinfos
  remove_orphan_nars
  current_bytes=$(nix_cache_size_bytes)
  echo "nix-cache: cache size after maintenance is $(nix_cache_size_mb "$current_bytes") MB"

  if [ "$current_bytes" -le "$CACHE_MAX_BYTES" ]; then
    echo "nix-cache: under limit ($(nix_cache_size_mb "$CACHE_MAX_BYTES") MB), no pruning needed"
    mark_cache_maintenance
    return
  fi

  echo "nix-cache: over limit, pruning to $(nix_cache_size_mb "$CACHE_MAX_BYTES") MB..."

  # Delete oldest cache entries first. A cache entry is a .narinfo and its
  # referenced NAR payload; deleting one without the other corrupts the index.
  # Re-check size every 50 deletions to avoid running du on every iteration.
  count=0
  write_narinfo_list /tmp/narinfos-to-prune.txt

  while read -r narinfo; do
    remove_narinfo_entry "$narinfo"

    count=$((count + 1))
    if [ $((count % 50)) -eq 0 ]; then
      echo "nix-cache: deleted $count entries so far, recalculating size..."
      current_bytes=$(nix_cache_size_bytes)
      echo "nix-cache: cache size is $(nix_cache_size_mb "$current_bytes") MB"
      if [ "$current_bytes" -le "$CACHE_MAX_BYTES" ]; then
        break
      fi
    fi
  done < /tmp/narinfos-to-prune.txt
  rm -f /tmp/narinfos-to-prune.txt

  current_bytes=$(nix_cache_size_bytes)
  echo "nix-cache: pruning done, removed $count entries, final size: $(nix_cache_size_mb "$current_bytes") MB"
  mark_cache_maintenance
}

save_nix_cache() {
  # Always save — nix copy skips paths already present in the destination,
  # so this is cheap on warm runs but ensures newly-built derivations
  # (updated deps, new flake inputs) are cached for next time.
  echo "nix-cache: saving new store paths to local cache..."
  nix copy --to "file://$NIX_CACHE_DIR?compression=none" --all 2>/dev/null || true
  prune_nix_cache
  echo "nix-cache: done saving to local cache"
}

echo "=== with-nix-cache: start $(date -u +%H:%M:%S) ==="
setup_fetch_retries
setup_nix_cache
trap save_nix_cache EXIT

# Raise fd soft limit to hard limit early so all child processes
# (nix builds, process-compose services) inherit the higher value.
# shellcheck disable=SC3045
hard_limit=$(ulimit -Hn 2>/dev/null || echo 1024)
# shellcheck disable=SC3045
ulimit -n "$hard_limit" 2>/dev/null || true
# shellcheck disable=SC3045
echo "fd limits: soft=$(ulimit -Sn) hard=$(ulimit -Hn)"

echo "=== with-nix-cache: setup done $(date -u +%H:%M:%S), running task ==="

"$@"

