---
pina: none
---

# Bound the symbolic compact proofs to their reachable loops

Three compact-layout Kani harnesses kept an unwind bound of 32 even though their two-element tails only reach loops that complete within eight unwindings. Pinapod 0.4.0 changed array validation from a trivial `[u8; N]` check to a generic element-wise loop, so CBMC replicated the new validation path across the larger bound. One symbolic update proof then exceeded the entire 50-minute CI budget.

The three symbolic state proofs now use an unwind bound of eight. Kani's unwinding assertions still prove that every reachable loop completes, all 16 compact harnesses pass, and the full compact suite finishes within the CI budget.
