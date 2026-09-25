---
pina_cli: fix
---

# Pin client generation to the Kit 8 Codama toolchain

The `npx` and `pnpm dlx` fallbacks installed `codama@1.10.1` with `@codama/renderers-js@2.3.1` and `codama-renderers-dart@0.5.5`, so every freshly scaffolded TypeScript client pinned `@solana/kit` to the Kit 7 line even though the workspace itself targets Kit 8. The pinned install specs move to `codama@1.11.0`, `@codama/renderers-js@2.5.0`, and `codama-renderers-dart@0.5.6`, the workspace's local renderer floors match, and all committed clients are regenerated from the Kit 8 renderer with their scaffolded manifests consciously aligned to `^8.3.0`.

Because scaffolded manifests are never rewritten by later runs, a manifest created by an older renderer keeps its stale Kit pin forever while the regenerated sources around it move forward. TypeScript generation now fails closed when an existing scaffold pins a `@solana/kit` major older than the generated sources require, naming the manifest and the range to update instead of leaving a client that cannot compile.
