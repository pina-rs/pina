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

If the current version has never been deployed to a non-local cluster, `make` replaces that draft. If a publication receipt or pending deployment contains the version, `make` appends the next version.

Commit the manifest, publication ledger, and transition files. Do not generate them during a build.

## Disambiguate renames

A field that disappears while another field of the same type appears is ambiguous: a rename preserves the stored bytes, a remove-plus-add discards them and starts the new field zeroed. `pina migrations make` refuses to guess:

- On a terminal it prompts field by field and records the answer.
- With `--no-interactive`, or when no terminal is attached, it fails with one line per question naming the exact flags that answer it:

```bash
pina migrations make --rename score:points          # preserve the renamed data
pina migrations make --assume-removed score         # discard it; `points` starts zeroed
```

`--json` emits the open questions as a machine-readable array so agents can parse, decide, and re-run. With `--json`, `--no-interactive`, or no terminal attached, an unanswered question is a hard failure with a defined contract: the question array prints on stdout, the human-readable error prints on stderr, and the exit status is 1; capture both streams and re-invoke with the flags each question names. Answered renames are recorded in the manifest transition, so repeated `make` runs never re-ask, the generated transition copies the field's bytes, and `--assume-removed` prints a data-loss warning. Type changes and unpaired removals always fall back to a manual transition with a TODO body; nothing is dropped silently.

## Resolve a manual transition

Pina generates automatic transitions only for direction-safe fixed-layout changes. A type change, compact layout, or ambiguous field move creates a manual Rust file with `TODO(pina-manual-migration)`.

Replace the generated body. Pina preflights the exact historical shape for every account. Fixed transitions have generated size constants and their generated `migrate` stub starts with a length guard; keep it. A transition involving compact data also has `target_size` and `working_size` functions. They inspect already-validated historical bytes and must return a valid destination allocation without mutating the account. The `migrate` function is then total for that accepted source and must fully initialize every active destination byte.

A manual account transition cannot reject a value it cannot interpret: by the time `migrate` runs, rent funding and resizing may already have taken effect, so a `migrate` that cannot produce a valid destination aborts the whole instruction instead of returning a catchable error. Validate unambiguous value constraints inside `target_size` and `working_size` (they run before any mutation) and reserve genuinely rejectable conversions for instruction or event transitions, which run in scratch space before any account is touched.

Pina runs adjacent account transitions one at a time inside one invocation. It validates and commits each intermediate version before planning the next, which lets a later compact allocation depend on the prior compact result without allocating a copy of the account on the SBF stack. If any later step fails, Pina aborts the instruction so Solana rolls back all earlier resizes, lamport transfers, and byte writes. A manual instruction conversion instead runs in scratch space and may reject invalid semantic values before dispatch. Then run:

```bash
pina migrations check
pina test --compatibility
```

`check` rejects a remaining marker. Once publication is pending or complete, it also rejects any change to the transition file or either schema hash. Fix frozen transition code with another migration version.

IDL generation runs the same check. The current IDL keeps each migration-aware account or instruction's `migrationVersion` field in place with `defaultValueStrategy: "omitted"` and its default value: generated client inputs omit it, encoders stamp the current value into the envelope automatically, and decoders reject any other version. Historical schemas and transition code remain exclusively in `migrations/manifest.json`. Historical schemas and transition code remain exclusively in `migrations/manifest.json`.

## Change an instruction process

Trailing optional accounts may be omitted entirely from the end of an account list; every earlier slot must still be present, using the program address as a filler where a middle optional account is absent. Omitting a middle optional account without a filler shifts every later account into an earlier slot, which only surfaces as a confusing missing-account error or a privilege check failure. Treat a trailing `Option` account as fully untrusted: it can be absent from any request, not only from old ones.

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

Use `pina deploy` for a persistent cluster. Before the remote command starts, Pina atomically records the exact program ID, RPC target, executable digest, manifest digest, and current contract versions as pending. Receipts and pending records also pin the schema hash and transition-implementation hash of every published version, so rewriting published history — even with consistently recomputed hashes — fails every later check. A pending version is frozen because it may already be live. After deployment succeeds, Pina rechecks the planned files and converts that record into a hash-chained receipt.

Local deployments do not publish versions. A published version is immutable even if a later deployment replaces it.

If deployment or receipt recording fails, stop the release. The pending record remains, and `pina migrations status` reports `publication pending`. Restore the exact planned inputs and rerun the same deployment to reconcile it; Pina rejects a different deployment while the outcome is ambiguous. The current ledger does not prove the deployed program-data hash, genesis hash, slot, or transaction signature.

When the exact planned inputs cannot be reproduced (for example a cleaned build directory), inspect the pending deployment with `pina migrations reconcile`. It prints the cluster, RPC endpoint, program, and executable digest that must be resumed. Once you are certain the deployment never went live, `pina migrations reconcile --abandon` converts the pending record into an abandoned receipt that still freezes its pinned versions and unblocks the next deployment. Losing `migrations/publications.json` entirely fails every later check while the manifest still records advanced versions, because published history must stay pinned; restore the ledger from version control instead of regenerating it.

## ABI document upgrades

`formatVersion` belongs to Pina's migration document. It is independent of each account or instruction version. Pina rejects newer document formats and migrates supported older formats through adjacent internal converters before it reads the typed model. The `pina_abi` crate can also encode a validated current model through adjacent downgrade converters. A downgrade fails instead of discarding information that the older format cannot represent. `pina migrations make` writes the current document format.

Manifest format 3 freezes the PinaPod codec plus payload-relative fixed offsets and compact header, prefix, capacity, tail-order, and alignment metadata. Pina derives that descriptor from its closed field grammar and rejects a stored descriptor that disagrees. Historical manifest format 2 documents are upgraded by deriving and rehashing this metadata; a format 3 manifest can downgrade to format 2 only through the matching inverse converter.

Publication-ledger format 3 adds the recoverable pending deployment record. Format 2 ledgers upgrade with no pending deployment. A format 3 ledger can downgrade to format 2 only when no deployment is pending.

An ABI document upgrade does not consume an on-chain migration version.

## Commands

| Command                  | Result                                                        |
| ------------------------ | ------------------------------------------------------------- |
| `pina migrations make`   | Capture source changes and create or refresh one draft        |
| `pina migrations check`  | Fail on source, schema, process, transition, or version drift |
| `pina migrations status` | Show each current version and its publication state           |

Add `--json` for machine-readable output. Add `--project <DIR>` to select a program from another directory.
