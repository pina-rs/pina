# `pina docs`

List or render reference topics in the terminal.

## Synopsis

```text
pina docs [TOPIC]
```

Run without a topic to list bundled topics:

```bash
pina docs
```

The binary currently bundles:

| Topic             | Contents                                                     |
| ----------------- | ------------------------------------------------------------ |
| `pina-idl`        | IDL extraction rules and supported Rust source shapes.       |
| `pina-overview`   | Framework concepts, crates, features, and workflows.         |
| `pina-validation` | Declarative rules, manual alternatives, and code generation. |

Render one with:

```bash
pina docs pina-idl
pina docs pina-overview
pina docs pina-validation
```

Markdown is rendered for the terminal rather than printed as raw source.

Each bundled topic is an mdt consumer of the repository's `templates/*.t.md` providers, so `docs:check` fails when a topic and the template it mirrors disagree.

## Custom topics

Set `PINA_TEMPLATES_DIR` to a directory containing `<topic>.t.md` files:

```bash
PINA_TEMPLATES_DIR=./team-docs pina docs deployment
```

For that example, Pina reads `./team-docs/deployment.t.md`. A custom file takes precedence over a bundled topic with the same name. If no matching custom file exists, Pina falls back to the bundled topic.

Bare `pina docs` lists bundled topics only; it does not scan the custom directory. This keeps discovery deterministic even when the environment points at a large template tree.

## Failure modes

An unknown topic returns a non-zero exit code and prints the bundled topic index. If `PINA_TEMPLATES_DIR` resolves a topic to a file that cannot be read, the command reports that path and fails.
