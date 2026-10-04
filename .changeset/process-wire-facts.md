---
pina_abi: fix
pina_cli: fix
---

# Judge instruction process compatibility on wire facts only

Each recorded instruction account slot holds four wire facts, `name`, `writable`, `signer`, and `optional`, and two client hints, `defaultValue` (the known address generated clients fill in) and `pda`. Compatibility compared all six, so a change that only affected what generated clients fill in looked like a broken account list. A published instruction refuses any change to an existing slot, so improving the IDL extractor's PDA or known-address detection could leave a program unable to pass `pina migrations check` or generate its IDL, with neither the program nor any request changed.

Compatibility now compares wire facts only:

- `pina_abi` adds `ProcessAccount::same_wire` and `ProcessContract::same_wire`. `classify_process_transition` uses them, so a hint-only difference is `Unchanged` and appending optional slots still counts as `AppendOptional` when the existing slots differ only in hints. Every name, signer, writable, or optional change, reorder, removal, or new required slot still fails.
- `pina migrations create`, `check`, `status`, and IDL generation compare a recorded snapshot with source the same way. A hint-only change is not drift, consumes no version, and does not rewrite the manifest: the recorded hints are carried forward until a wire change rewrites the draft or appends a version.

Nothing in the document format changes, and no stored hash moves. Publication receipts pin only `schemaSha256` and `transitionSha256`, and neither covers a process hint. See ADR 0012 for the reasoning and the alternatives considered.
