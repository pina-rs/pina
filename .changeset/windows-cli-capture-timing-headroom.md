---
pina_cli: fix
pina: none
# `pina_root` propagates from every core-group change; unpublished, so no bump.
pina_root: none
---

# Widen the headroom in the bounded-capture timing assertion

`doctor::tests::command_capture_bounds_pipes_held_by_descendants` asserts that a capture returns before it would have waited out the descendant holding the inherited pipes. It proved the bound with a 100 ms deadline, a descendant sleeping 500 ms, and a ceiling of 400 ms on the whole call, which leaves 300 ms for the process spawn the capture also pays for. A Windows runner took longer than that to start the test binary, so the job failed on `main` while the identical tree passed the same job on the pull request.

The descendant now sleeps 2 s and the ceiling is 1 s, so the assertion still separates a bounded capture from one that waited the holder out — the 900 ms gap between them is what the check depends on — while leaving the spawn several hundred milliseconds of room. The deadline itself is unchanged at 100 ms, so nothing about the behavior under test is relaxed: mutating the capture to ignore its deadline still fails the assertion, which was verified by trying it.

The assertion message now reports the measured time, the ceiling, and the holder's sleep, so a future failure says which of the three moved.
