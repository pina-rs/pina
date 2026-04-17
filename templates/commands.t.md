<!-- {@devEnvironmentSetupCommands} -->

```bash
devenv shell
install:all
```

<!-- {/devEnvironmentSetupCommands} -->

<!-- {@buildAndTestCommands} -->

```bash
cargo build --all-features
cargo test
```

<!-- {/buildAndTestCommands} -->

<!-- {@commonQualityChecksCommands} -->

```bash
lint:clippy
lint:format
verify:docs
```

<!-- {/commonQualityChecksCommands} -->

<!-- {@docsBuildCommand} -->

```bash
docs:build
```

<!-- {/docsBuildCommand} -->

<!-- {@dailyDevelopmentLoop} -->

```bash
devenv shell
cargo build --all-features
cargo test
lint:all
verify:docs
verify:security
test:idl
```

<!-- {/dailyDevelopmentLoop} -->

<!-- {@codamaWorkflowCommands} -->

```bash
# Generate Codama IDLs for all examples.
codama:idl:all

# Generate Rust + JS clients.
codama:clients:generate

# Generate IDLs + Rust/JS clients in one command.
pina codama generate

# Run the complete Codama pipeline.
codama:test

# Run IDL fixture drift + validation checks used by CI.
test:idl

# Run Quasar SVM generated-client e2e checks alongside LiteSVM.
pnpm run test:quasar-svm
```

<!-- {/codamaWorkflowCommands} -->

<!-- {@releaseWorkflowCommands} -->

```bash
knope document-change
knope release
knope publish
```

<!-- {/releaseWorkflowCommands} -->
