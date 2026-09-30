# Manage ABI migrations

Migration support is opt-in. Configure one version width for the program:

```toml
[migrations]
version_type = "u8"
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

Each kind uses the version differently. An account migrates on-chain, one adjacent transition at a time. An instruction with `migrations` has an older payload converted to the current layout before its handler runs. An event is versioned but never converted: the program emits only the current version, and generated clients decode each historical version with its own schema. [How a migration flows](../migrations/flow.md#what-each-contract-kind-carries) compares the three.

## Opt whole kinds in

A program that wants every contract versioned can opt in by kind instead of annotating each declaration:

```toml
[migrations]
version_type = "u8"
auto = ["accounts", "events", "instructions"] # or `auto = true` for every kind
```

`auto` accepts `true` (every kind), `false` (the default), or a list of `accounts`, `events`, and `instructions`; any other name is a configuration error. The policy envelopes accounts and events. It records instructions as snapshots without an envelope, so covering them does not change a payload byte; see [Instructions without an envelope](#instructions-without-an-envelope).

`pina migrations create` records the resolved policy as `auto` in `migrations/manifest.json` and snapshots every contract of the listed kinds. The manifest is the single source of truth for macros: they never read `pina.toml`, because a proc macro does not re-expand when an unrelated toml file changes. A struct that is not yet snapshotted still fails the build with the existing "run `pina migrations create`" error, so the workflow is unchanged.

Because the policy lives in the manifest, flipping it re-expands every contract without a source edit. When a policy is recorded, `create` scaffolds a `build.rs` containing:

```rust
fn main() {
	println!("cargo:rerun-if-changed=migrations/manifest.json");
}
```

The scaffold is idempotent and never overwrites an existing hand-written build script; `create` prints the exact line to add instead, and `pina migrations check` fails until it is present.

Per-item `migrations = false` keeps one contract out of an auto policy. Removing the envelope from a contract the manifest already records is an error rather than a silent opt-out: stripping an envelope is a wire-format change, so the build fails with the contract identity and the required remedy, and `create` refuses it with `EnvelopeRemoval`. The one exception is an unpublished instruction, which `create` may turn back into a snapshot. Dropping a kind from `[migrations].auto` is rejected the same way. Enabling auto on an already-launched program inserts an envelope into every account and event of the listed kinds — one recorded history entry per contract through `create`, behind `--envelope-ack` — while a new program simply captures that baseline.

### Instructions without an envelope

An instruction covered by `auto` without the `migrations` token keeps its `[discriminator][payload]` wire format. The manifest records exactly one version of it with `"envelope": false`:

```json
{
	"contracts": {
		"instruction:1:00": {
			"rustName": "InitializeInstruction",
			"envelope": false,
			"versions": [
				{
					"schema": {
						"layout": "fixed",
						"fields": [{ "name": "bump", "rustType": "u8" }]
					},
					"process": {
						"accounts": [
							{
								"name": "authority",
								"writable": true,
								"signer": true,
								"optional": false,
								"defaultValue": null,
								"pda": null
							}
						]
					}
				}
			]
		}
	}
}
```

The snapshot stops wire-breaking changes rather than recording them:

- The `#[instruction]` macro fails the build when the struct drifts from the snapshot.
- While nothing is published, `create` replaces the snapshot.
- After publication, a payload change fails with `PublishedPayloadChanged`: declare a new discriminator, or restore the published fields.
- Appending optional accounts to a published snapshot extends it in place and consumes no version.
- A published snapshot cannot gain an envelope (`EnvelopeAddition`); declare a new discriminator for the migration-aware instruction.
- When an instruction stops asking to be recorded (`migrations = false`, or an `auto` policy that no longer covers instructions), `check` fails with `StaleSnapshot` and `create` releases the snapshot. Nothing on the wire changes.

`#[instruction(discriminator = X, migrations)]` opts one instruction into full migrations: a version envelope, adjacent transitions under `migrations/transitions/instruction_<width>_<hex>/`, and a generated dispatcher that normalizes a historical payload before the handler runs. Before publication, removing the token and running `create` turns it back into a snapshot; after publication it fails with `EnvelopeRemoval`. The macro reports the same situation at build time: "removing an envelope is a wire-format change that `pina migrations create` must record deliberately". A program whose instructions were recorded under ABI `0.20` has them enveloped, and must add `migrations` to keep a published instruction's wire format; see [Upgrading from ABI 0.20](../migrations/abi-versioning.md#upgrading-from-abi-020).

