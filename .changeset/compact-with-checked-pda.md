---
pina: breaking
pina_cli: feat
pina_macros: breaking
---

# Split the compact PDA loader by bump verification

A compact account with `#[pda(..., bump = bump)]` now generates two closure-scoped loaders instead of one, because the single loader could not serve both a fast read and a trustless one.

`Type::with_pda` keeps its name and derives the address once from the account's own stored bump, so it performs a single `create_program_address` instead of searching bump values from 255 down. The search was the most expensive part of loading a compact PDA, and the common case already had the bump on hand: every compact PDA this program creates went through `CreateCompactProgramAccount` or `CreateCompactProgramAccountWithBump`, and both reject a noncanonical bump, so the stored field is canonical for accounts the program itself wrote.

The new `Type::with_checked_pda` keeps the previous behavior exactly — it searches the seeds for the canonical bump and rejects both an account at any other address and a stored bump that is not that canonical bump. It is the only compact loader that rejects a shadow account.

The distinction matters because a stored-bump check cannot prove namespace uniqueness on its own. A creation instruction that accepts a bump proves only that the supplied bump derives the account's address; an attacker who calls it again with a different valid bump derives a second, still-empty address, so the emptiness check passes and two accounts exist for one logical seed namespace. Canonical derivation finds one of them, and the other is invisible to it. Only a canonical search on read rejects the second one.

Choose between them by who picks the account. Use `with_pda` when the address is already established — a per-signer namespace whose handlers require that signer — and `with_checked_pda` when an untrusted caller chooses which account the handler loads, or when the program must be certain exactly one address exists for the seeds. A caller that relied on `with_pda` rejecting noncanonical bumps should switch to `with_checked_pda`; the change is a rename for that case, not a rewrite.

The CLI's validation parser recognizes `with_checked_pda` as a PDA loader alongside `with_pda`, so IDL account properties are unchanged either way.

Three examples are fixed alongside the loader change, because each created a PDA that a noncanonical bump could duplicate:

`staking_rewards_program` takes a bump for its pool and its positions, and its pool seeds are `[pool, stake_mint, reward_mint]` — no signer. `InitializePool` requires the caller to sign, but that signature authorizes the creator and not the namespace, so anyone could pass a noncanonical bump and create a shadow pool for an existing mint pair with themselves as admin, at an address canonical derivation never returns. Every read validates stored fields rather than the seeds, so the shadow pool was fully functional; its vaults are ATAs of its own address, so the attacker controlled them outright. Position seeds are `[position, pool, owner]`, and a duplicate position doubled the reward accrual because the math is flat per position with no pro-rata term. Both now use `CreateProgramAccountWithBump`.

`pina_bpf_program`'s `CreatePda` creates from the global `[SEED_STATE_PREFIX]` with no signer at all, so a noncanonical bump minted unlimited shadow "State" singletons. It now uses `CreateProgramAccountWithBump`.

The remaining examples that pass a bump to `CreateProgramAccountWithUncheckedBump` are creator-gated: their seeds bind the signer the handler requires, so a duplicate is confined to the namespace of the party who could already create it, and no third party can be substituted into a later read. Each site now carries a comment recording that argument.

Two adversarial tests are added for the staking program and one for `pina_bpf_program`, and each was confirmed to fail against the previous code before the fix landed. The existing `pina_bpf` test paired the canonical address with a wrong bump, which even a single-derivation check rejects; it never covered a _valid_ noncanonical bump's own address, which is the actual shadow.
