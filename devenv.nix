{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  currentDir = dirOf __curPos.file;
  llvm = pkgs.llvmPackages_21;
  custom = inputs.ifiokjr-nixpkgs.packages.${pkgs.stdenv.hostPlatform.system};
  # Must match the toolchain the Kani release bundle was built against: the
  # nixpkgs package fetches kani's prebuilt binaries, and `kani-compiler` links
  # `librustc_driver` from `$out/toolchain` at runtime. Kani 0.68.0 requires
  # nightly-2026-08-21 (its `rust-toolchain-version` file); linking any other
  # nightly aborts every harness with "Library not loaded: librustc_driver".
  kaniToolchain = pkgs.rust-bin.nightly."2026-08-21".minimal;
  kani = custom.kani.overrideAttrs (old: {
    buildInputs = (old.buildInputs or [ ]) ++ lib.optionals pkgs.stdenv.isLinux [ kaniToolchain ];
    postInstall = (old.postInstall or "") + ''
      if [ ! -e "$out/toolchain" ]; then
        ln -s ${kaniToolchain} "$out/toolchain"
      fi
    '';
  });
  # `cargo-llvm-cov`'s own test suite fails under the Nix rust infrastructure
  # on both Darwin (the instrumented test binary faults in the unwinder) and
  # Linux (its fixture workspaces fail after the current nixpkgs rust update).
  # The nixpkgs package documents this class of failure as tooling-only: the
  # installed `cargo llvm-cov` works, which is all `coverage:all` runs.
  cargo-llvm-cov = pkgs.cargo-llvm-cov.overrideAttrs (_: {
    doCheck = false;
  });
in

{
  packages =
    with pkgs;
    [
      binaryen
      cargo-audit
      cargo-binstall
      cargo-deny
      cargo-insta
      cargo-llvm-cov
      cargo-mutants
      cargo-nextest
      cargo-run-bin
      chromedriver
      cmake
      curl
      custom.agave
      custom.mdt
      custom.surfpool
      custom.wait-for-them
      dart
      dprint
      gcc
      git
      gitleaks
      jq
      libiconv
      mdbook
      custom.monochange
      nodejs_24
      pnpm
      llvm.bintools
      llvm.clang
      llvm.clang-tools
      llvm.libclang.lib
      llvm.lld
      llvm.llvm
      llvm.mlir
      ninja
      nixfmt-rfc-style
      openssl
      perl
      pkg-config
      protobuf
      rust-jemalloc-sys
      rustup
      shfmt
      zizmor
      zlib
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
    OPENSSL_NO_VENDOR = "1";
    # Rustup otherwise fails immediately on transient manifest or component
    # download errors, including while cargo-build-sbf inspects a base worktree.
    RUSTUP_MAX_RETRIES = "3";
    LIBCLANG_PATH = "${llvm.libclang.lib}/lib";
    CC = "${llvm.clang}/bin/clang";
    CXX = "${llvm.clang}/bin/clang++";
    PROTOC = "${pkgs.protobuf}/bin/protoc";
    LD_LIBRARY_PATH = "${config.env.DEVENV_PROFILE}/lib";
    # Rust toolchain used by the no-link `bpfel-unknown-none` check for
    # generated CPI crates. Deployable programs use cargo-build-sbf instead.
    PINA_BPF_TOOLCHAIN = "nightly-2025-11-20";
    # Shared by the program-e2e and Surfpool builders so every SBF artifact is
    # produced with the same pinned platform tools.
    SBF_TOOLS_VERSION = "v1.54";
  }
  # cc-rs compiles vendored C/C++ (e.g. libfuzzer-sys) by passing
  # `--target=arm64-apple-macosx` to `clang++`. The nix cc-wrapper explicitly
  # does not support that override ("multi-target compilers" warning) and its
  # mishandled include paths then fail to resolve libc++ headers against the
  # Xcode sysroot (`unknown type name 'uint8_t'`). Hand cc-rs Apple's toolchain
  # on macOS instead; it pairs with the global Xcode SDK, matching
  # `apple.sdk = null` above.
  #
  # HOST_* is required because the nix stdenv setup hooks re-export
  # CC=clang/CXX=clang++ after `env`, silently overriding shell-level CC/CXX
  # values; cc-rs prefers HOST_* over CC/CXX, and nothing overwrites those.
  #
  # Rust links host test binaries through the same nix cc-wrapper, which ends at
  # `cctools-binutils-darwin`'s `ld64` instead of Apple's linker. Binaries it
  # produces cannot initialise their unwinder, so every `panic!` aborts the
  # whole test binary with `fatal runtime error: failed to initiate panic,
  # error 5` — a failed assertion becomes indistinguishable from a crash, and
  # the `lint:push` hook (which re-enters devenv) can never pass.
  #
  # `CARGO_TARGET_<TARGET>_LINKER` redirects the host target only, leaving the
  # `bpfel-unknown-none` cross builds and Kani untouched. `/usr/bin/cc` is
  # Apple's driver, so it resolves the global SDK the same way `HOST_*` does.
  #
  # A release build strips the build scripts and proc macros it compiles for
  # the host, and on macOS rustc strips them with the llvm-objcopy bundled in
  # its own toolchain. Before LLVM 23 that tool leaves `__LINKEDIT` entries
  # misaligned, and Darwin 27's dyld refuses to load the result ("mis-aligned
  # LINKEDIT string pool"), so every proc macro under the pinned platform-tools
  # (LLVM 20) fails with "can't find crate". Host units never ship, so keeping
  # them unstripped changes no SBF artifact. `pina build` sets the same
  # override for projects built outside this shell.
  // lib.optionalAttrs pkgs.stdenv.hostPlatform.isDarwin {
    HOST_CC = "/usr/bin/clang";
    HOST_CXX = "/usr/bin/clang++";
    CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER = "/usr/bin/cc";
    CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER = "/usr/bin/cc";
    CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP = "false";
  };

  # Rely on the global sdk for now as the nix apple sdk is not working for me.
  # apple.sdk = if pkgs.stdenv.isDarwin then pkgs.apple-sdk_15 else null;
  apple.sdk = null;

  profiles.kani.module.packages = [ kani ];

  git-hooks = {
    package = pkgs.prek;
    hooks = {
      "secrets:commit" = {
        enable = true;
        verbose = true;
        pass_filenames = false;
        name = "secrets";
        description = "Scan staged changes for leaked secrets with gitleaks.";
        entry = "${pkgs.gitleaks}/bin/gitleaks protect --staged --verbose --redact";
        stages = [ "pre-commit" ];
      };
      dprint = {
        enable = true;
        verbose = true;
        pass_filenames = true;
        name = "dprint fmt";
        description = "Format changed files with dprint before commit.";
        entry = "${pkgs.dprint}/bin/dprint fmt --allow-no-files";
        stages = [ "pre-commit" ];
      };
      "lint:test" = {
        enable = true;
        verbose = true;
        pass_filenames = false;
        name = "lint:push";
        description = "Run the local CI lint rules and test suite before push.";
        entry = "${config.env.DEVENV_PROFILE}/bin/lint:push";
        stages = [ "pre-push" ];
      };
    };
  };

  tasks."devenv:git-hooks:install".exec = lib.mkForce ''
    if ! ${pkgs.git}/bin/git rev-parse --git-dir &> /dev/null; then
      echo 1>&2 "WARNING: git-hooks: .git not found; skipping hook installation."
      exit 0
    fi

    ${pkgs.git}/bin/git config --local --unset-all core.hooksPath 2>/dev/null || true

    # `git push` opens the SSH connection and completes the ref advertisement
    # before it runs the pre-push hook, so the connection sits idle for the
    # whole gate. GitHub closes a connection idle for six minutes, and the
    # lint:push gate runs longer than that: the push then dies with exit 141
    # (SIGPIPE) after the gate has already passed, and nothing is pushed.
    # Keepalives hold the connection open across the gate. Only set this when
    # the checkout has no ssh command of its own, so a custom identity or
    # ProxyCommand is never replaced.
    if [ -z "$(${pkgs.git}/bin/git config --local --get core.sshCommand)" ]; then
      ${pkgs.git}/bin/git config --local core.sshCommand \
        "ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=30"
    else
      echo 1>&2 "NOTE: core.sshCommand is set locally; add -o ServerAliveInterval=15 -o ServerAliveCountMax=30 so the pre-push gate cannot outlive the connection."
    fi

    GIT_CONFIG_GLOBAL=/dev/null ${pkgs.prek}/bin/prek install -f -c .pre-commit-config.yaml -t pre-commit
    GIT_CONFIG_GLOBAL=/dev/null ${pkgs.prek}/bin/prek install -f -c .pre-commit-config.yaml -t pre-push
  '';

  # Use the stdenv conditionally.
  # stdenv = if pkgs.stdenv.isLinux then llvm.stdenv else pkgs.stdenv;
  stdenv = pkgs.stdenv;

  enterShell = ''
    set -e
    export LDFLAGS="$NIX_LDFLAGS";
  '';

  # disable dotenv since it breaks the variable interpolation supported by `direnv`
  dotenv.disableHint = true;

  scripts = {
    "pina" = {
      exec = ''
        set -euo pipefail
        cargo run -p pina_cli -- $@
      '';
      description = "Run the `pina` CLI from source.";
      binary = "bash";
    };
    "codama:idl:all" = {
      exec = ''
        set -euo pipefail
        "$DEVENV_ROOT/scripts/generate-pina-clients.sh"
        dprint fmt "codama/**"
      '';
      description = "Generate IDLs and clients for all example programs.";
      binary = "bash";
    };
    "codama:clients:generate" = {
      exec = ''
        set -euo pipefail
        pnpm --dir "$DEVENV_ROOT" install --frozen-lockfile
        "$DEVENV_ROOT/scripts/generate-pina-clients.sh"
        dprint fmt "codama/**"
        # `dart format` picks its style from the package's resolved language
        # version, so the package config has to exist before formatting.
        (cd "$DEVENV_ROOT/codama/clients/dart" && dart pub get --enforce-lockfile)
        dart format "$DEVENV_ROOT/codama/clients/dart"
        (cd "$DEVENV_ROOT/codama/clients/cli/dart" && dart pub get --enforce-lockfile)
        dart format "$DEVENV_ROOT/codama/clients/cli/dart"
      '';
      description = "Generate Rust/CPI/JS/Dart/CLI clients for all examples with pina generate.";
      binary = "bash";
    };
    "codama:test" = {
      exec = ''
        set -euo pipefail
        bash "$DEVENV_ROOT/scripts/verify-pina-clients.sh"
      '';
      description = "Run the full client generation and validation pipeline.";
      binary = "bash";
    };
    "generate:keypair" = {
      exec = ''
        set -euo pipefail
        solana-keygen new -s -o $DEVENV_ROOT/$1.json --no-bip39-passphrase || true
      '';
      description = "Generate a local solana keypair. Must provide a name.";
      binary = "bash";
    };
    "install:all" = {
      exec = ''
        set -euo pipefail
        install:cargo:bin
      '';
      description = "Install all packages.";
      binary = "bash";
    };
    "install:cargo:bin" = {
      exec = ''
        set -euo pipefail
        cargo bin --install
      '';
      description = "Install cargo binaries locally.";
      binary = "bash";
    };
    "update:deps" = {
      exec = ''
        set -euo pipefail
        cargo update
        devenv update
      '';
      description = "Update dependencies.";
      binary = "bash";
    };
    "build:all" = {
      exec = ''
        set -euo pipefail
        if [ -z "''${CI:-}" ]; then
          echo "Building project locally"
          cargo build --all-features
        else
          echo "Building in CI"
          cargo build --all-features --locked
        fi
      '';
      description = "Build all crates with all features activated.";
      binary = "bash";
    };
    "build:default" = {
      exec = ''
        set -euo pipefail
        cargo build --locked
      '';
      description = "Build workspace crates with the default feature set.";
      binary = "bash";
    };
    "build:pina:default" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --locked
      '';
      description = "Verify `pina` builds with the default feature set.";
      binary = "bash";
    };
    "build:pina:no-default-only" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --no-default-features --locked
      '';
      description = "Verify `pina` builds with `--no-default-features`.";
      binary = "bash";
    };
    "build:pina:token-only" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --no-default-features --features token --locked
      '';
      description = "Verify `pina` builds with only the `token` feature enabled.";
      binary = "bash";
    };
    "build:pina:compact-only" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --no-default-features --features compact --locked
      '';
      description = "Verify `pina` builds with only the `compact` feature enabled.";
      binary = "bash";
    };
    "build:pina:floats-only" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --no-default-features --features floats --locked
      '';
      description = "Verify `pina` builds with only the `floats` feature enabled.";
      binary = "bash";
    };
    "build:pina:fixed-only" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --no-default-features --features fixed --locked
      '';
      description = "Verify `pina` builds with only the `fixed` feature enabled.";
      binary = "bash";
    };
    "build:pina:account-resize-only" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --no-default-features --features account-resize --locked
      '';
      description = "Verify raw account resizing does not enable compact mode.";
      binary = "bash";
    };
    "build:pina:alloc-only" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --no-default-features --features alloc --locked
      '';
      description = "Verify the heap opt-in builds without enabling anything else.";
      binary = "bash";
    };
    "build:pina:all-features" = {
      exec = ''
        set -euo pipefail
        cargo check -p pina --all-features --locked
      '';
      description = "Verify `pina` builds with all features enabled.";
      binary = "bash";
    };
    "build:pina:no-default" = {
      exec = ''
        set -euo pipefail
        build:pina:no-default-only
        cargo check -p pina --no-default-features --features derive --locked
        build:pina:compact-only
        build:pina:floats-only
        build:pina:fixed-only
        build:pina:account-resize-only
        build:pina:alloc-only
        build:pina:token-only
        cargo check -p pina --no-default-features --features token,derive --locked
      '';
      description = "Verify `pina` builds without default features and across key feature subsets.";
      binary = "bash";
    };
    "build:pina:feature-matrix" = {
      exec = ''
        set -euo pipefail
        build:pina:default
        build:pina:no-default-only
        build:pina:compact-only
        build:pina:floats-only
        build:pina:fixed-only
        build:pina:account-resize-only
        build:pina:alloc-only
        build:pina:token-only
        build:pina:all-features
      '';
      description = "Verify the explicit `pina` feature matrix used in CI.";
      binary = "bash";
    };
    "test:all" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" test:all
      '';
      description = "Run workspace tests, compile fuzz targets, and verify npm packages.";
      binary = "bash";
    };
    # NOTE: `test:all` and `coverage:all` must cover the same packages. `test:all`
    # is the post-merge and `ci-full` tier; `coverage:all` is the pull request
    # gate. Adding a suite to one belongs in the other.
    "build:pina-test" = {
      exec = ''
        set -euo pipefail
        cargo check \
          --manifest-path "$DEVENV_ROOT/crates/pina_test/Cargo.toml" \
          --all-targets \
          --locked
      '';
      description = "Check the isolated host-side pina_test package with its committed lockfile.";
      binary = "bash";
    };
    "test:pina-test" = {
      exec = ''
        set -euo pipefail
        cargo test \
          --manifest-path "$DEVENV_ROOT/crates/pina_test/Cargo.toml" \
          --all-targets \
          --locked
      '';
      description = "Test the isolated host-side pina_test package.";
      binary = "bash";
    };
    "lint:pina-test" = {
      exec = ''
        set -euo pipefail
        cargo clippy \
          --manifest-path "$DEVENV_ROOT/crates/pina_test/Cargo.toml" \
          --all-targets \
          --locked \
          -- \
          -D warnings
      '';
      description = "Lint the isolated host-side pina_test package.";
      binary = "bash";
    };
    "doc:pina-test" = {
      exec = ''
        set -euo pipefail
        RUSTDOCFLAGS="-D warnings" cargo doc \
          --manifest-path "$DEVENV_ROOT/crates/pina_test/Cargo.toml" \
          --no-deps \
          --locked
      '';
      description = "Build warning-free API documentation for pina_test.";
      binary = "bash";
    };
    "verify:pina-test" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" verify:pina-test
      '';
      description = "Run locked test, Clippy, and documentation gates for pina_test.";
      binary = "bash";
    };
    "test:npm-packages" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" test:npm-packages
      '';
      description = "Verify npm metadata, platform launchers, the CPI visitor, the skill installer, and release packaging scripts.";
      binary = "bash";
    };
    "test:fuzz:smoke" = {
      exec = ''
        set -euo pipefail
        export PATH=${pkgs.cargo-fuzz}/bin:"$PATH"
        ${pkgs.bash}/bin/bash ${lib.escapeShellArg "${currentDir}/scripts/run-fuzz-smoke.sh"}
      '';
      description = "Replay the committed fuzz corpus and run every target for a bounded duration.";
      binary = "bash";
    };
    "test:miri" = {
      exec = ''
        set -euo pipefail

        TOOLCHAIN="nightly-2026-02-20"
        rustup component add miri --toolchain "$TOOLCHAIN"
        cargo +"$TOOLCHAIN" miri setup

        MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
          cargo +"$TOOLCHAIN" miri test --locked -p pina --test miri_loader_guards --all-features
        MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
          cargo +"$TOOLCHAIN" miri test --locked -p pina --test schema_boundary --all-features
        # The entrypoint deserializes the loader's raw input through `unsafe`;
        # its tests build that input byte for byte.
        MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
          cargo +"$TOOLCHAIN" miri test --locked -p pina --lib --all-features entry::
        # `dispatch_entrypoint!` programs walk that input from generated code.
        MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
          cargo +"$TOOLCHAIN" miri test --locked -p pina --test dispatch_entrypoint --all-features
      '';
      description = "Run Miri regressions for loader guards, macro-generated schema storage, and the program entrypoints.";
      binary = "bash";
    };
    "test:kani:quick" = {
      exec = ''
        set -euo pipefail
        cargo-kani \
          -p pina \
          --lib \
          --no-default-features \
          --features account-resize,compact \
          --output-format terse \
          --harness quick_
      '';
      description = "Run fast Kani proofs for arithmetic, parsers, compact sizing, CPI metadata, and fixed layouts.";
      binary = "bash";
    };
    "test:kani:compact" = {
      exec = ''
        set -euo pipefail
        cargo-kani \
          -p pina \
          --lib \
          --no-default-features \
          --features compact \
          --output-format terse \
          --harness compact_
      '';
      description = "Run bounded Kani state-machine proofs for compact account layouts.";
      binary = "bash";
    };
    "test:kani" = {
      exec = ''
        set -euo pipefail
        test:kani:quick
        test:kani:compact
      '';
      description = "Run every Kani proof harness.";
      binary = "bash";
    };
    "test:pina:default" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --lib --locked
      '';
      description = "Run `pina` library tests with the default feature set.";
      binary = "bash";
    };
    "test:pina:no-default" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --no-default-features --lib --locked
      '';
      description = "Run `pina` library tests with `--no-default-features`.";
      binary = "bash";
    };
    "test:pina:token-only" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --no-default-features --features token --lib --locked
      '';
      description = "Run `pina` library tests with only the `token` feature enabled.";
      binary = "bash";
    };
    "test:pina:compact-only" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --no-default-features --features compact --lib --test compact_account --test cpi_ui --locked
      '';
      description = "Run compact schema and loader tests without unrelated optional features.";
      binary = "bash";
    };
    "test:pina:floats-only" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --no-default-features --features floats --lib --test floats --locked
      '';
      description = "Run IEEE-754 float schema tests without unrelated optional features.";
      binary = "bash";
    };
    "test:pina:fixed-only" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --no-default-features --features fixed --lib --test fixed --locked
      '';
      description = "Run fixed-point schema tests without unrelated optional features.";
      binary = "bash";
    };
    "test:pina:account-resize-only" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --no-default-features --features account-resize,derive --lib --test cpi_helpers --locked
        cargo test -p pina_macros --no-default-features --lib --locked
      '';
      description = "Test raw resizing with compact mode disabled, including the macro feature diagnostic.";
      binary = "bash";
    };
    "test:pina:all-features" = {
      exec = ''
        set -euo pipefail
        cargo test -p pina --all-features --lib --locked
      '';
      description = "Run `pina` library tests with all features enabled.";
      binary = "bash";
    };
    "doc:pina:no-default" = {
      exec = ''
        set -euo pipefail
        cargo doc -p pina --no-default-features --no-deps --locked
      '';
      description = "Build `pina` docs without default features to catch hidden default-feature coupling.";
      binary = "bash";
    };
    "doc:pina:compact-only" = {
      exec = ''
        set -euo pipefail
        cargo doc -p pina --no-default-features --features compact --no-deps --locked
      '';
      description = "Build `pina` docs with only compact mode enabled.";
      binary = "bash";
    };
    "test:pina:feature-matrix" = {
      exec = ''
        set -euo pipefail
        test:pina:default
        test:pina:no-default
        doc:pina:no-default
        test:pina:compact-only
        doc:pina:compact-only
        test:pina:floats-only
        test:pina:fixed-only
        test:pina:account-resize-only
        test:pina:token-only
        test:pina:all-features
      '';
      description = "Run the explicit `pina` feature matrix used in CI.";
      binary = "bash";
    };
    "test:program-e2e" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" test:program-e2e
      '';
      description = "Build SBF binaries and run end-to-end program tests including mollusk-svm integration.";
      binary = "bash";
    };
    "profile:cu:tracked" = {
      exec = ''
        set -euo pipefail
        rm -rf "$DEVENV_ROOT/target/cu/current"
        node "$DEVENV_ROOT/scripts/profile-tracked-examples.ts" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/cu/current"
      '';
      description = "Build every SBF example program and capture its static CU and binary-size profile.";
      binary = "bash";
    };
    "report:cu:compare:main" = {
      exec = ''
        set -euo pipefail

        git -C "$DEVENV_ROOT" fetch origin

        worktree_dir=$(mktemp -d "''${TMPDIR:-/tmp}/pina-cu-main-XXXXXX")

        cleanup() {
          git -C "$DEVENV_ROOT" worktree remove --force "$worktree_dir" >/dev/null 2>&1 || true
          rm -rf "$worktree_dir"
        }

        trap cleanup EXIT

        git -C "$DEVENV_ROOT" worktree add --detach "$worktree_dir" origin/main

        rm -rf "$DEVENV_ROOT/target/cu/base" "$DEVENV_ROOT/target/cu/head"

        node "$DEVENV_ROOT/scripts/profile-tracked-examples.ts" \
          "$worktree_dir" \
          "$DEVENV_ROOT/target/cu/base"

        node "$DEVENV_ROOT/scripts/profile-tracked-examples.ts" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/cu/head"

        node "$DEVENV_ROOT/scripts/build-runtime-compute-units.ts" \
          "$worktree_dir" \
          "$DEVENV_ROOT/target/cu/runtime-base-elf" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/cu/runtime-head-elf"

        node "$DEVENV_ROOT/scripts/measure-example-compute-units.ts" \
          "$DEVENV_ROOT" \
          "$worktree_dir" \
          "$DEVENV_ROOT/target/cu/runtime-base-elf" \
          "$DEVENV_ROOT/target/cu/runtime-base.json" \
          --allow-incomplete

        node "$DEVENV_ROOT/scripts/measure-example-compute-units.ts" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/cu/runtime-head-elf" \
          "$DEVENV_ROOT/target/cu/runtime-head.json"

        node "$DEVENV_ROOT/scripts/compare-compute-units.ts" \
          --policy-file "$DEVENV_ROOT/scripts/compute-unit-policy.json" \
          --base-revision "$(git -C "$worktree_dir" rev-parse HEAD)" \
          --base-dir "$DEVENV_ROOT/target/cu/base" \
          --head-dir "$DEVENV_ROOT/target/cu/head" \
          --base-runtime "$DEVENV_ROOT/target/cu/runtime-base.json" \
          --head-runtime "$DEVENV_ROOT/target/cu/runtime-head.json" \
          --markdown-output "$DEVENV_ROOT/target/cu/comparison.md" \
          --json-output "$DEVENV_ROOT/target/cu/comparison.json"

        cat "$DEVENV_ROOT/target/cu/comparison.md"
      '';
      description = "Compare all example instruction CU, static CU, and program sizes against origin/main.";
      binary = "bash";
    };
    "benchmark:frameworks" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/benchmark-frameworks.ts" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/framework-comparison" \
          --update-doc
      '';
      description = "Rebuild the Pina, Pinocchio, Quasar, Anchor v1 and Anchor v2 fixtures, measure size and compute units, and refresh docs/src/framework-comparison.md and the readme comparison tables.";
      binary = "bash";
    };
    "benchmark:frameworks:check" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/benchmark-frameworks.ts" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/framework-comparison-check"
      '';
      description = "Measure the framework fixtures without rewriting the published tables.";
      binary = "bash";
    };
    "idl:generate" = {
      exec = ''
        set -euo pipefail
        "$DEVENV_ROOT/scripts/generate-pina-clients.sh"
      '';
      description = "Generate IDLs and clients for all examples with pina generate.";
      binary = "bash";
    };
    "verify:idls" = {
      exec = ''
        set -euo pipefail
        ${lib.escapeShellArg "${currentDir}/scripts/verify-pina-clients.sh"}
      '';
      description = "Verify client generation, fixture drift, validation, and deterministic output.";
      binary = "bash";
    };
    "test:idl" = {
      exec = ''
        set -euo pipefail
        ${lib.escapeShellArg "${currentDir}/.devenv/profile/bin/verify:idls"}
      '';
      description = "Run full client integration and deterministic generation checks.";
      binary = "bash";
    };
    "test:surfpool" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" test:surfpool
      '';
      description = "Build, deploy, and adversarially exercise every SBF example through the Surfpool SDK.";
      binary = "bash";
    };
    "walkthrough:migrations" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" walkthrough:migrations "$@"
      '';
      description = "Run the ten-deployment migration walkthrough end to end.";
      binary = "bash";
    };
    "coverage:all" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" coverage:all
      '';
      description = "Run the full test suite under coverage and generate lcov reports.";
      binary = "bash";
    };
    "coverage:pina-test" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" coverage:pina-test
      '';
      description = "Generate coverage for the isolated pina_test package.";
      binary = "bash";
    };
    "coverage:vm:experimental" = {
      exec = ''
        set -euo pipefail
        if ! command -v mucho >/dev/null 2>&1; then
          echo "Skipping VM coverage: mucho is not installed."
          exit 0
        fi

        set +e
        mucho coverage
        status=$?
        set -euo pipefail

        if [ "$status" -ne 0 ]; then
          echo "Experimental VM coverage failed with status $status (non-blocking)."
        fi
      '';
      description = "Run experimental Solana VM coverage via mucho when available (non-blocking).";
      binary = "bash";
    };
    "mutants:run" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" mutants:run "$@"
      '';
      description = "Run cargo-mutants with the given arguments, failing when they select no mutants.";
      binary = "bash";
    };
    "mutants:prepare" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" mutants:prepare
      '';
      description = "Install the npm packages and renderer bundle that pina_cli's tests need under mutation testing.";
      binary = "bash";
    };
    "mutants:packages" = {
      exec = ''
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" mutants:packages
      '';
      description = "Print the crates covered by nightly mutation testing, one per line.";
      binary = "bash";
    };
    "mutants:all" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" mutants:all "$@"
      '';
      description = "Run mutation testing across all core workspace crates.";
      binary = "bash";
    };
    "mutants:diff" = {
      exec = ''
        set -euo pipefail

        base_ref="''${1:-}"
        if [ -z "$base_ref" ]; then
          base_ref="''${CI_MERGE_REQUEST_DIFF_BASE_REF:-''${GITHUB_BASE_REF:-main}}"
        fi
        echo "Base ref: $base_ref"

        # Determine changed files relative to the base ref.
        changed_files=$(git diff --name-only "$base_ref"...HEAD || true)
        if [ -z "$changed_files" ]; then
          echo "No changed files detected; skipping mutation testing."
          exit 0
        fi

        # Resolve package names from the root workspace so standalone nested
        # workspaces cannot produce a misleading zero-mutant run.
        changed_packages=()
        while IFS=$'\t' read -r package manifest; do
          relative_manifest="''${manifest#"$DEVENV_ROOT"/}"
          package_dir=$(dirname "$relative_manifest")
          if echo "$changed_files" | grep -qE "^''${package_dir}/(Cargo.toml|build.rs|src/|tests/|benches/|examples/)"; then
            changed_packages+=("$package")
          fi
        done < <(
          cargo metadata --no-deps --format-version 1 |
            jq -r --arg root "$DEVENV_ROOT/crates/" \
              '.packages[] | select(.manifest_path | startswith($root)) | [.name, .manifest_path] | @tsv'
        )

        if [ ''${#changed_packages[@]} -eq 0 ]; then
          echo "No workspace packages changed; skipping mutation testing."
          exit 0
        fi

        echo "Changed packages: ''${changed_packages[*]}"

        pkg_args=()
        for pkg in "''${changed_packages[@]}"; do
          pkg_args+=("-p" "$pkg")
        done

        mutants:run "''${pkg_args[@]}"
      '';
      description = "Run mutation testing only on crates changed relative to a base branch (PR).";
      binary = "bash";
    };
    "mutants:crate" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" mutants:crate "$@"
      '';
      description = "Run mutation testing on a single workspace crate; extra arguments go to cargo-mutants.";
      binary = "bash";
    };
    "fix:all" = {
      exec = ''
        set -euo pipefail
        fix:clippy
        fix:format
        codama:idl:all
        codama:clients:generate
      '';
      description = "Fix all autofixable problems.";
      binary = "bash";
    };
    "fix:format" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" fix:format
      '';
      description = "Format files with dprint, then re-sync mdt-managed docs.";
      binary = "bash";
    };
    "fix:clippy" = {
      exec = ''
        set -euo pipefail
        mapfile -t generated_client_manifests < <(find "$DEVENV_ROOT/codama/clients/rust" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
        exclude_args=()
        for manifest in "''${generated_client_manifests[@]}"; do
          package_name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1)"
          if [ -n "$package_name" ]; then
            exclude_args+=(--exclude "$package_name")
          fi
        done

        cargo clippy --fix --allow-dirty --allow-staged --workspace --all-features --locked ''${exclude_args[@]}

        mapfile -t lint_manifests < <(find "$DEVENV_ROOT/lints" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
        for manifest in "''${lint_manifests[@]}"; do
          cargo clippy --fix --allow-dirty --allow-staged --manifest-path "$manifest" --all-features --all-targets --locked
        done
      '';
      description = "Fix clippy lints for rust.";
      binary = "bash";
    };
    "security:pina-lint" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" security:pina-lint
      '';
      description = "Run Pina's security lints (pina_lint_driver) against the example and security program crates.";
      binary = "bash";
    };
    "security:deny" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" security:deny
      '';
      description = "Run cargo-deny checks (advisories, bans, licenses, sources).";
      binary = "bash";
    };
    "security:audit" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" security:audit
      '';
      description = "Run RustSec advisory audit using deny.toml's ignore policy.";
      binary = "bash";
    };
    "security:npm-audit" = {
      exec = ''
        set -euo pipefail
        ${pkgs.pnpm}/bin/pnpm --dir ${lib.escapeShellArg currentDir} audit
      '';
      description = "Fail on moderate-or-higher advisories in the pnpm lockfile.";
      binary = "bash";
    };
    "security:zizmor" = {
      exec = ''
        set -euo pipefail
        # --no-online-audits keeps the gate reproducible and sandbox-friendly;
        # offline audits still cover workflow injection, credential persistence,
        # dependency pinning, and permissions hardening.
        zizmor --no-online-audits --no-progress ${lib.escapeShellArg "${currentDir}/.github"}
      '';
      description = "Audit GitHub Actions workflows and composite actions with zizmor.";
      binary = "bash";
    };
    "verify:security" = {
      exec = ''
        set -euo pipefail
        security:pina-lint
        security:deny
        security:audit
        security:npm-audit
        security:zizmor
      '';
      description = "Run all custom and dependency security checks.";
      binary = "bash";
    };
    "lint:push" = {
      exec = ''
        set -euo pipefail

        profile_bin=${lib.escapeShellArg "${currentDir}/.devenv/profile/bin"}
        export PATH="$profile_bin''${PATH:+:$PATH}"

        # Git invokes hooks outside `devenv shell`. Re-enter the pinned shell
        # once so the compiler, SDK, and library paths are complete.
        if [ -z "''${DEVENV_ROOT:-}" ]; then
          exec ${pkgs.devenv}/bin/devenv shell -- "$profile_bin/lint:push"
        fi

        # The Nix clang wrapper does not infer Xcode's SDK when cc-rs compiles
        # vendored C dependencies. Supply it explicitly without affecting Linux.
        if [ "$(uname -s)" = "Darwin" ]; then
          sdk_root="$(/usr/bin/xcrun --sdk macosx --show-sdk-path)"
          export SDKROOT="$sdk_root"
          export CFLAGS="-isysroot $sdk_root''${CFLAGS:+ $CFLAGS}"
          export CXXFLAGS="-isysroot $sdk_root''${CXXFLAGS:+ $CXXFLAGS}"
        fi

        run_step() {
          local name="$1"
          shift
          echo "Currently running: $name"
          "$@"
        }

        run_step "gitleaks detect" ${pkgs.gitleaks}/bin/gitleaks detect --verbose --redact
        run_step "lint:clippy" "$profile_bin/lint:clippy"
        run_step "lint:format" "$profile_bin/lint:format"
        run_step "verify:docs" "$profile_bin/verify:docs"
        run_step "security:pina-lint" "$profile_bin/security:pina-lint"
        run_step "lint:monochange" "$profile_bin/lint:monochange"
        run_step "test:all" "$profile_bin/test:all"
        run_step "test:idl" "$profile_bin/test:idl"
      '';
      description = "Run the full local CI suite before push, independent of the active shell environment.";
      binary = "bash";
    };
    "lint:all" = {
      exec = ''
        set -euo pipefail
        lint:clippy
        lint:format
        pnpm install --frozen-lockfile --ignore-scripts
        pnpm run check:scripts
        # Type-checks the committed Codama JS clients plus the generated
        # TypeScript CLI apps, which no other task covers.
        pnpm run check:js
        verify:docs
        security:pina-lint
        lint:monochange
      '';
      description = "Run all checks, including all Pina lint rules.";
      binary = "bash";
    };
    "lint:monochange" = {
      exec = ''
        set -euo pipefail
        ${custom.monochange}/bin/monochange check
      '';
      description = "Validate monochange release metadata.";
      binary = "bash";
    };
    "docs:build" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" docs:build
      '';
      description = "Build the mdBook documentation and the pina.rs landing page.";
      binary = "bash";
    };
    "docs:sync" = {
      exec = ''
        set -euo pipefail
        mdt update --path "$DEVENV_ROOT"
      '';
      description = "Sync reusable documentation blocks with mdt.";
      binary = "bash";
    };
    "docs:check" = {
      exec = ''
        set -euo pipefail
        ${custom.mdt}/bin/mdt check --path ${lib.escapeShellArg currentDir}
      '';
      description = "Check reusable documentation blocks are synchronized.";
      binary = "bash";
    };
    "lint:format" = {
      exec = ''
        set -euo pipefail
        ${pkgs.dprint}/bin/dprint check
      '';
      description = "Check that all files are formatted.";
      binary = "bash";
    };
    "verify:docs" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" verify:docs
      '';
      description = "Verify docs folder structure, build mdBook, and check API docs.";
      binary = "bash";
    };
    "docs:api" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" docs:api
      '';
      description = "Build API documentation and fail on broken doc links.";
      binary = "bash";
    };
    "lint:clippy" = {
      exec = ''
        set -euo pipefail
        node "$DEVENV_ROOT/scripts/ci-tasks.ts" lint:clippy
      '';
      description = "Check that all rust lints are passing.";
      binary = "bash";
    };
  };
}
