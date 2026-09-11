# @pina-rs/codama-renderer-cli

Codama visitors that render command-line applications from program IDLs.

## TypeScript (commander)

`renderVisitor` consumes a normalized Codama root node and emits a complete Node CLI package: one commander subcommand per instruction with typed flags, automatic PDA derivation through the generated `@solana/kit` client, a `fetch` command per state account, and shared RPC / keypair / send / simulate plumbing with https-only endpoint enforcement.

```ts
import { renderVisitor } from "@pina-rs/codama-renderer-cli";
import { createFromJson } from "codama";

const codama = createFromJson(readFileSync("counter_program.json", "utf8"));
await codama.accept(
	renderVisitor("./generated/counter-cli", {
		clientImportPath: "../client/src/generated/index",
	}),
);
```

Run the result with `npx tsx src/main.ts --help`.

Dart (`args`) support renders `args`-package CLIs; the Rust (clap) renderer lives in the `pina_cli_renderer` crate of the Pina workspace.
