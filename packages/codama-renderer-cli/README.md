# @pina-rs/codama-renderer-cli

Codama visitors that render command-line applications from program IDLs.

IDL names used by the CLI must start with an ASCII letter and contain only ASCII letters, digits, and single underscores between nonempty segments. TypeScript and Dart reserved identifiers are rejected. Names are checked before deleting or writing output; unsafe names are never silently renamed. Rendering also rejects destinations outside the package and symbolic links within the output path.

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
