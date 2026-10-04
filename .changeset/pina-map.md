---
pina_cli: feat
---

# Render an interactive program map

The new `pina map` command writes one self-contained HTML page that charts a program's instructions against the accounts they lock, from the same analysis as `pina locks`.

```bash
open "$(pina map)"
pina map --output ./docs/program-map.html
pina map --json
```

The page is drawn as a signal box's interlocking chart. Rows are instructions and columns are shared accounts, with writes, reads, signers, and optional accounts marked, and hotspot columns in red. Hovering or focusing a row lights up every instruction it can never run alongside (red) or may collide with for the same seeds (amber), outlines the accounts responsible, and fades the rest. Hotspot plates state each one's cost, and a detail panel shows an instruction's conflicts, accounts, constraints, and arguments, or an account's docs, fields, PDA derivation with its derived address, writers, and readers. The chart is keyboard accessible, follows the light or dark system preference, honors reduced motion, and scrolls inside its frame on narrow screens.

The page inlines its styles, script, and data and makes no network requests. The data is embedded as JSON escaped for a `<script>` block and rendered as text, so doc comments cannot inject markup. It is written to `<target>/pina/map.html` unless `--output` names another path, and stdout carries only the written path. `--json` prints the data instead: the `pina locks --json` document plus `instructionDetails` and `accountTypes`.

The map is also a library API: `pina_cli::map::map_project`, `build_map`, `render_html`, and `write_html`.
