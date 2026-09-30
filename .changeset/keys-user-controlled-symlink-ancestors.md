---
pina_cli: fix
# The source change belongs to `pina_cli`; `pina_root` covers it only because
# the unpublished root harness owns the repository path, so it is recorded
# without a bump.
pina_root: none
---

# Accept root-owned symlinked ancestors in keypair paths

- `pina keys` generation and source sync refused any destination whose path traversed a symbolic link, including stable root-owned aliases such as macOS's `/var` link, so generating a keypair under the system temporary directory failed with `refusing non-regular, symbolic-link, or reparse-point destination`. Keypair destinations and source writes now use the same user-controlled-link check as generated-client output: symlinks the invoking user can replace are still refused, while root-owned aliases that cannot be swapped out are accepted.
