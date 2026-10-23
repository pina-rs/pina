---
pina: breaking
pina_macros: breaking
pina_cli: breaking
pina_lints: docs
pina_skill: docs
---

# Remove the deprecated pre-release compatibility surface

- **`pina`** drops `find_program_address` (use `try_find_program_address`), `introspection::assert_no_cpi` (use `assert_current_instruction_program_id`), and the `AllocateAccountWithBump` alias (use `AllocateAccountWithNonCanonicalBump`).
- **`pina_macros`** no longer generates the deprecated `with_pda` alias for the compact stored-bump PDA loader (use `with_stored_bump_pda`) and no longer parses the `min`/`max`/`min_len`/`max_len`/`exact_len` validation spellings or warns about them. Write every bound as a comparison, as in `value >= 1` or `len == 4`.
- **`pina_cli`** no longer discovers the uppercase `Pina.toml` spelling, accepts the squashed client spellings (`clirust`, `clits`, `clidart`), the kebab override tables (`[clients.cli-rust]`; write `[clients.cli_rust]`), or the `assume-removed` alias in `[migrations.answers]`. The `[migrations].version_type` and `[migrations].auto` keys are unknown fields like any other misspelling; record those settings with `pina migrations create --version-type` and `--auto`.
- The book drops the release-upgrade migration guides and every deprecated-spelling mention; the on-chain migration feature docs (`migrations/flow.md`, `migrations/abi-versioning.md`, `cli/migrations.md`) stay.
