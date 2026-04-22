{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  llvm = pkgs.llvmPackages_21;
  custom = inputs.ifiokjr-nixpkgs.packages.${pkgs.stdenv.hostPlatform.system};
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
      curl
      custom.agave
      custom.mdt
      custom.surfpool
      dprint
      gcc
      git
      gitleaks
      libiconv
      mdbook
      custom.knope
      custom.pnpm-standalone
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
      python3
      rust-jemalloc-sys
      # Upstream rustup 1.28+ fails in nix builds: check suite is network-sensitive
      # and the install phase fails generating shell completions because the sandbox
      # creates an empty settings.toml missing the required `version` field.
      (rustup.overrideAttrs (old: {
        doCheck = false;
        preInstall = (old.preInstall or "") + ''
          export HOME="$(mktemp -d)"
          mkdir -p "$HOME/.rustup"
          echo 'version = "12"' > "$HOME/.rustup/settings.toml"
        '';
      }))
      shfmt
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
    LIBCLANG_PATH = "${llvm.libclang.lib}/lib";
    CC = "${llvm.clang}/bin/clang";
    CXX = "${llvm.clang}/bin/clang++";
    PROTOC = "${pkgs.protobuf}/bin/protoc";
    LD_LIBRARY_PATH = "${config.env.DEVENV_PROFILE}/lib";
  };

  # Rely on the global sdk for now as the nix apple sdk is not working for me.
  # apple.sdk = if pkgs.stdenv.isDarwin then pkgs.apple-sdk_15 else null;
  apple.sdk = null;

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
      "secrets:push" = {
        enable = true;
        verbose = true;
        pass_filenames = false;
        name = "secrets";
        description = "Scan repository history for leaked secrets with gitleaks before push.";
        entry = "${pkgs.gitleaks}/bin/gitleaks detect --verbose --redact";
        stages = [ "pre-push" ];
      };
      "lint:test" = {
        enable = true;
        verbose = true;
        pass_filenames = false;
        name = "lint and test";
        description = "Run the local CI lint rules suite before push.";
        entry = ''
          set -euo pipefail

          ${config.env.DEVENV_PROFILE}/bin/lint:all
          ${config.env.DEVENV_PROFILE}/bin/test:all
          ${config.env.DEVENV_PROFILE}/bin/test:idl
        '';
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

    GIT_CONFIG_GLOBAL=/dev/null ${pkgs.prek}/bin/prek install -f -c .pre-commit-config.yaml -t pre-commit
    GIT_CONFIG_GLOBAL=/dev/null ${pkgs.prek}/bin/prek install -f -c .pre-commit-config.yaml -t pre-push
  '';

  # Use the stdenv conditionally.
  # stdenv = if pkgs.stdenv.isLinux then llvm.stdenv else pkgs.stdenv;
  stdenv = pkgs.stdenv;

  enterShell = ''
    set -e
    export LDFLAGS="$NIX_LDFLAGS";
    if command -v pnpm-activate-env >/dev/null 2>&1; then
      eval "$(pnpm-activate-env)"
    fi
  '';

  # disable dotenv since it breaks the variable interpolation supported by `direnv`
  dotenv.disableHint = true;

  scripts = {
    "query-security-txt" = {
      exec = ''
        set -euo pipefail
        cargo bin query-security-txt $@
      '';
      description = "The `query-security-txt` executable";
      binary = "bash";
    };
    "wait-for-them" = {
      exec = ''
        set -euo pipefail
        cargo bin wait-for-them $@
      '';
      description = "The `wait-for-them` executable";
      binary = "bash";
    };
    "sbpf-linker" = {
      exec = ''
        set -euo pipefail

        if [ -n "''${XDG_CACHE_HOME:-}" ]; then
          CACHE_BASE="$XDG_CACHE_HOME"
        elif [ -n "''${HOME:-}" ] && [ "$HOME" != "/" ]; then
          CACHE_BASE="$HOME/.cache"
        else
          CACHE_BASE="$DEVENV_ROOT/.cache"
        fi

        gallery_sbpf_linker="$CACHE_BASE/sbpf-linker-upstream-gallery/bin/sbpf-linker"
        if [ -x "$gallery_sbpf_linker" ]; then
          "$gallery_sbpf_linker" "$@"
          exit 0
        fi

        cargo bin sbpf-linker "$@"
      '';
      description = "The `sbpf-linker` executable";
      binary = "bash";
    };
    "solana-verify" = {
      exec = ''
        set -euo pipefail
        cargo bin solana-verify $@
      '';
      description = "The `solana-verify` executable";
      binary = "bash";
    };
    "dylint-link" = {
      exec = ''
        set -euo pipefail
        cargo bin dylint-link $@
      '';
      description = "The `dylint-link` executable";
      binary = "bash";
    };
    "kani" = {
      exec = ''
        set -euo pipefail
        cargo bin kani $@
      '';
      description = "The `kani` executable";
      binary = "bash";
    };
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
        "$DEVENV_ROOT/scripts/generate-codama-idls.sh"
        dprint fmt "codama/**"
      '';
      description = "Generate Codama IDLs for all example programs.";
      binary = "bash";
    };
    "codama:clients:generate" = {
      exec = ''
        set -euo pipefail
        pnpm --dir "$DEVENV_ROOT" install --frozen-lockfile
        pina codama generate \
          --examples-dir "$DEVENV_ROOT/examples" \
          --idls-dir "$DEVENV_ROOT/codama/idls" \
          --rust-out "$DEVENV_ROOT/codama/clients/rust" \
          --js-out "$DEVENV_ROOT/codama/clients/js" \
          --npx node
        dprint fmt "codama/**"
      '';
      description = "Generate Codama IDLs and Rust/JS clients for all examples.";
      binary = "bash";
    };
    "codama:test" = {
      exec = ''
        set -euo pipefail
        bash "$DEVENV_ROOT/codama/test.sh"
      '';
      description = "Run the full Codama integration pipeline.";
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
    "install:sbpf-gallery" = {
      exec = ''
        set -euo pipefail

        if [ -n "''${XDG_CACHE_HOME:-}" ]; then
          CACHE_BASE="$XDG_CACHE_HOME"
        elif [ -n "''${HOME:-}" ] && [ "$HOME" != "/" ]; then
          CACHE_BASE="$HOME/.cache"
        else
          CACHE_BASE="$DEVENV_ROOT/.cache"
        fi

        CACHE_DIR="$CACHE_BASE/sbpf-linker-upstream-gallery"
        LLVM_SRC="$CACHE_DIR/llvm-project"
        LLVM_BUILD="$CACHE_DIR/llvm-build"
        LLVM_INSTALL="$CACHE_DIR/llvm-install"
        LLVM_CONFIG="$LLVM_INSTALL/bin/llvm-config"
        SBPF_SRC="$CACHE_DIR/sbpf-linker"
        SBPF_BIN="$CACHE_DIR/bin/sbpf-linker"

        mkdir -p "$CACHE_DIR"

        # Step 1: Build custom LLVM (BPF target only)
        if [ ! -f "$LLVM_CONFIG" ]; then
          if [ ! -d "$LLVM_SRC" ]; then
            echo "=== [1/3] Cloning Blueshift LLVM fork ==="
            git clone --depth 1 --branch upstream-gallery-21 \
              https://github.com/blueshift-gg/llvm-project.git "$LLVM_SRC"
          fi

          mkdir -p "$LLVM_BUILD" "$LLVM_INSTALL"

          echo "=== [2/3] Building LLVM (BPF target only, this may take 30+ minutes) ==="
          cmake -S "$LLVM_SRC/llvm" -B "$LLVM_BUILD" \
            -G Ninja \
            -DCMAKE_BUILD_TYPE=Release \
            -DCMAKE_INSTALL_PREFIX="$LLVM_INSTALL" \
            -DLLVM_ENABLE_PROJECTS= \
            -DLLVM_ENABLE_RUNTIMES= \
            -DLLVM_TARGETS_TO_BUILD=BPF \
            -DLLVM_BUILD_LLVM_DYLIB=OFF \
            -DLLVM_LINK_LLVM_DYLIB=OFF \
            -DLLVM_BUILD_TESTS=OFF \
            -DLLVM_INCLUDE_TESTS=OFF \
            -DLLVM_ENABLE_ASSERTIONS=ON \
            -DLLVM_ENABLE_ZLIB=OFF \
            -DLLVM_ENABLE_ZSTD=OFF \
            -DLLVM_INSTALL_UTILS=ON

          NUM_CPUS=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)
          cmake --build "$LLVM_BUILD" --target install -- -j"$NUM_CPUS"

          echo "LLVM installed: $("$LLVM_CONFIG" --version)"
        else
          echo "LLVM already built at $LLVM_INSTALL ($("$LLVM_CONFIG" --version))"
        fi

        # Step 2: Clone sbpf-linker
        if [ ! -d "$SBPF_SRC" ]; then
          echo "=== Cloning sbpf-linker ==="
          git clone --depth 1 https://github.com/blueshift-gg/sbpf-linker.git "$SBPF_SRC"
        else
          echo "Updating sbpf-linker..."
          git -C "$SBPF_SRC" pull --ff-only 2>/dev/null || true
        fi

        # Step 3: Build sbpf-linker with gallery features
        echo "=== Building sbpf-linker with upstream-gallery-21 ==="
        if [ "$(uname)" = "Darwin" ]; then
          # On macOS, point to Nix-provided static libs for the link step.
          # CXXSTDLIB_PATH: libc++ from the Nix LLVM toolchain
          # ZLIB_PATH / LIBZSTD_PATH: compression libs for the linker
          export CXXSTDLIB_PATH="${llvm.libcxx}/lib"
          export ZLIB_PATH="${pkgs.zlib}/lib"
          export LIBZSTD_PATH="${pkgs.zstd}/lib"
        elif [ "$(uname)" = "Linux" ]; then
          # bpf-linker static linking expects libstdc++.a to be discoverable.
          # On Nix-based CI, that path may not be present in compiler
          # search dirs, so resolve it explicitly.
          CXXSTDLIB_ARCHIVE="$(gcc -print-file-name=libstdc++.a 2>/dev/null || true)"
          if [ -z "$CXXSTDLIB_ARCHIVE" ] || [ "$CXXSTDLIB_ARCHIVE" = "libstdc++.a" ] || [ ! -f "$CXXSTDLIB_ARCHIVE" ]; then
            CXXSTDLIB_ARCHIVE="$(g++ -print-file-name=libstdc++.a 2>/dev/null || true)"
          fi

          if [ -n "$CXXSTDLIB_ARCHIVE" ] && [ "$CXXSTDLIB_ARCHIVE" != "libstdc++.a" ] && [ -f "$CXXSTDLIB_ARCHIVE" ]; then
            export CXXSTDLIB_PATH="$(dirname "$CXXSTDLIB_ARCHIVE")"
          else
            echo "warning: failed to locate libstdc++.a via gcc/g++; falling back to Nix GCC lib path"
            export CXXSTDLIB_PATH="${pkgs.gcc.cc.lib}/lib"
          fi
        fi

        LLVM_PREFIX="$LLVM_INSTALL" \
          cargo install \
            --path "$SBPF_SRC" \
            --root "$CACHE_DIR" \
            --no-default-features \
            --features "upstream-gallery-21,bpf-linker/llvm-link-static" \
            --force

        # Symlink into the cache bin directory so it's discoverable on PATH.
        mkdir -p "$CACHE_DIR/bin"
        if [ "$SBPF_BIN" != "$CACHE_DIR/bin/sbpf-linker" ]; then
          ln -sf "$SBPF_BIN" "$CACHE_DIR/bin/sbpf-linker"
        fi
        export PATH="$CACHE_DIR/bin:$PATH"

        echo ""
        echo "Done! sbpf-linker (gallery) installed."
        echo "Cache directory: $CACHE_DIR"
        "$SBPF_BIN" --version 2>/dev/null || echo "(binary ready)"
      '';
      description = "Build sbpf-linker with custom Blueshift LLVM (upstream-gallery-21). First run builds LLVM from source (~30 min).";
      binary = "bash";
    };
    "clean:sbpf-gallery" = {
      exec = ''
        set -euo pipefail

        if [ -n "''${XDG_CACHE_HOME:-}" ]; then
          CACHE_BASE="$XDG_CACHE_HOME"
        elif [ -n "''${HOME:-}" ] && [ "$HOME" != "/" ]; then
          CACHE_BASE="$HOME/.cache"
        else
          CACHE_BASE="$DEVENV_ROOT/.cache"
        fi

        CACHE_DIR="$CACHE_BASE/sbpf-linker-upstream-gallery"

        if [ -d "$CACHE_DIR" ]; then
          echo "Removing $CACHE_DIR ..."
          rm -rf "$CACHE_DIR"
          echo "Cleaned."
        else
          echo "Nothing to clean (no cache at $CACHE_DIR)."
        fi

        # Remove the symlink from cache bin if present
        if [ -L "$CACHE_DIR/bin/sbpf-linker" ]; then
          rm -f "$CACHE_DIR/bin/sbpf-linker"
          echo "Removed cached sbpf-linker symlink."
        fi
      '';
      description = "Remove the cached Blueshift LLVM build and gallery sbpf-linker binary.";
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
        if [ -z "$CI" ]; then
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
        build:pina:token-only
        build:pina:all-features
      '';
      description = "Verify the explicit `pina` feature matrix used in CI.";
      binary = "bash";
    };
    "test:all" = {
      exec = ''
        set -euo pipefail
        cargo test --all-features --locked
      '';
      description = "Run all tests across the crates";
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
      '';
      description = "Run the dedicated Miri regression suite for guard-backed loader behavior.";
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
    "test:pina:feature-matrix" = {
      exec = ''
        set -euo pipefail
        test:pina:default
        test:pina:no-default
        doc:pina:no-default
        test:pina:token-only
        test:pina:all-features
      '';
      description = "Run the explicit `pina` feature matrix used in CI.";
      binary = "bash";
    };
    "test:program-e2e" = {
      exec = ''
        set -euo pipefail

        # Ensure sbpf-linker is built against the Blueshift LLVM upstream gallery.
        install:sbpf-gallery

        # Run unit and parity tests for all example programs.
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
          -p escrow_program \
          -p pina_bpf

        # Blueshift's upstream-gallery-21 linker is LLVM 21-based.
        # Build the BPF artifact with a Rust toolchain that also uses LLVM 21
        # to avoid producer/reader attribute mismatches at link time.
        BPF_TOOLCHAIN="nightly-2025-11-20"
        if ! rustup toolchain list | grep -q "^$BPF_TOOLCHAIN"; then
          rustup toolchain install "$BPF_TOOLCHAIN" --profile minimal --component rust-src
        else
          rustup component add rust-src --toolchain "$BPF_TOOLCHAIN"
        fi

        cargo +"$BPF_TOOLCHAIN" build-bpf
        cargo test --locked -p pina_bpf bpf_build_ -- --ignored

        # Run mollusk-svm e2e tests against the compiled SBF binaries.
        # These verify that generated clients produce valid instructions
        # that the on-chain programs accept and process correctly.
        SBF_OUT_DIR="$DEVENV_ROOT/target/deploy" \
          cargo test --locked \
            -p role_registry_program --test e2e \
            -p staking_rewards_program --test e2e \
            -p vesting_program --test e2e \
            -- --nocapture

        # Run LiteSVM e2e tests with the generated TypeScript clients.
        # These verify that TS instruction builders with pina's discriminator
        # model produce transactions the on-chain programs accept.
        pnpm --dir "$DEVENV_ROOT/codama/tests/litesvm" install --frozen-lockfile
        SBF_OUT_DIR="$DEVENV_ROOT/target/deploy" \
          pnpm --dir "$DEVENV_ROOT/codama/tests/litesvm" test

        # Run Quasar SVM tests alongside LiteSVM. These execute generated
        # instructions directly against the compiled program ELF in-process,
        # which is useful for fast instruction/account-cycle validation
        # without a validator.
        pnpm --dir "$DEVENV_ROOT/codama/tests/quasar-svm" install --frozen-lockfile
        SBF_OUT_DIR="$DEVENV_ROOT/target/deploy" \
          pnpm --dir "$DEVENV_ROOT/codama/tests/quasar-svm" test
      '';
      description = "Build SBF binaries and run end-to-end program tests including mollusk-svm integration.";
      binary = "bash";
    };
    "profile:cu:tracked" = {
      exec = ''
        set -euo pipefail
        rm -rf "$DEVENV_ROOT/target/cu/current"
        "$DEVENV_ROOT/scripts/profile-tracked-examples.sh" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/cu/current"
      '';
      description = "Build tracked SBF example programs and capture static CU profiles for the current checkout.";
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

        "$DEVENV_ROOT/scripts/profile-tracked-examples.sh" \
          "$worktree_dir" \
          "$DEVENV_ROOT/target/cu/base"

        "$DEVENV_ROOT/scripts/profile-tracked-examples.sh" \
          "$DEVENV_ROOT" \
          "$DEVENV_ROOT/target/cu/head"

        python3 "$DEVENV_ROOT/scripts/compare-compute-units.py" \
          --policy-file "$DEVENV_ROOT/scripts/compute-unit-policy.json" \
          --base-dir "$DEVENV_ROOT/target/cu/base" \
          --head-dir "$DEVENV_ROOT/target/cu/head" \
          --markdown-output "$DEVENV_ROOT/target/cu/comparison.md" \
          --json-output "$DEVENV_ROOT/target/cu/comparison.json"

        cat "$DEVENV_ROOT/target/cu/comparison.md"
      '';
      description = "Compare tracked static CU profiles for the current checkout against origin/main.";
      binary = "bash";
    };
    "idl:generate" = {
      exec = ''
        set -euo pipefail
        "$DEVENV_ROOT/scripts/generate-codama-idls.sh"
      '';
      description = "Generate Codama IDLs for all examples.";
      binary = "bash";
    };
    "verify:idls" = {
      exec = ''
        set -euo pipefail
        "$DEVENV_ROOT/scripts/verify-codama-idls.sh"
      '';
      description = "Verify Codama generation, fixture drift, validation, and deterministic output.";
      binary = "bash";
    };
    "test:idl" = {
      exec = ''
        set -euo pipefail
        verify:idls
      '';
      description = "Run full Codama integration and deterministic generation checks.";
      binary = "bash";
    };
    "test:surfpool-idl" = {
      exec = ''
        set -euo pipefail
        pnpm install --frozen-lockfile
        "$DEVENV_ROOT/scripts/test-surfpool-idl-smoke.sh"
      '';
      description = "Deploy a generated program to Surfpool and invoke it using generated IDL metadata.";
      binary = "bash";
    };
    "coverage:all" = {
      exec = ''
        set -euo pipefail
        mkdir -p "$DEVENV_ROOT/target/coverage"
        rm -rf "$DEVENV_ROOT/target/llvm-cov-target"
        cargo llvm-cov \
          --all-features \
          --locked \
          -p pina \
          -p pina_cli \
          --lcov \
          --output-path "$DEVENV_ROOT/target/coverage/lcov.info"
      '';
      description = "Run coverage for pina + pina_cli and generate an lcov report.";
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
    "mutants:all" = {
      exec = ''
        set -euo pipefail
        mkdir -p "$DEVENV_ROOT/target/mutants"
        cargo mutants --all-features --locked --output "$DEVENV_ROOT/target/mutants"
      '';
      description = "Run mutation testing across all core workspace crates (nightly).";
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

        # Map changed files to workspace crates by checking crates/* directories.
        changed_packages=()
        for crate_dir in "$DEVENV_ROOT"/crates/*/; do
          crate_name=$(basename "$crate_dir")
          if echo "$changed_files" | grep -qE "^crates/$crate_name/"; then
            changed_packages+=("$crate_name")
          fi
        done

        if [ ''${#changed_packages[@]} -eq 0 ]; then
          echo "No workspace packages changed; skipping mutation testing."
          exit 0
        fi

        echo "Changed packages: ''${changed_packages[*]}"
        mkdir -p "$DEVENV_ROOT/target/mutants"

        pkg_args=()
        for pkg in "''${changed_packages[@]}"; do
          pkg_args+=("-p" "$pkg")
        done

        cargo mutants --all-features --locked --output "$DEVENV_ROOT/target/mutants" "''${pkg_args[@]}"
      '';
      description = "Run mutation testing only on crates changed relative to a base branch (PR).";
      binary = "bash";
    };
    "mutants:crate" = {
      exec = ''
        set -euo pipefail
        if [ $# -eq 0 ]; then
          echo "Usage: mutants:crate <package-name>" >&2
          exit 1
        fi
        mkdir -p "$DEVENV_ROOT/target/mutants"
        cargo mutants --all-features --locked --output "$DEVENV_ROOT/target/mutants" -p "$1"
      '';
      description = "Run mutation testing on a single workspace crate.";
      binary = "bash";
    };
    "kani:proofs" = {
      exec = ''
        set -euo pipefail
        # Run all Kani harnesses in pina_pod_primitives.
        cargo kani --manifest-path "$DEVENV_ROOT/crates/pina_pod_primitives/Cargo.toml" --all-features --locked
      '';
      description = "Run Kani model-checking proofs for pina_pod_primitives.";
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
        dprint fmt --config "$DEVENV_ROOT/dprint.json"
        docs:sync
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
    "security:dylint" = {
      exec = ''
        set -euo pipefail

        cargo_dylint_bin="$(find "$DEVENV_ROOT/.bin" -path '*/cargo-dylint/*/bin/cargo-dylint' | sort | tail -n 1)"

        if [ -z "$cargo_dylint_bin" ]; then
          echo "Missing cargo-dylint in $DEVENV_ROOT/.bin. Run 'install:cargo:bin'." >&2
          exit 1
        fi

        if ! command -v dylint-link >/dev/null 2>&1; then
          echo "Missing dylint-link command. Run 'install:cargo:bin'." >&2
          exit 1
        fi

        mapfile -t target_manifests < <(
          find "$DEVENV_ROOT/examples" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort
          find "$DEVENV_ROOT/security" -mindepth 3 -maxdepth 3 -path '*/secure/Cargo.toml' | sort
        )

        package_args=()
        for manifest in "''${target_manifests[@]}"; do
          package_name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1)"
          if [ -n "$package_name" ]; then
            package_args+=(--package "$package_name")
          fi
        done

        if [ "''${#package_args[@]}" -eq 0 ]; then
          echo "Could not discover any example or security packages to lint." >&2
          exit 1
        fi

        CARGO_INCREMENTAL=0 \
          cargo dylint --all --no-deps "''${package_args[@]}" -- --all-features --all-targets --locked
      '';
      description = "Run custom security dylint checks against the example and security program crates.";
      binary = "bash";
    };
    "security:deny" = {
      exec = ''
        set -euo pipefail
        cargo bin cargo-deny check --config "$DEVENV_ROOT/deny.toml" bans licenses sources
      '';
      description = "Run cargo-deny checks (bans, licenses, sources).";
      binary = "bash";
    };
    "security:audit" = {
      exec = ''
        set -euo pipefail
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
        set -euo pipefail
        security:dylint
        security:deny
        security:audit
      '';
      description = "Run all custom and dependency security checks.";
      binary = "bash";
    };
    "lint:all" = {
      exec = ''
        set -euo pipefail
        lint:clippy
        lint:format
        verify:docs
        security:dylint
      '';
      description = "Run all checks, including all custom dylint rules.";
      binary = "bash";
    };
    "docs:build" = {
      exec = ''
        set -euo pipefail
        mdbook build "$DEVENV_ROOT/docs"
      '';
      description = "Build the mdBook documentation.";
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
        mdt check --path "$DEVENV_ROOT"
      '';
      description = "Check reusable documentation blocks are synchronized.";
      binary = "bash";
    };
    "lint:format" = {
      exec = ''
        set -euo pipefail
        dprint check
      '';
      description = "Check that all files are formatted.";
      binary = "bash";
    };
    "verify:docs" = {
      exec = ''
        set -euo pipefail
        docs:check
        [ -f "$DEVENV_ROOT/docs/book.toml" ]
        [ -f "$DEVENV_ROOT/docs/src/SUMMARY.md" ]
        mdbook build "$DEVENV_ROOT/docs" -d "$DEVENV_ROOT/target/mdbook"
        docs:api
      '';
      description = "Verify docs folder structure, build mdBook, and check API docs.";
      binary = "bash";
    };
    "docs:api" = {
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

        RUSTDOCFLAGS="-D warnings" cargo doc \
          --workspace \
          --all-features \
          --no-deps \
          --locked \
          --document-private-items \
          ''${exclude_args[@]}
      '';
      description = "Build API documentation and fail on broken doc links.";
      binary = "bash";
    };
    "lint:clippy" = {
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

        cargo clippy --workspace --all-features --locked ''${exclude_args[@]}

        mapfile -t lint_manifests < <(find "$DEVENV_ROOT/lints" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
        for manifest in "''${lint_manifests[@]}"; do
          cargo clippy --manifest-path "$manifest" --all-features --all-targets --locked
        done
      '';
      description = "Check that all rust lints are passing.";
      binary = "bash";
    };
    "setup:vscode" = {
      exec = ''
        set -euo pipefail
        rm -rf .vscode
        cp -r $DEVENV_ROOT/setup/editors/vscode .vscode
      '';
      description = "Setup the environment for vscode.";
      binary = "bash";
    };
    "setup:helix" = {
      exec = ''
        set -euo pipefail
        rm -rf .helix
        cp -r $DEVENV_ROOT/setup/editors/helix .helix
      '';
      description = "Setup for the helix editor.";
      binary = "bash";
    };
  };
}
