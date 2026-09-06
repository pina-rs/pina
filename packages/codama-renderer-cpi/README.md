# @pina-rs/codama-renderer-cpi

Codama visitor that renders a standalone, `no_std` Pina CPI crate from the pipeline's current `RootNode`.

```sh
pnpm add @pina-rs/codama-renderer-cpi codama
```

Use it in `codama.json`. Codama accepts both Codama and Anchor IDLs and performs Anchor normalization before visitors run.

```json
{
	"idl": "./target/idl/counter.json",
	"scripts": {
		"cpi": {
			"from": "@pina-rs/codama-renderer-cpi",
			"args": ["./clients/counter-cpi"]
		}
	}
}
```

```sh
codama run cpi
```

The visitor uses the native Pina CLI bundled by `@pina-rs/cli` and sends the current pipeline root over standard input. The generated crate exposes typed account sets and instruction builders with `.invoke()` and `.invoke_signed()` methods backed by Pina's validated, allocator-free CPI context.

Pass visitor options from a programmatic Codama pipeline when you need explicit destination handling:

```ts
await codama.accept(renderVisitor("./clients/counter-cpi", {
	mode: "update",
	scaffold: false,
}));
```

`mode` accepts `auto` (the default), `create`, `update`, or `overwrite`. Updates replace only `src/generated` and preserve an existing `Cargo.toml` and `src/lib.rs`. `scaffold: false` generates only source files; `overwrite` removes the complete destination before rendering.
