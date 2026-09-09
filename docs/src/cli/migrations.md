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

Replace the generated body. Pina preflights the exact historical shape for every account. Fixed transitions have generated size constants. A transition involving compact data also has `target_size` and `working_size` functions. They inspect already-validated historical bytes and must return a valid destination allocation without mutating the account. The `migrate` function is then total for that accepted source and must fully initialize every active destination byte.

Pina runs adjacent account transitions one at a time inside one invocation. It validates and commits each intermediate version before planning the next, which lets a later compact allocation depend on the prior compact result without allocating a copy of the account on the SBF stack. If any later step fails, Pina aborts the instruction so Solana rolls back all earlier resizes, lamport transfers, and byte writes. A manual instruction conversion instead runs in scratch space and may reject invalid semantic values before dispatch. Then run:

```bash
pina migrations check
pina test --compatibility
```

`check` rejects a remaining marker. After publication, it also rejects any change to the transition file or either schema hash. Fix a published transition with another migration version.

IDL generation runs the same check. The current IDL contains one omitted `migrationVersion` constant for each migration-aware account or instruction, so generated clients serialize the current envelope without asking the application developer for a version. Historical schemas and transition code remain exclusively in `migrations/manifest.json`.

## Change an instruction process

Pina snapshots the instruction payload and its positional account list under the same instruction version. An old request remains compatible only when:

- each existing account slot is unchanged;
- existing slots keep the same order;
- every new slot is appended at the end; and
- every appended slot is optional.

All account properties are part of a slot. A name, signer flag, writable flag, default, PDA, known address, or declarative constraint change is breaking. Create a new instruction discriminator for a breaking process.

The runtime treats an omitted optional suffix as absent. It never creates an account, signature, writable privilege, or PDA for an old client.

## Decode historical events

Events are immutable, so Pina projects rather than rewrites them. A migratable event generates `with_current_event_data(bytes, |current, source_version| ...)`. The current bytes use the latest event schema; `source_version` records which historical schema actually emitted the log. Unknown versions, future versions, trailing bytes, and invalid historical values fail before a transition runs. Keep golden log bytes in `pina_test::HistoricalEvent` rather than encoding fixtures with the current event type.

## Publish a version

Use `pina deploy` for a persistent cluster. After deployment succeeds, Pina rechecks the planned files and appends a hash-chained receipt with the program ID, RPC target, executable digest, manifest digest, and current contract versions.

Local deployments do not publish versions. A published version is immutable even if a later deployment replaces it.

If deployment succeeds but receipt recording fails, stop the release. The remote deployment is ambiguous until you reconcile it. The current ledger does not prove the deployed program-data hash, genesis hash, slot, or transaction signature.

## ABI document upgrades

`formatVersion` belongs to Pina's migration document. It is independent of each account or instruction version. Pina rejects newer document formats and migrates supported older formats through adjacent internal converters before it reads the typed model. The `pina_abi` crate can also encode a validated current model through adjacent downgrade converters. A downgrade fails instead of discarding information that the older format cannot represent. `pina migrations make` writes the current document format.

Format 3 freezes the PinaPod codec plus payload-relative fixed offsets and compact header, prefix, capacity, tail-order, and alignment metadata. Pina derives that descriptor from its closed field grammar and rejects a stored descriptor that disagrees. Historical format 2 documents are upgraded by deriving and rehashing this metadata; a format 3 document can downgrade to format 2 only through the matching inverse converter.

An ABI document upgrade does not consume an on-chain migration version.

## Commands

| Command                  | Result                                                        |
| ------------------------ | ------------------------------------------------------------- |
| `pina migrations make`   | Capture source changes and create or refresh one draft        |
| `pina migrations check`  | Fail on source, schema, process, transition, or version drift |
| `pina migrations status` | Show each current version and its publication state           |

Add `--json` for machine-readable output. Add `--project <DIR>` to select a program from another directory.
