# Release Process and Changesets

## Release workflow

This repo uses the monochange release workflow.

```sh
monochange run change
monochange run release
monochange step publish-packages
```

## Required changesets

Any pull request that modifies code in:

- `crates/`
- `examples/`

must include at least one changeset file in `.changeset/`.

## Changeset format

Interactive:

```sh
monochange run change --package <package-id> --bump <bump> --reason <reason>
```

Manual:

```md
---
package_name: change_type
---

# Short heading

Detailed description of the change.
```

After creating or editing changesets:

```sh
dprint fmt .changeset/* --allow-no-files
```

## Change types

- `major` — breaking changes
- `feat` — new backwards-compatible features
- `fix` — bug fixes
- `docs` — documentation-only changes
- `none` — general notes

## Package names

- `pina`
- `pina_macros`
- `pina_sdk_ids`
- `pina_cli`
- `pina_codama_renderer`
- `pina_profile`
- `pina_codama_nodes`
- `pina_cli_npm`
- `pina_cli_darwin_arm64`
- `pina_cli_darwin_x64`
- `pina_cli_freebsd_x64`
- `pina_cli_linux_arm64_gnu`
- `pina_cli_linux_arm64_musl`
- `pina_cli_linux_x64_gnu`
- `pina_cli_linux_x64_musl`
- `pina_cli_win32_arm64_msvc`
- `pina_cli_win32_x64_msvc`
- `pina_skill`

A single changeset file may reference multiple packages. All publishable packages share one release identity via the `core` group, so any changeset bumps the whole group to the same version.

## Commit scope convention

Conventional commit scopes should map to package names where relevant.
