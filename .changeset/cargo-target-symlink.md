---
pina_cli: fix
---

# Accept a Cargo target directory that is a symbolic link

Cargo's own `target_directory` is commonly a symbolic link, as mr boxington, CI caches, and tmpfs layouts arrange. Pina now accepts the path Cargo reports, including links along it, while paths `pina.toml` configures still may not traverse symbolic links.