## Capture a draft

Run the migration generator after the source ABI changes:

```bash
pina migrations create
pina migrations status
```

Pina writes `migrations/manifest.json`, `migrations/publications.json`, and adjacent transition files under `migrations/transitions/`.

If the current version has never been deployed to a non-local cluster, `create` replaces that draft. If a publication receipt or pending deployment contains the version, `create` appends the next version. A changed event appends a version with no transition file. A published instruction whose payload is unchanged but whose account list gained appended optional slots is extended in place instead (`Appended optional accounts to instruction:1:00@0`), and a published snapshot-only instruction cannot change its payload at all.

When a transition grows an account, `create` prints the estimated rent deficit (about 6,960 lamports per grown byte), names the program constant to raise (`max_lamports`, for example `MAX_INLINE_MIGRATION_LAMPORTS`), and points at the on-chain error an undersized budget produces: `MigrationLamportBudgetExceeded`. Cumulative worst-case growth across the supported stale ladder — every version a stale account may still hold within `MAX_INLINE_STEPS`, not only the adjacent hop — beyond the runtime's 10,240-byte (`MAX_PERMITTED_DATA_INCREASE`) per-instruction realloc cap warns separately, because no budget raises that limit; it points at `MigrationAccountGrowthExceeded`. Both warnings quote the same numbers as the `PinaProgramError` rustdoc, so the pre-deploy estimate and a failed transaction name the same fix.

Commit the manifest, publication ledger, and transition files. Do not generate them during a build.

## Preview deployment costs

`pina migrations status` ends with a cost preview derived from the checked-in history and, when the compiled SBF artifact exists, from `pina profile`'s static per-function estimates. It reports:

- per account contract: current size, the bytes a version-0 (day-one) account grows, and the approximate rent deficit at the same 6,960-lamports-per-grown-byte convention the `create` warning uses;
- per instruction process: the worst-case adjacent-step ladder a stale account the process names can trigger, with the step count, the rent that ladder funds, and a static CU estimate;
- one program-wide summary that sizes both budgets deliberately: the touching transaction funding the most rent (`max_lamports`) and, independently, the longest worst-case ladder (`MAX_INLINE_STEPS`) — each names its instruction, because they need not be the same one.

The preview prints the ladder model and the CU model so the numbers stay interpretable. When the history has more than eight transitions, the quoted ladder starts partway up and a note explains that a version-0 account instead fails with `MigrationUnavailable`; the day-one growth figure still counts every pending byte.

Two models keep the numbers interpretable. Rent reuses `RENT_EXEMPT_LAMPORTS_PER_BYTE` from the `create` warning, and the CU estimate sums `pina profile`'s static estimates for the generated adjacent `migrate` functions, so it excludes executor overhead (resize, rent transfer, validation) and runtime branch or loop effects. An artifact that has not been built, cannot be parsed, or lacks a transition function makes the CU figure print an explicit `CU unavailable: ...` reason instead of a zero.

Instruction processes link to account contracts by account-slot name, and only a `writable`, non-signer slot can hold an account the executor migrates. A writable, non-signer slot that names no checked-in account contract produces an explicit note instead of silently costing nothing — the symptom of a renamed account or a slot typo. Read-only and signer slots (authorities, payers, programs) can never be migrated, so they are skipped without noise.

`--json` emits the status array unchanged under `statuses` plus the additive cost section:

```json
{
	"statuses": [
		{
			"identity": "account:1:01",
			"kind": "account",
			"rustName": "State",
			"envelope": true,
			"currentVersion": 2
		}
	],
	"costPreview": {
		"rentLamportsPerByte": 6960,
		"maxInlineSteps": 8,
		"ladderModel": "the oldest version within MAX_INLINE_STEPS (8) of the current version, ...",
		"cuModel": "sum of `pina profile` static estimates for the generated adjacent `migrate` functions, ...",
		"artifact": "target/deploy/my_program.so",
		"contracts": [
			{
				"identity": "account:1:01",
				"currentSizeBytes": 44,
				"dayOneGrowthBytes": 2,
				"dayOneRentDeficitLamports": 13920
			}
		],
		"instructions": [
			{
				"identity": "instruction:1:00",
				"totalSteps": 2,
				"totalRentDeficitLamports": 13920
			}
		],
		"mostExpensive": {
			"status": "identified",
			"instructionRustName": "UpdateInstruction",
			"steps": 2,
			"rentDeficitLamports": 13920
		}
	}
}
```

