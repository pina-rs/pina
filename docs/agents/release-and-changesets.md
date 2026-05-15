# Release Process and Changesets

## Release workflow

This repo uses the monochange release workflow.

```sh
mc change
mc release
mc step:publish-packages
```

## Required changesets

Any pull request that modifies code in:

- `crates/`
- `examples/`

must include at least one changeset file in `.changeset/`.

## Changeset format

Interactive:

```sh
mc change --package <package-id> --bump <bump> --reason <reason>
```

Manual:

```md
---
package_name: minor
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
- `minor` — new backwards-compatible features
- `patch` — bug fixes
- `docs` — documentation-only changes
- `note` — general notes

## Package names

- `pina`
- `pina_macros`
- `pina_sdk_ids`

A single changeset file may reference multiple packages.

## Commit scope convention

Conventional commit scopes should map to package names where relevant.
