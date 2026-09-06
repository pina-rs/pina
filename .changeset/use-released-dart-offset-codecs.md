---
pina_cli: fix
---

# Use released Dart offset codecs

Upgrade the pinned Dart renderer to 0.5.5, which contains the upstream pre/post-offset collection-length fix. Remove the local renderer patch and renderer-specific dependency overrides so generated clients use the published package directly.
