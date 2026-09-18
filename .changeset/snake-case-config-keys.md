---
pina_cli: fix
---

# Standardize `pina.toml` keys on snake_case

`[migrations]` was the only table in `pina.toml` that parsed kebab-case keys, and two of its documented spellings did not parse at all. Snake case is now canonical everywhere in the file, matching `[project].idl_dir` and the rest of the TOML surface:

- `[migrations].version_type` replaces `version-type`. The kebab spelling keeps parsing as an alias, so existing checkouts and the checked-in `examples/migrations_program/pina.toml` keep building.
- `[migrations.answers].assume_removed` replaces `assume-removed`, which the docs showed but the parser rejected. The kebab spelling keeps parsing as an alias.
- `[clients].languages` now accepts `cli-rust`, `cli-ts`, and `cli-dart`. The previous spelling was the squashed `clirust`/`clits`/`clidart`, so the documented and CLI-flag spelling `["cli-rust"]` previously failed to parse. Both spellings parse; the squashed form is a deprecated alias.
- `[clients.cli_rust]`, `[clients.cli_ts]`, and `[clients.cli_dart]` replace the kebab table names, which were documented and rejected. The kebab tables keep parsing as aliases.

Documentation, CLI help, the bundled skill, and the migrations walkthrough script all use the canonical spellings. Unknown keys still fail closed, so a typo cannot silently change a build.