A figure that cannot be estimated serializes as `{ "status": "unavailable", "reason": "..." }`; `pina migrations check --json` still emits only the status array.

## Disambiguate renames

A field that disappears while another field of the same type appears is ambiguous: a rename preserves the stored bytes, a remove-plus-add discards them and starts the new field zeroed. `pina migrations create` refuses to guess:

- On a terminal it prompts field by field and records the answer.
- With `--no-interactive`, or when no terminal is attached, it fails with one line per question naming the exact flags that answer it:

```bash
pina migrations create --rename score:points        # preserve the renamed data
pina migrations create --assume-removed score       # discard it; `points` starts zeroed
```

`--json` emits the open questions as a machine-readable array so agents can parse, decide, and re-run. With `--json`, `--no-interactive`, or no terminal attached, an unanswered question is a hard failure with a defined contract: the question array prints on stdout, the human-readable error prints on stderr, and the exit status is 1; capture both streams and re-invoke with the flags each question names. Answered renames are recorded in the manifest transition, so repeated `create` runs never re-ask, the generated transition copies the field's bytes, and `--assume-removed` prints a data-loss warning. Type changes and unpaired removals always fall back to a manual transition with a TODO body; nothing is dropped silently.

A third answer hands one field's conversion to you. `--manual <field>` names an **added** field and makes the whole transition manual, so the generated file is a stub you complete instead of a byte move Pina chose:

```bash
# Combine `first_name` and `last_name` into `name` yourself.
pina migrations create --rename first_name:name --assume-removed last_name --manual name
```

`--manual` is also the way to make a rename whose type changed legal: a generated rename copies bytes verbatim and so requires identical types, while `--manual` records that you own the interpretation. The answer is recorded, so repeated `create` runs keep generating the manual draft rather than re-deriving an automatic transition over your body.

Answers that contradict each other fail closed rather than picking a winner, whichever source they came from: a field cannot be both renamed and discarded, and a manual conversion cannot also discard the stored bytes it reads.

## Resolve a manual transition

Pina generates automatic transitions only for direction-safe fixed-layout changes: copies, insertions and removals that shift later fields, and zero-filled additions. An instruction argument is never zero-filled, because the handler could not tell that default from a value a client sent, so an instruction transition that adds an argument is always manual. A type change (including a widening such as `u64` to `u128`), a reorder of existing fields, a compact layout, an ambiguous field move, or a `--manual` answer creates a manual Rust file with `TODO(pina-manual-migration)`. `--manual <field>` also converts a draft that `create` already recorded as automatic.

A finished body belongs to the two layouts it was written for. While the draft's destination schema is unchanged, `create` keeps it. If you change the draft's layout again, `create` moves the finished body to `vN_to_vM.rs.stale`, writes a new stub whose header prints the new offsets, and says so. The new stub's marker blocks the build until you port the old body; delete the `.stale` file once you have. This keeps an old body from compiling against a layout it was not written for and silently misplacing bytes.

Every generated transition reads its byte offsets from the **stored** schema. A removed field keeps occupying its bytes, so a transition that drops a field in the middle of a layout still reads the fields after it from their original offsets — and its `SOURCE_SIZE` counts the bytes that are actually on the account.

Replace the generated body. Pina preflights the exact historical shape for every account. Fixed transitions have generated size constants and their generated `migrate` stub starts with a length guard; keep it. A transition involving compact data also has `target_size` and `working_size` functions. They inspect already-validated historical bytes and must return a valid destination allocation without mutating the account. The `migrate` function is then total for that accepted source and must fully initialize every active destination byte.

