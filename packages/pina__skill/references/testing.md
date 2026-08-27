# Testing and Verification

## Choose the smallest useful layer

- Pure parsing, arithmetic, and state-transition logic: ordinary Rust unit tests.
- Account metadata, instruction execution, and expected `ProgramError` values: Mollusk or the project's established VM harness.
- Entrypoint, deployment, linker, or serialized-program behavior: an SBF build plus an integration test.
- Public macro or IDL changes: fixture tests and generated-artifact drift checks.
- Layout or borrowing invariants: focused boundary tests; use Miri only when the repository already supports it or the task specifically warrants it.

Test the rejected path as well as the successful path for authorization and validation changes. Verify that failed instructions leave relevant account data and lamports unchanged.

## Baseline Cargo checks

Use project task aliases when they exist. For a standalone project without them:

```sh
cargo test
cargo build --all-features
cargo clippy --all-features --all-targets
```

Format with the project's configured formatter. In the Pina repository, use `fix:format` or `dprint fmt`; do not invoke `rustfmt` directly.

## SBF builds

Prefer the project-aware build, which uses the discovered program, pinned toolchain, and Cargo target directory:

```sh
pina build
```

Use the equivalent low-level Cargo command only when diagnosing the compiler or linker invocation:

```sh
cargo build --release --target bpfel-unknown-none \
  -p counter_program -Z build-std=core,alloc -F bpf-entrypoint
```

Do not silently skip an SBF-dependent test because the artifact is missing. Build it or report the missing prerequisite.

## Generated contracts

After a public program-surface change:

1. Generate the IDL.
2. Validate the JSON.
3. Regenerate each committed client target.
4. Type-check or compile generated clients.
5. Fail on unreviewed drift.

Counts are useful guardrails: declared instructions, accounts, and errors should match their generated IDL counterparts.

## Release checks

Follow the repository's release policy. When it uses changesets, add release intent for publishable code or package changes and validate the release metadata before handoff. Do not publish, tag, or open a release request unless the user explicitly authorizes that external action.
