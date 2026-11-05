---
"pina_cli": fix
---

Clear an inherited program-keypair variable before applying the deployment command environment, so upgrade-only commands cannot receive a stale keypair path.
