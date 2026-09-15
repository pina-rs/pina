---
pina: fix
---

# Build assertion messages without the formatting logger

`pina::assert` logged its message through the always-formatting `log_verbose!` arm, which assembles the message with a stack `Logger` buffer before writing it. When `verbose-logs` is off the failure path now hands the caller's `&str` straight to the log syscall, so the message reaches the log without an intermediate buffer. With `verbose-logs` on nothing changes: the formatted message and caller location are still written, and the built binary is byte-identical to before.

The saving is small — a few hundred bytes in a program that actually reaches the path — because the logger never linked `core::fmt` in the first place. With `logs` enabled the message is still logged in every configuration; with `logs` off entirely nothing is logged, exactly as before. Only how the message is assembled changed.
