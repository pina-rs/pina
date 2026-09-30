# Migrate to the reset ABI document

Pina `0.20` replaced the ABI document's two integer counters with one `abiVersion` pinned to the `pina_abi` release line, and removed every derived field from the document. The old documents are not readable by the new release — not rejected as unsupported, but unparseable — so a program with checked-in migration documents must convert them once.

**Your on-chain bytes are untouched.** Discriminators, version envelopes, PinaPod payloads, and generated clients are identical before and after. This migration rewrites JSON files on disk; no account data moves and no instruction changes. If your program never enabled `migrations`, there is nothing to do.

The design is recorded in [ADR 0009](../adrs/0009-abi-document-versioning.md), and the versioning rules it introduced are in [ABI document versioning](./abi-versioning.md).

The current release writes `abiVersion` `0.21`. The conversion below still produces `0.20` documents. Step 3 rewrites the ledger at the current version, and every reader converts the manifest in memory until the next `pina migrations create` writes it at the current version. `0.21` also changed how instructions and events are recorded, so read [Upgrading from ABI 0.20](./abi-versioning.md#upgrading-from-abi-020) once this guide is done.

## The symptom

After upgrading, the first Pina command that reads a migration document fails:

```text
error: Could not decode migration file migrations/manifest.json: migration manifest is
missing a string `abiVersion` field
```

That is the old document. The new reader fails closed on it because every removed field is rejected by `deny_unknown_fields`, and the version key itself was renamed. There is no converter from the integer formats: they described a document shape nothing external ever consumed, so the reset retires them instead of migrating them.

## Choose a path

| Situation                                                    | Path                                    |
| ------------------------------------------------------------ | --------------------------------------- |
| `migrations` never enabled                                   | nothing to do                           |
| Enabled, program never deployed, draft history is disposable | [Start fresh](#start-fresh)             |
| Enabled and deployed, or history worth keeping               | [Keep your history](#keep-your-history) |

A program that was never deployed can always start fresh: version numbers have no on-chain meaning until an account is written. A deployed program must keep its history — accounts on chain carry the version numbers your manifest recorded, and resetting that history makes every existing account read as a future version the program refuses to load.

Either way, first delete `version_type` (or `version-type`) and `auto` from the `[migrations]` table of `pina.toml`, keeping any `[migrations.answers]`. Current releases record the version width and the auto policy only in the manifest and refuse both keys, naming the `pina migrations create` flag and value that record the same setting.

## Start fresh

From the program directory:

```sh
rm -rf migrations
pina migrations create --no-interactive --auto true --version-type u8
pina migrations check
```

Pass the `--auto` and `--version-type` values your `pina.toml` used to set, or omit either flag for no auto policy and `u8` versions. `create` rebuilds the manifest from source at the current `abiVersion`, regenerates `tests/abi_layout.rs`, and recreates the publication ledger on your next deploy. Draft version history collapses to version zero — which is exactly why this path is for programs nothing has been deployed to yet.

## Keep your history

Two edits and one repair command, then the normal verify loop. Work from the program directory.

### 1. Convert the manifest

Save this as `convert-manifest.jq`:

```jq
del(.formatVersion)
| .abiVersion = "0.20"
| .contracts |= with_entries(
	.value.versions |= map(
		del(.version, .schemaSha256, .processSha256)
		| .schema |= del(.physical)
		| .transition |= del(
			.from,
			.to,
			.sourceSchemaSha256,
			.destinationSchemaSha256,
			.sourceProcessSha256,
			.destinationProcessSha256,
			.process
		)
		| if .process then .process.accounts |= map(del(.constraints)) else . end
	)
)
```

Every deleted key is a field the new model derives on load: the version number is the entry's position, the hashes are computed from the decoded content, the physical descriptor comes from the field grammar, and a transition's adjacency is implied by the version it sits on. Apply it:

```sh
jq -f convert-manifest.jq migrations/manifest.json > manifest.next &&
	mv manifest.next migrations/manifest.json
```

### 2. Convert the publication ledger

Save this as `convert-publications.jq`:

```jq
del(.formatVersion)
| .abiVersion = "0.20"
| .receipts |= map(
	del(.cluster)
	| .versions |= with_entries(.value |= {version: .version, history: []})
)
| if .pending then
	.pending.versions |= with_entries(.value |= {version: .version, history: []})
else
	.
end
```

```sh
jq -f convert-publications.jq migrations/publications.json > publications.next &&
	mv publications.next migrations/publications.json
```

Two things happen here, and both are deliberate. The `cluster` label is gone from receipts — the credential-free `rpcUrl` is the record now, and the pending record keeps its label because `pina deploy` matches on it. And each receipt's `history` is emptied: the old pins hashed a document shape that no longer exists, so keeping them would fail every future check, while emptying them keeps what matters — the recorded version, which the next step pins again.

### 3. Pin the receipts

A current ledger must pin every version each receipt made live, so every command refuses the emptied histories until they are pinned again, naming the repair:

```text
publication ledger 0.20 cannot be upgraded: receipt 0 names `account:1:01` without pinning its
published schemas. Confirm with version control that migrations/manifest.json still records exactly
what was deployed, then run `pina migrations reconcile --pin-legacy` to pin it
```

Once version control confirms that the manifest you converted in step 1 still describes exactly what those receipts made live, run:

```sh
pina migrations reconcile --pin-legacy
```

For each emptied entry it pins versions `0` through the recorded version from the manifest: the schema hash of each and, when it has one, the hash of the transition that enters it. That is trust on first use, which is why the confirmation comes first. The command writes the ledger in the current shape, so `sequence`, `programId`, `manifestSha256`, the `previousReceiptSha256` chain link, and each contract's `version` are gone and each contract becomes its list of pins. Nothing checks the old chain, so the edit in step 2 needs no chain repair.

### 4. Regenerate and verify

```sh
pina migrations create --no-interactive
pina migrations check
pina migrations status
```

`create` rewrites the manifest canonically and regenerates `tests/abi_layout.rs`. `check` must pass before anything builds. `status` is your proof the conversion preserved state: every contract shows the same version number it showed before the upgrade, and deployed contracts still read `published`. Confirm one live account still loads with `pina migrations inspect <ADDRESS>`, then commit the rewritten documents.

## What `pina_abi` library consumers must change

If you import `pina_abi` directly — a custom indexer, a verification tool — the API moved from stored fields to derived values. The mapping:

| Removed                                                                                      | Replacement                                                            |
| -------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `MANIFEST_FORMAT_VERSION`, `PUBLICATION_FORMAT_VERSION`                                      | `ABI_VERSION`, `ABI_VERSION_KEY`, `ABI_OLDEST_SUPPORTED`               |
| `manifest.format_version: u32`                                                               | `manifest.abi_version: String`                                         |
| `SchemaVersion::version`                                                                     | the entry's index; `ContractHistory::current_version()`, `.version(n)` |
| `SchemaVersion::schema_sha256`                                                               | method `schema_sha256()`                                               |
| `SchemaVersion::process_sha256`                                                              | method `process_sha256()`                                              |
| `DataSchema::physical`                                                                       | method `physical() -> Result<PhysicalLayout, String>`                  |
| `Transition::from` / `::to`                                                                  | implied: the transition on index `i` converts `i - 1` into `i`         |
| `Transition` neighbour hashes and `process` proof                                            | re-derive: `ContractHistory::process_transition_into(n)`               |
| `ProcessAccount::constraints`                                                                | removed — validation rules live in the IDL                             |
| `PublicationReceipt::cluster`                                                                | removed — receipts keep `rpc_url`; the pending record keeps `cluster`  |
| `encode_manifest_for_format`, `convert_manifest_format`, `convert_publication_ledger_format` | deleted — use `encode_manifest`, `encode_publication_ledger`           |

Added: `walk_document` and `AbiStep` for converter tables, `parse_document_version` and `current_abi_version` for comparisons, and `document_schema` / `render_document_schema` / `schema_url` behind the new `pina abi schema` command.

`0.21` changed the API again. `DataCodec` and `DataSchema::codec` are removed: the wire codec is `SCHEMA_CODEC`, implied by `abiVersion`. `ContractHistory::identity` is no longer serialized and is filled from the contract key when a manifest is read (`ContractIdentity::from_key` parses one). `ContractHistory` gains `envelope` and `is_migrated()`. `AbiStep` carries one converter per document (`manifest` and `publications`), and `walk_document` takes an `AbiDocument` instead of a label.

The `0.21` ledger keeps only what nothing else records. `PublicationReceipt` and `PendingPublication` lose `sequence`, `program_id`, `manifest_sha256`, and `previous_receipt_sha256`, and `PublicationReceipt::sha256` is removed. `PublishedContract` serializes as its list of pins: its `version` field and `PublishedContract::legacy` are gone, `version()` returns the position of the last pin, and `pins(n)` reports whether version `n` is pinned. `pin_legacy_publications` pins a `0.20` ledger's unpinned entries from a manifest. `wire_type` and `DataSchema::same_wire` compare types by what they store, and `MigrationAuto` and `MigrationVersionType` implement `FromStr` for the `--auto` and `--version-type` spellings.

## Troubleshooting

| Error                                                                                                       | Cause and remedy                                                                                          |
| ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `missing a string \`abiVersion\` field`                                                                     | a pre-0.20 document — this guide                                                                          |
| `records ABI version 9.9, but this Pina build supports 0.21; upgrade Pina`                                  | the document is newer than your build — upgrade Pina; the check is a capability marker, like `Cargo.lock` |
| `predates the oldest supported version 0.20; regenerate it with \`pina migrations create\``                 | a document older than the reset — [Start fresh](#start-fresh)                                             |
| `names ... without pinning its published schemas`                                                           | a converted ledger whose histories were emptied — [Pin the receipts](#3-pin-the-receipts)                 |
| `` `[migrations].auto` no longer belongs in pina.toml ``                                                    | a retired `pina.toml` key — delete it; the manifest records the setting                                   |
| `pins ... for ... , which the manifest does not record` or `pinned schema ... but the manifest now records` | a receipt history that was not emptied — [Keep your history](#keep-your-history)                          |

After the conversion, `abiVersion` is maintained for you: `pina migrations create` writes the current version, and the value advances only when a future `pina_abi` release changes the document contract — never for a CLI fix, never for a program schema change.
