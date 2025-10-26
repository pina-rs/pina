# 🧷 pina

> A performant smart contract framework for on chain solana programs.

### Description

`pina` provides a comprehensive toolkit for developing, building and deploying optimized, performant programs to the solana blockchain. It uses pinocchio to massively reduce the dependency bloat and compute units required for your code to run.

### Ideology

- macros are minimal syntactic sugar to reduce repetition of code.
- idl generation is automated based on code you write, rather than annotations. So `payer.assert_signer()?` will generate an idl that specifies that the account is a signer.
- everything in rust from the on chain program to the client code used on the browser, this project strives to make it possible to build everything in your favourite language.

### Examples

You can take a look at the [examples available](examples/escrow_program).
