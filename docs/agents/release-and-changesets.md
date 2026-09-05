# Release Process and Changesets

## Release workflow

This repo uses the monochange release workflow.

```sh
monochange run change
monochange run release
monochange step publish-packages
```

## First-time package publication

Trusted publishing cannot publish a new crates.io crate or npm package until the package exists and its registry-side trusted publisher has been configured. Before merging a release that introduces a public package:

1. Ask a registry owner to preview and create its `0.0.0` placeholder:

   ```sh
   monochange step placeholder-publish --dry-run --package <package-id>
   monochange step placeholder-publish --package <package-id>
   ```

2. Ask the owner to configure the package's trusted publisher for repository `pina-rs/pina`, workflow `publish.yml`, and environment `publisher`.
3. Confirm the PR's `release-publish` job passes before merging.

The placeholder reserves the package name and gives the registry an existing package on which to configure OIDC trust. Agents must not attempt the real release and wait for it to fail as a way to discover missing setup, and must not use a maintainer's local registry credentials themselves.

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
- `pina_cpi_renderer`
- `pina_profile`
- `pina_test`
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
