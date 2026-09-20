---
pina_cli: breaking
---

# Generate every repository client with `pina generate`

`pina generate` supersedes `pina codama generate`, which is removed. It discovers a project through its `pina.toml`, refreshes the IDL, and renders only the configured client ecosystems, so a repository generates its clients by running the command once per project instead of through a second command with its own path flags.

Dogfooding this path in the repository surfaced a bug in shared Dart CLI packages: `pina generate` published the whole staged `bin/` directory for each program, so generating one project deleted the entrypoints of every other project sharing the package. Only the single entrypoint owned by the invocation is published now.
