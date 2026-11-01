# @pina-rs/cli

<p align="center">
	<img
		src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png"
		alt="Pina: a pineapple made from interlocking ribbons"
		width="140"
	>
</p>

Prebuilt `pina` binaries for macOS, Linux, Windows, and FreeBSD.

<!-- {=npmReadmeBadgeRow:"@pina-rs/cli"} -->

<p align="center">
	<a href="https://www.npmjs.com/package/@pina-rs/cli"><img alt="npm" src="https://img.shields.io/npm/v/@pina-rs/cli?logo=npm&style=flat-square"></a>
	<a href="https://github.com/pina-rs/pina/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/pina-rs/pina/ci.yml?label=CI&logo=githubactions&style=flat-square"></a>
	<a href="https://opensource.org/license/apache-2.0"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue?logo=apache&style=flat-square"></a>
</p>

<!-- {/npmReadmeBadgeRow} -->

## Install

<br>

```sh
npm install --global @pina-rs/cli
pina --help
```

Then scaffold, build for SBF, and run tests in one loop:

```sh
pina init my_program
pina build
pina test --unit
```

The package installs a small Node.js launcher and the native package for the current operating system, CPU, and Linux C library. It does not compile Rust during installation.

The Rust-native alternative is:

```sh
cargo install pina_cli
```

See the [CLI reference](https://pina.rs/cli/index.html) for commands, output contracts, and automation guidance.
