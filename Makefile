install-dev-deps:
	nix develop -c cargo install cargo-nextest cargo-watch cargo-audit sqlx-cli

build:
	nix build

watch:
	nix develop -c bash -c 'RUST_BACKTRACE=full cargo watch -s "cargo test -- --nocapture"'

next-watch:
	nix develop -c cargo watch -s 'cargo nextest run'

test-integration: reset-deps
	nix develop -c cargo nextest run --verbose --locked

check-code:
	nix develop -c bash -c 'SQLX_OFFLINE=true cargo fmt --check --all'
	nix develop -c bash -c 'SQLX_OFFLINE=true cargo clippy --all-features'
	nix develop -c cargo audit
	
local-daemon:
	nix run .#local-daemon

build-x86_64-unknown-linux-musl-release:
	nix develop -c bash -c 'SQLX_OFFLINE=true cargo build --release --locked --target x86_64-unknown-linux-musl'

build-x86_64-apple-darwin-release:
	bin/osxcross-compile.sh

clean-deps:
	docker compose down

start-deps:
	docker compose up -d integration-deps && sleep 2

reset-deps: clean-deps start-deps setup-db

setup-db:
	nix develop -c cargo sqlx migrate run

integration-tests-in-container:
	nix develop -c bash -c 'DATABASE_URL=postgres://user:password@postgres:5432/pg cargo sqlx migrate run'
	nix develop -c bash -c 'DATABASE_URL=postgres://user:password@postgres:5432/pg cargo nextest run --verbose --locked'

test-in-ci: start-deps
	nix develop -c bash -c 'DATABASE_URL=postgres://user:password@127.0.0.1:5432/pg cargo sqlx migrate run'
	nix develop -c bash -c 'DATABASE_URL=postgres://user:password@127.0.0.1:5432/pg cargo nextest run --verbose --locked'

e2e-tests-in-container:
	git config --global --add safe.directory /repo # otherwise bats complains
	nix develop -c bash -c 'SQLX_OFFLINE=true cargo build --locked'
	nix develop -c bats -t bats

e2e: clean-deps build start-deps
	nix develop -c bats -t bats

