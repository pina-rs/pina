---
pina_cli: fix
---

# Protect program map output from filesystem links

Reject symbolic links and reparse points in default and explicit map output paths. Open parent directories without following links and publish the complete HTML atomically through the pinned directory, preserving files reached through planted links or concurrent directory replacements.
