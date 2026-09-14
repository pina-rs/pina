# 11: Admin Key Compromise

<br>

## The Vulnerability

<br>

Most Solana programs concentrate every privileged action behind one authority key: a single signature can sweep treasuries, upgrade parameters, or redirect the whole program. The vulnerability is not a missing ownership check on one instruction — it is the absence of any on-chain mechanism that limits what one key (possibly fooled, phished, or malware-infected) can do in a single transaction, or ever.

Three failure modes recur in real incidents:

- **Leaked sweep key.** The 2022 Raydium compromise started from a trojan-infected operator machine whose private key controlled every pool's token custody; the attacker hid withdrawal instructions inside otherwise legitimate transactions. DEXX (2024, ~$21M) drained custodial user keys the same way. When one signature can move an account's entire balance, the key _is_ the exploit.
- **Fooled signing ceremonies.** Radiant Capital (2024, ~$53M) and Bybit (2025, ~$1.5B) lost funds to signers who approved transactions that looked benign on a compromised signing UI. Radiant had removed its timelock after an earlier incident, so three spoofed signatures executed immediately. Drift (2026, ~$285M) lost control when two council members pre-signed durable-nonce transactions with no timelock between approval and execution.
- **Lost or irreversible keys.** The mirror image: a key that is lost freezes the program, and an unguarded privileged action can destroy it. OptiFi (2022) bricked its own program with a single `program close` during a routine upgrade.

The on-chain mitigations that real protocols converged on (Mango v4's instruction gates and net-borrow window limits, Drift's per-market pause bitmask, MarginFi's expiring panic pause, BPF-loader `SetAuthorityChecked`) all share one property: **they bound the blast radius of a single key inside the program itself**, where no UI, host, or operator compromise can bypass them.

## Insecure Example

<br>

See [`insecure/src/lib.rs`](insecure/src/lib.rs). The vault has two privileged instructions and no containment:

- `sweep` moves the vault's **entire** lamport balance to an arbitrary recipient. There is no pause flag, no withdrawal cap, and no second key that can halt the drain.
- `rotate_authority` overwrites the stored authority with any address in one step, so one signature permanently redirects every privileged path — and there is no recovery path if the current key is simply lost.

## Why This Is Dangerous

<br>

An attacker who obtains the authority key once can:

- Drain every depositor's funds in the same transaction they first sign
- Rotate authority to an address they control and keep the program
- Repeat both against any account the program owns

A user or co-signer has no on-chain window to notice, pause, or exit between the compromised signature and the drained funds.

## Secure Example

<br>

See [`secure/src/lib.rs`](secure/src/lib.rs). The same vault, rebuilt so no single key can drain it:

- **Circuit breaker.** `sweep` moves at most the remaining allowance for the current withdrawal window (a cap plus a Clock-driven window reset, in the shape of Mango v4's per-bank net-borrow limits). The vault can never empty in one instruction.
- **Asymmetric pause.** A dedicated `guardian` key — able to pause the program, unable to sweep, rotate, or unpause alone — contains an in-progress key compromise. Unpausing requires dual control (guardian + authority), so neither leaked key can resume the program on its own.
- **Two-phase rotation.** `propose_authority` records a candidate; the rotation only takes effect when the candidate itself signs `accept_authority` (the application-level equivalent of `SetAuthorityChecked`). One fooled signing ceremony can no longer hand the program to an attacker.

The design rule the Solana Operational Security Standard states and this example follows: pause switches are risk-reducing only, and every privilege that can move funds must be bounded by state the program — not the signer — enforces.

## Pina API Reference

<br>

- `AccountView::send_owned(program_id, lamports, recipient)` — lamport transfer used by both sweep paths; the insecure version passes `vault.lamports()` (full balance), the secure version passes the capped amount
- `sysvars::clock::Clock::from_account_view(&AccountView)` — reads the Clock sysvar for the circuit-breaker window reset
- `AccountView::assert_sysvar(&Address)` — verifies the passed Clock account is the real sysvar
- `assert_signer()` / `assert_address()` — the authority, guardian, and pending-authority proofs behind each privileged instruction
