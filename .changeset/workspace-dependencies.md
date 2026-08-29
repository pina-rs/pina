---
pina_cli: none
---

Move `pina_cli` dependency declarations to workspace level.

- Move `comfy-table`, `owo-colors`, `rayon`, and `termimad` from direct `pina_cli` dependencies into `[workspace.dependencies]`.
- Normalize the `base64`, `bs58`, and `url` workspace entries to `default-features = false` with caret version requirements.
- `pina_cli` now opts into `default-features = true` explicitly per dependency, so resolved features are unchanged.
