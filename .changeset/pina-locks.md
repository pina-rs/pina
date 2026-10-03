---
pina_cli: feat
---

# Report write-lock contention between instructions

The new `pina locks` command shows which instructions can never run in parallel, read straight from the program source.

Solana's scheduler runs two transactions in parallel only when neither write-locks an account the other locks. A writable PDA whose seeds are all constants has the same address in every transaction, so every instruction that writes it serializes all of its traffic across the cluster. `pina locks` sorts every instruction account into a `fixed` address (a constant-seed PDA, with its derived address, or a known program or sysvar), a `keyed` PDA (with its seed names and types), or a `caller`-chosen account, then reports:

- **Hotspots**: each fixed account some instruction writes, with its seeds, address, writers, readers, and a one-line cost such as "Every `deposit` and `transfer` in the cluster runs one at a time".
- **Conflicts**: an instruction-by-instruction matrix marking pairs that `always` contend (a shared fixed account) or `may` contend (a shared keyed PDA, which collides only for the same seeds). Caller-chosen accounts are never compared, and the legend says so. Programs too wide for a matrix get a list.

```bash
pina locks
pina locks --json
pina locks --deny-hotspots
```

`--json` prints a schema-version-1 document of nodes, per-instruction reads and writes, conflicts, and hotspots. `--deny-hotspots` exits with status 1 when a hotspot is not accepted by the new `[locks] allow` list in `pina.toml`, for intentional singletons such as an admin configuration. Every allowed name must be a current hotspot, so the list cannot go stale silently.

The analysis is also a library API, `pina_cli::locks::analyze` and `analyze_project`, and `Project::locks_config` reads the `[locks]` table.
