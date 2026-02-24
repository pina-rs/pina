# codama-nodes-from-pina

Utilities for consuming Pina-generated IDLs and turning them into Codama `RootNode`s.

## Install

```sh
pnpm add codama-nodes-from-pina codama
```

## Usage

```ts
import { rootNodeFromPina } from "codama-nodes-from-pina";

const root = rootNodeFromPina(idlJsonString);
```

`rootNodeFromPina` applies the default visitor, which currently runs Codama's fixed-account-size normalization so account sizes are populated when they can be inferred.

If you need the raw parsed node, use `rootNodeFromPinaWithoutDefaultVisitor`.
