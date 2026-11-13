# Changelog (English)

> **This file starts at 0.6.3.** Entries for **0.6.2 and earlier** live in the Chinese
> [`CHANGELOG.md`](CHANGELOG.md), which was the only source of truth up to that point.
> From **0.6.3** onward the two files are written together: the same sections, in the
> same order, carrying the same evidence (the English side may **condense** long notes to
> one line per change). If they ever disagree, the Chinese file is the one to trust (it is
> the one the release notes are cut from) — and the English file should be corrected to match.

Conventions carried over from `CHANGELOG.md`:

- Version numbers follow Cargo's reading of semver: in the **0.x** stage a **MINOR**
  bump counts as **incompatible** (0.6 → 0.7 would stop downstream from upgrading
  automatically), while PATCH is for compatible additions. So "a new backend / a new
  API" goes to PATCH (0.6.0 → 0.6.1 → 0.6.2 → 0.6.3); only genuinely breaking changes
  move MINOR.
- `swsml` (the library) and `smltools` (the CLI) are versioned independently; a release
  names both in its section header.
- Sections are prefixed with what they are — `Added` / `Changed` / `Fixed` / `Docs` —
  and every behavioural change says so explicitly instead of hiding in a feature note.

---

## [Unreleased]

> Condensed mirror of the Chinese `[未发布]` section: same categories, one line per change.
> The Chinese file remains authoritative for detail and evidence.

### 🚨 Release policy (planned: cross-end sync, not yet implemented)

- **Breaking public API on four ports**: C / C++ / JS / Lua will each gain a
  "success + diagnostics list" exit for `W-PARSE-001` / `W-PARSE-002`. The current
  entry points are binary success/failure only, so callers are affected.
- **Version policy**: under the 0.x semver rule at the top of this file (MINOR =
  incompatible), the next stable release is **0.7.0**; `swsml` and `smltools` are
  versioned independently. Aligning the C library / VSIX versions is B8 in
  `docs/swsml/spec/v5.md` table B.
- **Changelog split**: "library API breaking" and "document syntax breaking" stay
  in separate sections.
- **Capability parity**: everything `swsml` (Rust) can do should also be available,
  **functionally**, through the FFI (`rust/src/c_abi.rs` / `c/sml_rs.h` / `cpp/sml_rs.hpp`)
  and on the other ports, including the five bindings. The criterion is equivalent behaviour
  and observable results, not identical API shapes or internals: Rust-only extension hooks
  that take trait objects / closures do not have to become C function pointers.
  No permanent cuts on the grounds of "no consumer yet"; staged delivery is fine.
  Criteria: `docs/swsml/plan/crossend-sync.md` §0.6.
- **C-ABI**: new `CSmlDiagnostic` / weak side-tables / extended options are added as new
  symbols alongside the old ones (see the `sml_dump_err` precedent) and mirrored in
  `rust/src/c_abi.rs`, `c/sml_rs.h`, `cpp/sml_rs.hpp` and the five bindings.
- Policy only: no version bump, no release, `E-FEATURE-004` stays disabled.
  See `docs/swsml/plan/crossend-sync.md` §0.5 / P4 / P7.

### Added (cross-end sync P4: diagnostics exits + `weakbind` soft checks)

- **C**: `sml_parse_ex` + `sml_diag` / `sml_diag_free` expose diagnostics with line numbers;
  a `sml_builtin` handle injects the builtin contract table, and
  `sml_parse_with_builtins` emits the seven `W-CONTRACT-001` categories plus the weak-binding
  side table (`sml_weakbind ** / size_t *`, freed by `sml_weakbind_free`).
- **C++**: `Parser::parse(..., &diags)`; new `ParseOptions` + `Parser::parse_with`
  (`builtins` / `weak_binds` / `diags` / `include_dir`), `BuiltinTable`, `WeakBind`.
- **JS**: `parseSafe()` returns `warnings` and `weakBinds`; `opts.builtinContracts` injects the table.
- **Lua**: `Sml.parse_with_warnings`; `Sml.parse_with_builtins(text, builtins, base)` returns
  `value, err, warnings, weak_binds`.
- **All ports**: with `type.weak` off, `@weakbind` / `@weakdata` / `@weakdoc` report
  `E-FEATURE-001` everywhere; the soft check is **read-only** (no defaults filled, no key edits,
  value tree byte-identical).
- `sml_parse_with_builtins` is new in this cycle (never released), so widening its signature
  is not a second break; it ships with 0.7.0.

### Added (cross-end sync P7: C-ABI diagnostics exit + all five bindings aligned)

- **C-ABI**: `sml_parse_with_diags(text, flags, sml_diag_list *, sml_error *)` and
  `sml_diag_list_free`; new types `sml_diag` / `sml_diag_list` / `sml_severity`. They hand out
  the stable code (`W-PARSE-001` / `W-PARSE-002` / `W-CONTRACT-001` …), text, line and severity;
  the value tree is **byte-identical to `sml_loads`**, fatal errors still go through `sml_error`.
  No old symbol changed.
- **Rust**: new `sml_parse::parse_with_features_warnings`.
- **C++ wrapper**: `Severity` / `Diagnostic` / `DiagLoadResult` + `loads_with_diags`;
  regression `cpp/tests/test_rs_diag.cpp`, run as `RS-DIAG` in `cpp/build_verify.py`.
- **Five bindings** (Python / Ruby / Java / Go / C#): a `loads_with_diags`-style exit each;
  the diagnostics list is freed as a whole. `bindings/README.md` alignment list now covers v3.1.
- Not yet: `errors`, `contract_map` / `contracts`, the three weak side tables, extended options,
  and a file + diagnostics entry (see `docs/swsml/plan/crossend-sync.md` P7).

### Added (cross-end sync P7, second slice: extended options + builtin contract table)

- **C-ABI**: `sml_builtin` (opaque handle: `_new` / `_free` / `_count` / `_add_field` /
  `_set_range` / `_add_enum`, same shape as the pure C `sml.h`) plus `sml_opts`
  (`struct_size` / `flags` / `env` / `env_len` / `builtins`) and `sml_parse_full`.
  This makes `W-CONTRACT-001` reachable over the C ABI.
- **Rust**: new `sml_parse::parse_full(text, allowed, env, opts)`; `parse_with_features_warnings`
  is now a thin wrapper (one code path, no second semantic).
- **C++ wrapper**: `sml::Builtin` (RAII), `sml::Opts`, `loads_full`.
- ⚠️ **`flags` is a feature cap, not "extra features"**: `type.weak` is opt-in and not in the
  baseline, so `flags = 0` intersects it away and `@weakbind` then reports `E-FEATURE-001`.
  Pass `SML_F_BASIC | SML_F_WEAK` (or `sml_features_mask()`); there is an explicit test for this.
- Not yet: the five bindings' options/builtin wrappers, the three weak side tables,
  `errors` / `contract_map` / `contracts`, and a file + diagnostics entry.

### Added (cross-end sync P7, third slice: all five bindings get the full option set)

- Python `loads_full(text, flags, env, builtins)` + `Builtin`; Ruby
  `loads_full(text, flags:, env:, builtins:)` + `Builtin`; Java `loadsFull(text, Opts)` +
  `Builtin` (`AutoCloseable`); Go `LoadsFull(text, Opts)` + `NewBuiltin`; C#
  `LoadsFull(text, Opts, out diags)` + `Builtin` (`IDisposable`).
- Each port also exposes `SML_F_WEAK` and a regression covering "build table → 4 ×
  `W-CONTRACT-001` → tree unchanged → env injection → baseline without `type.weak` yields
  `E-FEATURE-001`". `bindings/README.md` gains a "full options" column.
- Two cross-language layout bugs fixed: Java's `sml_diag` layout had an extra 4-byte tail pad
  (offset 28 instead of 24 from the second diagnostic on, rejected by FFM as misaligned);
  Go's `env` pointer array is now allocated in C memory (cgo rejects Go memory holding
  pointers).

### Added (cross-end sync P7, fourth slice: weak side tables + file entry over the C ABI)

- **C-ABI**: `sml_weakbind` / `sml_weakdata` / `sml_weakdoc` / `sml_weaklists` +
  `sml_weaklists_free`; `sml_parse_full_lists` (`sml_parse_full` is it with `lists = NULL`)
  and `sml_load_file_full`. The three tables mirror the pure-C shapes (`path` / `name` / `line`);
  `weakdata.value` is an owned value-tree handle released **with the table** (never free it alone).
- **Rust**: new `sml_parse::parse_file_full(path, allowed, env, opts)`.
- **C++ wrapper**: `sml::WeakLists` (RAII), `loads_full(text, opts, WeakLists *)`,
  `load_file_full(path, opts, WeakLists *)`.
- Regressions: `rust/tests/c_abi.rs` +2 cases, `cpp/tests/test_rs_diag.cpp` +2 sections (11 checks).
- Not yet: the five bindings' weak-table wrappers, `errors` / `contract_map` / `contracts`.

