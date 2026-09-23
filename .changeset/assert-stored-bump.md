---
pina: feat
pina_macros: feat
pina_lints: feat
pina_cli: none
---

# Add assert_stored_bump for one-parse PDA validation

`#[pda]` accounts gain an `assert_stored_bump` method: the identical single-derivation address check `assert_seeds` performs, but taking a bump value the handler already parsed from the account instead of re-parsing the account to read it again. A handler that captures its state's fields in one `as_account`/`with_compact_account` pass — the shape every value-moving handler uses, since the seeds it validates against come from the same parse — can now validate the PDA without paying for the account a second time.

The provenance is the contract, and it is enforced rather than assumed: `require_canonical_bump_before_pda_write` blesses `assert_stored_bump` only when the bump argument resolves, through alias chains, to a parse of the same account. A bump taken from instruction data, a literal, or a different account fails the lint, so the method name cannot be used to launder an attacker-chosen bump. Tuple destructures and field reads now keep their alias provenance through the lint's fact collector, so the common capture shape (`let (maker, seed, bump) = { let state = account.as_account()?; ... }`) is recognized.

Adopted in the four handlers whose seeds and bump come from one parse: vesting `Claim` and `Cancel`, escrow `Take` and `Cancel`, saving ~100 compute units per instruction (claim 20,642 to 20,544, cancel 27,722 to 27,624, take 32,354 to 32,254, escrow cancel 13,972 to 13,872) with the identical runtime check — proven by the foreign-vault and noncanonical-account adversarial suites, which still reject every substitution.
