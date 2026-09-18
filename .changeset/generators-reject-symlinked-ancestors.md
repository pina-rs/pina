---
pina_cli: feat
pina_cli_renderer: fix
pina_codama_renderer: fix
pina_cpi_renderer: fix
---

# Reject symlinked generator destinations

Reject generation output paths when an existing path component is a symbolic link or Windows reparse point. Overwrite mode can no longer resolve a linked ancestor and remove a directory outside the requested output tree.
