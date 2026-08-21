# @pina-rs/cli

<p align="center">
	<img src="https://raw.githubusercontent.com/pina-rs/pina/main/.github/assets/logo.png" alt="The Pina logo: a low-poly origami pineapple" width="140">
</p>

Prebuilt `pina` binaries for macOS, Linux, Windows, and FreeBSD.

## Install

```sh
npm install --global @pina-rs/cli
pina --help
```

The package installs a small Node.js launcher and the native package for the current operating system, CPU, and Linux C library. It does not compile Rust during installation.

The Rust-native alternative is:

```sh
cargo install pina_cli
```

See the [CLI reference](https://pina-rs.github.io/pina/cli/index.html) for commands, output contracts, and automation guidance.
