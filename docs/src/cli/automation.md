# Automation and Agent Usage

The CLI help and stream contracts are designed to let an agent discover capabilities without reading repository source.

## Discovery protocol

Use this sequence before constructing a command:

```bash
pina --version
pina --help
pina <command> --help
```

For Codama, inspect both levels:

```bash
pina codama --help
pina codama generate --help
```

For framework and extractor constraints:

```bash
pina docs
pina docs pina-overview
pina docs pina-idl
```

Do not infer flags from examples alone. The command-specific `--help` output is the authoritative interface and includes defaults, output routing, requirements, and examples.

## Machine-readable workflows

Generate and validate an IDL through stdout:

```bash
pina idl --path ./programs/counter_program --compact > /tmp/counter.json
jq -e . /tmp/counter.json
```

Write directly to a known artifact path:

```bash
mkdir -p ./artifacts
pina idl \
  --path ./programs/counter_program \
  --output ./artifacts/counter.json
```

Profile a binary as JSON:

```bash
pina profile ./target/deploy/counter_program.so --json > /tmp/profile.json
jq -e '.functions | type == "array"' /tmp/profile.json
```

## Automation rules

- Check the exit status before consuming output.
- Treat stderr as diagnostics and progress, not as part of IDL JSON.
- Use explicit paths; relative paths depend on the process working directory.
- Create the parent of an `idl --output` file before invoking the command.
- Treat Codama output roots as replaceable generated directories.
- Use repeated `--example` flags instead of assuming comma-separated parsing.
- Inspect `pina docs` before requesting a topic.
- Never use the input `.so` path as the profile output path.

## Stable verification

CLI help is snapshot-tested at every command level. IDL stdout is also regression-tested as valid JSON. The book is built by `verify:docs` and published by the repository's GitHub Pages workflow, so command changes should update help tests and this reference in the same change.
