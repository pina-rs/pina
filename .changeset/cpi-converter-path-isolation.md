---
pina_cli: patch
---

# Scrub the Anchor converter's `PATH`

`pina cpi` runs the pinned `@codama/nodes-from-anchor` converter through `npx -p <package> node`, and `npx` resolves the `node` interpreter through the inherited `PATH`. The converter child now runs with workspace-local `PATH` entries removed and `NODE_PATH` dropped — the same isolation the client renderers already apply — so a committed `node_modules/.bin/node` can no longer shadow the interpreter during Anchor IDL conversion. The npx cache the converter script resolves its package from lives outside the workspace and is unaffected. Passing a Node executable with `--npx <node>` keeps resolving from the project on purpose.
