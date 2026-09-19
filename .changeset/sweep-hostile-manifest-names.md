---
pina_abi: fix
pina_macros: fix
---

# Reject hostile migration manifest names and duplicate schema fields

A migration manifest whose `rustName` is not a plain Rust identifier decoded and validated cleanly and then panicked inside the macro expansion at `syn::Ident::new`, presenting as `proc macro panicked` instead of the crate's standard typed error. `ContractHistory::validate` now rejects non-identifier and raw-identifier `rustName` values before codegen runs, naming the contract and the remedy. Duplicate field names within one schema version are grammar-valid but silently break rename matching and client byte mapping; `DataSchema::validate` now rejects them per version. Both checks are fail-closed for every well-formed document — existing manifests, including all shipped examples, validate unchanged.
