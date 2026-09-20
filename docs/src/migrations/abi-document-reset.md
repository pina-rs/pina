# Migrate to the reset ABI document

Pina `0.20` replaced the ABI document's two integer counters with one `abiVersion` pinned to the `pina_abi` release line, and removed every derived field from the document. The old documents are not readable by the new release — not rejected as unsupported, but unparseable — so a program with checked-in migration documents must convert them once.

**Your on-chain bytes are untouched.** Discriminators, version envelopes, PinaPod payloads, and generated clients are identical before and after. This migration rewrites JSON files on disk; no account data moves and no instruction changes. If your program never enabled `migrations`, there is nothing to do.

The design is recorded in [ADR 0009](../adrs/0009-abi-document-versioning.md), and the versioning rules it introduced are in [ABI document versioning](./abi-versioning.md).

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

## Start fresh

From the program directory:

```sh
rm -rf migrations
pina migrations create --no-interactive
pina migrations check
```

`create` rebuilds the manifest from source at `abiVersion` `"0.20"`, regenerates `tests/abi_layout.rs`, and recreates the publication ledger on your next deploy. Draft version history collapses to version zero — which is exactly why this path is for programs nothing has been deployed to yet.

## Keep your history

Three edits, then the normal verify loop. Work from the program directory.

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

Two things happen here, and both are deliberate. The `cluster` label is gone from receipts — the credential-free `rpcUrl` is the record now, and the pending record keeps its label because `pina deploy` matches on it. And each receipt's `history` is emptied, which marks it _unpinned_: the old pins hashed a document shape that no longer exists, so keeping them would fail every future check, while emptying them keeps what matters — the recorded version, which is what freezes published history and keeps `create` append-only.

`create` rewrites the manifest canonically but never rewrites the ledger, so the converted ledger stays as you wrote it.

### 3. Repair the receipt chain — only if you hold more than one receipt

Dropping `cluster` from a receipt changes that receipt's own hash, so every receipt after the first now names a `previousReceiptSha256` that no longer matches. With a single receipt the previous hash is `null` and there is nothing to repair. With several, either keep only the most recent receipt and set its `previousReceiptSha256` to `null` — older versions stay frozen because the surviving receipt records the highest version — or re-link the chain:

```rust
use pina_abi::PublicationLedger;
use pina_abi::encode_publication_ledger;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let path = std::env::args().nth(1).expect("pass the ledger path");
	// Parse without validation: the chain is stale until this repairs it.
	let mut ledger: PublicationLedger = serde_json::from_slice(&std::fs::read(&path)?)?;
	let mut previous = None;
	for receipt in &mut ledger.receipts {
		receipt.previous_receipt_sha256 = previous.clone();
		previous = Some(receipt.sha256());
	}
	std::fs::write(path, encode_publication_ledger(&ledger)?)?;
	Ok(())
}
```

Run it once on `migrations/publications.json` with `serde_json` added as a dependency. `encode_publication_ledger` validates before writing, so a chain it accepts is a chain every later check accepts.

### 4. Regenerate and verify

```sh
pina migrations create --no-interactive
pina migrations check
pina migrations status
```

`create` rewrites the manifest canonically and regenerates `tests/abi_layout.rs`. `check` must pass before anything builds. `status` is your proof the conversion preserved state: every contract shows the same version number it showed before the upgrade, and deployed contracts still read `published`. Confirm one live account still loads with `pina migrations inspect <ADDRESS>`, then commit the rewritten documents.

### Re-pin if published-schema tamper evidence matters to you

An unpinned receipt freezes versions but no longer detects a hand edit to a published version's schema. Re-pinning means writing the new `schemaSha256` into the receipt — and the new hashes are the `SCHEMA_SHA256` constants in the regenerated `tests/abi_layout.rs`, one per contract for its current version. If you need historical versions pinned too, that is a small script against `pina_abi`, not a hand edit. Unpinned is the documented state for history that cannot be reconstructed, and for most programs it is the honest one.

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

## Troubleshooting

| Error                                                                                                       | Cause and remedy                                                                                                |
| ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `missing a string \`abiVersion\` field`                                                                     | a pre-0.20 document — this guide                                                                                |
| `records ABI version 9.9, but this Pina build supports 0.20; upgrade Pina`                                  | the document is newer than your build — upgrade Pina; the check is a capability marker, like `Cargo.lock`       |
| `predates the oldest supported version 0.20; regenerate it with \`pina migrations create\``                 | a document older than the reset — [Start fresh](#start-fresh)                                                   |
| `publication receipt 1 does not extend the previous hash`                                                   | a converted multi-receipt ledger whose chain was not repaired — [Re-link the receipt chain](#keep-your-history) |
| `pins ... for ... , which the manifest does not record` or `pinned schema ... but the manifest now records` | a receipt history that was not emptied — [Keep your history](#keep-your-history)                                |

After the conversion, `abiVersion` is maintained for you: `pina migrations create` writes the current version, and the value advances only when a future `pina_abi` release changes the document contract — never for a CLI fix, never for a program schema change.
