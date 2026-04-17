# Security Policy

## Current status

Pina is **pre-1.0, unaudited, and not yet ready for production deployments that hold real funds**.

Use it for experimentation, internal prototyping, education, and controlled testing. If you deploy a program built with Pina today, you should still assume that:

- independent security review is required,
- application-level audits are required,
- framework hardening work is still in progress.

Current hardening work is tracked openly in the repository, including the umbrella effort in #119 and follow-up items such as #120, #121, #122, #127, and #130.

Supported does **not** mean audited. It only means where security fixes are expected to land.

## What Pina aims to make safer

Pina tries to make several low-level Solana patterns more explicit and easier to audit:

- signer, owner, writable, sysvar, and PDA validation through chained `AccountView` assertions,
- discriminator-first account and instruction layouts,
- zero-copy account access for fixed-size POD types,
- typed helper APIs for common CPI, token, lamport, and account-lifecycle operations,
- repository-level security examples and negative test fixtures under `security/`.

## What Pina does not guarantee for you

Pina cannot make an application secure by itself. Program authors are still responsible for:

- authority design and signer relationships,
- business-logic correctness,
- economic invariants and value conservation,
- CPI allowlists and external program trust decisions,
- PDA seed design and account namespace separation,
- upgrade authority, deployment, and operational controls,
- state migrations, realloc strategy, and backward compatibility,
- reviewing generated clients, examples, and tutorial code before production use.

Treat framework helpers as building blocks, not as a substitute for protocol-specific review.

## Known limitations and active hardening areas

The current repository already tracks several security-focused follow-ups. Important examples include:

- loader borrow-soundness hardening for typed account access: `#120` and `#121`,
- dedicated Miri regression coverage for loader and token-helper paths: `#122`,
- adversarial regression coverage for helper and account invariants: `#127`,
- architecture documentation for zero-copy safety and runtime invariants: `#130`.

For deeper technical context, see:

- [`security/loaders-audit.md`](security/loaders-audit.md) — focused audit notes for the account-loader layer,
- [`security/readme.md`](security/readme.md) — example-driven security guide organized by vulnerability class,
- [`docs/src/security-model.md`](docs/src/security-model.md) — high-level framework security model and guardrails.

## Supported versions

| Branch or release line | Status      | Notes                                                        |
| ---------------------- | ----------- | ------------------------------------------------------------ |
| `main`                 | Supported   | Security fixes and hardening work land here first.           |
| Latest published `0.x` | Best effort | Upgrade quickly; prompt backports are not guaranteed.        |
| Older releases         | Unsupported | No maintenance branches or routine security backport policy. |

Because the project is still unaudited, the safest option is to track the latest code and review the current open hardening work before any serious deployment decision.

## Reporting a vulnerability

**Do not open a public GitHub issue for an undisclosed vulnerability.**

Please report sensitive findings privately by email:

- **Email:** `ifiokotung@gmail.com`
- **Subject suggestion:** `Pina security report: <short summary>`

Please include as much of the following as you can:

- affected crate(s), version(s), and commit SHA if known,
- impact and attacker assumptions,
- minimal reproduction steps or proof of concept,
- whether the issue is already public anywhere,
- any suggested fix or mitigation.

## Response and disclosure process

The intended process is:

1. Acknowledge receipt within a few business days.
2. Reproduce and assess severity.
3. Work on a fix and coordinate validation.
4. Release or merge the fix.
5. Publish a disclosure note or changelog entry once users have a remediation path.

Please avoid sharing exploit details publicly until maintainers confirm that a fix or mitigation is available.

## Security resources

- Repository security examples: [`security/readme.md`](security/readme.md)
- Loader audit notes: [`security/loaders-audit.md`](security/loaders-audit.md)
- Framework security model: [`docs/src/security-model.md`](docs/src/security-model.md)
- General issue tracker: <https://github.com/pina-rs/pina/issues>

If you are unsure whether something is a security issue, report it privately first.
