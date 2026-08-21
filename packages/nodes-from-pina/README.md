# @pina-rs/codama-nodes

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

Utilities for consuming Pina-generated IDLs and turning them into Codama `RootNode`s.

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
