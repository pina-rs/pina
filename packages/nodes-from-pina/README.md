# @pina-rs/codama-nodes

<p align="center">
	<img
		src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png"
		alt="Pina: a pineapple made from interlocking ribbons"
		width="140"
	>
</p>

Utilities for consuming Pina-generated IDLs and turning them into Codama `RootNode`s.

<!-- {=npmReadmeBadgeRow:"@pina-rs/codama-nodes"} -->

<p align="center">
	<a href="https://www.npmjs.com/package/@pina-rs/codama-nodes"><img alt="npm" src="https://img.shields.io/npm/v/@pina-rs/codama-nodes?logo=npm&style=flat-square"></a>
	<a href="https://github.com/pina-rs/pina/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/pina-rs/pina/ci.yml?label=CI&logo=githubactions&style=flat-square"></a>
	<a href="https://opensource.org/license/apache-2.0"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue?logo=apache&style=flat-square"></a>
</p>

<!-- {/npmReadmeBadgeRow} -->

## Install

```sh
pnpm add @pina-rs/codama-nodes codama
```

## Usage

```ts
import { rootNodeFromPina } from "@pina-rs/codama-nodes";

const root = rootNodeFromPina(idlJsonString);
```

`rootNodeFromPina` applies the default visitor, which currently runs Codama's fixed-account-size normalization so account sizes are populated when they can be inferred.

If you need the raw parsed node, use `rootNodeFromPinaWithoutDefaultVisitor`.
