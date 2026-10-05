# `pina map`

Render an interactive map of a program's instructions and the accounts they lock.

```text
pina map [OPTIONS]
```

```bash
pina map
open "$(pina map)"
pina map --project ./programs/privacy_pool
pina map --output ./docs/program-map.html
pina map --json
```

`pina map` reads the program source, runs the same analysis as [`pina locks`](./locks.md), and writes one self-contained HTML file. The page inlines its styles, script, and data and makes no network requests, so it opens from disk, attaches to a pull request, or ships with documentation as is.

The file goes to `<target>/pina/map.html` under the project's Cargo target directory unless `--output` names another path; missing parent directories are created. stdout carries only the written path, so `open "$(pina map)"` (or `xdg-open`) opens it in one step. Progress goes to stderr.

## What the page shows

The page is drawn as an interlocking chart, the table a railway signal box uses to show which levers lock each other. Here the levers are accounts and the rows are instructions.

- **Header**: the program name, its program ID with a copy button, and counts of instructions, accounts, PDAs, and hotspots.
- **Hotspots**: one plate per fixed account that some instruction writes, with the cost in one sentence, for example "Every `deposit` and `transfer` in the cluster runs one at a time". Select a plate to open the account.
- **Lock chart**: rows are instructions in declaration order and columns are the accounts shared between instructions, hotspots first, then other fixed accounts, then keyed PDAs. A filled square is a write, an outlined square a read, a blue dot a signer, and a dashed square an account the caller may omit. A column's top band gives its class: red for a hotspot, ink for another fixed account, blue for a keyed PDA. The last column counts each instruction's caller-chosen accounts, which are never shared.
- **Tracing**: hover or focus a row and every row it conflicts with lights up, red when they can never run in parallel and amber hatching when they collide only for the same seeds; the accounts behind each conflict are outlined and everything else fades. Selecting a column lights up the rows that lock that account instead.
- **Detail panel**: selecting an instruction shows its docs, the instructions it never runs alongside or may wait for (with the accounts responsible), every account slot with its flags, PDA, and declarative constraints, and its arguments. Selecting an account shows its class, its account type's docs and fields, the hotspot cost, the PDA derivation from constant seeds and typed seed slots to the derived address, and the instructions that write and read it.

The chart is keyboard accessible: rows and columns are buttons, the arrow keys move between rows (up and down) and between columns (left and right), Home and End jump to the ends, and Enter selects. It follows the system's light or dark preference, honors reduced motion, and on a narrow screen scrolls the chart inside its frame rather than the page.

## Trust boundary

Doc comments and seed strings come from the program source. The page embeds its data as JSON in a `<script type="application/json">` block with `<`, `>`, `&`, U+2028, and U+2029 escaped, so no string can end the block, and it renders every string as text, never as markup.

## Agent JSON

```bash
pina map --json > map.json
jq -e '.instructionDetails | length > 0' map.json
```

`--json` prints the map's data instead of writing HTML, and conflicts with `--output`. The document is the [`pina locks --json`](./locks.md#agent-json) schema-version-1 document with two more top-level fields:

- `instructionDetails`: each instruction's `name`, `docs`, `arguments` (`name`, `type`, `docs`), and `accounts` in slot order, each with its `slot`, `node`, `writable`, `signer`, `optional`, `pda`, `constraints`, and `docs`;
- `accountTypes`: each `#[account]` type's `name`, the `pda` it declares (or `null`), `docs`, and `fields`.

Errors such as a missing project, unparseable source, or an unwritable output path exit with code `1`; an invalid flag combination exits with code `2`.
