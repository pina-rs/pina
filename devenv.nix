{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  currentDir = builtins.dirOf __curPos.file;
  llvm = pkgs.llvmPackages_21;
  custom = inputs.ifiokjr-nixpkgs.packages.${pkgs.stdenv.hostPlatform.system};
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
      custom.sbpf-linker
      custom.surfpool
      custom.wait-for-them
      dprint
      gcc
      git
      gitleaks
      libiconv
      mdbook
      custom.monochange
      nodejs_22
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
    # Single source of truth for the BPF/SBF Rust toolchain pin. The
    # compute-units workflow and the profile script read this instead of
    # hardcoding the version themselves.
    PINA_BPF_TOOLCHAIN = "nightly-2025-11-20";
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
    "dylint-link" = {
      exec = ''
        set -euo pipefail

        repo_root="''${DEVENV_ROOT:-}"
        if [ -z "$repo_root" ]; then
          repo_root="$(${pkgs.git}/bin/git rev-parse --show-toplevel 2>/dev/null || true)"
        fi
        if [ -z "$repo_root" ]; then
          for arg in "$@"; do
            case "$arg" in
              -*) ;;
              */target/dylint/libraries/*)
                repo_root="$(printf '%s\n' "$arg" | sed 's#/target/dylint/libraries/.*##')"
                break
                ;;
            esac
          done
        fi
        if [ -z "$repo_root" ]; then
          repo_root="$(pwd)"
        fi

        if [ -z "''${RUSTUP_TOOLCHAIN:-}" ] && [ -f "$repo_root/rust-toolchain.toml" ]; then
          RUSTUP_TOOLCHAIN="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\(.*\)".*/\1/p' "$repo_root/rust-toolchain.toml" | head -n 1)"
          export RUSTUP_TOOLCHAIN
        fi

        if [ -d "$repo_root/.bin" ]; then
          dylint_link_bin="$(find "$repo_root/.bin" -path '*/dylint-link/*/bin/dylint-link' | sort | tail -n 1)"
          if [ -n "$dylint_link_bin" ] && [ -x "$dylint_link_bin" ]; then
            if "$dylint_link_bin" --version >/dev/null 2>&1; then
              exec "$dylint_link_bin" "$@"
            fi

            echo "Ignoring broken cached dylint-link at $dylint_link_bin" >&2
            rm -f "$dylint_link_bin"
          fi
        fi

        cd "$repo_root"
        exec cargo bin dylint-link "$@"
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
        # The toolchain version is pinned once in the env section above.
        BPF_TOOLCHAIN="$PINA_BPF_TOOLCHAIN"
        if ! rustup toolchain list | grep -q "^$BPF_TOOLCHAIN"; then
          rustup toolchain install "$BPF_TOOLCHAIN" --profile minimal --component rust-src
        else
          rustup component add rust-src --toolchain "$BPF_TOOLCHAIN"
        fi

        PATH="${custom.sbpf-linker-21}/bin:$PATH" \
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
        if [ -z "''${HOME:-}" ]; then
          export HOME="$DEVENV_ROOT/.cache/home"
        fi
        mkdir -p "$HOME"

        litesvm_dir="$DEVENV_ROOT/codama/tests/litesvm"
        pnpm --dir "$litesvm_dir" install --frozen-lockfile
        (
          cd "$litesvm_dir"
          SBF_OUT_DIR="$DEVENV_ROOT/target/deploy" \
            node "$litesvm_dir/node_modules/vitest/vitest.mjs" run --pool threads
        )

        # Run Quasar SVM tests alongside LiteSVM. These execute generated
        # instructions directly against the compiled program ELF in-process,
        # which is useful for fast instruction/account-cycle validation
        # without a validator.
        quasar_svm_dir="$DEVENV_ROOT/codama/tests/quasar-svm"
        pnpm --dir "$quasar_svm_dir" install --frozen-lockfile
        (
          cd "$quasar_svm_dir"
          SBF_OUT_DIR="$DEVENV_ROOT/target/deploy" \
            node "$quasar_svm_dir/node_modules/vitest/vitest.mjs" run --pool threads
        )
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
        "${currentDir}/scripts/verify-codama-idls.sh"
      '';
      description = "Verify Codama generation, fixture drift, validation, and deterministic output.";
      binary = "bash";
    };
    "test:idl" = {
      exec = ''
        set -euo pipefail
        ${currentDir}/.devenv/profile/bin/verify:idls
      '';
      description = "Run full Codama integration and deterministic generation checks.";
      binary = "bash";
    };
    "test:surfpool-idl" = {
      exec = ''
        set -euo pipefail
        if [ -z "''${HOME:-}" ]; then
          export HOME="$DEVENV_ROOT/.cache/home"
        fi
        mkdir -p "$HOME"
        pnpm install --frozen-lockfile
        PATH="${custom.sbpf-linker-21}/bin:$PATH" \
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
        status=0
        cargo mutants --all-features --cargo-arg --locked --output "$DEVENV_ROOT/target/mutants" || status=$?
        if [ "$status" -eq 2 ] || [ "$status" -eq 3 ]; then
          echo "cargo-mutants reported missed mutants; keeping CI non-blocking and uploading the report."
          exit 0
        fi
        exit "$status"
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

        status=0
        cargo mutants --all-features --cargo-arg --locked --output "$DEVENV_ROOT/target/mutants" "''${pkg_args[@]}" || status=$?
        if [ "$status" -eq 2 ] || [ "$status" -eq 3 ]; then
          echo "cargo-mutants reported missed mutants; keeping CI non-blocking and uploading the report."
          exit 0
        fi
        exit "$status"
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
        status=0
        cargo mutants --all-features --cargo-arg --locked --output "$DEVENV_ROOT/target/mutants" -p "$1" || status=$?
        if [ "$status" -eq 2 ] || [ "$status" -eq 3 ]; then
          echo "cargo-mutants reported missed mutants; keeping CI non-blocking and uploading the report."
          exit 0
        fi
        exit "$status"
      '';
      description = "Run mutation testing on a single workspace crate.";
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

        # cargo-dylint and its dependencies need openssl at build time; expose
        # the nix openssl through pkg-config so installs work outside the
        # devenv shell too.
        export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        export PATH="${pkgs.pkg-config}/bin:$PATH"

        # The dylint library links pass -nodefaultlibs, so -lz/-lc++/-liconv
        # must resolve through the nix cc/ld wrappers with explicit -L paths.
        # Reconstruct that environment so the check works outside the devenv
        # shell too.
        case "$(uname -s)" in
          Darwin)
            host_triple="$(uname -m | tr -d '\n')_apple_darwin"
            ;;
          Linux)
            host_triple="$(uname -m | tr -d '\n')_unknown_linux_gnu"
            ;;
          *)
            host_triple=""
            ;;
        esac
        if [ -n "$host_triple" ]; then
          export "NIX_CC_WRAPPER_TARGET_HOST_''${host_triple}=1"
          export "NIX_BINTOOLS_WRAPPER_TARGET_HOST_''${host_triple}=1"
          export NIX_LDFLAGS="-L${pkgs.zlib}/lib -L${pkgs.libcxx}/lib -L${pkgs.libiconv}/lib''${NIX_LDFLAGS:+ $NIX_LDFLAGS}"
          export PATH="${pkgs.gcc}/bin:${pkgs.stdenv.cc.bintools}/bin:$PATH"
        fi

        resolve_bin_root() {
          if [ -d "${currentDir}/.bin" ]; then
            echo "${currentDir}/.bin"
            return 0
          fi

          while read -r worktree_path; do
            if [ -d "$worktree_path/.bin" ]; then
              echo "$worktree_path/.bin"
              return 0
            fi
          done < <(${pkgs.git}/bin/git -C "${currentDir}" worktree list --porcelain | ${pkgs.gawk}/bin/awk '/^worktree / { print $2 }')

          return 1
        }

        if ! bin_root="$(resolve_bin_root)"; then
          echo "Missing .bin directory for this repository. Run 'install:cargo:bin'." >&2
          exit 1
        fi

        cargo_dylint_version="5.0.0"
        cargo_dylint_root="$bin_root/manual/cargo-dylint/$cargo_dylint_version"
        cargo_dylint_bin="$cargo_dylint_root/bin/cargo-dylint"

        if [ ! -x "$cargo_dylint_bin" ]; then
          mkdir -p "$cargo_dylint_root"
          CARGO_TARGET_DIR="${currentDir}/target/cargo-install/cargo-dylint" \
            cargo install \
              --locked \
              --root "$cargo_dylint_root" \
              cargo-dylint \
              --version "$cargo_dylint_version"
        fi

        if [ ! -x "$cargo_dylint_bin" ]; then
          echo "Unable to install cargo-dylint into $cargo_dylint_root." >&2
          exit 1
        fi

        dylint_link_wrapper="${currentDir}/.devenv/profile/bin/dylint-link"
        if [ ! -x "$dylint_link_wrapper" ]; then
          echo "Missing dylint-link command. Run 'install:cargo:bin'." >&2
          exit 1
        fi

        mapfile -t target_manifests < <(
          find "${currentDir}/examples" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort
          find "${currentDir}/security" -mindepth 3 -maxdepth 3 -path '*/secure/Cargo.toml' | sort
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
          PATH="$(dirname "$dylint_link_wrapper"):$PATH" \
          "$cargo_dylint_bin" dylint --all --no-deps "''${package_args[@]}" -- --locked
      '';
      description = "Run custom security dylint checks against the example and security program crates.";
      binary = "bash";
    };
    "security:deny" = {
      exec = ''
        set -euo pipefail
        # cargo-deny 0.20+ auto-discovers deny.toml from the working directory;
        # the --config flag was removed from the CLI.
        cargo-deny check bans licenses sources
      '';
      description = "Run cargo-deny checks (bans, licenses, sources).";
      binary = "bash";
    };
    "security:audit" = {
      exec = ''
        set -euo pipefail
        # The advisory DB lives inside the cached target dir; a stale or
        # partial clone from a cache restore makes cargo-audit refuse to
        # (re)initialize it. Remove it so the clone always starts fresh.
        rm -rf "$DEVENV_ROOT/target/advisory-db-audit"
        cargo-audit audit \
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
    "lint:push" = {
      exec = ''
        set -euo pipefail

        run_step() {
          local name="$1"
          shift
          echo "Currently running: $name"
          "$@"
        }

        run_step "gitleaks detect" ${pkgs.gitleaks}/bin/gitleaks detect --verbose --redact
        run_step "lint:clippy" ${currentDir}/.devenv/profile/bin/lint:clippy
        run_step "lint:format" ${currentDir}/.devenv/profile/bin/lint:format
        run_step "verify:docs" ${currentDir}/.devenv/profile/bin/verify:docs
        run_step "security:dylint" ${currentDir}/.devenv/profile/bin/security:dylint
        run_step "lint:monochange" ${currentDir}/.devenv/profile/bin/lint:monochange
        run_step "test:all" ${currentDir}/.devenv/profile/bin/test:all
        run_step "test:idl" ${currentDir}/.devenv/profile/bin/test:idl
      '';
      description = "Run the full local CI suite before push, independent of the active shell environment.";
      binary = "bash";
    };
    "lint:all" = {
      exec = ''
        set -euo pipefail
        lint:clippy
        lint:format
        verify:docs
        security:dylint
        lint:monochange
      '';
      description = "Run all checks, including all custom dylint rules.";
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
        ${custom.mdt}/bin/mdt check --path ${currentDir}
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
        ${currentDir}/.devenv/profile/bin/docs:check
        [ -f "${currentDir}/docs/book.toml" ]
        [ -f "${currentDir}/docs/src/SUMMARY.md" ]
        ${pkgs.mdbook}/bin/mdbook build "${currentDir}/docs" -d "${currentDir}/target/mdbook"
        ${currentDir}/.devenv/profile/bin/docs:api
      '';
      description = "Verify docs folder structure, build mdBook, and check API docs.";
      binary = "bash";
    };
    "docs:api" = {
      exec = ''
        set -euo pipefail

        mapfile -t generated_client_manifests < <(find "${currentDir}/codama/clients/rust" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
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

        # The dylint lint crates pull in openssl-sys (via dylint_linting); expose
        # the nix openssl through pkg-config so the build works outside the
        # devenv shell too.
        export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
        export PATH="${pkgs.pkg-config}/bin:$PATH"

        mapfile -t generated_client_manifests < <(find "${currentDir}/codama/clients/rust" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
        exclude_args=()
        for manifest in "''${generated_client_manifests[@]}"; do
          package_name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1)"
          if [ -n "$package_name" ]; then
            exclude_args+=(--exclude "$package_name")
          fi
        done

        cargo clippy --workspace --all-features --locked ''${exclude_args[@]}

        mapfile -t lint_manifests < <(find "${currentDir}/lints" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
        for manifest in "''${lint_manifests[@]}"; do
          cargo clippy --manifest-path "$manifest" --all-features --all-targets --locked
        done
      '';
      description = "Check that all rust lints are passing.";
      binary = "bash";
    };
  };
}
