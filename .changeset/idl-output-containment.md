---
pina_cli: fix
---

# Contain configured IDL and build outputs

Reject configured IDL and Cargo target directories that escape the repository or traverse symbolic links before creating outputs. Workspace-contained sibling paths remain supported. Set `CARGO_TARGET_DIR` in the invoking environment to select an external build directory; `generate --output` continues to relocate clients only.
