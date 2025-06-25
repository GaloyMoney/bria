install-dev-deps:
	cargo install cargo-nextest cargo-watch cargo-audit sqlx-cli

build:
	nix build

watch:
	RUST_BACKTRACE=full cargo watch -s 'cargo test -- --nocapture'

next-watch:
	cargo watch -s 'cargo nextest run'

test-integration: reset-deps
	cargo nextest run --verbose --locked

local-daemon:
	nix run .#local-daemon

build-x86_64-unknown-linux-musl-release:
	SQLX_OFFLINE=true cargo build --release --locked --target x86_64-unknown-linux-musl

build-x86_64-apple-darwin-release:
	bin/osxcross-compile.sh

clean-deps:
	docker compose down

start-deps:
	docker compose up -d integration-deps && sleep 2

reset-deps: clean-deps start-deps setup-db

setup-db:
	cargo sqlx migrate run

integration-tests-in-container:
	DATABASE_URL=postgres://user:password@postgres:5432/pg cargo sqlx migrate run
	DATABASE_URL=postgres://user:password@postgres:5432/pg cargo nextest run --verbose --locked

test-in-ci: start-deps
	DATABASE_URL=postgres://user:password@127.0.0.1:5432/pg cargo sqlx migrate run
	DATABASE_URL=postgres://user:password@127.0.0.1:5432/pg cargo nextest run --verbose --locked

e2e-tests-in-container:
	git config --global --add safe.directory /repo # otherwise bats complains
	SQLX_OFFLINE=true cargo build --locked
	bats -t bats

e2e: clean-deps build start-deps
	bats -t bats

# Nix convenience targets
nix-build:
	nix build

nix-shell:
	nix develop

nix-show:
	nix flake show

# Run the application
run:
	nix run
