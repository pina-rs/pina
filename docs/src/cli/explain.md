# `pina explain`

Explain why a transaction to the current program failed.

```text
pina explain <SIGNATURE> [--network localnet|devnet|testnet|mainnet | --rpc-url <URL>] [--project <DIR>] [--json]
pina explain --transaction-file <PATH> [--network ... | --rpc-url <URL>] [--project <DIR>] [--json]
```

```bash
pina explain 4MeRynoBFLczcN7jND2spt4Vvwn5K8kasDeG7vaJU7fjTDDA2YzwFkYUqzTGS4m5UQPFGEDbzqpTqncwwg1rVYVV
pina explain <SIGNATURE> --network devnet
pina explain --transaction-file ./failed.json --project ./programs/counter
pina explain <SIGNATURE> --json
```

A failed Pina check returns a bare error code, and several checks share one: `writable`, `address`, `executable`, `data_len`, and `distinct_from` all return `InvalidAccountData`. A distinct code per check would cost every program size and compute units, so Pina keeps the codes and moves the diagnosis off-chain. `pina explain` reads the failed transaction's account flags, error, and logs, matches them against the program's `#[derive(Accounts)]` structs and processors, and names the field and rule that most likely failed, with its `path:line`.

## What it reports

```text
Transaction 4MeRyno…VYVV (slot 62, from localnet)
Instruction #0 failed: check_policy (CheckAccounts) in validation_program
Error: InvalidAccountData

Most likely cause:
  audit: writable [confirmed, matches the program log] at src/lib.rs:221
    account #2 (EdmxWPmx2WH6WgFfTdu9xfkYf3k1g5wD1zccTVySEEh1) is read-only in this transaction

Accounts:
  #   field           signer  writable  address
  0   authority       yes     no        9hSR6S7WPtxmTojgo6GG3k4yDPecgJY292j7xrsUGWBu
  1   policy          no      no        GyGKxMyg1p9SsHfm15MkNUu1u9TN2JtTspcdmrtGUdse
  2   audit           no      no        EdmxWPmx2WH6WgFfTdu9xfkYf3k1g5wD1zccTVySEEh1
  3   system_program  no      no        11111111111111111111111111111111

Program logs (last 4 lines):
  …
```

- **The failing instruction**, identified by its discriminator, with the accounts struct its dispatch parses.
- **The decoded error.** Built-in runtime errors keep their names. Codes in Pina's reserved range decode to their `PinaProgramError` variant, and other custom codes to the program's `#[error]` variant with its first documentation line.
- **Ranked candidates**: the checks whose error matches the observed one, each with its field, rule, location, and a confidence class. The program stops at its first failing check, so a check that runs after one the transaction proves failing is left out.
- **Construction sites**: where the program writes the error (`ValidationError::InvalidAmount`, `ProgramError::InvalidArgument`), sites in the failing instruction's accounts struct, instruction struct, processor, and validation hooks first.
- **The account table**: each instruction slot with its field (`parent.child` for a nested struct, `members[0]` for a remaining slice), address, and the signer and writable flags the transaction gave it.
- **The failing instruction's last log lines.** Pina logs a message for the checks that share `InvalidAccountData`, such as `account has not been marked as writable`, and a candidate whose message appears is ranked first.

## Confidence

| Class                           | Meaning                                                                                                                                                                                                                                                  |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `confirmed`                     | Provable from the transaction alone: a missing signer or writable flag, too few or too many accounts, a mutable account repeated in a later slot, equal keys for `distinct_from`, or an address that differs from a known constant such as `system::ID`. |
| `checked_against_current_state` | `owner`, `executable`, `empty`, `not_empty`, and `data_len` evaluated against account state read after the transaction ran. The state may have changed since.                                                                                            |
| `possible`                      | Can return this error, but the check needs values pina cannot read offline: PDA seeds, value rules on instruction arguments, unresolved constants, or `with` hooks.                                                                                      |

A rule whose `validate(...)` group sets `error = ...` is matched by that error's code instead of the default.

## Inputs and network

A signature is fetched with one `getTransaction` request at `confirmed` commitment. The default network is **localnet** (`http://127.0.0.1:8899`); Pina never queries mainnet unless `--network mainnet` or `--rpc-url` names it. When the failing instruction targets the explained program, one `getMultipleAccounts` request then reads the current owner, executable flag, and size of its accounts. Neither request is retried, and a failed state lookup becomes a note instead of an error.

`--transaction-file` reads a saved `getTransaction` result, or the complete JSON-RPC response around one, and works offline. Add `--network` or `--rpc-url` to also check state-dependent rules.

A transaction rejected by preflight simulation never lands, so `getTransaction` cannot find it. Send with preflight disabled, or explain a transaction captured from a test validator, when you need the landed failure.

`--rpc-url` accepts credential-free HTTP(S) URLs with a host. User information, query parameters, fragments, and control characters are rejected, plaintext `http` is accepted only for a loopback host, and redirects are not followed. Reports and errors name a custom endpoint as `custom RPC endpoint` rather than echoing its URL.

## Limits

- The failing instruction must target the program in `--project`. A failure in another program is reported with its decoded error and accounts, and nothing more.
- A failure inside a CPI is attributed to the callee the logs name. The program's own checks ran before the call, so no candidate is listed.
- A field whose type is not a `#[derive(Accounts)]` struct of the program stops the account mapping at that field.
- Processor checks are found by name on `self.<field>` (`assert_signer`, `assert_owner`, `load_pda`, and the rest), including through local aliases. A check behind a helper function is not attributed to a field, but its error constructions are still listed.
- Writability demoted by the runtime, for example a reserved account marked writable, is read as the message declares it.

## Exit status and JSON

A produced explanation exits with code `0`, including for a transaction that succeeded, which prints that there is nothing to explain. An invalid signature, file, or URL, a project that cannot be parsed, a transaction that cannot be fetched or found, and a malformed transaction exit with code `1`. Conflicting or missing transaction sources are usage errors with code `2`.

```bash
pina explain <SIGNATURE> --json > /tmp/pina-explain.json
jq -e '.candidates[0].confidence' /tmp/pina-explain.json
```

JSON is the only stdout content. Schema version `1` contains `signature`, `slot`, `source` (`file` or the endpoint label), `program`, `status` (`succeeded` or `failed`), `failure` (instruction index, program, instruction, decoded `error` with `kind`, and `cpi`), ranked `candidates`, `errorSites`, `accounts`, `logs`, and `notes`. Each candidate has `scope` (`account`, `argument`, or `instruction`), `field`, `accountIndex`, `rule`, `location`, `confidence`, `logConfirmed`, and `reason`. Operational failures print only the error to stderr.
