---
pina_cli: breaking
pina_codama_nodes: fix
pina_codama_renderer_cpi: fix
pina_codama_renderer_cli: fix
---

# Pin client generation to the Kit 8 toolchain

`CodamaError` gains the `StaleKitScaffold` variant (now carrying an `ecosystem` label and a string `minimum`), which is a breaking change for downstream exhaustive matches over the error enum.

The Dart scaffolds move the Solana Kit Dart packages to `">=0.10.0 <1.0.0"` — a range rather than a caret pin, so pre-1.0 minor releases flow in without regeneration — and the fail-closed scaffold guard now also covers a Dart `pubspec.yaml` whose `solana_kit_*` ranges floor below what the generated sources compile against.

The `npx` and `pnpm dlx` fallbacks installed `codama@1.10.1` with `@codama/renderers-js@2.3.1` and `codama-renderers-dart@0.5.5`, so every freshly scaffolded TypeScript client pinned `@solana/kit` to the Kit 7 line even though the workspace itself targets Kit 8. The pinned install specs move to `codama@1.11.0`, `@codama/renderers-js@2.5.0`, and `codama-renderers-dart@0.5.6`, the workspace's local renderer floors match, and all committed clients are regenerated from the Kit 8 renderer with their scaffolded manifests consciously aligned to the current ranges.

Because scaffolded manifests are never rewritten by later runs, a manifest created by an older renderer keeps its stale Kit pin forever while the regenerated sources around it move forward. Generation now fails closed when an existing scaffold pins a Kit version older than the generated sources require — `@solana/kit` for TypeScript, the `solana_kit_*` packages for Dart — naming the manifest and the range to update instead of leaving a client that cannot compile. `overwrite` mode remains the explicit way to start a scaffold over on the current ranges.