A manual account transition cannot reject a value it cannot interpret: by the time `migrate` runs, rent funding and resizing may already have taken effect, so a `migrate` that cannot produce a valid destination aborts the whole instruction instead of returning a catchable error. Validate unambiguous value constraints inside `target_size` and `working_size` (they run before any mutation) and reserve genuinely rejectable conversions for instruction transitions, which run in scratch space before any account is touched.

Pina runs adjacent account transitions one at a time inside one invocation. It validates and commits each intermediate version before planning the next, which lets a later compact allocation depend on the prior compact result without allocating a copy of the account on the SBF stack. If any later step fails, Pina aborts the instruction so Solana rolls back all earlier resizes, lamport transfers, and byte writes. A manual instruction conversion instead runs in scratch space and may reject invalid semantic values before dispatch. Then run:

```bash
pina migrations check
pina test --compatibility
```

`check` rejects a remaining marker. Once publication is pending or complete, it also rejects any change to the transition file or either schema hash. Fix frozen transition code with another migration version.

IDL generation runs the same check. The current IDL keeps each enveloped account, instruction, or event's `migrationVersion` field in place with `defaultValueStrategy: "omitted"` and its default value: generated client inputs omit it, encoders stamp the current value into the envelope automatically, and decoders reject any other version. A snapshot-only instruction has no `migrationVersion` field. Each earlier version of an event is listed as its own event node, `<Event>V<n>`, so clients can decode old log records; historical account and instruction schemas and all transition code remain exclusively in `migrations/manifest.json`. When `pina.toml` enables `auto` but no manifest exists yet, `pina idl` and `pina generate` refuse to run, because the IDL would omit the version byte the program gains once the baseline is recorded.

## Change an instruction process

Trailing optional accounts may be omitted entirely from the end of an account list; every earlier slot must still be present, using the program address as a filler where a middle optional account is absent. Omitting a middle optional account without a filler shifts every later account into an earlier slot, which only surfaces as a confusing missing-account error or a privilege check failure. Treat a trailing `Option` account as fully untrusted: it can be absent from any request, not only from old ones.

Pina snapshots the instruction payload and its positional account list under the same instruction version. Appending optional accounts to a published version extends its recorded account list in place: no version is consumed and no transition is written. An old request remains compatible only when:

- each existing account slot is unchanged;
- existing slots keep the same order;
- every new slot is appended at the end; and
- every appended slot is optional.

All account properties are part of a slot. A name, signer flag, writable flag, default, PDA, known address, or declarative constraint change is breaking. Create a new instruction discriminator for a breaking process.

The runtime treats an omitted optional suffix as absent. It never creates an account, signature, writable privilege, or PDA for an old client.

## Decode historical events

Events are immutable, so Pina versions them instead of migrating them. The program emits only the current version, and nothing converts an older record. Changing a published event's schema appends a version with its own schema and no transition file.

Generated clients decode each version with its own schema. The IDL lists every earlier version as a separate event node named `<Event>V<n>` (for example `ValueChangedEventV0`) next to the current event. The program-level log parser (`parse<Program>EventsFromLogs` in TypeScript and Dart) routes each record by discriminator and version and throws on a version no generated event describes, for example `event "valueChangedEvent" log carries migration version 2, which this client cannot decode; regenerate it`. In Rust, each event's `try_from_bytes` separates a stale record from a future one and names the event to decode it with. A decoded record holds exactly the fields its version emitted; no field is zero-filled. Keep golden log bytes in `pina_test::HistoricalEvent` and decode them with the generated event for their version rather than encoding fixtures with the current event type.

## Publish a version

Use `pina deploy` for a persistent cluster. Before the remote command starts, Pina atomically records the exact program ID, RPC target, executable digest, manifest digest, and current contract versions as pending. Receipts and pending records also pin the schema hash and transition-implementation hash of every published version, so rewriting published history — even with consistently recomputed hashes — fails every later check. A pending version is frozen because it may already be live. After deployment succeeds, Pina rechecks the planned files and converts that record into a hash-chained receipt.

Local deployments do not publish versions unless you pass `--record-publication`. A published version is immutable even if a later deployment replaces it.

If the deploy program cannot even start, for example because `solana` is not on `PATH`, nothing can have reached the cluster, so Pina discards the pending record it just wrote and your drafts stay editable. A pending record that an earlier attempt left behind is never discarded this way.

