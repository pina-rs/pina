---
pina_cli: fix
---

# Keep Skill Installation Inside Its Destination

Reject linked skill destinations and documents, pin destination directories against replacement races, and create fresh document files so installation cannot truncate external symlink or hard-link targets. Require `--force` before replacing a partially installed document.
