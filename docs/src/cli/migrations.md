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

## Opt whole kinds in

A program that wants every contract versioned can opt in by kind instead of annotating each declaration:

```toml
[migrations]
version_type = "u8"
auto = ["accounts", "events", "instructions"] # or `auto = true` for every kind
```

`auto` accepts `true` (every kind), `false` (the default), or a list of `accounts`, `events`, and `instructions`; any other name is a configuration error. Staging a subset is meaningful because the kinds have different costs: instruction envelopes change payload bytes and ripple into CPI call sites, so `auto = ["accounts", "events"]` is a useful middle step.

`pina migrations create` records the resolved policy as `auto` in `migrations/manifest.json` and snapshots every contract of the listed kinds. The manifest is the single source of truth for macros: they never read `pina.toml`, because a proc macro does not re-expand when an unrelated toml file changes. A struct that is not yet snapshotted still fails the build with the existing "run `pina migrations create`" error, so the workflow is unchanged.

Because the policy lives in the manifest, flipping it re-expands every contract without a source edit. When a policy is recorded, `create` scaffolds a `build.rs` containing:

```rust
fn main() {
	println!("cargo:rerun-if-changed=migrations/manifest.json");
}
```

The scaffold is idempotent and never overwrites an existing hand-written build script; `create` prints the exact line to add instead, and `pina migrations check` fails until it is present.

Per-item `migrations = false` keeps one contract out of an auto policy. Removing the envelope from a contract the manifest already records is an error rather than a silent opt-out: stripping an envelope is a wire-format change, so the build fails with the contract identity and the required remedy. Dropping a kind from `[migrations].auto` is rejected the same way. Enabling auto on an already-launched program inserts an envelope into every contract of the listed kinds — one recorded history entry per contract through `create` — while a new program simply captures that baseline.

## Capture a draft

Run the migration generator after the source ABI changes:

```bash
pina migrations create
pina migrations status
```

Pina writes `migrations/manifest.json`, `migrations/publications.json`, and adjacent transition files under `migrations/transitions/`.

If the current version has never been deployed to a non-local cluster, `create` replaces that draft. If a publication receipt or pending deployment contains the version, `create` appends the next version.

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

Pina generates automatic transitions only for direction-safe fixed-layout changes. A type change, compact layout, an ambiguous field move, or a `--manual` answer creates a manual Rust file with `TODO(pina-manual-migration)`.

Every generated transition reads its byte offsets from the **stored** schema. A removed field keeps occupying its bytes, so a transition that drops a field in the middle of a layout still reads the fields after it from their original offsets — and its `SOURCE_SIZE` counts the bytes that are actually on the account.

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

`abiVersion` belongs to Pina's migration document. It is independent of each account or instruction version, and it names the `pina_abi` release line that wrote the document: the value is the `major.minor` committed in `crates/pina_abi/ABI_VERSION`, which advances with a breaking `pina_abi` release and with nothing else.

Reads reject a document stamped above the running build, naming the supported version, and reject one below the oldest supported version with the remedy that regenerates it. Anything between is normalized through an ordered table of adjacent converters before the typed model is read, so a document an older release wrote still opens. Conversions run in memory only: no command rewrites a checked-in document as a side effect of reading it.

The 0.20 reset replaced the integer `formatVersion` counters, and documents from older releases must be converted once before any Pina command can read them — `create` and `sync` included. [Migrate to the reset ABI document](./migrations/abi-document-reset.md) walks the conversion for both deployed and not-yet-deployed programs.

`pina abi schema` prints the JSON Schema for a document, generated from the same types that read and write it:

```sh
pina abi schema --document manifest > manifest.schema.json
pina abi schema --document publications
```

Each version's schema is checked in under `crates/pina_abi/schemas/`, frozen beside its fixture under `crates/pina_abi/fixtures/<version>/`, and published under this book at a permanent URL — `https://pina-rs.github.io/pina/abi/schemas/<version>/manifest.schema.json` — which its `$id` names. Only the version this build writes can be printed; an older version's shape is recorded by its frozen fixture rather than reproduced under a new build.

An ABI document upgrade does not consume an on-chain migration version.

## Commands

| Command                  | Result                                                         |
| ------------------------ | -------------------------------------------------------------- |
| `pina migrations create` | Capture source changes and create or refresh one draft         |
| `pina migrations check`  | Fail on source, schema, process, transition, or version drift  |
| `pina migrations status` | Show each current version, its publication state, and the cost |

Add `--json` for machine-readable output. Add `--project <DIR>` to select a program from another directory.
