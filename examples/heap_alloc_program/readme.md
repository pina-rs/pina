# `heap_alloc_program`

<br>

The opt-in heap allocation entrypoint.

## What this demonstrates

<br>

- `nostd_entrypoint_alloc!` instead of `nostd_entrypoint!`: the only difference is that it installs `pinocchio::default_allocator!` — a `BumpAllocator` over the runtime heap region — rather than `no_allocator!`.
- `extern crate alloc` in a `no_std` program, giving `Box` and `Vec`.
- A heap round-trip that only confirms when the value read back through the heap matches what was written.
- Heap-frame sizing, including what happens when an allocation exceeds the frame the runtime granted.

Every other example denies allocation, because `no_allocator!` is the default. This one exists to show what changes when a program opts in.

## Instructions

<br>

| Variant    | Description                                                                           |
| ---------- | ------------------------------------------------------------------------------------- |
| `Allocate` | Box a `u64`, read it back through the heap, and reject the transaction on a mismatch. |
| `Fill`     | Allocate, fill, and sum a heap buffer of a caller-chosen size.                        |

## What the tests prove

<br>

`tests/heap_frames.rs` runs the compiled SBF artifact through `mollusk-svm`, which exposes the compute budget directly, so it can hold the heap frame at a fixed size:

- The runtime's default frame is exactly 32 KiB.
- A fill inside the frame completes; a fill past it aborts.
- Raising the frame makes the same fill succeed, so the frame — not the program — decides the outcome.
- The allocator keeps its position in the first word of the region, so an allocation sized to the raw frame fails. The usable bytes are the frame minus one word.

`tests/surfpool/src/lib.rs` runs the same artifact on a real runtime through Surfpool, which is where the bump allocator is genuinely the one under test.

## Costs of the heap

<br>

- **Never reclaimed.** `dealloc` is a no-op, so a transaction's peak heap use is the sum of every allocation it makes, not the sum of the ones still alive.
- **Aborts on exhaustion.** An allocation the frame cannot satisfy panics inside `alloc`; there is no `ProgramError` to handle.
- **A caller-side requirement.** More than 32 KiB needs the transaction to request a larger frame, and only the transaction that asks gets it.
- **Transaction-scoped.** The heap is not persistent storage: nothing allocated is visible to another transaction, so data that must outlive the instruction belongs in account data.

## Run

<br>

```bash
devenv shell -- cargo test -p heap_alloc_program
devenv shell -- cargo-build-sbf --manifest-path examples/heap_alloc_program/Cargo.toml \
    --features bpf-entrypoint --sbf-out-dir target/deploy
devenv shell -- cargo test -p heap_alloc_program --test heap_frames
```
