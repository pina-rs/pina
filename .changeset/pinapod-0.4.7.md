---
# The workspace manifest and lock files the dependency bump edits release
# with the `core` group alongside `pina` itself, so the one entry covers both.
pina: fix
---

# Upgrade pinapod to 0.4.7

Bump the workspace `pinapod` dependency from 0.4.5 to 0.4.7 and refresh the root, `pina_fuzz`, and `pina_fuzz/fuzz` lock files (0.4.6 in between was a formatting-only release). No pina source changes are required — pina re-exports and type-mentions the pinapod API, and 0.4.7 adds no public API and changes no wire format, validation order, or error variants.

The release hardens the contracts around the generated code. The generated compact modules bind the prelude names they spell to `core` explicitly, so a schema module that defines an `Ok`-, `Some`-, or `None`-shaped item, or a primitive-named type, can no longer rebind generated code — previously a matching `Ok` function compiled into every generated validation return and panicked at runtime inside the commit path. Zero-variant enums are rejected with a focused derive diagnostic instead of opaque errors from the generated module, and `PodString`'s accessors now document the UTF-8 precondition they trust rather than implying the length clamp is the whole contract.

The wincode writers additionally validate stored content at the boundary before writing, so invalid values fail serialization instead of producing bytes their own reader refuses; pina does not enable the `wincode` feature, so that path does not compile into pina at all.

The one source-visible change in this repository is `tests/expand/pda.expanded.rs`: the generated module gains the three prelude-binding `use` lines and nothing else. The generated module bodies are unchanged, so the compiled instruction paths are identical by construction and no compute-unit movement is expected.

The same release also pins the fuzz dependency set behind a `cargo metadata --locked` check, adds advisories to cargo-deny and gitleaks to CI, and moves the coverage upload's `id-token: write` into a job that runs no repository code — verification-boundary changes on the pinapod side, recorded here for completeness.
