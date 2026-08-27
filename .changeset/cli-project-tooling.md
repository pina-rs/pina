---
pina_cli:
  bump: minor
  type: feat
pina_skill:
  bump: minor
  type: feat
---

# Add project-aware CLI diagnostics and identity tooling

Pina can now inspect, generate, and safely synchronize program identities with `pina keys`; diagnose project paths and Rust, Solana, Surfpool, and client tools through human or versioned JSON reports with `pina doctor`; and generate completion scripts for five shells with `pina completions`.

`pina profile` keeps accepting an explicit SBF binary while also discovering the current program's canonical Cargo deploy artifact when the path is omitted. Atomic output rejects lexical aliases, hardlinks, symlinks, and reparse-point paths that could overwrite the input program. Keypair inputs are bounded regular files and validate the full Ed25519 secret/public relationship without printing secret bytes. Generated keypair and source destinations reject linked path ancestors, preserve concurrent editor/keypair replacements, roll back failed multi-file updates only when the generated identity still owns the path, and require `keys new --force` before replacing an existing identity. Platforms that cannot atomically guarantee private keypair permissions fail before creating secret material. Doctor probes close stdin, bound output, escape diagnostics, and time out with child cleanup for deterministic agent use.
