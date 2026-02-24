{
  pkgs,
  lib,
  config,
  ...
}:
let
  llvm = pkgs.llvmPackages_21;
in

{
  packages =
    with pkgs;
    [
      binaryen
      cargo-binstall
      cargo-run-bin
      chromedriver
      cmake
      dprint
      eget
      gcc
      libiconv
      nodejs
      pnpm
      mdbook
      nodejs
      pnpm
      llvm.bintools
      llvm.clang
      llvm.clang-tools
      llvm.libclang.lib
      llvm.lld
      llvm.llvm
      llvm.mlir
      nixfmt-rfc-style
      openssl
      perl
      pkg-config
      protobuf # needed for `solana-test-validator` in tests
      rust-jemalloc-sys
      # Upstream rustup check suite is network-sensitive (e.g. socks proxy test) and flakes in CI.
      (rustup.overrideAttrs (_: {
        doCheck = false;
      }))
      shfmt
      zstd
    ]
    ++ lib.optionals stdenv.isDarwin [
      coreutils
    ]
    ++ lib.optionals stdenv.isLinux [
      libgcc.lib
      udev
    ];

  env = {
    EGET_CONFIG = "${config.env.DEVENV_ROOT}/.eget/.eget.toml";
    OPENSSL_NO_VENDOR = "1";
    LIBCLANG_PATH = "${llvm.libclang.lib}/lib";
    CC = "${llvm.clang}/bin/clang";
    CXX = "${llvm.clang}/bin/clang++";
    PROTOC = "${pkgs.protobuf}/bin/protoc";
    LD_LIBRARY_PATH = "${config.env.DEVENV_PROFILE}/lib";
  };

  # Rely on the global sdk for now as the nix apple sdk is not working for me.
  # apple.sdk = if pkgs.stdenv.isDarwin then pkgs.apple-sdk_15 else null;
  apple.sdk = null;

  # Use the stdenv conditionally.
  # stdenv = if pkgs.stdenv.isLinux then llvm.stdenv else pkgs.stdenv;
  stdenv = pkgs.stdenv;

  enterShell = ''
    set -e
    export PATH="$DEVENV_ROOT/.eget/bin:$PATH";
    export LDFLAGS="$NIX_LDFLAGS";
  '';

  # disable dotenv since it breaks the variable interpolation supported by `direnv`
  dotenv.disableHint = true;

  scripts = {
    "knope" = {
      exec = ''
        set -e
        cargo bin knope $@
      '';
      description = "The `knope` executable";
      binary = "bash";
    };
    "query-security-txt" = {
      exec = ''
        set -e
        cargo bin query-security-txt $@
      '';
      description = "The `query-security-txt` executable";
      binary = "bash";
    };
    "sbpf-linker" = {
      exec = ''
        set -e
        cargo bin sbpf-linker $@
      '';
      description = "The `sbpf-linker` executable";
      binary = "bash";
    };
    "solana-verify" = {
      exec = ''
        set -e
        cargo bin solana-verify $@
      '';
      description = "The `solana-verify` executable";
      binary = "bash";
    };
    "pina" = {
      exec = ''
        set -e
        cargo run -p pina_cli -- $@
      '';
      description = "Run the `pina` CLI from source.";
      binary = "bash";
    };
    "mdt" = {
      exec = ''
        set -e
        cargo bin mdt $@
      '';
      description = "Run the pinned `mdt` CLI used for reusable docs.";
      binary = "bash";
    };
    "codama:idl:all" = {
      exec = ''
        set -e
        mkdir -p "$DEVENV_ROOT/codama/idls"
        for program_dir in "$DEVENV_ROOT"/examples/*/; do
          program_name=$(basename "$program_dir")
          echo "Generating IDL for $program_name"
          pina idl --path "$program_dir" --output "$DEVENV_ROOT/codama/idls/$program_name.json"
        done
      '';
      description = "Generate Codama IDLs for all example programs.";
      binary = "bash";
    };
    "codama:clients:generate" = {
      exec = ''
        set -e
        pnpm --dir "$DEVENV_ROOT/codama" install --frozen-lockfile
        pnpm --dir "$DEVENV_ROOT/codama" run generate
      '';
      description = "Generate Codama Rust and JS clients from IDLs.";
      binary = "bash";
    };
    "codama:test" = {
      exec = ''
        set -e
        bash "$DEVENV_ROOT/codama/test.sh"
      '';
      description = "Run the full Codama integration pipeline.";
      binary = "bash";
    };
    "generate:keypair" = {
      exec = ''
        set -e
        solana-keygen new -s -o $DEVENV_ROOT/$1.json --no-bip39-passphrase || true
      '';
      description = "Generate a local solana keypair. Must provide a name.";
      binary = "bash";
    };
    "install:all" = {
      exec = ''
        set -e
        install:cargo:bin
        install:eget
      '';
      description = "Install all packages.";
      binary = "bash";
    };
    "install:eget" = {
      exec = ''
        HASH=$(nix hash path --base32 ./.eget/.eget.toml)
        echo "HASH: $HASH"
        if [ ! -f ./.eget/bin/hash ] || [ "$HASH" != "$(cat ./.eget/bin/hash)" ]; then
          echo "Updating eget binaries"
          eget -D --to "$DEVENV_ROOT/.eget/bin"
          echo "$HASH" > ./.eget/bin/hash
        else
          echo "eget binaries are up to date"
        fi
      '';
      description = "Install github binaries with eget.";
    };
    "install:cargo:bin" = {
      exec = ''
        set -e
        cargo bin --install
      '';
      description = "Install cargo binaries locally.";
      binary = "bash";
    };
    "update:deps" = {
      exec = ''
        set -e
        cargo update
        devenv update
      '';
      description = "Update dependencies.";
      binary = "bash";
    };
    "build:all" = {
      exec = ''
        set -e
        if [ -z "$CI" ]; then
          echo "Builing project locally"
          cargo build --all-features
        else
          echo "Building in CI"
          cargo build --all-features --locked
        fi
      '';
      description = "Build all crates with all features activated.";
      binary = "bash";
    };
    "test:all" = {
      exec = ''
        set -e
        cargo test --all-features --locked
      '';
      description = "Run all tests across the crates";
      binary = "bash";
    };
    "test:anchor-parity" = {
      exec = ''
        set -e
        cargo test --locked \
          -p anchor_declare_id \
          -p anchor_declare_program \
          -p anchor_duplicate_mutable_accounts \
          -p anchor_errors \
          -p anchor_events \
          -p anchor_floats \
          -p anchor_realloc \
          -p anchor_system_accounts \
          -p anchor_sysvars \
          -p escrow_program
        rustup component add rust-src --toolchain nightly-2025-10-15
        cargo +nightly-2025-10-15 build-bpf
        cargo test --locked -p pinocchio_bpf_starter hello_world -- --ignored
      '';
      description = "Run Anchor parity example tests and starter BPF execution checks.";
      binary = "bash";
    };
    "idl:generate" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/scripts/generate-codama-idls.sh"
      '';
      description = "Generate Codama IDLs for all anchor_* examples.";
      binary = "bash";
    };
    "verify:idls" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/scripts/verify-codama-idls.sh"
      '';
      description = "Verify Codama IDL fixtures and JS/Rust IDL validation tests.";
      binary = "bash";
    };
    "test:idl" = {
      exec = ''
        set -e
        verify:idls
      '';
      description = "Run IDL fixture drift + Rust/JS Codama validation tests.";
      binary = "bash";
    };
    "coverage:all" = {
      exec = ''
        set -e
        mkdir -p "$DEVENV_ROOT/target/coverage"
        cargo llvm-cov --workspace --all-features --locked --lcov --output-path "$DEVENV_ROOT/target/coverage/lcov.info"
      '';
      description = "Run workspace coverage and generate an lcov report.";
      binary = "bash";
    };
    "fix:all" = {
      exec = ''
        set -e
        fix:clippy
        fix:format
      '';
      description = "Fix all autofixable problems.";
      binary = "bash";
    };
    "fix:format" = {
      exec = ''
        set -e
        dprint fmt --config "$DEVENV_ROOT/dprint.json"
        docs:sync
      '';
      description = "Format files with dprint, then re-sync mdt-managed docs.";
      binary = "bash";
    };
    "fix:clippy" = {
      exec = ''
        set -e
        cargo clippy --fix --allow-dirty --allow-staged --all-features --locked
      '';
      description = "Fix clippy lints for rust.";
      binary = "bash";
    };
    "security:deny" = {
      exec = ''
        set -e
        cargo bin cargo-deny check --config "$DEVENV_ROOT/deny.toml" bans licenses sources
      '';
      description = "Run cargo-deny checks (bans, licenses, sources).";
      binary = "bash";
    };
    "security:audit" = {
      exec = ''
        set -e
        cargo bin cargo-audit \
          --db "$DEVENV_ROOT/target/advisory-db-audit" \
          --url "https://github.com/RustSec/advisory-db.git" \
          --deny yanked \
          --file "$DEVENV_ROOT/Cargo.lock"
      '';
      description = "Run RustSec advisory audit for Cargo.lock.";
      binary = "bash";
    };
    "verify:security" = {
      exec = ''
        set -e
        security:deny
        security:audit
      '';
      description = "Run all dependency security checks.";
      binary = "bash";
    };
    "lint:all" = {
      exec = ''
        set -e
        lint:clippy
        lint:format
        verify:docs
      '';
      description = "Run all checks.";
      binary = "bash";
    };
    "docs:build" = {
      exec = ''
        set -e
        mdbook build "$DEVENV_ROOT/docs"
      '';
      description = "Build the mdBook documentation.";
      binary = "bash";
    };
    "docs:sync" = {
      exec = ''
        set -e
        mdt update --path "$DEVENV_ROOT"
      '';
      description = "Sync reusable documentation blocks with mdt.";
      binary = "bash";
    };
    "docs:check" = {
      exec = ''
        set -e
        mdt check --path "$DEVENV_ROOT"
      '';
      description = "Check reusable documentation blocks are synchronized.";
      binary = "bash";
    };
    "lint:format" = {
      exec = ''
        set -e
        dprint check
      '';
      description = "Check that all files are formatted.";
      binary = "bash";
    };
    "verify:docs" = {
      exec = ''
        set -e
        [ -f "$DEVENV_ROOT/docs/book.toml" ]
        [ -f "$DEVENV_ROOT/docs/src/SUMMARY.md" ]
        docs:check
        mdbook build "$DEVENV_ROOT/docs" -d "$DEVENV_ROOT/target/mdbook"
      '';
      description = "Verify docs folder structure and build docs.";
      binary = "bash";
    };
    "lint:clippy" = {
      exec = ''
        set -e
        cargo clippy --all-features --locked
      '';
      description = "Check that all rust lints are passing.";
      binary = "bash";
    };
    "setup:vscode" = {
      exec = ''
        set -e
        rm -rf .vscode
        cp -r $DEVENV_ROOT/setup/editors/vscode .vscode
      '';
      description = "Setup the environment for vscode.";
      binary = "bash";
    };
    "setup:helix" = {
      exec = ''
        set -e
        rm -rf .helix
        cp -r $DEVENV_ROOT/setup/editors/helix .helix
      '';
      description = "Setup for the helix editor.";
      binary = "bash";
    };
  };
}
