---
pina: major
---

Remove the `pina_token_2022_extensions` crate from the workspace entirely.

The upstream `pinocchio-token-2022` crate is adding native extension parsing support, making this crate redundant. The crate was never widely adopted and removing it simplifies the workspace.

**What was removed:**

- `crates/pina_token_2022_extensions/` directory and all source files
- Workspace member entry in root `Cargo.toml`
- Package configuration in `knope.toml`
- All references in documentation and changeset files

Extensions support can be re-added once `pinocchio-token-2022` ships its built-in extension types.
