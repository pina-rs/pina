# `pina generate`

Refresh the current program's IDL and generate selected client ecosystems.

## Synopsis

```text
pina generate [OPTIONS]
```

| Input                 | Default           | Meaning                                                                               |
| --------------------- | ----------------- | ------------------------------------------------------------------------------------- |
| `-p, --project <DIR>` | current directory | Start directory for project discovery.                                                |
| `--client <LANGUAGE>` | `pina.toml`       | `cpi`, `rust`, `typescript`, `dart`, `cli-rust`, `cli-ts`, or `cli-dart`; repeatable. |
| `-o, --output <DIR>`  | configured output | Override the client output root.                                                      |
| `--mode <MODE>`       | `pina.toml`       | Override with `auto`, `create`, `update`, or `overwrite`.                             |
| `--no-scaffold`       | off               | Generate sources without creating manifests or entrypoints.                           |
| `--npx <COMMAND>`     | `npx`             | Codama runner for TypeScript or Dart.                                                 |

```bash
pina generate
pina generate --client rust
pina generate --client cpi
pina generate --client typescript --client dart
pina generate --client cli-rust
pina generate --client cli-ts
pina generate --client cli-dart
pina generate --mode create
pina generate --mode update --no-scaffold
pina generate --mode overwrite
```

Repeating a language is harmless. Explicit `--client` values replace the configured list for that invocation. CPI-only, Rust-only, and `cli-rust` generation do not invoke Node.js. Each CLI variant implies its base client (`cli-rust` ⇒ `rust`, `cli-ts` ⇒ `typescript`, `cli-dart` ⇒ `dart`); projects normally pick one CLI, and Pina warns when several are selected together.

`auto` initializes an empty destination and otherwise updates it. Updates replace only renderer-owned generated source, preserving customized manifests and crate/package entrypoints. `create` and `update` enforce the expected destination state. `overwrite` deletes the entire selected client target before regeneration; use it for an intentional clean sweep. Command-line `--mode` and `--no-scaffold` override every selected target for that invocation.

Outputs are grouped by ecosystem:

```text
clients/
├── cpi/<library-name>/
├── rust/<library-name>/
├── typescript/<library-name>/
├── dart/
├── cli-rust/<library-name>/
├── cli-ts/<library-name>/
└── cli-dart/
```

Pina rejects filesystem-root and symbolic-link generation targets before a renderer runs.

Generate every program in a repository by running the command once per project; `scripts/generate-pina-clients.sh` does exactly that for this repository's examples.

## Compute unit limits

When the program has a `compute-units.json` recorded by [`pina test --record-compute-units`](./test.md#recording-compute-units), the IDL carries each measured instruction's budget as a `pinaComputeUnits` plugin node, and every client requests a tight, evidence-based limit instead of the runtime default:

```json
{
	"kind": "pluginNode",
	"name": "pinaComputeUnits",
	"payload": { "measured": 379, "limit": 800 }
}
```

The limit is computed once, here, and copied by every generator:

```text
limit = round_up_to_100(measured × (100 + margin_percent) / 100) + 300
```

- `margin_percent` comes from `[compute_units]` in `pina.toml` and defaults to `20`, so a limit absorbs inputs more expensive than the recorded fixtures. The margin rounds up, and rounding to a hundred keeps limits stable when a measurement moves by a few units.
- The `300` units cover the compute budget instructions a priority-fee transaction carries: `SetComputeUnitLimit` and `SetComputeUnitPrice` each consume 150 compute units. Pina's test suite measures both rather than assuming them.
- The limit never exceeds the 1,400,000 unit transaction maximum. An instruction whose measurement leaves no room for the compute budget instructions fails generation.

Each client exposes the budget its own way:

| Client                           | What it gets                                                                                                                                                                                  |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust                             | `<NAME>_MEASURED_COMPUTE_UNITS` and `<NAME>_COMPUTE_UNIT_LIMIT` next to each discriminator, plus a crate-level `set_compute_unit_limit_instruction(units)`.                                   |
| TypeScript                       | The same constants and `get<Program>ComputeUnitLimit(instructions)`, which sums the limits of the program's instructions in a transaction, ready for `setTransactionMessageComputeUnitLimit`. |
| Dart                             | `<name>MeasuredComputeUnits`, `<name>ComputeUnitLimit`, and `get<Program>ComputeUnitLimit(instructions)`.                                                                                     |
| `cli-rust`, `cli-ts`, `cli-dart` | Every command adds one `SetComputeUnitLimit` with its instruction's limit. `--compute-unit-limit <UNITS>` overrides it, and `--simulate` reports consumption against the limit requested.     |

The summing helpers return no limit when a transaction carries no instruction for the program, or one without a measurement, so the runtime default applies. Their sum is conservative: every limit carries its own margin and compute budget reserve, which a transaction pays only once. They do not count instructions for other programs; add those budgets yourself.

```ts
import { setTransactionMessageComputeUnitLimit } from "@solana/kit";
import { getCounterProgramComputeUnitLimit } from "./clients/typescript/counter_program/src/generated";

const limit = getCounterProgramComputeUnitLimit(instructions);
const budgeted = setTransactionMessageComputeUnitLimit(limit, message);
```

An instruction without a measurement gets no plugin, and its clients are unchanged. CPI clients never set a limit: a compute unit limit belongs to the outer transaction.

A measurement names an instruction by its IDL name. If `compute-units.json` names one the program no longer declares, `pina generate` fails until you run `pina test --record-compute-units` again or remove the stale entry (or the whole file), so a renamed instruction cannot silently lose its budget in committed clients. `pina build`, `pina idl`, and `pina test` print a warning naming the stale entries and ignore them, so they never block the recording that replaces them. When the build in `target/deploy` differs from the one recorded in `artifactSha256`, `pina generate` still generates the limits and prints a warning naming the command that measures the current build.

See [Project Configuration](./configuration.md) for client defaults and the distinction between configuration-relative and command-line paths.
