# Pina skill evaluations

Evaluations for the `pina` agent skill (`packages/pina__skill`).

The skill's job is to let an agent work on a real Pina program without getting lost: knowing that migrations are snapshot-checked, that an ambiguous rename needs an explicit answer, that a type change needs a hand-written transition, and that a live instruction's account slots are a compatibility surface. Every scenario here exercises one of those decision points against the real `pina` CLI, and grades the artifacts the agent leaves behind rather than its prose.

## Why the checks are artifact-based

An agent that explains the migration rules beautifully but leaves the manifest stale has failed the task. So each scenario runs a real program change in a throwaway crate and then grades with the same tools CI would:

- `pina migrations check` for drift,
- `pina migrations status --json` for recorded versions,
- `cargo check` for the macro expansion gate,
- targeted file and transcript assertions for the parts a command cannot see.

## Layout

```
evals/pina-skill/
├── run.ts                 CLI runner: selects scenarios, drives the agent, reports
├── lib/
│   ├── agent.ts           headless agent invocation and transcript capture
│   ├── grade.ts           check execution
│   ├── paths.ts           layout, fixture copying, `pina` CLI resolution
│   └── types.ts           scenario and result types
├── fixtures/              throwaway Rust programs the agent modifies
├── scenarios/             one JSON file per evaluation
├── skill-variants/        complete `pina` skills to compare
└── results/               transcripts and reports (git-ignored)
```

## Running

Run from the repository root, inside `devenv shell`:

```sh
pnpm exec tsx evals/pina-skill/run.ts --list
pnpm exec tsx evals/pina-skill/run.ts --all --variant baseline
pnpm exec tsx evals/pina-skill/run.ts --scenario add-optional-field --variant baseline
pnpm exec tsx evals/pina-skill/run.ts --all --variant baseline --variant candidate --repeats 3
```

Useful flags: `--model`, `--timeout`, `--repeats`, `--instruction-variant`, `--skill-source`, `--list`. `--instruction-variant` swaps in the wording from a scenario's `variants` map, which is how different phrasings of the same task are compared.

`--skill-source installed` (the default) copies the variant into the run workdir as a project skill. `--skill-source cli` installs nothing: the agent starts with only the toolkit and has to find the guidance through the CLI, which is the path `pina skill read` and `pina skill install` serve. A scenario can pin the channel with a top-level `"skillSource": "cli"`, as `no-skill-manual-transition` does; runs are keyed by variant plus channel so the two modes never overwrite each other. `cli-self-sufficiency` grades the CLI surface itself with `"agent": false`, so that contract is checked deterministically instead of through a stochastic run.

### Prerequisites

1. Build the CLI the fixtures are graded with. The harness resolves `<repo>/target/debug/pina` first and never falls back to `PATH`, because a released `pina` commonly has different subcommands (`make` instead of `create`) and grading against it measures nothing:

   ```sh
   devenv shell -- cargo build -p pina_cli
   ```

2. The agent runtime must be authenticated. Runs use `--setting-sources project` and `--strict-mcp-config`, so the operator's personal hooks, plugins, and MCP servers stay out of the measurement.

## How isolation works

Each run copies a fixture into `.work/runs/<scenario>__<variant>__<n>`; for `installed` runs it also installs the chosen skill as `<workdir>/.claude/skills/pina` (cli runs install nothing). Because the run uses `--setting-sources project`, that copy is the only `pina` skill the agent sees — a globally installed skill of the same name cannot leak in.

The workspace `pina` binary is prepended to `PATH` so `pina migrations ...` inside the fixture resolves to the build under test.

## Authoring a scenario

A scenario is a JSON file in `scenarios/`. The fields that matter:

