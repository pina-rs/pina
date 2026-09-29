---
pina_cli: fix
# `rustfmt.toml` belongs to `pina_root`, which is unpublished, so its
# `format_strings` change is recorded without a bump.
pina_root: none
---

# Make fresh `pina init` projects and `pina deploy` work

- `pina init` scaffolded a nightly no Pina release ships lint drivers for, so `pina lint` could never succeed on a new project. The scaffold now pins the workspace toolchain (with `clippy`), and a test keeps the two in step.
- TypeScript and Dart generation failed in any project without a local `codama` install, because the stdin render script could not resolve `npx -p` packages. It now loads the pinned packages from the `npx`/`pnpm dlx` install.
- Freshly scaffolded Rust, CPI, and `cli-rust` clients inherited dependencies from a workspace a `pina init` project does not have, so they failed to parse. Outside a workspace that declares `pina`, their manifests now name concrete, tested versions and their own `[workspace]`. The program scaffold itself no longer declares a `[workspace]` table, so it can live inside an existing workspace.
- `pina init` rejects names Cargo or Rust cannot use (a leading digit or `-`, Rust keywords, more than 64 characters), prints `pina keys new` in its next steps, and warns about the git-ignored program keypair. `pina doctor` and `pina migrations create` warn while `declare_id!` is still the shared placeholder. The scaffold's Surfpool test no longer has doubled braces.
- `pina deploy` discards the pending publication it just wrote when the deploy program never started (a program whose exit could not be observed keeps it), so a missing `solana` no longer forces `--abandon` to freeze draft versions. The input snapshot directory is owner-only, a mistyped cluster name lists the accepted names, and the plan warns when the program keypair is also the upgrade authority or the artifact is not an ELF file.
- `pina keys sync` names `pina keys new` when no keypair exists, the `--build-driver` failure no longer prints an invalid `rustup --toolchain` argument, and `pina verify submit`/`status --help` and several migration error messages no longer carry stray tabs and backslashes.
- The repository's rustfmt configuration no longer sets `format_strings`. It split long string literals in the middle of escape sequences, which corrupted several help texts and error messages and made one test's source rewrite a silent no-op. Existing code is unaffected; generated Rust clients now keep long string literals on one line.
