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
      
      cargoVendorDir = craneLib.vendorCargoDeps {
        inherit (commonArgs) src cargoLock;

        outputHashes = {
          "git+https://github.com/HyperparamAI/sqlxmq?rev=52c3daf6af55416aefa4b1114e108f968f6c57d4#52c3daf6af55416aefa4b1114e108f968f6c57d4" = "sha256-nYD3c/Pj95bOHHhFS+rdXVpJgFl9BkVmWZ05/Dot6rY=";
        };

      };
      
      cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
        pname = "bria-deps";
        version = "0.0.0";
        cargoVendorDir = cargoVendorDir;     
        
        preConfigure = ''
          export CARGO_NET_GIT_FETCH_WITH_CLI=true
          export PROTOC="${pkgs.protobuf}/bin/protoc"
          export PATH="${pkgs.protobuf}/bin:${pkgs.gitMinimal}/bin:${pkgs.coreutils}/bin:$PATH"
          export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          export CARGO_HTTP_CAINFO="$SSL_CERT_FILE"
          export GIT_SSL_CAINFO="$SSL_CERT_FILE"
        '';
      });
      
      bria = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        cargoVendorDir = cargoVendorDir;     
        doCheck = false;
        
        preConfigure = ''
          export CARGO_NET_GIT_FETCH_WITH_CLI=true
          export PROTOC="${pkgs.protobuf}/bin/protoc"
          export PATH="${pkgs.protobuf}/bin:${pkgs.gitMinimal}/bin:${pkgs.coreutils}/bin:$PATH"
          export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          export CARGO_HTTP_CAINFO="$SSL_CERT_FILE"
          export GIT_SSL_CAINFO="$SSL_CERT_FILE"
        '';
      });
      

      
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
        };
        
        checks = {
          inherit bria;
          
          bria-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--lib --bins -- --deny warnings";
          });
          
          bria-fmt = craneLib.cargoFmt {
            inherit (commonArgs) src;
          };
        };
        
        apps = {
          default = flake-utils.lib.mkApp {
            drv = bria;
          };
          
          local-daemon = flake-utils.lib.mkApp {
            drv = pkgs.writeShellScriptBin "bria-local-daemon" ''
              export SIGNER_ENCRYPTION_KEY="0000000000000000000000000000000000000000000000000000000000000000"
              exec ${bria}/bin/bria daemon --config ./bats/bria.local.yml postgres://user:password@127.0.0.1:5432/pg run
            '';
          };
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
