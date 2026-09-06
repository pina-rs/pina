---
pina: none
pina_cli: none
---

# Rename every example manifest to pina.toml

The lowercase `pina.toml` name is the canonical project configuration file, and the examples were the last remaining place still carrying the legacy uppercase `Pina.toml` spelling. Every example manifest is renamed with git so history follows the file, and the pina_cli example-discovery test message says pina.toml. Project discovery already prefers `pina.toml`, so nothing else changes; the legacy `Pina.toml` spelling remains discovered with a deprecation warning for existing projects.
