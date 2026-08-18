# Temporary Codama Dart Renderer Patch

`codama-renderers-dart@0.5.0.patch` adds the package's missing Node ESM bundle. The bundle is built from a clean composition of the exact upstream pull-request heads recorded in [`docs/src/codama-workflow.md`](../docs/src/codama-workflow.md). It contains upstream renderer behavior only; Pina does not rewrite generated Dart codecs.

The patch is temporary. Remove it when a released renderer includes the schema, discriminator, exact-consumption, fixed-capacity, and wide-enum fixes and all of Pina's generated-client contract tests pass against that release.
