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
```

<!-- {/dailyDevelopmentLoop} -->
