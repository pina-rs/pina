---
pina_cli_renderer: fix
pina_cpi_renderer: fix
---

# Keep IDL-derived output directories inside the output root

Reject IDL file stems that name parent directories, use non-UTF-8 names, or contain platform-specific path syntax before generating CLI and multi-IDL CPI crates. Single-IDL CPI rendering continues to use the explicitly selected output directory.
