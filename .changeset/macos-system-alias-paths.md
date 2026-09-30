---
pina_cli: fix
# `pina_root` owns the whole repository path, so it also claims these
# `crates/pina_cli` files; it is unpublished, so no bump is recorded.
pina_root: none
---

# Accept macOS system path aliases in link checks

`pina keys new`, `pina profile --output`, migration storage, and generated-client output checks no longer refuse paths that pass through a root-owned system alias directly below the filesystem root, such as macOS's `/tmp` and `/var` links into `/private`. Keypair generation for a project under `/tmp`, or in any macOS temporary directory, previously failed with `refusing non-regular, symbolic-link, or reparse-point destination`.

Every other symbolic link or reparse point in a destination path is still refused, whoever owns it. Generated-client output checks previously trusted any root-owned link at any depth, so a command running as root trusted a project's own committed links; they now apply the same stricter rule.