### Added (cross-end sync P7, fifth slice: all five bindings hand out the weak tables)

- Python `loads_full_lists` → `(value, [Diagnostic], WeakLists)`; Ruby
  `Sml.loads_full_lists` → `[value, diags, WeakLists]`; Java `Sml.loadsFullLists` →
  `Parsed.lists`; Go `LoadsFullLists` → `(any, []Diagnostic, WeakLists, error)`; C#
  `LoadsFullLists(..., out diags, out lists)`.
- Contents match the C ABI (`path` / `name` / `line`); the `@weakdata` payload is materialised
  immediately and the C table is then freed, so hosts never own C memory.
- Each port carries the same regression: 1/1/1 tables, readable payload, empty top-level path
  for `@weakdoc`, no trace in the main tree, 100 iterations.
- Not wired: the bindings' **file entry** (`sml_load_file_full` is C-ABI / C++ only for now).

### Added (cross-end sync P7, sixth slice: aggregated result handle `sml_result`)

- **C-ABI**: `sml_parse_all(text, opts, want, err)` plus the `sml_result_*` getter family
  (`value` / `diags` / `errors` / `weak` / `contract_map` / `contracts`) and `sml_result_free`;
  `want` is a mask of `SML_WANT_*` bits. Rule: **one parse -> one handle -> getters**, so future
  outputs add a bit and a getter instead of reshaping existing signatures.
- New types `sml_contract_ref` / `sml_contractmap` (flat "entity -> contract name" rows).
  Every getter returns a **borrowed** pointer that dies with `sml_result_free`.
- With `SML_WANT_ERRORS`, a document with syntax/contract errors no longer returns NULL: the
  value is a best-effort partial tree and all errors land in `sml_result_errors`
  (`severity = 1`, line derived from the byte offset).
- The `sml_builtin` from `SML_WANT_CONTRACTS` can be injected as a builtin table, closing the
  "write contracts in SML, inject them back" loop (`docs/swsml/spec/weakbind.md` §6.1).
- **C++ wrapper**: `sml::Result` (RAII), `sml::Want`, `sml::Builtin::borrow()`.
- Regressions: `rust/tests/c_abi.rs` +2 (33 total), `cpp/tests/test_rs_diag.cpp` +2 sections (17 checks).
- Not yet: the five bindings' `sml_parse_all` wrappers.

### Changed (error-code ledger backfilled to match reality)

