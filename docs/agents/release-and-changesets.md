# Release Process and Changesets

## Release workflow

This repo uses the knope bot workflow.

```sh
knope document-change
knope release
knope publish
```

## Required changesets

Any pull request that modifies code in:

- `crates/`
- `examples/`

must include at least one changeset file in `.changeset/`.

## Changeset format

Interactive:

```sh
knope document-change
```

Manual:

```md
---
package_name: change_type
---

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
