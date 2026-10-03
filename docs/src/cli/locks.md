# `pina locks`

Report which instructions can never run in parallel, and the accounts that make them wait.

```text
pina locks [OPTIONS]
```

```bash
pina locks
pina locks --project ./programs/privacy_pool
pina locks --json
pina locks --deny-hotspots
```

## Why write locks matter

Solana's scheduler runs two transactions in parallel only when neither one write-locks an account the other one locks. Every account a transaction marks writable is locked for the whole transaction, and every other transaction that touches that account waits.

Most accounts are chosen per user, so their locks rarely collide. A PDA whose seeds are all constants is different: it has the same address in every transaction. Every instruction that writes it takes the same lock, so all of that instruction's traffic across the cluster runs one transaction at a time, however many users send it.

Pina knows this before the program is deployed. The IDL extractor records which accounts each instruction writes and which of them are PDAs, and `pina locks` reads that record without building or running the program.

## Address classes

Every instruction account is grouped into an account node and given one of three classes:

| Class    | Address                                                                                                                                                            | Unified across instructions               |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------- |
| `fixed`  | One address for every caller: a PDA whose seeds are all constants, or a known address such as a program or sysvar. The report includes the derived base58 address. | Yes, by PDA name or by address            |
| `keyed`  | A PDA with variable seeds. Two transactions lock the same account only when they pass the same seed values. The report lists the seed names and types.             | Yes, by PDA name                          |
| `caller` | Any account the caller chooses.                                                                                                                                    | No: each instruction slot is its own node |

A slot is a PDA when its processor validates or derives it as one, or loads it as a `#[pda]` account type, as [Generate an IDL](./idl.md) describes.

## Reading the report

The text report starts with **hotspots**: fixed accounts that at least one instruction writes. Each lists its derived address and seeds, the instructions that write it, the instructions that only read it, and what that costs:

```text
merkle_tree
  address  BLquaQVntisnQpUoLpzbcFGDN9eG9Qf12TZZS5KVLaLz
  seeds    "privacy-pool-tree"
  writers  initialize, deposit, transfer
  readers  withdraw
  Every `initialize`, `deposit`, and `transfer` in the cluster runs one at a time; `withdraw` waits for each one.
```

A program without hotspots says so.

Next comes the **conflict matrix**. Rows and columns are the instructions, numbered in declaration order. The diagonal tells you whether an instruction conflicts with other transactions of itself.

| Symbol | Kind     | Meaning                                                                                                                                                    |
| ------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `●`    | `always` | Both lock the same fixed account and at least one writes it. They never run in parallel.                                                                   |
| `◐`    | `may`    | Both lock the same keyed PDA, or a fixed account one of them may omit, and at least one writes it. They wait for each other only when the addresses match. |
| `·`    | none     | Pina can prove no shared write lock.                                                                                                                       |

`none` is not a promise of parallelism. Accounts the caller chooses are never compared, because two transactions may or may not pass the same one, and nothing in the program decides that. A program with too many instructions for a readable matrix gets a per-instruction list instead.

## Fixing a hotspot

A hotspot is a design decision, not a bug, but it is usually an accidental one. Common fixes:

- **Shard the account.** Add a variable seed, such as a shard index or the user's address, so writers spread over many accounts. A global counter becomes per-shard counters that a reader sums.
- **Split hot fields out.** Move the fields every user writes into per-user or per-position accounts, and keep the global account for values that rarely change.
- **Make readers read-only.** An instruction that only reads a fixed account should take it as a read-only account. Two readers never conflict; a slot declared `&mut AccountView` is writable even when the handler never writes it.
- **Keep the singleton and say so.** An admin configuration written only by admin instructions is a fine hotspot. Accept it in `pina.toml` so CI stops flagging it.

## Accepting a hotspot

List intentional hotspots by account name under `[locks]` in `pina.toml`:

```toml
[locks]
allow = ["program_config"]
```

Allowed hotspots are still reported, marked `(allowed in pina.toml)`. Every name must be a current hotspot of the program: a name that matches none, from a typo or a hotspot that was since removed, fails the command with the list of hotspots it could have named, so the allow list cannot silently go stale.

## CI gate

```bash
pina locks --deny-hotspots
```

`--deny-hotspots` prints the same report, then exits with code `1` when any hotspot is not allowed. Errors such as a missing project, unparseable source, or an unknown allow entry also exit with code `1` and print the error to stderr.

## Agent JSON

```bash
pina locks --json > locks.json
jq -e '.hotspots | map(select(.allowed | not)) | length == 0' locks.json
```

JSON is the only stdout content. Schema version `1` is a camelCase document:

- `schemaVersion`, `program`, and `programId`;
- `nodes`: every account node with its `id` (`pda:<name>`, `address:<base58>`, or `caller:<instruction>.<slot>`), `name`, `class` (`fixed`, `keyed`, or `caller`), `address` (or `null`), `pda` (or `null`), and `seeds`, each `{"kind": "constant", "hex", "text"}` or `{"kind": "variable", "name", "type"}`;
- `instructions`: each instruction's `writes` and `reads`, every entry naming its `node`, `slot`, `signer`, and `optional` flags;
- `conflicts`: each conflicting pair as `instructions` (declaration order, the same name twice for a self-conflict), `kind` (`always` or `may`), and the `nodes` that decide it;
- `hotspots`: each fixed account some instruction writes, with its `node`, `name`, `writers`, `readers`, and `allowed`.
