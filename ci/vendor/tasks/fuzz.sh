#!/usr/bin/env bash

#! Shared cargo-fuzz runner — the single source of truth for fuzzing logic,
#! vendored from galoy-concourse-shared into each repo's ci/vendor/tasks/.
#!
#! Used by:
#!   - the Concourse `fuzz` job (fuzz_job() in pipeline-fragments.lib.yml):
#!     the restore-corpus / store-corpus steps pass the corpus in/out as a
#!     tarball via CORPUS_TARBALL_IN/OUT (GCS handled by those gsutil steps)
#!   - `make fuzz` / a repo-specific flake app, for local runs
#!
#! Repo-agnostic: discovers targets via `cargo fuzz list`. The repo must ship a
#! `fuzz/` cargo-fuzz crate.
#!
#! Env vars (all optional):
#!   FUZZ_SECONDS        seconds to fuzz each target (default: 60)
#!   FUZZ_JOBS           libFuzzer `-jobs` per target; cores ~= #targets * FUZZ_JOBS
#!   CORPUS_TARBALL_IN   glob of a corpus tarball to extract before fuzzing
#!   CORPUS_TARBALL_OUT    path to write the evolved corpus tarball after fuzzing
#!   ARTIFACTS_TARBALL_OUT path to write a tarball of crash/oom artifacts (if any)
#!
#! A repo may optionally ship a curated seed corpus at `fuzz/seeds/<target>/`
#! (one input per file). It is merged into `fuzz/corpus/<target>/` before every
#! run, after the stored corpus is restored — so coverage re-bootstraps even if
#! the stored corpus is pruned, and seed changes travel with the code. Repos
#! without `fuzz/seeds/` are unaffected (backward compatible).
#!
#! Requires: bash, git, cargo, tar, and cargo-fuzz (auto-installed if missing).

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

if ! command -v cargo-fuzz >/dev/null 2>&1; then
  echo "cargo-fuzz not found on PATH; installing..."
  cargo install cargo-fuzz --locked
fi

FUZZ_SECONDS="${FUZZ_SECONDS:-60}"

#! Build with the committed SQLx offline cache (.sqlx/) instead of connecting to
#! a database — there is no Postgres in a fuzz build, and coverage-guided fuzzing
#! wants pure, deterministic targets anyway. Harmless for non-SQLx repos (the
#! var is ignored) and overridable via the environment. Mirrors what each repo's
#! local `make fuzz` already does.
export SQLX_OFFLINE="${SQLX_OFFLINE:-true}"

#! Restore the corpus (no-op unless CORPUS_TARBALL_IN is set and matches a file).
mkdir -p fuzz/corpus
if [ -n "${CORPUS_TARBALL_IN:-}" ] && ls -d $CORPUS_TARBALL_IN >/dev/null 2>&1; then
  echo "restoring corpus from $CORPUS_TARBALL_IN"
  tar -xzf $CORPUS_TARBALL_IN -C fuzz/
fi

mapfile -t targets < <(cd fuzz && cargo fuzz list)
echo "discovered ${#targets[@]} target(s): ${targets[*]}"

#! Merge any committed seed corpus (fuzz/seeds/<target>/) into fuzz/corpus/.
#! Optional + backward compatible: skipped per-target when the dir is absent,
#! so repos without seeds (e.g. es-entity) are unaffected. Done after the
#! stored corpus is restored and per discovered target (no phantom corpus dirs
#! for stale seed folders). libFuzzer de-duplicates, so re-merging each run is
#! cheap and keeps the curated baseline present even if the corpus is pruned.
for target in "${targets[@]}"; do
  seed_dir="fuzz/seeds/$target"
  [ -d "$seed_dir" ] || continue
  mkdir -p "fuzz/corpus/$target"
  # `/.` copies the directory's contents (incl. dotfiles) into the corpus.
  cp -R "$seed_dir"/. "fuzz/corpus/$target/" 2>/dev/null || true
done
[ -d fuzz/seeds ] && echo "merged fuzz/seeds/ into fuzz/corpus/"

(cd fuzz && cargo fuzz build --sanitizer=none)

JOBS_ARG=""
if [ -n "${FUZZ_JOBS:-}" ]; then
  JOBS_ARG="-jobs=$FUZZ_JOBS"
fi

echo "fuzzing ${#targets[@]} target(s) in parallel for ${FUZZ_SECONDS}s${JOBS_ARG:+ ($JOBS_ARG per target)}"
pids=""
rc=0
for target in "${targets[@]}"; do
  (cd fuzz && cargo fuzz run "$target" --sanitizer=none -- \
    -max_total_time="$FUZZ_SECONDS" -timeout=25 $JOBS_ARG \
    -artifact_prefix="artifacts/$target/") &
  pids="$pids $!"
done
for p in $pids; do
  wait "$p" || rc=1
done

if [ "$rc" -ne 0 ]; then
  echo "==== FUZZ CRASH DETECTED ===="
  find fuzz/artifacts -type f -print || true
fi

#! Always package the corpus and any crash artifacts — even when a target
#! crashed. A discovery must not discard the run's coverage gains or the
#! failing input. Packaging runs before the (possible) non-zero exit so the
#! job's ensure-steps can still upload both; the script still exits non-zero
#! so the build surfaces the discovery (e.g. via on_failure).
if [ -n "${CORPUS_TARBALL_OUT:-}" ]; then
  mkdir -p "$(dirname "$CORPUS_TARBALL_OUT")"
  tar -czf "$CORPUS_TARBALL_OUT" -C fuzz corpus
  echo "packaged corpus -> $CORPUS_TARBALL_OUT"
fi
if [ -n "${ARTIFACTS_TARBALL_OUT:-}" ] && [ -d fuzz/artifacts ] && \
   [ -n "$(find fuzz/artifacts -type f 2>/dev/null | head -1)" ]; then
  mkdir -p "$(dirname "$ARTIFACTS_TARBALL_OUT")"
  tar -czf "$ARTIFACTS_TARBALL_OUT" -C fuzz artifacts
  echo "packaged artifacts -> $ARTIFACTS_TARBALL_OUT"
fi

exit "$rc"
