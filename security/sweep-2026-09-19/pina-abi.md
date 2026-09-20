# ABI security sweep — `crates/pina_abi` and consumers

Sweep date: 2026-09-19 (night sweep, owner-authorized, read-only). Branch: `feat/abi-version-reset`, HEAD `3f7d38f9`. Scope: `crates/pina_abi` (fresh code) plus its consumers (`pina_cli` migrations/abi command, `pina_macros`, `client_events.rs`, codama pipeline adjacency). Method: full read of `pina_abi/src/lib.rs` (~3.6k lines), `consts.rs`, all `decode_manifest` / `decode_publication_ledger` / `transition_path` call sites, macro codegen paths; empirical malformed-document probes against the real decoders (scratch harness under `tmp/sweep/scratch-abi/`, untouched tracked files).

---

## 1. Threat model of an ABI document

**Producers:** `pina migrations make` (CLI, derives everything from the user's parsed source and writes `migrations/manifest.json` + `migrations/transitions/*.rs` via `encode_manifest`, which validates before writing — `pina_abi/src/lib.rs:1304`) and `pina abi schema` (renders JSON Schema artifacts from the same types, `pina_abi/src/lib.rs:2249`).

**Consumers and what a malicious/corrupted document can do to each:**

| Consumer                                         | Entry                                                                       | What the document drives                                                                                                     | Blast radius of a hostile document                                                                                                                                        |
| ------------------------------------------------ | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pina_macros` proc-macros (highest value)        | `decode_manifest` at `crates/pina_macros/src/migration.rs:1206, 1307, 1385` | generated migration ladders, `include!` paths to transition files, historical struct definitions, reserved `Migrate` routing | code injection into the compiled program — but blocked at every observed sink (see §3); residual: compile-time panic (F1), historical-layout tamper pre-publication (O2)  |
| `pina_cli` migrations commands                   | `ledger.rs:381` / `decode_publication_ledger` at `ledger.rs:396`            | file writes under `migrations/transitions/…`, publication receipts, hash pins                                                | file-write primitive — path is identity-derived and identity validation blocks traversal (blocked, §3)                                                                    |
| codama / client pipeline                         | `crates/pina_cli/src/client_events.rs:158`                                  | event byte-mapping (`EventFieldMove` offsets) baked into generated JS/Dart/Rust clients                                      | wrong decode of historical event logs — grammar-bounded, pin-checked after publication (O2)                                                                               |
| migrations manifest pinning (on-chain adjacency) | `ledger.rs:326` `validate_published_contract`                               | `manifest_sha256` / `schema_sha256` pins in the append-only publication ledger                                               | a schema rewrite that changes how old on-chain account bytes are interpreted fails the pin check (blocked after first publication; see O2 for the pre-publication window) |

Trust level: the manifest lives in the developer's repo, so the realistic attacker is a malicious clone / tampered checkout / supply-chain PR, not a remote party. The tooling's job is to (a) fail closed with a clear error, (b) make published history tamper-evident. Both hold, with two low-severity exceptions below.

**Prior-art check (09-18 audit): M2 and M3 are fixed on this branch.**

- **M2 (IDL-name doc-comment injection):** `crates/pina_cpi_renderer/src/render/instructions.rs:475` now routes the interpolated account name through the newline-splitting `render_doc` helper; `mods.rs:156` uses `str::lines()` splitting; the remaining `docs.push(format!("\t/// {privilege}"))` (`instructions.rs:489`) interpolates only a static internal string. No raw single-line `///` interpolation of IDL-controlled names remains.
- **M3 (`[lib]` name path traversal):** `crates/pina_cli/src/project.rs:1194` calls `validate_library_name` (`project.rs:1211`, `[A-Za-z0-9_-]+`) inside `library_details`, the single point where the Cargo-derived name enters the project model. All downstream sinks (`idl/`, client dirs, `target/deploy/`) inherit the validation.
- **Sibling sinks in the new ABI paths:** none found. `rust_name` is used as a _lookup key_ (`contract_for_source`, `lib.rs:1121`) — generated code never interpolates it except via `syn::Ident::new`, which is finding F1. Manifest field names reach codegen only through `syn::parse_str::<syn::Ident>` with a proper error (`migration.rs:1489`). Manifest `rustType` strings are rejected at decode unless they fit the closed grammar (`fixed_type_size`, `lib.rs:1670`), so the pass-through arm of `render_abi_type` (`migration.rs:1652`, unknown path names rendered verbatim) is unreachable for non-grammar types — the type must already have passed `fixed_type_size` to get there.

---

## 2. Encode/decode guards and probe results

Guards in place (`pina_abi/src/lib.rs`): version walk (`walk_document_to`, :1217) with `ABI_STEPS` (:1182, empty — baseline equals current), `deny_unknown_fields` on every document type, `ContractIdentity::validate` (:527, canonical hex + width + key anti-aliasing), manifest key/identity match (:1103), closed-grammar type sizing with `MAX_TYPE_NESTING_DEPTH = 32` (:1666) and fully `checked_*` arithmetic, receipt hash chain + sequence monotonicity + version-regression rejection (:1468-1620), canonical lowercase `is_sha256` (:1622), control-character rejection on `rpcUrl`/contract keys (:1492, :1553), derive-time proof that the grammar accepts each schema (`DataSchema::try_new` → `physical()`, :603).

Empirical probes (all against the real `decode_manifest` / `decode_publication_ledger` / `walk_document`; full transcript in the sweep notes):

| Malformed input                                                                               | Result                   | Guard that stops it                                                                                                       |
| --------------------------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| Future version `9.9`                                                                          | rejected                 | `start > current` → "upgrade Pina"                                                                                        |
| Below-baseline `0.19`                                                                         | rejected                 | "predates the oldest supported version 0.20"                                                                              |
| Prerelease `0.20.0-rc.1`                                                                      | rejected                 | `< oldest` (semver prerelease sorts below)                                                                                |
| Build metadata `0.20.0+evil`                                                                  | rejected                 | orders _greater_ than current on this semver build → future-version error (fails closed; see F4 for the API-contract nit) |
| Missing / non-string `abiVersion`                                                             | rejected                 | `document_abi_version`                                                                                                    |
| Duplicate `abiVersion` keys                                                                   | **accepted** (last wins) | none — F3                                                                                                                 |
| Patch `0.20.1`, garbage `%%%`                                                                 | rejected                 | `parse_document_version`                                                                                                  |
| Unknown top-level field                                                                       | rejected                 | `deny_unknown_fields`                                                                                                     |
| Traversal `ab/../../../evil` in hex + key                                                     | rejected                 | `ContractIdentity::validate` (invalid hex)                                                                                |
| Non-canonical `AB`, width `3`, key mismatch                                                   | rejected                 | identity validation / key match                                                                                           |
| Non-grammar `rustType: "EvilType"`                                                            | rejected                 | `fixed_type_size` → schema invalid                                                                                        |
| Compact with no tail, empty versions, `version > 256` under `u8`                              | rejected                 | `compact_physical_layout` / `ContractHistory::validate`                                                                   |
| `String<u64::MAX>` capacity, `Option^40` type, 2000-deep JSON                                 | rejected                 | `checked_add` overflow / depth 32 / serde recursion limit                                                                 |
| Non-UTF8, trailing garbage, empty body                                                        | rejected                 | serde_json parse                                                                                                          |
| Ledger: sequence gap, uppercase sha256, history-length mismatch, raw control byte in `rpcUrl` | rejected                 | chain validation, canonical hash check, `validate_published_history`, JSON parse                                          |

**No path was found that panics on decoded input inside `pina_abi` itself.** The three `unwrap_or_else(panic!…)` sites (`hash_json` :2148, `document_schema` :2236, `current_abi_version` :90) operate on in-memory, already-validated model state or the committed constant. The guarded `panic!("validated above…")` in `ContractHistory::validate` (:1044-1049) is genuinely guarded by the same-loop instruction/process checks. The two panics reachable with hostile input live in the macro consumer (F1) and are compile-time, not runtime.

**Blocked attacks worth logging as regression anchors:** the path-traversal-via-identity attack has a dedicated test (`manifest_rejects_path_traversal_in_contract_identities`, `lib.rs:2646`) documenting the pre-fix write primitive — keep it; the below-baseline walk error names the remedy (`pina migrations make`), matching the reset's fail-closed posture.

---

## 3. Findings

### F1 — CONFIRMED (Low): proc-macro panics on a manifest `rustName` that is not a Rust identifier

1. **Location:** `/Users/ifiokjr/Developer/projects/pina-rs/pina/crates/pina_macros/src/migration.rs:1324` — `syn::Ident::new(&history.rust_name, enum_name.span())` inside `manifest_account_ladder_at`; reached from `crates/pina_macros/src/entrypoint.rs:388` whenever a program uses the reserved `Migrate` enum without an explicit ladder.
2. **Mechanism:** `decode_manifest` + `manifest.validate()` enforce nothing about `rustName`; the probe confirms `"rustName": "a b; drop table"` decodes cleanly. `proc_macro2::Ident::new` panics on any string that is neither a keyword nor a legal identifier. Every other malformed-document condition in this pipeline produces a spanned, remedial `syn::Error`; this one produces a bare proc-macro panic.
3. **Exploit scenario:** (a) attacker tampers `migrations/manifest.json` in a repo (malicious PR, corrupted clone); (b) victim builds any program whose entrypoint derives the reserved `Migrate` routing; (c) `manifest_account_ladder` panics inside the proc macro; (d) rustc reports "proc macro panicked" with no mention of the manifest field or the `pina migrations make` remedy — a confusing build failure instead of the intended diagnosable error.
4. **Severity justification:** Low. Fails closed (compile error, never miscompilation); the manifest is repo-trusted input anyway; impact is error-quality and build-tooling DoS-with-confusing-message, violating the crate's own "fail closed with a clear error" standard applied everywhere else.
5. **Fix:** validate at the choke point — in `ContractHistory::validate` (`pina_abi/src/lib.rs:930`), reject a `rust_name` that is not a valid Rust identifier (parse with `syn::parse_str::<syn::Ident>` or an `unreserved`/`xid` check). That protects the CLI (`make`/`verify`), the codama pipeline, and the macro in one place without changing the document shape (no ABI version bump: every existing 0.20 document in-repo already carries source idents). Add a UI test with a hostile `rustName` asserting the spanned error text.

### F2 — CONFIRMED missing validation (Low): duplicate field names decode successfully

1. **Location:** `pina_abi/src/lib.rs` — `DataSchema::validate` (:608) and `ContractHistory::validate` (:930) never check field-name uniqueness inside one `fields` array; probe confirmed `[{name:"value",rustType:"u64"},{name:"value",rustType:"u32"}]` decodes and validates.
2. **Mechanism:** field names are the join key for automatic transitions (`field_moves`, `crates/pina_cli/src/client_events.rs:262-291` — name→offset maps silently collapse duplicates, last-wins) and for rename validation (`lib.rs:1025-1041` BTreeSet, duplicates invisible). Downstream: the macro's drift check fails only for the _current_ version (a source struct cannot have duplicate idents), but a _historical_ version with duplicate names flows into `historical_struct` (`migration.rs:1489`) and emits a struct with two same-named fields → a confusing derive/parse compile error instead of a document error; the client pipeline can emit duplicate/dropped byte moves.
3. **Exploit scenario:** tamper a manifest's historical version `fields` to duplicate a name; `pina migrations verify`/build either fails with an unrelated compiler error (macro) or generates client code whose event migration mapping silently mis-orders historical event bytes (client_events path). Publication pins catch it only after the version was published once.
4. **Severity justification:** Low — same repo-trust model as F1; pre-publication window plus error-quality. The on-chain executor reads the macro-generated historical structs, so a pre-pin tamper that survives the grammar changes old-version interpretation (e.g. same-size `u64`↔`f64` swap on a historical version is grammar-valid and silent) — this is the sharpest reason to close it.
5. **Fix:** in `ContractHistory::validate`, require each version's field names to be unique (compare `fields.len()` against a `BTreeSet` of names, mirroring the existing rename-validation style). Add a decode test and a trybuild/`tests/ui` case.

### F3 — CONFIRMED (Informational): duplicate JSON keys are silently normalized (last wins)

1. **Location:** `pina_abi/src/lib.rs:1280` (`decode_manifest`) — `serde_json::Value` deduplicates object keys, keeping the last occurrence; probe: `{"abiVersion":"9.9","abiVersion":"0.20",…}` is _accepted_ as 0.20 while the reversed order is rejected.
2. **Mechanism:** a syntactically malformed (duplicate-key) document is silently repaired instead of rejected, so "the document I reviewed" and "the document that was parsed" can differ.
3. **Exploit scenario:** a diff-review evasion — a PR shows `"abiVersion": "0.20"` but an earlier duplicate line (easy to hide in a large `contracts` object) changes the effective value for parsers with first-wins semantics (some non-serde tooling). Not exploitable against hashing: `hash_json` (:2146) serializes the typed model, never raw bytes, so pins are unaffected.
4. **Severity justification:** Informational. serde_json gives no duplicate-key rejection without a custom deserializer; the typed-model hashing removes the high-consequence variant.
5. **Fix:** either document the semantics in `docs/src/migrations/abi-versioning.md`, or add a cheap scan in `decode_manifest` that errors when `abiVersion` (or any tracked key) appears twice in the raw `Value` (serde_json preserves insertion metadata only with `preserve_order`; otherwise compare against the typed model's stamp — `manifest.abi_version` is already rewritten nowhere, so a mismatch check on the walked value is possible). A fuzz target would catch regressions (see Recommendations).

### F4 — CONFIRMED (Informational): `parse_document_version` accepts prerelease/build spellings; `validate_document_version` alone accepts below-baseline versions

1. **Location:** `pina_abi/src/lib.rs:77-85` and `:1146-1156`. Probes: `parse_document_version("0.20.0-rc1")` → `Ok`; `validate_document_version("0.19")` → `Ok`; `validate_document_version("0.20.0-rc1")` → `Ok`.
2. **Mechanism:** only `walk_document` enforces the oldest-supported baseline and only `parse_document_version`'s patch check is strict; prerelease and build metadata pass parsing. Today this is unreachable misuse — every read path (`decode_manifest`, `decode_publication_ledger`) walks first, and the walk rejected all three probe values ("upgrade Pina" / "predates oldest"). But `validate_document_version` is a public API whose name promises more than it does, and the walk's rejection of `0.20.0+evil` relies on semver ordering behavior rather than an explicit canonical-spelling check.
3. **Exploit scenario:** none currently; risk is a future consumer calling `validate_document_version` alone and accepting a below-baseline or prerelease-stamped document.
4. **Severity justification:** Informational / hardening.
5. **Fix:** make `parse_document_version` reject prerelease and build metadata (the document contract is exactly `major.minor`), which turns both probes into parse errors and makes `validate_document_version` self-sufficient. Existing tests at `lib.rs:2920` extend naturally.

### O2 — Observation (documented design, worth an explicit note): historical-version schema integrity rests entirely on repo trust until first publication

1. **Location:** `crates/pina_macros/src/migration.rs:55` (`verify_source_schema` pins only `current.schema` to the Rust source); `crates/pina_cli/src/migrations/ledger.rs:326-374` (pins catch tamper only once published); `crates/pina_cli/src/migrations/transition.rs:576-624` (transition files hash-checked, frozen after publication).
2. **Mechanism:** the grammar bounds what a tampered historical schema can express, but same-size type swaps (`u64`↔`f64`, `PodU32`↔`f32`) on historical versions are grammar-valid, decode-valid, and unverified against anything until the version is published and pinned. `load_publication_ledger_for_manifest` (`ledger.rs:410`) already fail-closes on a _missing_ ledger for advanced versions — good — but a _draft_ (never-published) advanced version has no pin to check.
3. **Exploit scenario:** malicious PR rewrites a draft `v1` historical schema of a multi-version contract; reviewers must eyeball JSON; if merged, on-chain migration of still-stale accounts proceeds with the tampered layout until the first `pina deploy` freezes pins.
4. **Severity justification:** accepted-risk observation (repo trust is the baseline threat model; the reset's pin design fixes the post-publication half). Recording it because it is the one path where a manifest change reaches on-chain behavior without a derived cross-check.
5. **Fix / mitigation options:** have `pina migrations make`/`verify` print a schema diff for every _changed historical_ version (not only the new tail), or require an explicit `--accept-history-change` token for edits touching any version below `current`. A lint reminding reviewers that historical-version edits are wire-format events would fit `docs/src/migrations/abi-versioning.md`.

### Version reset / downgrade (audit question 3): blocked by construction

`ABI_STEPS` is empty and `ABI_OLDEST_SUPPORTED == ABI_VERSION == "0.20"` (`lib.rs:55, :1182`), so the reader accepts exactly one document version. Probes confirm `0.19`, `0.20.0-rc.1`, `9.9` all fail closed; the `converter_chain_is_gapless_and_reaches_current` test (:3076) plus the hole-in-table test (:3603) ensure that when the first real converter ships, a dropped/gapped step fails at test time and the walk reports "no ABI converter reaches version X" at runtime rather than silently decoding the wrong shape. Decode drift (client generating against a stale schema) requires the walk to accept an older version — currently impossible. The reset thereby _removes_ the classic N/N-1 downgrade surface instead of adding one. The only dual-shape acceptance is duplicate-key last-wins (F3).

### Published schema URL (audit question 6): clean

`schema_url` (`lib.rs:2241`) formats `https://pina-rs.github.io/pina/abi/schemas/{ABI_VERSION}/{file}` from the committed constant only — no user input, no interpolation of document contents. `pina abi schema` (`crates/pina_cli/src/abi_command.rs`) renders locally from the Rust types and _prints_ the URL inside `$id`; nothing ever fetches it (no HTTP client exists in `pina_abi` or the abi command — grep-verified). No SSRF, no fetch-time validation gap. The command refuses to print any version except the one the build writes (`abi_command.rs:29-43`), preventing a "historical shape" lie. Remaining nit: the `$id` URL is a claim about a published artifact; a consumer validating against a fetched schema should pin by content, not by URL — worth one sentence in the docs, not code.

---

## 4. Round-trip fidelity (audit question 4) — what the tests do and do not cover

Covered (`pina_abi/src/lib.rs` tests): encode→decode identity for minimal manifest + ledger (:3120), encode-time rejection of invalid documents (:3170), fixture decode matrix landing on the current version (:3453), frozen schema artifacts equal generated schemas (:3536), required-fields parity and `additionalProperties: false` (:3563), gapless walk (:3076), hostile identity/traversal/hex (:2646-2705), walk edges (:2893-2952), depth and capacity bounds (:2955-3013), ABI-version/changeset consistency (:3038).

Not covered (each verified by probe or reading, none exploitable beyond F2):

- **Field-order instability:** JSON key order is normalized by the typed model (struct order, `BTreeMap` contracts), and `schema_sha256` hashes canonical typed JSON — so reordering _keys_ cannot change pins. But _field-array_ order is semantic (physical layout); no test asserts that a reordered `fields` array changes the hash and is caught by publication pins — worth one test since it is the property that makes pin-tamper visible.
- **Defaulting / optional ambiguity:** `transition: null` vs absent, `auto`/`renames`/`pending`/`abandoned`/`tail` skip-and-default pairs are untested as round-trip pairs; all decode to identical models by serde semantics (verified by reading), so this is a test-coverage gap only.
- **Integer widths:** capacities beyond `u64` fail via `usize` parse + `checked_add`; `String<u64::MAX>` probe rejected. `u64::MAX`-adjacent `Vec` capacities compose through `checked_mul` — bounded by prefix validation. No truncation path found.
- **Discriminator representation:** canonical LE hex is thoroughly tested (:2323, :2646-2705) including the `key()` aliasing defense.
- **Duplicate JSON keys / duplicate field names:** untested and unhandled (F3 / F2) — the two genuine gaps.

---

## 5. Recommendations

1. **Fix now (tiny, no ABI bump):** F1 identifier validation in `ContractHistory::validate`; F2 duplicate-field-name validation in the same function. Both are decode-level tightenings of the 0.20 contract that existing documents already satisfy.
2. **UI tests:** hostile `rustName` (asserts the spanned error, not a panic — F1); duplicate `fields` in a historical version (F2); historical same-size type-swap diff visibility (O2, if the diff option is taken).
3. **Fuzz target:** `crates/pina_fuzz/fuzz/fuzz_targets/migration_decode.rs` covers generated on-chain decoders only. Add `manifest_decode`/`ledger_decode` targets over `pina_abi::decode_manifest` / `decode_publication_ledger` with the invariant "no panic; if decode succeeds, `validate()` re-run succeeds and `encode→decode` is identity". The probe corpus above is a ready seed set (future/below-baseline versions, traversal hex, duplicate keys, deep types, `u64::MAX` capacities).
4. **Docs:** one paragraph in `docs/src/migrations/abi-versioning.md` stating (a) duplicate JSON keys are parsed last-wins (or fixed per F3), (b) historical-version edits are wire-format events and pin-protected only after first publication, (c) schema consumers should pin the schema artifact by content, not by its `$id` URL.
5. **Lint candidate:** flag `abiVersion` values in checked-in manifests that are not the exact current spelling (e.g. `0.20.0` with patch, build metadata) even though they fail closed — cheap early warning that a generator somewhere is off-spec.

**Blocked-attack log (regression anchors to keep):** identity-hex traversal write primitive (`lib.rs:2646` test, now blocked by `ContractIdentity::validate`), non-canonical/aliasing identity keys (key-match check), future/below-baseline/prerelease/metadata versions (version walk), grammar-escaping types (`fixed_type_size` at decode, before any codegen sees the string), unknown fields (`deny_unknown_fields`), ledger regression/hash-chain/sequence tampering (`PublicationLedger::validate`), frozen transition edits (`FrozenImplementationChanged`), frozen schema rewrites (`validate_published_contract` pin comparison), and the M2/M3 host-tool sinks (verified fixed this branch).
