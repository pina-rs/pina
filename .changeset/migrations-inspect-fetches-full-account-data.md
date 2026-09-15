---
pina_cli: fix
---

# Fetch full account data in `pina migrations inspect`

`pina migrations inspect` requested account data with a zero-length JSON-RPC `dataSlice`. A spec-compliant RPC honors that slice and returns no bytes, so every existing account decoded as `UnknownContract` and the command exited `0` — a silent all-clear from the one check that is supposed to report stale or future account versions before a deployment.

The command now requests the full account body, so the discriminator and version envelope decode as intended and the exit code reflects account state. The test server honors the requested slice instead of returning a canned body, and a regression test pins the zero-length response a slice-ignoring request would have received.

The inspect request also gained a 30-second timeout so a stalled RPC endpoint fails with a message instead of blocking the command indefinitely.
