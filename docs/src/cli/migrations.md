# Manage ABI migrations

Migration support is opt-in. Configure one version width for the program:

```toml
[migrations]
version-type = "u8"
```

Add `migrations` to each account, instruction payload, or event that Pina must track:

```rust
#[account(discriminator = AccountType::Profile, migrations)]
pub struct Profile {
	pub authority: Address,
	pub score: u64,
}
```

Pina inserts the version after the existing discriminator. Version `0` is the first captured shape. The source code never declares a version number.

## Capture a draft

Run the migration generator after the source ABI changes:

```bash
pina migrations make
pina migrations status
```

Pina writes `migrations/manifest.json`, `migrations/publications.json`, and adjacent transition files under `migrations/transitions/`.

If the current version has never been deployed to a non-local cluster, `make` replaces that draft. If a publication receipt contains the version, `make` appends the next version.

Commit the manifest, publication ledger, and transition files. Do not generate them during a build.

## Resolve a manual transition

Pina generates automatic transitions only for direction-safe fixed-layout changes. A type change, compact layout, or ambiguous field move creates a manual Rust file with `TODO(pina-manual-migration)`.

Replace the generated body. Pina preflights the exact historical shape for fixed accounts, so an account conversion must be total for every valid source and fully initialize the destination bytes. It cannot return an error after mutation starts. A manual instruction conversion runs in scratch space and may reject invalid semantic values before dispatch. Then run:

```bash
pina migrations check
pina test --compatibility
```

`check` rejects a remaining marker. After publication, it also rejects any change to the transition file or either schema hash. Fix a published transition with another migration version.

## Change an instruction process

Pina snapshots the instruction payload and its positional account list under the same instruction version. An old request remains compatible only when:

- each existing account slot is unchanged;
- existing slots keep the same order;
- every new slot is appended at the end; and
- every appended slot is optional.

All account properties are part of a slot. A name, signer flag, writable flag, default, PDA, known address, or declarative constraint change is breaking. Create a new instruction discriminator for a breaking process.

The runtime treats an omitted optional suffix as absent. It never creates an account, signature, writable privilege, or PDA for an old client.

## Publish a version

Use `pina deploy` for a persistent cluster. After deployment succeeds, Pina rechecks the planned files and appends a hash-chained receipt with the program ID, RPC target, executable digest, manifest digest, and current contract versions.

Local deployments do not publish versions. A published version is immutable even if a later deployment replaces it.

If deployment succeeds but receipt recording fails, stop the release. The remote deployment is ambiguous until you reconcile it. The current ledger does not prove the deployed program-data hash, genesis hash, slot, or transaction signature.

## ABI document upgrades

`formatVersion` belongs to Pina's migration document. It is independent of each account or instruction version. Pina rejects newer document formats and migrates supported older formats through adjacent internal converters before it reads the typed model. The `pina_abi` crate can also encode a validated current model through adjacent downgrade converters. A downgrade fails instead of discarding information that the older format cannot represent. `pina migrations make` writes the current document format.

An ABI document upgrade does not consume an on-chain migration version.

## Commands

| Command                  | Result                                                        |
| ------------------------ | ------------------------------------------------------------- |
| `pina migrations make`   | Capture source changes and create or refresh one draft        |
| `pina migrations check`  | Fail on source, schema, process, transition, or version drift |
| `pina migrations status` | Show each current version and its publication state           |

Add `--json` for machine-readable output. Add `--project <DIR>` to select a program from another directory.
