---
pina_cpi_renderer: fix
---

# Keep untrusted IDL names inside generated doc comments

`CamelCaseString` derives `Deserialize` on its inner `String`, so the normalization its constructor applies never runs for JSON input: a name read from an IDL reaches the renderer verbatim, newlines included. Two sites interpolated such a name straight into a single-line `///` comment — the CPI account line in `render_account` and the argument line in `render_argument` — so everything after an embedded newline was emitted as uncommented Rust. A crafted IDL handed to `pina cpi --idl` produced a generated crate containing attacker-chosen items, which then compile (and execute under `#[cfg(test)]`) in the victim's workspace.

Both sites now route through `render_doc`, which splits on newlines and prefixes every line as a comment, so an embedded newline becomes another comment line instead of code. `pina_cpi_renderer` gains a regression test that deserializes a malicious IDL through the same `read_root_node` path `pina cpi` uses, asserts the newline survives parsing (so the test keeps exercising the real path), and fails if any payload line escapes its comment.
