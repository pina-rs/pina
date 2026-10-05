---
pina_cli: breaking
pina_codama_renderer: minor
pina_cpi_renderer: minor
pina_cli_renderer: minor
---

# Isolate renderer packages and bound client cleanup

`pina generate` and `pina idl` run their `npx`/`pnpm`/Node children from an isolated temporary directory with project-local `PATH` entries removed, so a committed `node_modules` can no longer shadow the pinned renderer packages and execute during generation. Passing a Node executable with `--npx node` remains the documented project-package mode and keeps resolving from the project on purpose.

`pina.toml` can no longer direct generation outside the project's Git worktree or select `overwrite`. Configured client outputs must resolve inside the worktree (`{{root}}`-anchored paths reach anywhere within it); publishing elsewhere takes the `--output` flag, and destructive regeneration takes `--mode overwrite`. `pina generate` now prints each client's resolved destination and mode.

Generated-client cleanup is bounded by a tracked-files record: every render writes `.pina-generated.json` at the client root listing the files Pina owns, and later runs remove only those paths, so files added to a generated tree survive regeneration. `overwrite` refuses a nonempty destination that predates tracked manifests instead of removing it; remove it by hand once, or generate without `overwrite` once to record its files.
