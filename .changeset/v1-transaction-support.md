---
pina_test: feat
---

# Build and execute v1 transactions in Surfpool tests

`pina_test` can now construct, sign, simulate, and submit v1 transactions — the format SIMD-0385 introduced to raise the per-transaction limit from 1,232 to 4,096 bytes:

```rust
use pina_test::{ProgramTest, default_v1_config};

program
	.send_v1(&[instruction as u8], accounts, default_v1_config())
	.expect("v1 event instruction confirms");
```

`default_v1_config` applies the compute unit limit and loaded accounts data size limit that legacy transactions get from their compute-budget instructions. V1 moves those limits into the message, so a v1 transaction that wants a legacy-equivalent budget has to state it.

The transport needed its own path. V1 messages have no serde representation, and `solana_rpc_client` serializes transactions through `Serialize`, so a v1 transaction submitted through `simulate_transaction` or `send_and_confirm_transaction` reaches the node as bytes it cannot parse. The harness instead encodes the message plus its trailing signature array in the runtime's own format and submits it base64-encoded through a generic JSON-RPC request. With that, an instruction that confirms as a legacy transaction also confirms as a v1 transaction, which the new `events_program` Surfpool test asserts.
