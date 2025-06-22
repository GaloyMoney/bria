{
  description = "Bria";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };
  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
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
        extensions = ["rust-analyzer" "rust-src" "rustfmt" "clippy"];
      };
      
      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      rustSource = pkgs.lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          craneLib.filterCargoSources path type
          || pkgs.lib.hasInfix "/migrations/" path
          || pkgs.lib.hasInfix "/proto/" path
          || pkgs.lib.hasInfix "/.sqlx/" path;
      };
      
      commonArgs = {
        src = rustSource;
        strictDeps = true;
        cargoToml = ./Cargo.toml;
        cargoLock = ./Cargo.lock;
        
        buildInputs = with pkgs; [
          protobuf
        ] ++ lib.optionals pkgs.stdenv.isDarwin [
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];
        
        nativeBuildInputs = with pkgs; [
          protobuf
          pkg-config
          cmake
          cacert
          gitMinimal
          coreutils
        ];
        
        SQLX_OFFLINE = true;
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        PROTOC_INCLUDE = "${pkgs.protobuf}/include";
      };
      
      cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
        pname = "bria-deps";
        version = "0.0.0";
        
        configurePhase = ''
          export CARGO_NET_GIT_FETCH_WITH_CLI=true
          export PROTOC="${pkgs.protobuf}/bin/protoc"
          export PATH="${pkgs.protobuf}/bin:${pkgs.gitMinimal}/bin:${pkgs.coreutils}/bin:$PATH"
          export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          export CARGO_HTTP_CAINFO="$SSL_CERT_FILE"
        '';
      });
      
      bria = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        doCheck = false;
        
        configurePhase = ''
          export CARGO_NET_GIT_FETCH_WITH_CLI=true
          export PROTOC="${pkgs.protobuf}/bin/protoc"
          export PATH="${pkgs.protobuf}/bin:${pkgs.gitMinimal}/bin:${pkgs.coreutils}/bin:$PATH"
          export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          export CARGO_HTTP_CAINFO="$SSL_CERT_FILE"
        '';
      });
      
      checkCode = craneLib.mkCargoDerivation {
        pname = "check-code";
        version = "0.1.0";
        src = rustSource;
        cargoToml = ./Cargo.toml;
        cargoLock = ./Cargo.lock;
        inherit cargoArtifacts;
        SQLX_OFFLINE = true;
        
        nativeBuildInputs = with pkgs; [
          protobuf
          cacert
          cargo-audit
        ];
        
        configurePhase = ''
          export CARGO_NET_GIT_FETCH_WITH_CLI=true
          export PROTOC="${pkgs.protobuf}/bin/protoc"
          export PATH="${pkgs.protobuf}/bin:${pkgs.gitMinimal}/bin:${pkgs.coreutils}/bin:$PATH"
          export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          export CARGO_HTTP_CAINFO="$SSL_CERT_FILE"
        '';
        
        buildPhaseCargoCommand = "check";
        buildPhase = ''
          cargo fmt --check --all
          cargo clippy --all-features
          cargo audit
        '';
        installPhase = "touch $out";
      };
      
      nativeBuildInputs = with pkgs;
        [
          rustToolchain
          protobuf
        ]
        ++ lib.optionals pkgs.stdenv.isDarwin [
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];
      devEnvVars = rec {
        PGDATABASE = "pg";
        PGUSER = "user";
        PGPASSWORD = "password";
        PGHOST = "127.0.0.1";
        DATABASE_URL = "postgres://${PGUSER}:${PGPASSWORD}@${PGHOST}:5432/pg";
        PG_CON = "${DATABASE_URL}";
      };
    in
      with pkgs; {
        packages = {
          default = bria;
          bria = bria;
          check-code = checkCode;
        };
        
        checks = {
          inherit bria;
          check-code = checkCode;
          
          bria-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--lib --bins -- --deny warnings";
          });
          
          bria-fmt = craneLib.cargoFmt {
            inherit (commonArgs) src;
          };
        };
        
        apps.default = flake-utils.lib.mkApp {
          drv = bria;
        };

        devShells.default = mkShell (devEnvVars
          // {
            inherit nativeBuildInputs;
            packages = [
              alejandra
              sqlx-cli
              bacon
              cargo-nextest
              cargo-audit
              cargo-watch
              postgresql
              docker-compose
              bats
              jq
              podman
              podman-compose
              bc
            ];
          });

        formatter = alejandra;
      });
}
