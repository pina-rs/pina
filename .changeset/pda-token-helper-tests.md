---
pina: test
---

# Test the public PDA and token-account helpers directly

`find_program_address`, `try_find_program_address`, and `try_get_associated_token_address` are now checked against addresses and bumps derived independently with `@solana/kit`, including a canonical bump of 250 and the Token-2022 associated address, and against seeds too long or too many to derive. `TokenAccountRef`'s state, native-reserve, and field readers are checked for both token programs on accounts whose fields all hold distinct values, covering initialized, frozen, and uninitialized accounts and native and non-native reserves.
