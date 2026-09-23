---
pina_cli: fix
---

# Reject Cargo library names that cannot be path components

Cargo accepts any string as a `[lib] name` and reports it verbatim through `cargo metadata`, so a checked-in manifest controls the value. Pina joined that name into file and directory paths (`target/deploy`, generated IDL files, client crate directories) without validating it, so `[lib] name = "../../escaped"` made `pina generate` write outside the project root — reproduced with an artifact landing next to the project directory while the command reported success.

`Project::discover` now validates the library name once, where it enters the project model, and rejects anything outside `[A-Za-z0-9_-]` with the new `ProjectError::InvalidLibraryName`. Cargo already refuses the empty name before metadata reaches pina. Validating at the ingestion point keeps every current and future path join safe instead of patching individual sinks, and no legitimate library name is affected because Cargo's own naming rules are a subset of the accepted set.
