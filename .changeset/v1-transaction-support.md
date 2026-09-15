---
pina_test: feat
---

# Build and execute v1 transactions in Surfpool tests

`pina_test` can now construct, sign, simulate, and submit v1 transactions — the format SIMD-0385 introduced to raise the per-transaction limit from 1,232 to 4,096 bytes:

```rust
use pina_test::{ProgramTest, TransactionFormat, default_budget};

let logs = program
	.simulate_transaction_logs(&[instruction as u8], accounts, &TransactionFormat::V1(default_budget()))
	.expect("simulate program instruction");

program
	.send_transaction(&[instruction as u8], accounts, &TransactionFormat::V1(default_budget()))
	.expect("v1 instruction confirms");
```

`default_budget` states the compute unit limit and loaded accounts data size limit that legacy transactions receive from their compute-budget instructions. V1 moves those limits into the message, so a v1 transaction that wants a legacy-equivalent budget has to declare them. `TransactionFormat::Legacy` keeps the original path.

The transport needed its own path. V1 messages have no serde representation, and `solana_rpc_client` serializes transactions through `Serialize`, so a v1 transaction submitted through `simulate_transaction` or `send_and_confirm_transaction` reaches the node as bytes it cannot parse. The harness instead encodes the message plus its trailing signature array in the runtime's own format and submits it base64-encoded through a generic JSON-RPC request, then waits for confirmation. An instruction that confirms as a legacy transaction confirms as a v1 transaction too.

One finding is recorded rather than worked around: the Surfpool runtime that CI exercises rejects the v1 config mask this client train encodes (`invalid transaction config mask`), while the local runtime accepts it. The construction, signing, and wire-encoding path is covered hermetically in `pina_test` — including that a v1 transaction carries instruction data past the 1,232-byte legacy limit while staying inside the 4,096-byte v1 limit — so the portability gap is pinned by a test instead of a passing doc example.