If deployment or receipt recording fails, stop the release. The pending record remains, and `pina migrations status` reports `publication pending`. Restore the exact planned inputs and rerun the same deployment to reconcile it; Pina rejects a different deployment while the outcome is ambiguous. The current ledger does not prove the deployed program-data hash, genesis hash, slot, or transaction signature.

When the exact planned inputs cannot be reproduced (for example a cleaned build directory), inspect the pending deployment with `pina migrations reconcile`. It prints the cluster, RPC endpoint, program, and executable digest that must be resumed. Once you are certain the deployment never went live, `pina migrations reconcile --abandon` converts the pending record into an abandoned receipt that still freezes its pinned versions and unblocks the next deployment. Losing `migrations/publications.json` entirely fails every later check while the manifest still records advanced versions, because published history must stay pinned; restore the ledger from version control instead of regenerating it.

A receipt must pin the schema hash of every version it made live. Ledgers written before pinning existed name only the highest version, and every command refuses them with `UnpinnedPublication`, because such a receipt cannot tell a rewritten published schema from the one that shipped. After confirming from version control that `migrations/manifest.json` still records exactly what those receipts shipped, run `pina migrations reconcile --pin-legacy` once and commit the ledger.

The manifest records the program ID its history belongs to. Before anything is published, `pina migrations create` rebinds the history to a changed `declare_id!` (for example after `pina keys new`) and says so. After publication, a different `declare_id!` fails every command until the original is restored.

## ABI document upgrades

`abiVersion` belongs to Pina's migration document. It is independent of each account or instruction version, and it names the `pina_abi` release line that wrote the document: the value is the `major.minor` committed in `crates/pina_abi/ABI_VERSION`, which advances with a breaking `pina_abi` release and with nothing else.

Reads reject a document stamped above the running build, naming the supported version, and reject one below the oldest supported version with the remedy that regenerates it. Anything between is normalized through an ordered table of adjacent converters before the typed model is read, so a document an older release wrote still opens. Conversions run in memory only: no command rewrites a checked-in document as a side effect of reading it.

The 0.20 reset replaced the integer `formatVersion` counters, and documents from older releases must be converted once before any Pina command can read them — `create` and `sync` included. [Migrate to the reset ABI document](../migrations/abi-document-reset.md) walks the conversion for both deployed and not-yet-deployed programs.

ABI `0.21` stores each fact once: the contract key `kind:width:hex` is the identity, the wire codec is implied by `abiVersion`, and a version without a transition omits the key. It also added `"envelope": false` for instructions recorded without an envelope and removed event transitions. A `0.20` document is converted in memory and rewritten by the next `create`; [Upgrading from ABI 0.20](../migrations/abi-versioning.md#upgrading-from-abi-020) lists the source changes that can follow.

`pina abi schema` prints the JSON Schema for a document, generated from the same types that read and write it:

```sh
pina abi schema --document manifest > manifest.schema.json
pina abi schema --document publications
```

Each version's schema is checked in under `crates/pina_abi/schemas/`, frozen beside its fixture under `crates/pina_abi/fixtures/<version>/`, and published under this book at a permanent URL — `https://pina-rs.github.io/pina/abi/schemas/<version>/manifest.schema.json` — which its `$id` names. Only the version this build writes can be printed; an older version's shape is recorded by its frozen fixture rather than reproduced under a new build.

An ABI document upgrade does not consume an on-chain migration version.

## Commands

| Command                                  | Result                                                                   |
| ---------------------------------------- | ------------------------------------------------------------------------ |
| `pina migrations create`                 | Capture source changes and create or refresh one draft                   |
| `pina migrations check`                  | Fail on source, schema, process, transition, or version drift            |
| `pina migrations status`                 | Show each current version, its publication state, and the cost           |
| `pina migrations sync`                   | Run `create`, `pina build`, and `pina generate` for unambiguous changes  |
| `pina migrations inspect <ADDRESS>`      | Compare one on-chain account's envelope with the manifest                |
| `pina migrations reconcile [--abandon]`  | Explain or abandon an ambiguous pending deployment                       |
| `pina migrations reconcile --pin-legacy` | Pin receipts written before schema pinning, after verifying the manifest |

Add `--json` for machine-readable output. Add `--project <DIR>` to select a program from another directory.
