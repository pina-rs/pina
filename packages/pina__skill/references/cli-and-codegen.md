# CLI and Code Generation

## Discover commands from help

The installed CLI is authoritative:

```sh
pina --help
pina idl --help
pina docs --help
pina init --help
pina profile --help
pina codama generate --help
```

Use `pina docs` to list bundled terminal topics. Custom topics can be supplied through `PINA_TEMPLATES_DIR` when a project maintains its own operational guidance.

## IDL extraction

Generate a Codama root-node document from a program crate:

```sh
pina idl --path ./programs/counter_program --output ./idls/counter_program.json
```

Without `--output`, JSON is the only stdout content; progress and extraction counts go to stderr. This makes the command safe in pipelines:

```sh
pina idl --path ./programs/counter_program --compact | jq -e '.program'
```

Treat the IDL as a public contract. Review instruction, account, PDA, error, and type changes rather than accepting generated churn wholesale.

## Client generation

Generate every selected client target with:

```sh
pina codama generate --examples-dir ./programs --idls-dir ./idls \
  --rust-out ./clients/rust --js-out ./clients/js --dart-out ./clients/dart
```

Use repeatable `--example` filters for a focused run. Generated roots may be replaced; never store hand-written code inside them.

Pina's generated clients preserve discriminator-first layouts and zeropod boundary checks. If a repository uses a custom renderer command, keep that command as the source of truth.

## Static SBF profiling

Profile a compiled shared object:

```sh
pina profile ./target/deploy/counter_program.so
pina profile ./target/deploy/counter_program.so --json --output ./profile.json
```

The report is a static estimate, not a validator execution trace. Use it for deterministic comparisons and investigate material changes in context.
