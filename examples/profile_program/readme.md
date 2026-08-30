# `profile_program`

<br>

User profile registry demonstrating fully initialized bounded text and list fields stored inline in zero-copy account state.

## What it covers

<br>

- `[u8; 33]` / `[u8; 129]` — length-prefixed UTF-8 with initialized capacity and checked `name_text()` / `bio_text()` accessors.
- `[u8; 66]` — an initialized count plus eight little-endian `u64` slots, with checked append, lookup, and removal helpers.
- `bool` / `Option<u64>` — semantic source fields mapped to audited `PodBool` / `PodOption<PodU64>` storage by Pina.
- Full lifecycle: initialize → update → add/remove tags, with custom `#[error]` codes for UTF-8, capacity, and index failures.

## Run

<br>

```bash
cd examples/profile_program
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build-sbf --manifest-path examples/profile_program/Cargo.toml \
    --sbf-out-dir target/deploy --features bpf-entrypoint
```

Then run the end-to-end tests against the SBF binary:

```bash
SBF_OUT_DIR=target/deploy \
    cargo test -p profile_program --test e2e -- --include-ignored --nocapture
```