- `prompt` — handed to the agent verbatim. Write it as a user would: state the goal and the constraint (live program, don't break clients), not the mechanism.
- `fixture` — the program to modify. Use `base-counter` for a fresh program and `base-counter-published` when the scenario needs an already-live contract, because version-freezing and disambiguation only apply once something is published.
- `expectation` — what a correct solution looks like, used in the report.
- `checks` — the graded assertions.

Check kinds:

| Kind         | Grades                                                          |
| ------------ | --------------------------------------------------------------- |
| `command`    | a shell command's exit status and output substrings             |
| `file`       | a file exists, and its contents match (or `absent` them)        |
| `absent`     | a path does not exist                                           |
| `transcript` | the agent's own text and tool inputs match (or avoid) a pattern |

Transcript checks look only at content the assistant authored. Tool results and skill files the agent read are excluded, so a check cannot accidentally match the skill's own documentation instead of the agent's work.

A `file` check takes `path`, or `paths` when several placements are equally correct, or `glob` (one directory level and a `*` in the file name) when the task leaves the file name to the author. It passes when any candidate matches.

Prefer a `command` check over a `file` check whenever the CLI can answer the question; prefer an outcome over one spelling. Grading `MAX_MIGRATION_LAMPORTS = 56_000` exactly would fail a correct solution that chose `60_000`, so the budget check asserts that the agent engaged with the deficit instead.

## Adding a skill variant

Copy a complete skill into `skill-variants/<name>/` (`SKILL.md` plus `references/`) and pass `--variant <name>`. Comparing two variants is how a skill change is shown to fix a failure rather than merely changing the text. See `skill-variants/README.md`; keep `baseline` frozen so recorded comparisons stay meaningful.

## Re-grading without re-running the agent

Agents are slow and stochastic, so a grader fix should not cost another round of runs:

```sh
pnpm exec tsx evals/pina-skill/run.ts --all --variant baseline --regrade
```

This re-applies the current checks to the saved workdirs and transcripts. Use it after correcting a check, and re-run the agent only when the scenario itself changed.

## What the checks must not do

Grading agent output is easy to get subtly wrong, and a wrong check is worse than no check because it looks like evidence. These failure modes bit this suite, all of them false negatives on _correct_ work:

- **Matching one spelling.** A correct solution may extract a `HEADER_SIZE` constant instead of writing `data[34..42]`, or name a bound `MAX_COUNT` instead of `1_000_000`. Grade the mistake you are actually hunting (indexing the payload offset directly, omitting the rule) rather than the phrasing you happened to see.

- **Matching the transcript for a file property.** A run that reports "no `TODO(pina-manual-migration)` left" contains that string. Assert the artifact (the file omits the TODO), not the sentence.

- **Matching content the agent read.** Skill bodies and tool results arrive in the stream too. Transcript checks only see assistant-authored text and tool inputs, or the skill's own docs would satisfy them.

- **Pinning one valid placement.** A task that allows the crate's own test module or a `tests/*.rs` integration test has two correct answers, and the author picks the file name. Use the `paths` or `glob` form so both placements grade, instead of naming the file one run happened to write.

- **Pinning one valid choice.** "Widen the version type" is satisfied by `u16` or `u32`, and the follow-on numbers differ (`65535` against `4294967295`). Derive the expectation from the agent's own declared choice rather than asserting the branch you happened to see.

- **Assuming one working directory.** `pina init <name> --path .` scaffolds into the current directory while `pina init <name>` creates a subdirectory; both answer "scaffold it here". Paths in checks have to accept either layout.

Prefer a `command` check whenever the CLI can answer the question, and prefer an outcome over a spelling.

## Fixtures

- `base-counter` — one migration-aware account, two instructions, `auto = true`, and a generated baseline snapshot. Its `Cargo.toml` carries a `${PINA_ROOT}` placeholder that the harness rewrites to the checkout root, so the fixture depends on the local `pina` crates by path and stays portable.
- `base-counter-published` — the same program with a publication receipt, so its contracts are live and version-freezing, disambiguation, and process-contract compatibility all apply.
- `compact-counter-published` — a published compact account with a `String<8>` tail, for the variable-length transition path (`target_size`/`working_size`) that a fixed-layout fixture cannot exercise.
