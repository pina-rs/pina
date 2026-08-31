# @pina-rs/cli

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

Prebuilt `pina` binaries for macOS, Linux, Windows, and FreeBSD.

<!-- {=npmReadmeBadgeRow:"@pina-rs/cli"} -->

[![npm](https://img.shields.io/npm/v/@pina-rs/cli?logo=npm&label=npm)](https://www.npmjs.com/package/@pina-rs/cli) [![CI](https://github.com/pina-rs/pina/actions/workflows/ci.yml/badge.svg)](https://github.com/pina-rs/pina/actions/workflows/ci.yml) [![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://opensource.org/license/apache-2.0)

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

See the [CLI reference](https://pina-rs.github.io/pina/cli/index.html) for commands, output contracts, and automation guidance.
