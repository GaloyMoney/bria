{
  description = "Bria";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };
  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem
    (system: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {
        inherit system overlays;
      };
      rustVersion = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      rustToolchain = rustVersion.override {
        extensions = ["rust-analyzer" "rust-src"];
      };
      nativeBuildInputs = with pkgs;
        [
          rustToolchain
        ]
        ++ lib.optionals pkgs.stdenv.isDarwin [
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];
    in
      with pkgs; {
        devShells.default = mkShell {
          inherit nativeBuildInputs;
          packages = [
            alejandra
            sqlx-cli
            cargo-nextest
            cargo-audit
            cargo-watch
            docker-compose
            bats
          ];
          shellHook = ''
            export DATABASE_URL=postgres://user:password@127.0.0.1:5432/pg
          '';
        };

        formatter = alejandra;
      });
}
