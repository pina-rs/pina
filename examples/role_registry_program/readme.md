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
cd examples/role_registry_program
pina test --unit
pina test
pina generate
```

## Optional SBF build

<br>

```bash
cargo build --release --target bpfel-unknown-none -p role_registry_program -Z build-std -F bpf-entrypoint
```
