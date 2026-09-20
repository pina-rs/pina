---
pina_cli: patch
---

# Generate every repository client with `pina generate`

The repository's own client generation now runs `pina generate` once per example instead of the repository-wide `pina codama generate` command, so the checked-in clients exercise the same project-aware path users run. Each example's `pina.toml` owns its IDL and client output paths.

This surfaced a bug in shared Dart CLI packages: `pina generate` published the whole staged `bin/` directory for each program, so generating one project deleted the entrypoints of every other project sharing the package. Only the single entrypoint owned by the invocation is published now, and the regenerated output is byte-identical apart from the provenance comment, which now names `pina generate`.
