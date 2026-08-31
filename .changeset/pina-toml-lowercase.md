---
pina_cli: feat
---

# Prefer the lowercase `pina.toml` config file

The canonical project configuration file name is now `pina.toml`. `pina init` writes `pina.toml`, and project discovery prefers it. The legacy uppercase `Pina.toml` spelling is still discovered, but selecting it prints a deprecation warning asking for the rename, and `pina.toml` always wins within the same directory.
