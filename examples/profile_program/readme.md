# `profile_program`

<br>

User profile registry built with Pina, demonstrating **Pod collections** — fixed-capacity `PodString` and `PodVec` fields stored inline in zero-copy account state.

## What it covers

<br>

- `PodString<N, PFX>` — fixed-capacity UTF-8 strings with a length prefix, validated with `try_as_str()` before storage.
- `PodVec<T, N, PFX>` — fixed-capacity element lists with a length prefix; push, pop, and in-place removal via `copy_within`.
- `PodBool` — single-byte boolean flags.
- Full lifecycle: initialize → update → add/remove tags, with custom `#[error]` codes for UTF-8, capacity, and index failures.

## Run

<br>

```bash
cargo test -p profile_program
pina idl --path examples/profile_program --output codama/idls/profile_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p profile_program -Z build-std -F bpf-entrypoint
```

Then run the end-to-end tests against the SBF binary:

```bash
SBF_OUT_DIR=target/bpfel-unknown-none/release \
    cargo test -p profile_program --test e2e -- --nocapture
```
