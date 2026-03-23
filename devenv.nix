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
  pnpmWorkspaceLines = lib.splitString "\n" (builtins.readFile ./pnpm-workspace.yaml);
  pnpmNodeVersionLine = lib.findFirst (
    line: builtins.match "^[[:space:]]*useNodeVersion:[[:space:]].*$" line != null
  ) null pnpmWorkspaceLines;
  pnpmNodeVersionMatch =
    if pnpmNodeVersionLine == null then
      null
    else
      builtins.match "^[[:space:]]*useNodeVersion:[[:space:]]*['\"]?([^'\"#[:space:]]+)['\"]?[[:space:]]*(#.*)?$" pnpmNodeVersionLine;
  pnpmNodeVersion =
    if pnpmNodeVersionMatch == null then null else builtins.elemAt pnpmNodeVersionMatch 0;
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
      custom.mdt
      dprint
      eget
      gcc
      git
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
  ''
  + lib.optionalString (pnpmNodeVersion != null) ''
        # Respect pnpm-workspace.yaml's `useNodeVersion` without mutating the
        # user's globally active Node.js version. `pnpm env use --global` updates
        # pnpm's shared global shim, so we instead install the requested version
        # with `pnpm env add --global` and prepend its bin directory to PATH for
        # this devenv shell only.
        export DEVENV_PNPM_NODE_VERSION="${pnpmNodeVersion}"

        if [ -z "''${PNPM_HOME:-}" ]; then
          if [ -n "''${XDG_DATA_HOME:-}" ]; then
            export PNPM_HOME="$XDG_DATA_HOME/pnpm"
          elif [ -n "''${HOME:-}" ]; then
            if [ "$(uname -s)" = "Darwin" ]; then
              export PNPM_HOME="$HOME/Library/pnpm"
            else
              export PNPM_HOME="$HOME/.local/share/pnpm"
            fi
          else
            export PNPM_HOME="$DEVENV_ROOT/.devenv/pnpm"
          fi
        fi

        mkdir -p "$PNPM_HOME"

        # `pnpm env add --global` requires PNPM_HOME to be present in PATH, but we
        # append it so devenv's standalone pnpm remains the active `pnpm` binary.
        case ":$PATH:" in
          *":$PNPM_HOME:"*) ;;
          *) export PATH="$PATH:$PNPM_HOME" ;;
        esac

        export DEVENV_PNPM_NODE_BIN="$PNPM_HOME/nodejs/$DEVENV_PNPM_NODE_VERSION/bin"

        # Install the requested version without switching pnpm's global active Node.
        if [ ! -x "$DEVENV_PNPM_NODE_BIN/node" ]; then
          echo "Installing pnpm-managed Node.js $DEVENV_PNPM_NODE_VERSION..."
          pnpm env add --global "$DEVENV_PNPM_NODE_VERSION"
        fi

        if [ ! -x "$DEVENV_PNPM_NODE_BIN/node" ]; then
          echo "Failed to find pnpm-managed Node.js $DEVENV_PNPM_NODE_VERSION at $DEVENV_PNPM_NODE_BIN" >&2
          exit 1
        fi

        # Only expose the Node.js toolchain from the pnpm-managed install. This
        # keeps `node`, `npm`, `npx`, and `corepack` aligned to useNodeVersion
        # while leaving `pnpm` resolved from the devenv package set.
        export DEVENV_PNPM_NODE_SHIM_DIR="$DEVENV_ROOT/.devenv/pnpm-node-shims/$DEVENV_PNPM_NODE_VERSION"
        export DEVENV_PNPM_NPM_CLI="$PNPM_HOME/nodejs/$DEVENV_PNPM_NODE_VERSION/lib/node_modules/npm/bin/npm-cli.js"
        export DEVENV_PNPM_NPX_CLI="$PNPM_HOME/nodejs/$DEVENV_PNPM_NODE_VERSION/lib/node_modules/npm/bin/npx-cli.js"
        mkdir -p "$DEVENV_PNPM_NODE_SHIM_DIR"

        for executable in node corepack; do
          if [ -x "$DEVENV_PNPM_NODE_BIN/$executable" ]; then
            ln -sfn "$DEVENV_PNPM_NODE_BIN/$executable" "$DEVENV_PNPM_NODE_SHIM_DIR/$executable"
          else
            rm -f "$DEVENV_PNPM_NODE_SHIM_DIR/$executable"
          fi
        done

        if [ -f "$DEVENV_PNPM_NPM_CLI" ]; then
          cat > "$DEVENV_PNPM_NODE_SHIM_DIR/npm" <<'EOF'
    #!/usr/bin/env sh
    exec "$DEVENV_PNPM_NODE_BIN/node" "$DEVENV_PNPM_NPM_CLI" "$@"
    EOF
          chmod +x "$DEVENV_PNPM_NODE_SHIM_DIR/npm"
        else
          rm -f "$DEVENV_PNPM_NODE_SHIM_DIR/npm"
        fi

        if [ -f "$DEVENV_PNPM_NPX_CLI" ]; then
          cat > "$DEVENV_PNPM_NODE_SHIM_DIR/npx" <<'EOF'
    #!/usr/bin/env sh
    exec "$DEVENV_PNPM_NODE_BIN/node" "$DEVENV_PNPM_NPX_CLI" "$@"
    EOF
          chmod +x "$DEVENV_PNPM_NODE_SHIM_DIR/npx"
        else
          rm -f "$DEVENV_PNPM_NODE_SHIM_DIR/npx"
        fi

        if [ ! -x "$DEVENV_PNPM_NODE_SHIM_DIR/node" ]; then
          echo "Failed to prepare Node.js shims in $DEVENV_PNPM_NODE_SHIM_DIR" >&2
          exit 1
        fi

        case ":$PATH:" in
          *":$DEVENV_PNPM_NODE_SHIM_DIR:"*) ;;
          *) export PATH="$DEVENV_PNPM_NODE_SHIM_DIR:$PATH" ;;
        esac

        hash -r
  '';

  # disable dotenv since it breaks the variable interpolation supported by `direnv`
  dotenv.disableHint = true;

  scripts = {
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
    "codama:idl:all" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/scripts/generate-codama-idls.sh"
      '';
      description = "Generate Codama IDLs for all example programs.";
      binary = "bash";
    };
    "codama:clients:generate" = {
      exec = ''
        set -e
        pnpm --dir "$DEVENV_ROOT" install --frozen-lockfile
        pina codama generate \
          --examples-dir "$DEVENV_ROOT/examples" \
          --idls-dir "$DEVENV_ROOT/codama/idls" \
          --rust-out "$DEVENV_ROOT/codama/clients/rust" \
          --js-out "$DEVENV_ROOT/codama/clients/js" \
          --npx node
      '';
      description = "Generate Codama IDLs and Rust/JS clients for all examples.";
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
        set -e
        if command -v nix >/dev/null 2>&1; then
          HASH=$(nix hash path --base32 ./.eget/.eget.toml)
        else
          HASH=$(shasum -a 256 ./.eget/.eget.toml | awk '{print $1}')
        fi
        echo "HASH: $HASH"
        if [ ! -f ./.eget/bin/hash ] || [ "$HASH" != "$(cat ./.eget/bin/hash)" ]; then
          echo "Updating eget binaries"
          rm -rf "$DEVENV_ROOT/.eget/bin"
          mkdir -p "$DEVENV_ROOT/.eget/bin"
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
    "install:sbpf-gallery" = {
      exec = ''
        set -e

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

        # Symlink into .eget/bin so it takes precedence on PATH for cargo
        # linker invocation (linker=sbpf-linker in .cargo/config.toml).
        mkdir -p "$DEVENV_ROOT/.eget/bin"
        ln -sf "$SBPF_BIN" "$DEVENV_ROOT/.eget/bin/sbpf-linker"

        echo ""
        echo "Done! sbpf-linker (gallery) installed and linked to .eget/bin/"
        echo "Cache directory: $CACHE_DIR"
        "$SBPF_BIN" --version 2>/dev/null || echo "(binary ready)"
      '';
      description = "Build sbpf-linker with custom Blueshift LLVM (upstream-gallery-21). First run builds LLVM from source (~30 min).";
      binary = "bash";
    };
    "clean:sbpf-gallery" = {
      exec = ''
        set -e

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

        # Remove the symlink from .eget/bin if present
        if [ -L "$DEVENV_ROOT/.eget/bin/sbpf-linker" ]; then
          rm -f "$DEVENV_ROOT/.eget/bin/sbpf-linker"
          echo "Removed .eget/bin/sbpf-linker symlink."
        fi
      '';
      description = "Remove the cached Blueshift LLVM build and gallery sbpf-linker binary.";
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
        set -e
        cargo build --locked
      '';
      description = "Build workspace crates with the default feature set.";
      binary = "bash";
    };
    "build:pina:no-default" = {
      exec = ''
        set -e
        cargo check -p pina --no-default-features --locked
        cargo check -p pina --no-default-features --features derive --locked
        cargo check -p pina --no-default-features --features token --locked
        cargo check -p pina --no-default-features --features token,derive --locked
      '';
      description = "Verify `pina` builds without default features and across feature subsets.";
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

        # Ensure sbpf-linker is built against the Blueshift LLVM upstream gallery.
        install:sbpf-gallery

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
      '';
      description = "Run Anchor parity tests plus pina_bpf build artifact checks using the gallery sbpf-linker.";
      binary = "bash";
    };
    "idl:generate" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/scripts/generate-codama-idls.sh"
      '';
      description = "Generate Codama IDLs for all examples.";
      binary = "bash";
    };
    "verify:idls" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/scripts/verify-codama-idls.sh"
      '';
      description = "Verify Codama generation, fixture drift, validation, and deterministic output.";
      binary = "bash";
    };
    "test:idl" = {
      exec = ''
        set -e
        verify:idls
      '';
      description = "Run full Codama integration and deterministic generation checks.";
      binary = "bash";
    };
    "test:surfpool-idl" = {
      exec = ''
        set -e
        pnpm --dir "$DEVENV_ROOT/codama" install --frozen-lockfile
        "$DEVENV_ROOT/scripts/test-surfpool-idl-smoke.sh"
      '';
      description = "Deploy a generated program to Surfpool and invoke it using generated IDL metadata.";
      binary = "bash";
    };
    "coverage:all" = {
      exec = ''
        set -e
        mkdir -p "$DEVENV_ROOT/target/coverage"
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
        set -e
        if ! command -v mucho >/dev/null 2>&1; then
          echo "Skipping VM coverage: mucho is not installed."
          exit 0
        fi

        set +e
        mucho coverage
        status=$?
        set -e

        if [ "$status" -ne 0 ]; then
          echo "Experimental VM coverage failed with status $status (non-blocking)."
        fi
      '';
      description = "Run experimental Solana VM coverage via mucho when available (non-blocking).";
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
        mapfile -t generated_client_manifests < <(find "$DEVENV_ROOT/codama/clients/rust" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
        exclude_args=()
        for manifest in "''${generated_client_manifests[@]}"; do
          package_name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1)"
          if [ -n "$package_name" ]; then
            exclude_args+=(--exclude "$package_name")
          fi
        done

        cargo clippy --fix --allow-dirty --allow-staged --workspace --all-features --locked ''${exclude_args[@]}
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
        docs:check
        [ -f "$DEVENV_ROOT/docs/book.toml" ]
        [ -f "$DEVENV_ROOT/docs/src/SUMMARY.md" ]
        mdbook build "$DEVENV_ROOT/docs" -d "$DEVENV_ROOT/target/mdbook"
      '';
      description = "Verify docs folder structure and build docs.";
      binary = "bash";
    };
    "lint:clippy" = {
      exec = ''
        set -e
        mapfile -t generated_client_manifests < <(find "$DEVENV_ROOT/codama/clients/rust" -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
        exclude_args=()
        for manifest in "''${generated_client_manifests[@]}"; do
          package_name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1)"
          if [ -n "$package_name" ]; then
            exclude_args+=(--exclude "$package_name")
          fi
        done

        cargo clippy --workspace --all-features --locked ''${exclude_args[@]}
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
