---
pina_cli: feat
pina_cpi_renderer: feat
---

# Generated CPI crates follow the source naming convention

The CPI scaffold previously forced a kebab-cased crate name (`counter-program-cpi`) even when the source program used underscores. The scaffold now preserves the source name's own convention: snake-cased names produce `<name>_cpi` and hyphenated names produce `<name>-cpi`.

`RenderConfig` gained an optional `package_name` field so callers can pass the exact package name; when it is unset, the renderer falls back to the snake-cased IDL program name plus `_cpi`. The CLI threads the example or project package name through generation, and the checked-in CPI clients were regenerated under the new convention.
