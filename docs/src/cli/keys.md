# `pina keys`

Inspect and manage the local program identity without exposing secret key bytes.

```text
pina keys [OPTIONS] [COMMAND]
```

`pina keys` and `pina keys show` are read-only aliases. They discover the nearest Cargo program package, read its single `declare_id!`, and compare it with the conventional keypair at `<cargo-target>/deploy/<lib-target>-keypair.json`. Use `--keypair` to inspect a different local file and `--json` for automation.

## Synchronize an existing keypair

```bash
pina keys sync
pina keys sync --keypair ./keys/counter-keypair.json
```

`sync` validates all 64 keypair bytes, derives the Ed25519 public key from the secret half, and refuses inconsistent files. It then parses Rust source and replaces only the string literal of exactly one `declare_id!`. Zero or multiple declarations fail before a write. Source identity and contents are checked again immediately before publication, so an editor save during synchronization fails instead of losing unrelated changes. The command never prints secret bytes.

## Create or rotate identity

```bash
pina keys new
pina keys new --keypair ./keys/counter-keypair.json
pina keys new --force
```

`new` generates a cryptographically random Solana keypair, writes it with owner-only permissions on Unix, and synchronizes source. It refuses an existing keypair. `--force` is intentionally required to replace one and rotate the program identity. Windows generation currently fails before creating secret material because Rust's safe standard API cannot construct a private ACL atomically; use a reviewed external keypair and `pina keys sync` instead.

Program deployments are identified by their address. Treat `keys new --force` as a destructive identity rotation: review downstream clients, deployment records, and funded accounts first.

## Output and failures

Human output identifies source, keypair path, public program IDs, and match status. `--json` emits only JSON on stdout. Failures are written to stderr and exit unsuccessfully.

Keypair inputs for `show` and `sync` must be regular, non-link files no larger than 4 KiB. Read-only selection may pass through an aliased ancestor. Generated keypair and source write destinations reject non-regular files, symbolic links, and reparse points anywhere in the destination path. The command also fails closed for malformed Rust, invalid addresses, ambiguous declarations, malformed JSON, incorrect keypair length, inconsistent secret/public halves, unavailable secure randomness, private-permission guarantees, and unauthorized replacement. Keypair and source publication use atomic replacement; failed source publication rolls the keypair back only while the generated file still has the identity created by the command. A concurrent keypair replacement is preserved and reported as a rollback failure.
