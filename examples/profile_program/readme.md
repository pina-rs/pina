# `profile_program`

<br>

User profile registry demonstrating bounded PinaPod text and list fields stored inline in fixed zero-copy account state.

## What it covers

<br>

- `String<32>` / `String<128>` — length-prefixed UTF-8 with inline capacity and direct `as_str()` / `try_set()` accessors.
- `Vec<u64, 8>` — a bounded list with `iter()`, `try_push()`, `remove()`, and `clear()` operations.
- `bool` / `Option<u64>` — semantic source fields mapped to audited `PodBool` / `PodOption<PodU64>` storage by Pina.
- Full lifecycle: initialize → update → add/remove tags. PinaPod rejects invalid UTF-8 and oversized prefixes at the parse boundary, while custom `#[error]` codes cover tag capacity and index failures.
- Atomic `invoke_with` creation and one-pass `ProfileState::load_pda_mut` mutations, so nested fixed fields are not recursively validated several times per instruction.

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
