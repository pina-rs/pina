---
default: note
pina: note
---

Harden CI setup reliability by adding retries to the shared `./.github/actions/devenv` action for transient Nix/devenv failures.

Also increase workflow timeouts for `release-preview`, `semver`, and `binary-size` so slow cold-cache environment provisioning does not cancel jobs before they execute their main steps.
