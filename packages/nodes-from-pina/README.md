# @pina-rs/codama-nodes

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

Utilities for consuming Pina-generated IDLs and turning them into Codama `RootNode`s.

<!-- {=npmReadmeBadgeRow:"@pina-rs/codama-nodes"} -->

[![npm](https://img.shields.io/npm/v/@pina-rs/codama-nodes?logo=npm&label=npm)](https://www.npmjs.com/package/@pina-rs/codama-nodes) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

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