- `errors/codes.sml`: `W-PARSE-001` now lists `[ rust js c cpp lua ]` with `status: done`
  (all five ports hand it out through their "success + diagnostics" exit, same trigger and line
  semantics); `W-CONTRACT-001` lists all five ports but stays `partial` (the four native ports
  lack `block.undeclared_field`, which needs contract strictness); `W-PARSE-002` remains Rust-only
  with a corrected note ("the other ports have a channel, the fragment-merge path does not report
  yet") instead of the stale "no diagnostics channel".
- Regenerating the eight artifacts also fixed a **stale artifact**: `W-CLI-001` was already in the
  ledger but missing from the outputs (`SML_CODES_COUNT` / `COUNT` 166 → 167, plus
  `c/sml_codes.h`, `js/sml-codes.mjs`, `rust/sml-codes/src/codes.rs`, `lua/sml_errors.lua`,
  `site/static/errors.json`).

### Added (`W-CLI-001`: a referenced profile is missing -> warning only)

- **`smltools`**: the plain parse path now warns on stderr (`[W-CLI-001]`) when a document says
  `@weakdoc X` and none of the project / user / system locations has `X.smlex`. The exit code
  stays 0 and the output is **byte-identical** to the same document without `@weakdoc`.
  `--no-profiles` stays silent; `--resolve-profiles` keeps its own exit code 1.
- **Code split fixed**: `--lint` used to tag the "not found" case with `E-LINT-005` (an *error*
  code); it now uses `W-CLI-001`, and `E-LINT-005` is reserved for "found but unreadable".
  Both notes in `errors/codes.sml` were rewritten; `W-CLI-001` moved from `partial` to `done`.
- **Regressions**: new `rust/smltools/tests/warn_missing_profile.rs` (5 cases) and the
  "missing profile" case in `profile_checks.rs` now expects `W-CLI-001`.
- The eight code artifacts were regenerated (notes/status only; still 167 codes).

### Added (cross-end sync P7, seventh slice: aggregated results in all five bindings)

- **C-ABI**: new `sml_builtin_clone(const sml_builtin *)` turns the **borrowed** table from
  `sml_result_contracts` into a host-owned handle, so "capture -> free the result -> inject the
  table" no longer depends on the result's lifetime.
- **Five bindings**: Python `parse_all(...)` -> `ParseAll`, Ruby `Sml.parse_all`, Java
  `Sml.parseAll`, Go `ParseAllOf`, C# `Sml.ParseAllOf`. The `want` bits mirror `SML_WANT_*` and
  everything comes back at once: value + diagnostics + errors + weak tables + contract map +
  contract table. `contracts` is a **clone** owned by the caller (`close()` / `Dispose()`).
- **C++ wrapper**: `sml::Builtin::adopt()` and `Result::contracts_owned()`.
- **Regressions**: C-ABI `builtin_clone_outlives_the_result`; one section per binding
  (map / capture / closed loop / two errors with lines 1 and 3 and severity 1 / wanting no
  ERRORS still throws / 100 iterations); the C++ `Result` test covers the clone.
- With this, the `errors` / `contract_map` / `contracts` rows of `crossend-sync.md` §0.6 are
  complete across C-ABI and all five bindings; only `load_file_full` remains on the binding side.

### Added (cross-end sync P7, eighth slice: file entry in the bindings)

- **Five bindings**: Python `load_file_full(path, flags, env, builtins)` ->
  `(value, diagnostics, WeakLists)`, Ruby `Sml.load_file_full`, Java `Sml.loadFileFull` ->
  `Parsed{value, diagnostics, lists}`, Go `LoadFileFull` ->
  `(any, []Diagnostic, WeakLists, error)`, C# `Sml.LoadFileFull(..., out diags, out lists)`.
  They expand `include` (paths relative to the file) and mirror `loads_full_lists`; line numbers
  can be 0 when tokens cross files.
- **Regressions**: one section per binding — include expanded, `@weakdoc` in the weak table,
  no diagnostics on a clean file, `E-IO-001` for a missing path.
- With this the **whole bindings column** of `crossend-sync.md` §0.6 is closed (C-ABI plus all
  five bindings aligned).

### Changed (cross-end sync P5: `@version` literals agree across five ports)

- **One literal parser per native port**: C `version_lit_major`, a C++ counterpart, JS
  `versionLitMajor`, Lua `version_lit_major`. All accept `vN` / `N` / `N.0` / `N0` and the
  codename `camelia`; the `5.0` family (`50` / `5.0` / `v5` / `"camelia"`) uniformly reports
  **`E-FEATURE-004`** with a message saying "5.0 is still Dev" instead of a vague "unsupported".
- **The edition now decides the feature base set** (previously Rust/JS only): v1 keeps
  `bareword-string`, v2 and later drop it. C uses its existing `ps->version`; C++ does it inside
  `scan_features` and Lua inside the `@version` branch — C++ must run before lexing, otherwise
  barewords after line 1 are already resolved with v1 semantics.
- **Six new conformance cases** (`tests/conformance/cases/55`–`60`): concatenated `10`, dotted
  `2.0`, `v4`, `50` (`E-FEATURE-004`), codename (`E-FEATURE-004`), and **`v2` + bareword =>
  `E-FEATURE-005`**. Default suite **54 -> 60, all AGREE**.
- `lua/test_codes.lua`'s "v2 is too new" assertion was replaced by a group matching the new rules.
- `docs/swsml/spec/version.md` gained §7 (implementation status: literals done; `Removed` tri-state
  and the migration tool still open).
- Note: `50` stays **disabled** (v5 is not finalized); this slice only unifies how literals are
  read and which base feature set applies.

### Changed (P6 wrap-up: three ledgers aligned + scanner cross-check)

- `tests/conformance/known_divergences.md`: new "2026-11-13 current" section
  (**AGREE 60 / DIVERGE 0**); the old 43 / 52 numbers were demoted to history — `scan.md` had
  been copying 43 while `unfinished.md` and the T table each carried their own figure, i.e. the
  very "generated artifacts drift from the live scan" trap the ledger itself warns about.
- `tools/scan_status.py`: new `scan_cases()`; the conformance row now reads
  "AGREE N (M case files)" and flags **⚠️ stale ledger** whenever N != M, instead of relying on
  someone noticing by hand.
- `docs/repo/ref/unfinished.md` §2 and `todo-ideas-tables.md` §0 now agree line by line:
  44 Rust-only codes / 36 at the language layer, 60 cases, weak* on five ports, version model
  (P5 literals synced), FFI (bindings column fully closed).
- Docs and generated artifacts only; `index.md` / `report.md` / `scan.md` regenerated.

### Added (`smltools --migrate`: scan / apply / rules)

- New module `rust/smltools/src/migrate.rs`: the rule registry (`RULES`, single source of truth)
  plus the report/rewrite passes.
- CLI: `--migrate <scan|apply|rules>`, `--rule a,b`, `--backup`.
  - **scan never writes** and exits **1** when it finds anything (CI can gate on it);
  - **apply is idempotent**: a second apply changes nothing and the following scan reports 0
    (pinned by `tests/migrate.rs`);
  - `--rule` runs a subset; an unknown id is `E-CLI-008`;
  - `--migrate` is mutually exclusive with `-o` / `--lint` / `--hugo` / `--zola` /
    `--to-amberice` / `--from-amberice` (`E-CLI-003`), and `apply` requires `-i`.
- Two rules today: `version-literal` and `feature-base-literal` — canonicalize the
  `@version` / `@feature base` literal (`v3` / `3` / `3.0` -> `30`) while keeping the codename
  and `dev` marker; the 5.0 family is only reported, never rewritten (migrating to `50` would
  still be `E-FEATURE-004`).
- Regressions: `cargo test -p smltools --bins` (92 tests, 4 of them module unit tests) and
  `--test migrate` (6 cases).
- Docs: `version.md` §7 marks the tool 🟡 and states that syntax-level rules come next;
  `rust/smltools/README.md` gained a "version migration" section and exit-code notes.

### Added (`--migrate` third rule: extension-directive positional args -> `name:`)

- New rule `directive-name-arg`: `@form F { }` -> `@form name: F { }` — the fixable counterpart
  of the `DiagnosticKind::Deprecated` warning (positional args were deprecated in v4).
- It **requires** `--migrate-directive <name,...>`: a plain text scan cannot tell an extension
  directive's positional argument from a fragment definition (`@name Type Name { }`), and
  guessing would corrupt fragment definitions. Without the whitelist the rule touches nothing;
  asking for `--rule directive-name-arg` without it is `E-CLI-003` (missing dependency) rather
  than a silent zero-hit.
- Matching mirrors `parser.rs::take_directive_arg`'s positional branch: a bare word immediately
  followed by `{` only. Explicit `name:`, quoted args, no `{`, and non-whitelisted names are left
  alone.
- Regressions: 6 module unit tests (including the five shapes that must not change) and 7 cases
  in `tests/migrate.rs` (whitelist path plus the missing-argument error).
- Docs: `smltools/README.md` rule table and `version.md` §7.

### Added (editor-side `@weakdoc` hint: `W-CLI-001`)

- **Bridge** `editors/vscode/src/sml-parse.mjs`: new `weakDocRefs` (positions) and
  `missingProfileRefs(text, known)` (pure, **no fs access**); `collectWeakDocs` now reuses the
  former, so there is a single judgment. `@weakdoc` inside a comment is not a reference.
- **VSCode extension**: `updateDiagnostics` appends an **Information + `W-CLI-001`** diagnostic
  when no `.smlex` of that name exists in the project / user / system namespaces. Hint only, never
  blocking; the message goes through `L()` and got an English translation (`_check_l10n.mjs`
  passes).
- **LSP**: `publishDiagnostics` appends the same hint, so any LSP client (Neovim / Helix) gets it;
  syntax errors stay Error and are not displaced.
- **Regressions**: five pure-function assertions in `_verify_ext.mjs`, plus a new
  `editors/lsp/_verify_diagnostics.mjs` that **spawns the real LSP process over stdio** (missing
  -> Information, present -> nothing, syntax error -> Error) and is wired into the
  `_prepublish.mjs` release gate.
- Docs: `weakdoc.md` §5 editor row -> ✅; T table `I-17` -> **Done**.

### Added (native file entries: C / C++ gain "file + diagnostics")

- **C**: `sml_parse_file_ex(path, err, errsz, diags, n_diags)` and
  `sml_parse_file_full(path, err, errsz, builtin, diags, n_diags, binds, n_binds)`;
  `sml_parse_file` becomes a thin wrapper. All three share one `parse_file_core`, so the
  "read file + expand includes" logic is not duplicated.
- **C++**: `Parser::load_file(path, opts, err)` (diagnostics / weak tables / builtin table still
  ride on `ParseOptions`); when `include_dir` is empty it defaults to the file's directory, the
  same rule as Rust's `parse_file` and C's `sml_parse_file`.
- **JS / Lua deliberately get no file entry**: both are designed for the host to read text
  (JS passes a `files` map, Lua uses a `base` directory for includes); putting `fs` inside would
  break browser / plain-Lua portability.
- 🐞 Two **line-number bugs** found by the new tests (Rust / JS / Lua were already correct):
  - **C**: `W-PARSE-001` took the line of the *next unconsumed token*, i.e. effectively the last
    line of the file; it now records the key token's line (`ps->key_line` + `diag_push_at`).
  - **C++**: same expression gave **0** at block end / EOF; it now records `cur_key_line`.
- Regressions: `c/tests/test_codes.c` (file entry diagnostics at line 2, plus
  `sml_parse_file_full`); `cpp/tests/test_codes.cpp` (diagnostic line, include resolved against
  the file's directory, `E-IO-001` for a missing path).

### Added (amberice platform aligned with swsml)

- **New guard** `tools/check/check_amberice_align.py` (stdlib only, read-only, wired into the CI
  guards job) turns "remember to follow v5 by hand" into an executable check:
  ① the last `Supported` major in `EDITIONS` ⇔ `Edition::CURRENT` ⇔
  `amberice::container::EDITION` ⇔ Lua `EDITION` ⇔ `04 00` (LE u16) in all 22 container vectors;
  ② `c/sml_rs.h` and all five bindings expose the full `SML_F_*` bit set `0..19`;
  ③ `AMBR_CAP_READ/WRITE` agree across ffi / C / JS / Lua (**deliberately not** compared with
  swsml `FEATURES`); ④ `errors/codes.sml` entry count matches the three generated `COUNT`s.
- **Five bindings gained feature bits 12..18** (`when` / `for` / `typed-block` /
  `case-insensitive` / `block-name` / `type.featag` / `lex.ident`); they previously mirrored
  only 0..11 + 19. A Ruby indentation slip that put the new constants outside the module was fixed.
- **Stale facts cleaned up**: `swsml 0.6.4` / `0.6.3` in C/C++ bridge and binding headers replaced
  by "provided by `sml_version()`"; `TODO.md`'s 146 codes now points at `COUNT`; 16 / 17 features
  corrected to 20; CI step names 41 -> 60 cases, container vector note 21 -> 22 (the pure-Lua
  writer still uses the 21 canonical cases).
- **The long-standing "C# ambr read-only gap" was a build artifact**, not a defect: rebuilding
  with `cargo build -p amberice-ffi --features sml` makes the whole C# suite `ALL OK`, and
  `check_ambr_write.py` (with explicit `AMBR_LIB`) reports 22/22 read + 22/22 round-trip.
- New doc `docs/amberice/ref/swsml-alignment.md`: alignment surface, deliberate differences
  (`ambr_*` vs `sml_*` are two ABIs, `AMBR_CAP_*` belong to amberice, `AMBR:` text pass-through),
  and open decisions; registered in `meta.sml` (`smldocs --check`: 56 docs).

### Added (amberice stands on its own: `E-AMBR-*` codes, Rust error type, platform parity)

- **Standalone `AMBR` domain**: eight codes `E-AMBR-001`..`008` in `errors/codes.sml`
  (invalid input / bad format / unsupported / integrity / crypto / canonical / host I/O /
  internal), `status: done`, `impls: [ amberice ]`. Container errors deliberately do **not**
  occupy SML language-layer codes. Code table **167 -> 175**; all five generated artifacts rebuilt.
- **i18n**: eight English `title/msg` pairs in `errors/msg.en.json`, an English domain name in
  `errors/gen_js_errors.py::DOMAIN_EN`, and a "container layer" row in `errors/README.md`.
  Chinese remains the single source of truth; missing English falls back to Chinese.
- **Rust error type** `rust/amberice/src/error.rs::AmbrError`: eight orthogonal variants with
  `code()` / `kind()` / `detail()` / `render()` plus `std::error::Error`.
  `From<ContainerError>` maps all **44 fine-grained variants** explicitly into the eight kinds,
  with **no catch-all** (a new variant is a compile error until classified);
  `From<CanonicalError>` maps to `E-AMBR-006`. Re-exported as `amberice::AmbrError`.
- **Codes on the ABI boundary**: `err` buffers now read `AMBR: [E-AMBR-xxx] <detail>`; the code
  sits in the first 19 bytes so even a 20-byte buffer keeps it. Hosts can branch on
  `\[E-AMBR-\d{3}\]` instead of parsing prose. C++/JS/Lua/C# wrapper errors were updated too,
  along with the `js/test-ambr-write.mjs` assertion.
- **Guard extended** `tools/check/check_amberice_align.py`: code table ⇔ `AmbrError::code()` ⇔
  five wrapper sites ⇔ English messages; any drift fails the check.
- **Platform parity**: the `ports.md` gap ("C# / Go / Ruby never retested at 22 vectors, not in
  CI") is closed — all three pass 22/22 (including round-trips) and, together with the pure-Java
  implementation, now run in the `non-rust` CI job. All 11 rows of the status table are in CI.
- Bilingual comments/docs synced: `c/ambr.h`, `cpp/ambr.hpp`, Java `Ambr.java`, `format.md`
  (two "no codes" claims and the `E-AMBR-non-finite-float` placeholder -> `E-AMBR-006`),
  `ports.md`, `swsml-alignment.md`; `meta.sml` `updated` / `scope` refreshed.
- **wasm carries the codes too**: `rust/amberice-ffi/Cargo.toml` now forwards the
  `allow-32bit` gate (`sml-value`'s 32-bit truncation risk) and `js/build_ambr_wasm.py`
  enables it only for the wasm build. After rebuilding `js/build/amberice_ffi.wasm`,
  decoder errors in `js/test-ambr-write.mjs` also read `AMBR: [E-AMBR-002] ...`.
  That test's pre-existing `typed-schema` false failure was fixed as well: its fixture
  carries extra TYPED/SCHEMA side segments while a text-to-bytes writer emits VALUE only,
  so byte identity cannot apply (skippable segments per spec); round-trip / text / fixed-point
  criteria cover it. Now 22/22 round-trips and 19/19 byte identities pass.
- Regressions: `cargo test -p amberice --lib` (24 tests, 3 new),
  `cargo test -p amberice-ffi --features sml` (coded FFI errors + a 20-byte truncation case),
  `js/test-ambr-write.mjs`, and the 22 container vectors in C# / Go / Ruby / Java / Python.
- User decision "**2 wait**": the v5 edition bump is intentionally **not** in this batch; the
  guard still turns red when `Edition::CURRENT` becomes V5, demanding the three copies and
  regenerated vectors.

### Added (`hash_id` dispatch review + reserved `0x33` for Chinese-standard PQC)

- **Dispatch audit** (now a table in `docs/amberice/ref/ports.md`): Rust
  `hash.rs::digest_by_id` and all five FFI bindings already dispatch on the manifest's
  `hash_id` — `0x30 SHA-256` / `0x31 SM3` / `0x32 BLAKE3` — and reject unknown ids. The gap
  is only in the **standalone decoders** (JS / C / C++ / pure Java / Lua), which hardcode
  SHA-256 and would report a false "digest mismatch" for SM3/BLAKE3 files. Whether to port
  the algorithms or first "skip explicitly" remains a decision; unchanged in this batch.
- **`0x33` permanently reserved for a Chinese-standard (SM family) post-quantum digest**
  (registered, not implemented): `HashAlgo::SmPqcDigest` resolves from the id and its
  `name()` says `reserved`, but `is_implemented() == false` and `digest` returns
  `HashError::NotImplemented(0x33)`. Readers must refuse explicitly instead of silently
  falling back to SHA-256; writers never emit `0x33`. Reserved ids are never reassigned.
- Spec `format.md`: a reserved row in §19.8, `0x33` in the §20 range table, a §20.1 rule
  that the reserved slot must also produce 32 B (otherwise a new segment layout is needed),
  and a new bilingual **§20.3 Reserved ids** section relating it to the existing
  `0x50 SM2-with-SM3` / `0xA1 SM2+ML-KEM-768` hybrid slots.
- Guard `check_amberice_align.py` gained a sixth group: `0x33` registered / not implemented
  / explicitly rejected / synced to the spec / `0x34` still unknown.
- **CI steps actually run under WSL/Linux**: `cargo build --release -p amberice-ffi
  --features sml` produces `libamberice_ffi.so`, `cargo build --release` produces
  `libsml.so` (needed by C#'s `DllImport`); with the CI env (`CGO_LDFLAGS`,
  `LD_LIBRARY_PATH`, `AMBR_LIB`) Java 22/22, Go `sml/amberice` and Ruby 22/22 + 22
  round-trips all pass. This WSL has no dotnet, so the C# step awaits the first GitHub
  runner run (library names/paths are verified).
- Regression: `cargo test -p amberice --lib hash` (4 tests, including the reserved-slot case).

### Added (group namespaces + more reserved ids to extend the format's lifetime)

Decision: "reserve several more ids — a one-byte field could limit the format's lifetime;
or treat the ranges as namespaces".

- **Code** `rust/amberice/src/hash.rs`: all **16 ids** of the hash group `0x30–0x3F` are now
  registered — 3 implemented (`0x30/31/32`) plus 13 reserved: `0x33` Chinese-standard SM
  post-quantum, `0x34` generic PQC, `0x35–0x3D` nine unassigned slots (`Reserved(u8)`),
  `0x3E` namespace escape, `0x3F` private/experimental. New constants `HASH_GROUP_FIRST/LAST`,
  `HASH_ESCAPE`, `HASH_PRIVATE` and `is_reserved()`. Every reserved id returns
  `HashError::NotImplemented(id)` — **never** a silent SHA-256 fallback; group boundaries
  `0x2F` / `0x40` remain `Unknown` so extra reservations cannot swallow other groups.
- **Spec** `format.md §20.3` (bilingual): §20 ranges become **group namespaces**; the last two
  ids of each group are fixed as `0x?E` escape / `0x?F` private. `0x3E` opens a
  `(0x3E, sub_id)` second-level namespace, so exhausting first-level ids does not require a
  new segment layout — the lifetime concern addressed directly. The escape is **registered but
  not implemented** (the manifest has no `hash_sub_id` yet; readers refuse it), and a future
  implementation must bump `fmt`. Signature-PQC / KEM / hybrid groups also reserve `0x62` /
  `0x83` / `0xA2` for Chinese-standard PQC; §19.8 gained a "12 remaining reserved slots" row
  and §20.1 now requires every reserved id to produce 32 B.
- **Guard** `check_amberice_align.py` group 6 rewritten: group constants, named reservations,
  unassigned slots, escape/private ids, uniform refusal, group boundaries, and the spec's
  §20.3 plus the other groups' PQC reservations must all agree.
- Regressions: `cargo test -p amberice --lib hash` (5 tests, including "all 16 group ids
  registered", "named reservations keep ids", "boundaries are Unknown").
- The ids and semantics of `0x30/31/32` are **unchanged**; published contracts untouched.

### Added (multi-level `hash` tuple + KDF relocation: writing the format's lifetime into the rules)

Decisions (2026-11-13): `hash` may be a **tuple** (possibly three levels, first sub-value is
a version), sub-ids are 2 bytes, and KDF moves out of `0x50–0x5F`.

- **Multi-level escape tuple** (`format.md §20.3.3`): `hash_id: 0x30` (one level, bytes
  unchanged) or `[0x3E, ns_version, sub1, sub2...]`. The **first sub-value is the namespace
  version** (starts at 1, only increases); every further level is a **2-byte** sub-id
  (`0..=65535`), **at most four levels**. Exhausting first-level ids no longer requires a new
  segment layout.
- **Rust implementation**: `hash.rs::HashPath` (`ns_version` + `subs`, full shape/range
  validation in `to_value` / `from_value`, `MAX_HASH_SUBS = 4`, six `HashPathError` reasons);
  `Manifest.hash_path` (`None` for one level), integer on write for one level and an array for
  multi-level, both accepted on read; new `ManifestError::BadHashPath`. Multi-level resolves to
  `HashAlgo::ExtendedNamespace`, whose `digest` still returns `NotImplemented(0x3E)` — the
  escape is registered but never used to hash in this build.
- **KDF relocation** (`format.md §20.3.1` / §20.4 migration rules): KDF moves to its own range
  **`0xB0–0xBF`**, with `Argon2id` at **`0xB1`**; the legacy `0x51` is accepted **only in the
  KDF context** (read-only) and counts as unassigned in the signature context. Writers stop
  emitting `0x51`; `0x50 SM2-with-SM3` keeps its id and meaning. The pre-existing
  `0x50–0x5F` collision between KDF and classic signatures is therefore resolved.
- **More reserved slots** (per the "reserve them" decision): symmetric SM `0x21` and classic
  signature `0x42` gain Chinese-standard PQC slots, alongside `0x62` / `0x83` / `0xA2`.
- **Guard** `check_amberice_align.py` gained four checks: KDF migration recorded, `HashPath`
  implemented, manifest round-trips a multi-level `hash_id`, and the spec documents the tuple
  shape with the version first.
- Regressions: `cargo test -p amberice --lib` (**31 tests**, including HashPath round-trip,
  malformed-value rejection, manifest multi-level round-trip, one-level stays an integer).

### Added (JS gains `hash_id` dispatch: SM3 + BLAKE3)

- **Dependency-free algorithms**: new `js/sm3.mjs` (GB/T 32905-2016) and `js/blake3.mjs`
  (unkeyed 32-byte digest, mirroring `rust/amberice/src/blake3.rs`: 1 KiB chunks, chunk
  chaining, parent merging, `ROOT` finalisation). All official vectors pass: SM3 empty / `abc`;
  BLAKE3 at 0 / 1 / 1024 / 1025 / 2049 / 3072 bytes.
- **Dispatch from the manifest**: `js/ambr.mjs` first scans the segment table for
  `MANIFEST` (0x04), decodes its self-contained canonical body to read `hash_id`, then picks
  the digest (`0x30` SHA-256 / `0x31` SM3 / `0x32` BLAKE3) — verifying the manifest segment
  itself too. SHA-256 is no longer hardcoded, so SM3/BLAKE3 files stop yielding false
  "digest mismatch" errors.
- **Reserved ids and multi-level tuples are not guessed**: for a reserved `hash_id`
  (`0x33`+) or an escape tuple, the decoder **skips** digest verification explicitly and
  records it (`options.report.skipped`) instead of falling back to SHA-256.
- **Cross-end fixture**: `tests/vectors/amberice-hash-algos.v1.json` (produced by Rust
  `dump_with(hash = Sm3 | Blake3)`, 724 B each) is read by the JS test to assert
  `report.hashId` / `verified`, plus a tampered-digest negative case proving the dispatch
  really changes algorithms.
- CI gained `node js/test-ambr-hash.mjs`.
- Regressions: `test-ambr-hash` (vectors + fixtures + negative case), `test-ambr-decoder`
  22/22, `test-ambr-write` ALL OK.

### 🚨 Breaking — version naming: `Version` enum → `Edition` lookup

An edition is just a **name for a set of default feature bits** (`docs/swsml/spec/version.md` v1.0).
Editions are **not ordered**, so `Ord` is gone and the criterion is always "is this feature in that set".

- `enum Version { V1..V4 }` is **deleted**; new `struct Edition { major, minor }` plus
  `pub use Edition as Version` (same type under the old name). `Version::V1..V4` survive as
  **associated constants**, so existing literals still compile — but comparisons (`v3 < v4`) no longer do.
- `FeatureSet::for_version(Version)` → **`FeatureSet::for_edition(Edition)`**, answering from the
  per-row `features` field of the `EDITIONS` registry instead of computing from a version number.
- `Edition::name()` / `Display` echo the canonical `1.0`..`4.0` (was `v1`..`v4`), so error text changes.
  `Edition::from_word` is gone — use `Edition::parse` (accepts `10` / `1.0` / `v1` / `50` / `5.0` / `v5`).
- `parse_versioned` / `parse_file_versioned` return `(Value, Edition)`; `parse_allowed(text, &[Edition])`
  (`&[Version::V1]` still compiles — same type).
- `smltools --feature` accepts the same literals but **only published editions**; the invalid-value
  hint is now `(1.0..4.0, i.e. v1..v4)`.
- The C-ABI `sml_loads` `opts.allow` version strings also go through `Edition::parse` (legacy forms still accepted).

### Added — error code `E-FEATURE-010` (edition removed)

- A **removed development codename** now has its own code, kept distinct from
  `E-FEATURE-004` ("invalid version declaration") so users can tell "I typed it wrong" from
  "I should migrate". The message always carries the codename, a migration rule id (`v50-version`)
  and `smltools --migrate scan`.
- The decision lives in a **single entry point** `scan::resolve_edition` shared by `@version` and
  `@feature base` (which identifiers are accepted, whether `dev` is required, which code `Removed` uses).
  Side effect: `@feature base 50` now demands the `dev` marker, exactly like `@version 50`.
- ⚠️ The registry has no `Removed` row yet, so the branch is **not reachable** today (code and wiring
  are in place; adding a row activates it). `smltools --migrate` (V11) is not implemented yet.

### Docs — featag S9: policy for the other four implementations (docs only, no code)

- `docs/swsml/ref/capability-matrix.md` gains **§1.7b**: feature status of featag and identifier validation
  across five ends (Rust ✅, the rest ❌), with the actual runtime shape **measured** on 2026-11-04:
  - JS on `@feature enable type.featag` ⇒ `E-FEATURE-003` (not in `KNOWN_FEATURES`);
  - Lua on `@feature` itself ⇒ `E-PARSE-005` (no feature set);
  - Rust with the bit off treats `!critical` as a bareword, while the same
    `wea !critical, name { }` is accepted by JS as "key + bareword value" — a **pre-existing
    cross-end divergence**, recorded here without blaming this change;
  - C / C++ marked `未测` (not measured) rather than guessed.
- `docs/swsml/spec/featag.md`: the status banner changes from "zero implementation" to
  "Rust implements S0–S8", and states that it currently rides the `type.featag` bit (development
  exception) rather than `@version 50`.
- **No code change**: the other four ends deliberately do not implement featag and keep reporting
  "unsupported on this end". Cross-end sync is scheduled separately by the user (2026-11-04).

### Changed — featag targets may now traverse arrays (§10 Q20, decided)

- Targets accept `[n]`: `!name: str -> rules[0].name` (previously rejected). Semantics per Q20:
  - `n < len` with a `null` element ⇒ **fill the gap** (null counts as an empty slot);
  - `n < len` with a non-null element ⇒ `E-CONTRACT-022` (**overwrite forbidden**);
  - `n == len` ⇒ **append**; under `!strict-paths` ⇒ `E-CONTRACT-023`;
  - `n > len` ⇒ **new code `E-CONTRACT-025`** (out of range, **never filled with null**).
- Declaration time **only checks syntax** (integer index, closed brackets); semantics belong to write
  time — consistent with "S4 collects, S6 writes".
- **A missing array is not auto-created** (objects are, arrays are not: length and gap content are
  undefined) ⇒ `E-CONTRACT-023`. The spec did not say this; it is an implementation addition, now
  written back into §6.1 rule 6 and §10 Q20.
- `-> [0]` (starting with a bare index) is still `E-CONTRACT-020` at declaration time: a target starts
  at a block, so its first segment must be a key.
- This resolves the spec conflict between §6.1 rule 6 and §10 Q12: Q12's "objects only" is now marked
  as superseded by Q18/Q20.

### Changed — `smltools --feature` accepts Dev editions and codenames

- `--feature` now uses the same literals as the language layer: besides `10` / `1.0` / `v1` …,
  **`50` / `5.0` / `v5` and the codename `camelia` are accepted** (previously only published editions).
- Accepting a Dev edition **must be said out loud**: a note is printed saying the edition is under
  development, its default feature set is not final, and this gating reflects the current
  implementation only (same channel as `I-FEATURE-001`).
- The invalid-value message now mentions "5.0 / camelia (in development)"; the code stays `E-FEATURE-004`.
- The rule is factored into `edition_arg()` with unit tests (four spellings → one edition; `nope` → `E-FEATURE-004`).

### Added — featag S8: the `!any` contract-level switch

- `any` means giving up type validation ⇒ **disabled by default**: a contract using `any` (or `[any]`)
  without `!any` reports `E-FEATURE-001` (same cause: the capability was not allowed), naming the
  contract and field and telling the user what to write.
- Writing **`!any`** in the contract body allows `any` **for that contract** (a **contract-level**
  switch, **not a feature bit**, §10 Q7); `null` inside `any` passes.
- **Both application paths share one rule**: the typed-block head and the in-block `@is` are both
  checked — otherwise the same contract would behave differently depending on how it is applied.
- Recursive contracts (A↔B) are deduplicated with `seen`, so they cannot loop.
- Gated by `type.featag`: with it off nothing is checked and v4 `any` keeps working.
- ⚠️ `!any` used to be collected as "a featag named `any`"; inside a contract body it is now a switch
  (and no longer lands in `featags`).

### Added — featag S7: `from-featag` source rule

- Field modifier **`from-featag`** (`FieldSpec.from_featag`): the key **may only be written by a
  featag target**; assigning it in the document is `E-CONTRACT-024`, reported with the **full field
  path** (e.g. `mid.inner.secret`).
- **The check runs before the featag fill**: afterwards, featag-written and document-written keys
  look identical, so "legal source" and "forged" could no longer be told apart — and the source is
  the whole point of the rule.
- **Nested contracts are checked** (recursing through `ContractRef`); ⚠️ **contract references
  inside arrays are not** (`[T]` needs per-element ownership info — deferred until there is a real need).
- **Two source tiers only**: default (anyone) / `from-featag` (featag only). There is deliberately
  **no `none` / `readonly` tier** — "nobody may write it" is expressed as "required but nobody
  provides it" or `optional` (§6.2 decision).
- Gated by `type.featag`: with the feature off the check does not run and documents may write such
  fields (v4 behaviour unchanged).

### Added — featag S6: target filling

- Arguments are written into the block along the declared `-> target` (§6.1 rules 1–5):
  ```sml
  @contract api { !cert: str -> tls.cert  !key: str -> tls.key }
  api cert["c.pem"] key["k.pem"], web { name: api }
  ;; ⇒ api: { tls: { cert: "c.pem", key: "k.pem" }, name: "api" }
  ```
- **An existing target is an error** (`E-CONTRACT-022`), **never silently skipped**: skipping would
  make "I thought it was written" the hardest bug to find. This is the language-level form of
  "a featag must not overwrite".
- **Parent objects are created automatically** (that is "insert", not "overwrite"); the contract-level
  switch **`!strict-paths`** requires them to exist and reports `E-CONTRACT-023` otherwise. A
  non-object in the middle of the path reports the same code.
- **`auto`** takes the declared `default`; without a `default` it is `E-CONTRACT-022`. ⚠️ Known
  limitation: arguments are already values by the time we see them, so a bareword `auto` and the
  quoted `"auto"` **cannot be told apart** (both are treated as `auto`). Fixing that requires
  `parse_array` to preserve "was this a bareword", which is a separate change.
- **No target ⇒ pure tag**: only `__featags` gets it, no key is written.
- **Ordering**: validation (S5) and filling (S6) now both run **before** contract application —
  otherwise fields that only a featag may supply would be reported as missing first (S7 relies on this).
- ~~Targets do not traverse arrays~~ — **Q20 decided the opposite: `[n]` is supported now**, see the
  "targets may now traverse arrays" entry above. Declaration time only validates syntax.

### Added — featag S5: validation opens (undeclared ⇒ reject, argument checks)

- Every featag used at a block head must be declared in a contract **applied to that block**,
  otherwise `E-CONTRACT-021` (**new code**, per spec §10 Q3 — not `E-CONTRACT-012`). Diagnostics
  spell out `<contract>.<featag>` and list what the contract does declare.
- **No contract applied** is also an error: featag scope is contract-private (§5), so
  `server !critical, api { }` has nobody declaring it — the message says "declare it with `@contract`".
- Usage that contradicts the declaration is the same `E-CONTRACT-021` (different detail):
  arguments on a tag-only featag, a typed featag without `[…]`, or a non-array declaration given
  more than one argument.
- **Argument types reuse the field-type chain** (new public `sml_contract::check_value`): an array
  type (`!obey: [int]`) validates the **whole argument list**; any other type (`!cert: str`) takes
  exactly one argument and validates it. Type mismatches report the **existing** type codes
  (same cause as field type errors, so the code must not fork).
- **Callers can opt out**: `ParseOptions::apply_contracts(false)` skips featag validation too —
  the **same switch** as contract validation (otherwise "contracts off but featag still blocks"
  would be a sandbox hole). `__featags` is still emitted: it is syntax-layer output.
- ⚠️ **Behaviour change**: featags can no longer hang off a bare block with **no contract**
  (the S2/S3 tests were updated to declare one).
- ⚠️ **`!any` (S8) does not exist yet**: "any-shaped arguments" must be typed concretely or the
  opt-out switch must be used.

### Added — featag S4: declaration collection + duplicate check

- `sml_contract::Contract` gains **`featags: Vec<FeatagDecl>`** (**declaration order**), filled by the
  `@contract` body parser: `!critical` (plain tag) / `!obey: [int]` (typed) / `!tls: [str] -> tls`
  (typed with target) / `!cert: str -> tls.cert` (dotted target).
- Parameter types **reuse the field-spec parser** (`parse_field_spec`), so `[T]`, enums, contract
  references, modifiers and external types all work — S5 needs no second type parser.
- Targets are stored as **`sml_value::Path`** (reusing S1a): **object keys only, never arrays** —
  rejected at declaration time with `E-CONTRACT-020`, covering both bare `node[0]` (the lexer breaks
  at `[`) and quoted `"node[0]"`.
- **Duplicate declaration** ⇒ `E-CONTRACT-019`; **duplicates at the use site stay legal**
  (`!a !a` is kept in `__featags`, B4).
- Identifier rules (`lex.ident`) now cover **declarations**: featag names and field names are
  validated, plus **one script per contract** — Han + Kana count as one family (`設定ファイル` works),
  digits / `_` / `-` are ignored, mixing scripts is `E-LEX-007`.
- When contracts are stacked (`@is A B`), featag declarations are **unioned in declaration order**:
  the same name in two contracts is two different featags (scope is contract-private).
- ⚠️ Still **not** done: "undeclared ⇒ reject" and argument type checking (S5), target filling (S6).
- ⚠️ Collection has **no external observation point** yet (`Contract.featags` is a library type):
  this slice's tests pin "valid declarations are accepted" and "each invalid form reports its code".
  It only becomes visible when S5 consumes it — noted here so nobody assumes it was untested.

### Added — S1b: identifier whitelist, gated by `lex.ident`

- New module `sml_lex::identifiers`: `check_ident` / `IdentError`; entry point
  `@feature enable lex.ident` (**off by default**).
- **Allowed**: ASCII letters/digits, `_` `-`, CJK Han (U+4E00–U+9FFF), Kana (U+3040–U+30FF),
  Hangul syllables (U+AC00–U+D7AF).
- **Rejected** (six classes, all sharing `E-LEX-007`, told apart by the detail text):
  `.` `[` `]` (reserved by path syntax), whitespace / control chars (Cc), zero-width and
  bidi controls (Cf), **combining marks (Mn/Mc)**, Cyrillic / Greek / Armenian homoglyphs,
  fullwidth and compatibility characters.
- **NFC for free, zero dependencies**: only **precomposed** code points are accepted and all
  combining marks are rejected (`é` = U+00E9 is fine, `e`+U+0301 is not) — no normalisation
  library, no Unicode tables.
- Positions are reported in **characters**, not bytes (`端口.x` reports the dot at char 3).
- `lex.ident` is a **new bit (19th, second dotted feature)**; off ⇒ v4 behaviour, `a.b: 1` is
  still one key. Fragment-merge keys (`&base`) are not identifiers and are not validated.
- ⚠️ **Deliberate deviation** from the cross-slice rule "every slice lives behind `type.featag`":
  identifier rules have nothing to do with the `!` marker, and sharing the bit would mean
  "enabling featag silently changes every key name" with coarser rollback. Logged in the plan.
- ⚠️ **Per-contract single-script rule is not here**: it compares several identifiers of one
  contract, so it ships with S4.
- ⚠️ The "generate both range tables to five ends" item **only covers Rust this round**
  (other ends have no identifier validation yet; `impls: [ rust ]`). The generator waits until
  cross-end consistency is actually needed — five tables nobody reads are pure overhead.

### Added — featag S3: `__featags` as an ordered array

- With `@feature enable type.featag` the block head's `!` region now **becomes data**:
  `wea !critical obey[1, 2], name { a: 1 }` →
  `{ wea: { __featags: [ { name: "critical" }, { name: "obey", args: [1, 2] } ], __name: "name", __type: "wea", a: 1 } }`
  - an **ordered array**, not a map: duplicates are allowed (`!a !a !b` ⇒ three items in order);
  - a no-arg item carries only `name`; an explicit `[]` keeps an empty `args`
    (`obey[]` and "no argument slot" are different things);
  - arguments are real values (reusing the existing `parse_array` path) — no second, hand-written
    token→Value mini-parser.
- **`__args` is not reused**: that key means "barewords after the block name" (spec §3).
- `__featags` is written **after** contract application, like `__type` / `__name` — it is syntax
  metadata and must not be rejected by a strict contract as an undeclared field
  (pinned by `featags_survive_a_strict_contract`).
- ⚠️ **No declaration checking yet** (undeclared featags and wrong argument types pass) — that is S4/S5.
- ⚠️ **Hand-written `__*` keys are still accepted**: rejecting them is a separate v5 item (§4.5 #3).

### Added — featag S2: block-head `!` region recognition

- With `@feature enable type.featag`, the `!` region after the block head's first word is recognized:
  `wea !critical obey[1, 2], name { }`, `wea !critical, { }` (name optional); no-arg items are
  `!name`, items with args are `name[ … ]`; **`,` terminates** the region (no commas between items).
- **Three ambiguities, three codes** (`docs/swsml/spec/featag.md` §2.3):
  - `E-PARSE-026` **empty featag region** (`wea ! , name { }`);
  - `E-PARSE-027` **`!` bareword in a value position** (`a: !b`) — **`bareword-string` does not exempt it**;
    quote it if you meant a string;
  - `E-PARSE-028` **`!`-prefixed name in a block head** (block-name position after `,`, or the type word).
- **S2 recognizes only, it emits no data**: `__featags` is S3, so the tree after that round gained
  **no key** (a test pins this so S2/S3 cannot silently merge). **S3 now emits the data — see the
  section above.**
- Look-ahead updated: with featag on, `bare_block_ahead` steps over `!name` / `name[…]` and the
  terminating comma — otherwise `wea !critical, name { }` reads as "key `wea` + value `!critical`"
  and misfires on B2 (first attempt did exactly that).
- Gated entirely behind `type.featag`; **without it v1–v4 is unchanged** (`a: !b` stays a bareword,
  `server web prod { }` keeps its `__type`/`__name`/`__args`).
- Cross-end: the three codes list `impls: [ rust ]` — C/C++/Lua have no feature set, JS has no featag,
  and they keep the old rules on the same input.

### Added — feature namespaces + the `type.featag` placeholder bit

- `@feature enable <domain>.<name>` now works (`docs/swsml/spec/features.md` N4):
  `Feature::parse_name` validates the shape before the lookup — **split at the first dot only**,
  three segments (`a.b.c`) are rejected outright; domains are a **closed whitelist**
  `FEATURE_DOMAINS = ["lex","mod","type","flow"]` (N1), and an unknown domain is reported with it.
- **Old names stay flat and get no aliases** (N2): `typed-block` still works, `type.typed-block`
  does **not** (reported as "not registered") — two names for one feature would show up as two
  spellings in diagnostics, `disable`, and the C-ABI echo.
- All three failures share `E-FEATURE-003` and are told apart **by the detail text**
  (three segments / unknown domain / not registered + the available list).
- The rule now lives in **one place**: `@feature enable/disable/whitelist/blacklist`, the contract
  `@requires`, and the C-ABI `opts.features` all go through it (was five copies).
- **`type.featag` is reserved** (bit 18, opt-in, not in `baseline()`, not in any `Edition`'s feature
  set): the bit and name are claimed now so S2 has a stable switch to attach to (bit order only grows).
  ⚠️ In that placeholder round, enabling it **changed nothing**; **S2 now recognizes the block-head
  grammar** (next section), but still **emits no data** (`__featags` is S3) — "switch first, grammar next".
- ⚠️ Cross-end: `type.featag` is **Rust-only**; JS / Lua still report `E-FEATURE-003`
  (the code's note says so).

### Added — value model: shared path syntax (featag S1a)

- New module `sml_value::path`: `Path::parse` / `Seg` / `PathError` plus **`Value::get_path`**.
  One grammar only — key / `.` / `[n]` (0-based) — so `a.b[0].c`, `[0][1]` and `a.[0]` (empty key =
  "current value") all parse.
- **Failures say why**: `NotFound` / `OutOfRange` / `NotAnObject` / `NotAnArray`, each carrying the
  1-based segment number; syntax errors (empty path, empty segment, bad/unclosed index, illegal
  character) are rejected at `parse` time, and `PathError::is_syntax()` tells the two phases apart.
  Out-of-range messages include the real length (`that array has 2 elements`).
- **`Value::get` is untouched**: still `Option`, still "literal key first" (documents whose key
  literally contains `[` keep working; regression pin `bracket_in_key_name_is_not_rewritten`).
  The two are deliberately **not** merged — turning `get` into `Result` would silently change every
  caller, and the compat rule only lives on `get`.
- Facade: `sml::{Path, PathError, Seg}`. Passes under both the default and `preserve_order` profiles.
- The same `Path` is the **write** entry for S6 (featag targets): `segments()` / `last()` /
  `Path::new()` let code assemble targets without going back through a string.
- ⚠️ **No error code yet**: `PathError` is type + message only. S6 maps target errors onto
  `E-CONTRACT-0xx` per `featag-spec-draft.md`, which is when attaching a code becomes decidable.

### Added — parse entry: file + options

- **`parse_file_with(path, ParseOptions)`** (exported from both `sml_parse` and the `sml` facade):
  the first file entry that both **expands includes** and **accepts caller options**. Previously the
  `parse_file*` family hard-coded `ParseOptions::new()` (contract opt-out unreachable from a file),
  while `parse_with` accepts options but only runs `tokenize_spans` and **does not expand includes** —
  so "read a file (with includes) and disable in-document contracts" was **impossible**.
  Returns `ParseOutput` (`value` / `diagnostics` / `contract_map` / `errors`), same shape as
  `parse_with`; with `ParseOptions::new()` the `value` and the error code match `parse_file`
  exactly. Existing entry points are unchanged (pure addition).

### Fixed — `--no-default-features` failed to compile

- `sml::MAX_VALUE_DEPTH` used to be re-exported from `crate::emit`, but `pub mod emit` is gated by
  the seven `emit-*` features ⇒ `cargo check -p swsml --no-default-features` died with
  `error[E0432]: unresolved import crate::emit`, contradicting the documented promise that
  `default-features = false` returns to a fully dependency-free build. It now comes from
  `sml_value` (compiled unconditionally), and `emit::MAX_VALUE_DEPTH` itself is defined as
  `sml_value::MAX_VALUE_DEPTH` so the two can no longer drift. Behaviour and value are unchanged
  (still 128); what changed is that the zero-feature build finally compiles.

### 🚨 Breaking

- `Value::Float` gains a raw-spelling field (`Float(f64, Option<String>)`): pattern matches and
  struct literals need updating. `parse → to_sml` now preserves `1e10` exactly as written.
- Contract symbolic modifiers `?` / `=` are deprecated (use `optional` / `default`); they now raise
  `E-PARSE-006` on every implementation.
- `sml::serde::to_string_direct` **rejects integers beyond `i64`** (`[E-DERIVE-002]`) instead of
  writing bare words that read back as `Str`.

### Added

- Contract strictness three-state: `strict` (default) / `warn` (allow + warn) / `loose` (silent),
  with block-level override (`@is X warn|loose`) and "strictest wins" when mixing contracts.
- External modifiers `one_of` / `exactly_one` / `any_of` / `items_min` / `items_max` / `depends_on`
  (Rust extension point; register via `ParseOptions::with_modifier`).
- `x_only "<prefix>"` (`sml_contract::XOnly`): under `loose` / `warn`, only extra keys with that
  prefix are allowed; mutually exclusive with `strict`; unregistered means disabled.
- `W-PARSE-001` (duplicate key promoted to array) / `W-PARSE-002` (fragment expands over a parent
  key), exposed via `Diagnostic::code()`.
- `ParseOptions::collect_errors(true)` + `*_with_warnings`: several errors per document, including
  `finish_block` and the `@for` sub-parser.
- File entry points **without** `include` now carry line / column in errors.

### Fixed

- Iterative (explicit-stack) parser is on trunk: default-stack threads parse 128 levels and reject
  129+ with `E-LIMIT-001` instead of aborting.
- `svg` / `slint`: nested arrays no longer recurse without increasing depth (was `0xC00000FD`);
  every emit backend now returns `E-LIMIT-004` on over-deep input.
- Lua: integers beyond f64 precision stay strings (no more `3.3010620150307e+17`).
- `include "a.sml" junk` now errors (`E-INCLUDE-012`) instead of silently dropping the tail.
- `smltools`: no hardcoded Chinese error text remains in Rust; messages come from the code table.

### Docs

- Top-level `README.md` / `README.en.md` / `llms.txt` (+ the `site/static/` copy) and book chapters
  5 & 13 synced with the contract modifiers; chapter 13 no longer cites `ext.rs` line numbers.

## [0.6.4] · swsml — 2026-10-15

> 📌 A **docs / packaging revision** right after 0.6.3: parser and API **unchanged**,
> binary identical to 0.6.3. Shipped as its own version so the crates.io README facade
> covers both English and Chinese readers.

### Docs
- The crate README (`rust/README.md`) is now **bilingual** (each passage in English then
  Chinese). crates.io renders only the single file named by Cargo.toml `readme`, and
  versions are immutable — so a bilingual facade requires a new publish.

### Chore
- Repo trim: `git rm --cached` + `.gitignore` about 9 MB of **generated** `.sml`
  (`story/novel.sml` + 7×`story/novel.out.*`, `examples/micro/CH32V103xx.sml`); the
  sources (`story/gen_novel.py`, `CH32V103xx.svd`) stay and can regenerate them — no
  effect on the crate package.
- `llms.txt` (and its `site/static/` copy) release status updated to 0.6.4.

### Dependency sub-crates
- Unchanged: still the already-published alpha versions of `sml-parse` / `sml-codes` /
  `sml-feature` / `sml-contract` / `sml-lex` (shipped alongside 0.6.3).

### Debian packages
- Unchanged: the cdylib binary is byte-identical to 0.6.3, so `libswsml*_0.6.3` stays
  valid; not relabeled for a docs-only bump.

## [0.6.3] · swsml (shipped together with `smltools`) — 2026-10-15

> 📌 This round (TASK-agent-w24): **contracts move from "one contract per block" to
> "several contracts per block"**, programs can finally ask *"which contracts apply to
> this entity?"*, and both contract inference and error positions land. Rust goes first;
> the other implementations hold the same line by **failing loudly** (see below).

### Added · Contract mixin: one block, several contracts

Three spellings, **exactly equivalent** (pinned case by case in
`rust/tests/contract_mixin.rs`, 11 tests):

```sml
api {
    @is HasHost, HasPort        # 1: comma separated
    host: example.com
}
api { @is HasHost HasPort … }   # 2: commas omitted (same as above)
HasHost HasPort api { … }       # 3: block-level (needs `@feature enable typed-block`)
```

- **Merge semantics (D3)**: `strict` wins (if any participating contract is strict, the
  whole block is validated as strict); **order-independent** (fields are unioned by
  name); **a conflict is an error** — the same field name with a different `ty` /
  `default` / `min` / `max` in two contracts raises
  `E-CONTRACT-016 contract mixin conflict`, naming the field and both contracts.
- **The single-contract path is byte-for-byte unchanged**: when only one contract name
  is collected, the old `apply_contract` runs (same error wording included). The P1-3
  red line `server web prod { }` produces exactly what it produced before.
- ⚠️ **Implementation status**: Rust only for now. JS / C / C++ / Lua — see
  "failing loudly" below.

### Changed · Several `@is` in one block: from "last one wins" to "all of them apply"

**Behavioural change** (D4). Previously a second `@is` in the same block silently
overrode the first; it is now synonymous with `@is A B` — the fields of both contracts
are unioned and validated together. Default fill-in, type checks and recursive
composition all run the old code path; only the input changed from one name to a list.

### Added · Contract map API: a program can ask "which contracts apply to this entity?"

- Enabled with `ParseOptions::trace_contracts(true)`. **When it is off, `ParseOutput`
  is byte-for-byte what it was** (`contract_map: None`) — a test pins that guarantee.
- `ContractMap::names_at(&[PathSeg]) -> &[String]`, with
  `PathSeg::Key(String) | Index(usize)` (an `Index` segment is an array position, a
  `Key` segment is a key; SML's own type inference tells them apart, so no new DSL).
- **The interchange form is SML itself** (`ContractMap::to_sml()`):
  `{ path: [ database primary ] contracts: [ Server ] }`; a top-level entity has
  `path: []`; entities without a contract do not appear; output is sorted by path and
  carries **no timestamp** (same input ⇒ byte-identical output).
- Blocks pulled in by `include` are recorded at **their position in the result tree**;
  fields spliced in by a `&base` fragment are not recorded separately.
- ⚠️ The public struct `ParseOutput` **gained a field** (`contract_map`) — see
  "Upgrading" at the end of this section.

### Added · Contract inference: `smltools --infer-contract`

- New `rust/sml-contract/src/infer.rs` (`infer_from_value`), re-exported from the main
  crate as `sml::infer`; `smltools` gained `--infer-contract` (SML `@contract` output by
  default, `--json` for a JSON field table).
- Type mapping (TASK-agent-w24 §7): `Int→int` / `Float→num` / `Bool→bool` / `Str→str` /
  `Null→any` / `Object→sub-contract` (UpperCamelCase from the field name, e.g.
  `address` → `Address`, same-named fields reuse one sub-contract) / `Array→[T]`
  (all-int ⇒ `[int]`, uniform objects ⇒ `[Sub]`, mixed ⇒ `[any]` with a warning).
- `optional` is inferred **only** when merging several instances (a field missing from
  some of them); a single instance is all-`required`, with a note saying so. It
  **never guesses** `default` / `min` / `max` / `enum` / custom `@type`.
- Generated contracts default to `loose`, so adding a field later is not rejected by
  strict mode.
- Deterministic: fields sorted by name, no timestamp, byte-identical for the same
  input; 9 golden unit tests inside `infer.rs` assert the exact output.

### Added · Errors carry a position: `[line N, column M]`

- `Tok` is **untouched** (smallest possible blast radius): the lexer gained a
  `tokenize_spans` side channel (byte offsets), `SmlError` gained
  `pos: Option<usize>`, and `sml-parse` converts the offset into a line/column pair
  that is **appended** to the message at the API boundary. `Display` stays plain text,
  so every old construction point that passes no position keeps its exact wording.
- Known edges: **file entry points (`parse_file` etc.) carry no position** — an
  `include` merges token streams from several files, so there is no single source text
  to map to (better missing than wrong); `E-LEX-*` lexer errors **have no position yet**
  (this round only wired up the parser layer).
- One old behaviour fixed along the way: `scan.rs`'s `strip_version` / `strip_features`
  used to **delete lines**, which is shorter than the original and therefore shifted
  every subsequent line/column. They now do an **equal-length replacement** (directive
  lines become the same number of spaces; CRLF and BOM are replaced with equal-width
  spaces), so offsets map 1:1 onto the source — the same fix JS already had in
  `stripFeatureLines`.

### Added · Other implementations fail loudly on multi-contract blocks: `E-CONTRACT-017`

- JS / C / C++ / Lua: a second bare word after `@is`, or a second registered contract
  in block-level args, raises **`E-CONTRACT-017 contract mixin not implemented`** —
  **never a silent tree**. The single-contract path is byte-for-byte unchanged on all
  four.
- **Why this had to be done**: a mixin **cannot be opt-in** (some of those
  implementations do not even have a `@feature` directive), so shipping Rust alone
  would replay the `typed-block` trap where the other ends silently degrade — a lesson
  already written up in the textbook, §5.2.2.
- JS also lost a **misleading error**: `@is tls, retry` used to produce a **wrong tree
  plus `E-CONTRACT-003`** (which says nothing about contract names); it now correctly
  reports `017`.

### Added · JS contract map API + editor reads the map (hover / go-to-definition)

- `js/sml.mjs` gained `opts.traceContracts` and `contractMap`, aligned with Rust:
  opt-in, `{ key }` / `{ index }` segments, sorted by path, no timestamp, and
  `contractMapToSml` is **byte-identical** to Rust's `to_sml`; **when off, the return
  value is byte-for-byte the old one**. JS still rejects multi-contract blocks loudly
  (untouched).
- The VS Code extension now reads the map: hover lists **all** contract names with
  their declaration, go-to-definition supports several targets, and the previous
  regex-based logic is kept as a fallback.

### Fixed (five, ordered by blast radius)

1. **Quantifier ceiling + program-size gate for patterns** (`sml-pattern`): a single
   quantifier is capped at `QUANT_MAX = 1000`, the compiled program at
   `MAX_PROG = 100_000` instructions, and the run at a `10_000_000` step budget;
   JS is aligned (compile-time estimate, `PATTERN_STEP_BUDGET = 100000`). A rule like
   `次: 999999999` is now **rejected at compile time** instead of being expanded in
   memory (OOM / hang, i.e. DoS).
2. **`required` is OR-ed when merging contracts**: it used to be left out of the merged
   view, which made the result depend on the order the contracts were written in —
   violating D3's "order-independent". It is now a logical **or**: if any contract
   requires a field, it is required.
3. **`smltools` main moved onto a 64 MB stack thread**: the parser promises 128 levels,
   but the Windows main-thread stack is 1 MB by default — it overflowed at **70 levels**
   (`lint_deep_nesting_is_wlint_005`, measured; the previous code merely got lucky).
4. **Include base directory falls back to the current directory when empty**:
   `-i bare-filename.sml` (`parent()` is empty) no longer reports `E-INCLUDE-010`
   (three base computations in `rust/sml-parse/src/api.rs` now `.filter` it).
5. **Not fixed, recorded only**: the library entry points `parse` / `parse_with` can
   still overflow on a 1 MB-stack thread — call the library from a thread with a large
   stack when parsing deeply nested documents.

### Docs / tooling

- README gained a "30 seconds" section in both languages: the same config in JSON /
  YAML / SML, side by side (**pure addition**, no existing paragraph touched; all three
  samples were actually parsed while writing them).
- Textbook: `ch05` gained §5.4.1 "Mixin", cross-referenced against §5.4 "Composition" to
  explain **why one cannot replace the other**; `ch10` documents the block-level
  multi-contract spelling and marks roadmap items; new `ch14 Capability map` (zh + en).
- `rust/fuzz/`: a `cargo-fuzz` scaffold with a `parse` target (coverage-guided fuzzing;
  the ASan runtime is still pending on this machine).
- **VS Code extension 0.4.3**: `@type` gained **go-to-definition and hover** (highlighting and
  completion were already there). Pressing F12 on a contract field's type position
  (`编号: 规则编号`) now jumps to `@type name: 规则编号`; hover shows a **signature**
  (e.g. `DS- <层: NET|IDC|APP|DAT|EGR> - <序号: 数字×4>`) plus the raw pattern, and works both
  on the type name in `@type` and on the contract field that uses it. When a signature cannot be
  rendered (nested `组` / `序列`), the hover **falls back to the raw source** rather than showing
  a wrong one. Assertions added in `scripts/_verify_ext.mjs`.

### Upgrading (this round)

- **Behavioural change**: several `@is` in one block now **all apply** instead of the
  last one winning (see above).
- **Public struct gained a field**: `ParseOutput` has a new
  `contract_map: Option<ContractMap>`. Code that only **reads** the result is
  unaffected; **only code that builds a `ParseOutput` with a struct literal** is
  affected (same rule as 0.6.2: adding a field is an API change, so it is called out
  here).
- Everything else (mixin, contract map, inference, error positions) is **pure
  addition** — existing paths are byte-for-byte unchanged.
