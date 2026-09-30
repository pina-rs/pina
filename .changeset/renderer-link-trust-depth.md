---
pina_cli_renderer: fix
pina_cpi_renderer: fix
pina_codama_renderer: fix
# `pina_root` owns every workspace path, is unpublished, and records the
# coverage without a bump.
pina_root: none
---

# Distrust root-owned project symlinks in renderers

The CLI, CPI, and Codama renderers trusted any root-owned symbolic link while validating output path components. When a renderer runs as root (Docker containers, many CI images) every file a checkout creates is root-owned, so a symlink committed to the project, such as `clients -> /elsewhere`, passed the check and redirected generated output.

Every link or reparse point is now untrusted except a root-owned link directly below the filesystem root, such as macOS's `/var` and `/tmp` or a merged-`/usr` system's `/bin` and `/lib`. Only root can create an entry at that depth, and no project tree lives there. This matches the rule `pina_cli` applies to the paths it publishes.
