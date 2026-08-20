# `pina codama generate`

Generate Codama IDLs and Rust, JavaScript, and Dart clients for a directory of Pina programs.

## Synopsis

```text
pina codama generate [OPTIONS]
```

| Option                 | Default               | Meaning                                               |
| ---------------------- | --------------------- | ----------------------------------------------------- |
| `--examples-dir <DIR>` | `examples`            | Directory whose child directories are program crates. |
| `--idls-dir <DIR>`     | `codama/idls`         | Generated IDL JSON directory.                         |
| `--rust-out <DIR>`     | `codama/clients/rust` | Generated Rust client root.                           |
| `--js-out <DIR>`       | `codama/clients/js`   | Generated JavaScript client root.                     |
| `--dart-out <DIR>`     | `codama/clients/dart` | Generated Dart package root.                          |
| `--example <NAME>`     | all programs          | Select a program. Repeat for multiple programs.       |
| `--npx <COMMAND>`      | `npx`                 | Command used to launch pinned Codama renderers.       |

## Generate all programs

```bash
pina codama generate
```

Pina sorts discovered program names for deterministic output. With no `--example` flags, every child directory below `--examples-dir` is selected.

## Generate a subset

```bash
pina codama generate \
  --example counter_program \
  --example todo_program
```

Repeated selections are deduplicated and sorted. An unknown name fails before generation and reports the available names.

## Pipeline

For every selected program, the command:

1. extracts and writes a pretty Codama IDL;
2. renders a Pina-style Rust client;
3. validates Dart semantic compatibility;
4. invokes pinned Codama JavaScript and Dart renderers;
5. adds Pina wire-boundary validation to generated JavaScript;
6. writes Dart package barrel exports.

The output roots are created automatically. Program-specific generated output may be replaced by the renderers, so treat these directories as generated artifacts rather than hand-edited source.

## Renderer command behavior

The default `npx` path downloads or reuses pinned renderer packages. If the default executable is missing, Pina attempts `pnpm dlx` as a fallback.

Pass `--npx node` to run the render script directly when the required JavaScript packages are already resolvable in the active Node environment. Other custom values are invoked with the same npx-compatible argument shape.

Network access may be required when the renderer packages are not already cached.

## Custom layout

```bash
pina codama generate \
  --examples-dir ./programs \
  --idls-dir ./generated/idls \
  --rust-out ./generated/rust \
  --js-out ./generated/js \
  --dart-out ./generated/dart
```

## Failure modes

Generation fails for a missing or empty program directory, unknown selections, IDL extraction errors, output-directory or file errors, unsupported renderer schemas, missing launcher commands, and non-zero renderer exits. The diagnostic identifies the failing program, path, or command.

See [Codama Workflow](../codama-workflow.md) for wire-format guarantees and repository verification.
