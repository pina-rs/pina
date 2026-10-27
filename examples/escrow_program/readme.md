# `escrow_program`

<br>

Token escrow program.

## What it covers

<br>

- Escrow lifecycle with `Make`, `Take`, and maker-authorized `Cancel` instructions.
- Vault PDA handling and seed validation.
- Token and Token-2022 program validation with ATA checks and the transfer flow.

`Make` and `Take` reject Token-2022 mint extensions. Makers can cancel offers created under an earlier extension policy: `Cancel` refunds token A without requiring the payment mint or applying the new admission policy. Cancellation also accepts the original 122-byte escrow layout from before schema-version envelopes, without changing the current account format. The canonical token program still enforces transfer and close rules. Extensions requiring extra transfer accounts or separate fee harvesting need a dedicated recovery flow; frozen accounts must be thawed by their freeze authority before cancellation.

## Run

<br>

```bash
cd examples/escrow_program
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p escrow_program -Z build-std -F bpf-entrypoint
```
