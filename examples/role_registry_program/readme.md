# `role_registry_program`

<br>

Role-based registry and configuration scaffold.

## What it covers

<br>

- Registry configuration PDA initialization.
- Per-role PDA entries keyed by registry + role id.
- `Initialize`, `AddRole`, `UpdateRole`, `DeactivateRole`, and `RotateAdmin` flows.
- Explicit validation chains for signer, writable, and PDA checks.

The `tests/e2e.rs` file exercises the full role lifecycle through Mollusk.

## Run

<br>

```bash
cargo test -p role_registry_program
pina idl --path examples/role_registry_program --output codama/idls/role_registry_program.json
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p role_registry_program -Z build-std -F bpf-entrypoint
```
