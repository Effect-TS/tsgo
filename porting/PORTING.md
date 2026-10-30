# Porting guide: effect-tsgo (Go) -> Rust

This repository's Effect Go code is ported to Rust on top of [tsrs](https://github.com/oxc-project/tsrs), a
Rust port of typescript-go, and the port is kept in step with the Go: every change to in-scope Go code updates the
Rust port in the same pull request. The plan, phases and policies are in [`TSRS_PORT.md`](TSRS_PORT.md). This file has the
rules for writing the port: read it completely before you port anything. The shim-to-tsrs API map is
[`TSRS_API.md`](TSRS_API.md); accepted differences are [`DIVERGENCES.md`](DIVERGENCES.md). The setup follows tsrs's
own port (its `porting/PORTING.md`, readable in `tsrs/` after `pnpm exec repoctl submodules setup --component tsrs`),
adapted to this repo.

Rust work needs the tsrs setup: `pnpm exec repoctl submodules setup --component tsrs` checks out the TypeScript
next compiler and tsrs, applies both patch stacks and regenerates the Go shims and `crates/ets_shim`, so Go and Rust
both build in one checkout. `pnpm setup-repo` resets to the Go-only setup and removes `tsrs/`. `pnpm check`, `lint`
and `test` run no Rust (only the port bookkeeping check); the Rust workspace is `pnpm exec repoctl tsrs
check|lint|test`.

## The fidelity rule: Go is the spec

The Go code at the current commit of this repository is the specification. Both live in the same tree, so the spec
moves with every merged pull request (see "Parity policy" in `TSRS_PORT.md`).

- The port is **mechanical and faithful**: same algorithms, same function decomposition, same order of operations,
  same observable behavior. Every Rust item should be recognizable next to its Go original. Do not redesign and do
  not "improve": behavior that matches the Go code is correct, and a difference is a bug even when the Rust output
  looks more reasonable. If Go has a quirk, port the quirk.
- Observable behavior is everything a user, an editor or a test can see: diagnostics (message text, code, category,
  range, order), quick fixes and refactors (titles, kinds, edits), completions, hover text, inlay hints, document
  symbols, the CLI diagnostics mode output and exit codes, config diagnostics, and every baseline file in
  `testdata/baselines/reference`.
- Internal-only differences (data layout, ownership, how caches are held) are fine when nothing observable changes;
  the sections below fix the ones that come up every time.
- If the Go code is wrong, fix the Go (with the Rust in the same pull request); never fix only the Rust.
- Never port from memory of how effect-tsgo, the TypeScript language service plugin or TypeScript used to work. Read
  the current source.

### One allowed source of difference: the TypeScript version

The Go build uses the TypeScript commit selected in `_packages/tsgo/upstream.json`; the Rust build uses tsrs, which
pins its own TypeScript snapshot. Rust is compared against a Go build **at tsrs's TypeScript version**
(`TSRS_PORT.md`, "Decisions"). A difference that the Go build at that version also shows is a TypeScript-version
difference, not a port bug; record it in `DIVERGENCES.md` with that evidence. Never assume it: most differences are
port bugs.

## Scope

[`porting/scope.toml`](scope.toml) is the authority; `repoctl tsrs port-*` reads it.

- In scope: `etscore`, `etscheckerhooks`, `etsdiagnostics`, `etsexecutehooks`, `etslshooks`, `etsgoapi`,
  `etstesthooks` (only if the Rust test harness needs fourslash mounting), and every package under `internal/`,
  including `internal/effecttest` (it becomes the Rust baseline runner).
- Not ported: `etsoxlintrunner` and the oxlint/tsgolint integration (Go-only by decision); Go tooling (`_tools/`,
  `shim/`); the TypeScript patches themselves (their tsrs counterparts live in `_patches/tsrs`, see "TypeScript:
  tsrs"); generated Go files (`// Code generated ... DO NOT EDIT.`, e.g. `etscore/version_generated.go`), whose
  Rust counterparts are generated from the same inputs (`build.rs` or the same generator).

## Layout

**One crate per Go package.** Go's import graph is acyclic only at package granularity, so grouping packages into
bigger crates creates cycles; one crate per package keeps the dependency graph identical to Go's and the mapping
mechanical. `pnpm exec repoctl tsrs port-crate <package>` prints the name; never invent one.

| Go package                          | Rust crate |
| ----------------------------------- | ---------- |
| `internal/<path>`                   | `ets_<path with non-alphanumerics as _>` (`internal/rules` -> `ets_rules`, `internal/typeparser` -> `ets_typeparser`) |
| `<path>` at the root (`etscore`, `etslshooks`, ...) | `ets_<path>` (`etscore` -> `ets_etscore`, `etsgoapi` -> `ets_etsgoapi`) |
| `shim/<pkg>` (generated Go shims)   | the generated `ets_shim` crate, module `ets_shim::<pkg>` |
| (no Go package) the executable      | a Rust-only binary crate that registers the hook crates and runs tsrs's CLI/LSP |

Each crate lives at `crates/<crate name>/`. Within a crate:

- **One Rust file per Go file**, same base name: `effect_type.go` -> `src/effect_type.rs`. Declarations stay in the
  Go file's order. A very large Go file may be split into `name_1.rs`, `name_2.rs`, ... by line range.
- A Go package is one namespace, so `src/lib.rs` declares every file module and re-exports it
  (`mod effect_type; pub use effect_type::*;`), and files refer to each other through `use crate::*;`.
- `foo_test.go` in the same package -> `src/foo_test.rs`, declared as `#[cfg(test)] mod foo_test;`. An external test
  package (`package foo_test`) -> `tests/foo_test.rs`; several such files sharing helpers ->
  `tests/<package>_test/main.rs` with a module per Go file.
- Test fixtures (`testdata/` next to a package) are copied byte for byte to `crates/<crate>/testdata/`, or read in
  place from the Go package's directory when both builds must share them (prefer reading in place: one copy).
- Shared baselines and test cases (`testdata/tests/**`, `testdata/baselines/reference/**`) are read in place by
  the Rust runner; never copied.

A new crate's `Cargo.toml`:

```toml
[package]
name = "ets_rules"                    # pnpm exec repoctl tsrs port-crate internal/rules
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false
description = "Port of effect-tsgo internal/rules"

[lints]
workspace = true

[dependencies]
ets_shim.workspace = true              # the only way to reach tsrs
ets_typeparser.workspace = true        # Go import edges, one for one
```

Add the crate to the root `[workspace.dependencies]` (`ets_x = { path = "crates/ets_x" }`) so dependents write
`ets_x.workspace = true`. Never add a `tsrs_*` dependency to an `ets_*` crate (other than `ets_shim`); `repoctl tsrs
port-check` / lint rejects it.

## Origin markers

Every ported Go declaration (function, method, type, const/var group) carries an origin marker on the line above the
Rust item (above its attributes and comments):

```rust
// go: internal/typeparser/effect_type.go:36 TypeParser.EffectType 8a9c52f8a845e081
pub fn effect_type(&self, t: &'static Type, node: &'static Node) -> Option<Effect> {
```

- The path is relative to this repository's root.
- The line is the Go declaration's line, a navigation hint only: `port-check` ignores it (a declaration is identified
  by path and name), `port-fix-lines` and `port-ack` refresh it.
- The name is `Func`, `Recv.Method` (receiver type without `*` or type parameters), a type name, or the first name of
  a `const`/`var` group (the group is ported as a whole), with `#2`, `#3`, ... for a name repeated in one file
  (several `init`s). Copy markers from `repoctl tsrs port-todo <prefix>`, which prints them without a hash.
- The trailing 16-hex-digit hash is the hash of the Go declaration the Rust was last synced to. Never write it by hand:
  `repoctl tsrs port-ack` writes it once the Rust is faithful. A marker without a hash is "not acked" and fails
  `port-check`; so does a marker whose hash differs from the current Go declaration's (Go changed since the port was
  synced).
- One Go declaration may become several Rust items (each carries the same marker; `port-ack` updates all of them), and a struct's methods may land in
  one `impl` block, but a Rust item never mixes code from two Go declarations.
- Ported tests carry `// go test: <path>:<line> <Name> <hash>` instead. Test declarations (`test`/`testhelper` kinds,
  plus every declaration of the `test_packages` in `porting/scope.toml`) are in the manifest and acked like code, but
  reported separately by `repoctl tsrs port-status`; `check` rejects a `// go:` marker on a test declaration and vice
  versa. Say in a comment which subtests are left out, and why.
- Hand-written Rust with no Go original carries `// Rust-only: <why>` (a tsrs gap helper, a `pub fn init()` that
  replaces Go `init()` registration, `ets_shim` glue).
- Keep Go comments that explain *why*. A Go doc comment stays a plain `//` comment, verbatim, between the marker and
  the item (Go prose trips rustdoc lints); `///` is for Rust-only items. No `// ported from` banners.
- A declaration whose Rust is complete except for a callee in a crate another agent owns (a typeparser method not
  ported yet) keeps the call site as `// TODO(port: needs <crate> <Name>)` with the Go code quoted, and is not acked.
  `check` fails on a marked but unacked declaration, so its marker is written
  `// TODO(port: unacked until <Name> is ported; restore the marker) go: <path>:<line> <Name>` (the marker pattern
  does not match it). When the callee lands: restore the plain marker, replace the TODO with the call, then
  `port-ack` (which also writes the hash and the current line).

### The porting files

| File | Kind | Content |
| ---- | ---- | ------- |
| `porting/scope.toml` | hand-written | in-scope packages and files, exclusions, options (`require_new`, on: every in-scope declaration must be ported or skipped) |
| `porting/skipped.tsv` | hand-written | `path`, `name`, `reason` of declarations deliberately not ported ("What not to port") |

Nothing generated is committed. The **manifest** (every in-scope Go declaration: path, line, name, kind, hash) is
computed on the fly by `_tools/port_manifest` (a Go program that parses the in-scope packages, a fraction of a second)
each time a `port-*` command runs; `repoctl tsrs port-manifest` prints it. The synced-to state lives in the markers'
hashes. So a Go edit rewrites no bookkeeping file, and two pull requests acking neighboring declarations do not
conflict.

A declaration is **ported** when a marker names it (acked: the marker has a hash), **skipped** when `skipped.tsv` lists it,
and otherwise still to do. The port is complete and `require_new` is on, so a declaration still to do fails
`repoctl tsrs port-check`: a pull request that adds in-scope Go code ports it (or skips it with a reason) itself.

### Commands

Run as `pnpm exec repoctl tsrs <command>` (the `repoctl` bin of `_tools/repoctl`, as for every other repoctl
command). The bookkeeping commands need only Node and Go, no tsrs checkout:

| Command | What it does |
| ------- | ------------ |
| `port-manifest` | print the in-scope Go declarations with their current line and hash (for debugging) |
| `port-status [prefix] [--files]` | progress per package (or per file) |
| `port-todo <prefix>` | declarations still to do, with markers ready to copy |
| `port-check [--summary <file>]` | the gate (run by `pnpm check`, and in CI by the Validate tsrs workflow, before the Rust build, which also posts its summary table): malformed markers; markers naming unknown declarations or with the wrong `go:`/`go test:` form; a declaration both skipped and marked; a marker without a hash (not acked); a marker hash mismatch (Go changed since the port was synced); with `require_new`, an in-scope declaration neither ported nor skipped. A stale marker line number is not an error |
| `port-fix-lines` | rewrite marker line numbers after Go lines moved (hashes untouched) |
| `port-ack <path> <name>...`, `port-ack --prefix <p>`, `port-ack --all` | write the current Go hash (and line) into every marker of these declarations |
| `port-diff <oldRef> <newRef>` | declarations removed, moved, changed and added between two commits, with their port status |
| `port-crate <pkg...>` | crate names for packages |
| `port-deps <pkg>` | the package's in-scope imports and whether each is ported |
| `port-ready` | packages whose imports are all ported, largest first |
| `check`, `lint`, `test` | the Rust workspace: `cargo check`; `cargo fmt --check` and Clippy with `-D warnings`; `cargo test --workspace` (with the baseline runner), then the tsrs binary's `--version`. They need the tsrs setup and fail without it; `pnpm check`, `lint` and `test` run no Rust |

### The workflow

1. `repoctl tsrs port-todo <pkg>/` -> write the Rust items with their markers.
2. `repoctl tsrs port-check` -> it reports the new markers as not acked.
3. When the Rust is complete and faithful for a declaration (and its tests pass), `repoctl tsrs port-ack <path> <name>`
   (or `port-ack --prefix <pkg>/` for a fully ported package).
4. When Go changes later, `check` reports a hash mismatch: read the Go change (`git diff origin/main -- <file>`),
   apply the same change to the Rust, then `port-ack` (and `port-fix-lines` for the line numbers of the declarations
   you did not touch). The `port-change` skill automates this.

Ack records "the Rust is in sync with this Go text". Acking without updating the Rust defeats the parity policy
and is rejected in review.

## Naming

As in tsrs (`tsrs/porting/PORTING.md`, "Naming conventions"):

- Functions and methods: snake_case of the Go name, always. Acronym runs are one word: `parseJSONConfig` ->
  `parse_json_config`, `ID`/`Id` -> `id`. A Rust keyword gets a trailing `_` (`type_`, `ref_`, `match_`).
- Exported Go identifiers -> `pub`; unexported -> `pub(crate)`. Package-level functions -> free functions.
- Types keep their Go names (`effectGenCall` -> `EffectGenCall`: capitalize the first letter only). Struct fields
  and variables: snake_case. Constants: `SCREAMING_SNAKE_CASE`; a Go `iota` enum becomes a Rust `enum` whose
  variants drop the shared type prefix (`SeverityError` -> `Severity::Error`).
- A named Go type over `string` or a number with neither constants nor methods is a type alias.
- A Go named type whose values are not limited to its constants (decoded from config, converted from outside, a
  `default:` case handles the others) -> a newtype over the underlying type with associated consts named like enum
  variants.
- A package with both `fooBar` and `FooBar`: the exported one gets the suffix `_exported` (tsrs's convention); two
  types that clash the same way: the exported one gets the suffix `Exported`.
- Getter `Foo()` over a private `foo` -> `foo()`; setter `SetFoo` -> `set_foo`.
- Go bit-flag types -> `bitflags!` with the Go constant name minus the type prefix.
- Rule, fixable and refactor names, diagnostic codes and message texts are data: copy them verbatim.

## Types

| Go | Rust |
| -- | ---- |
| `string` | `String` owned, `&str` borrowed (parameters); tsrs's AST text is `&'static str` |
| `[]byte` | `Vec<u8>` / `&[u8]` |
| `int`, `int64`, `uint32`, ... | the same width: `int` -> `i64`, `int32` -> `i32`, `uint32` -> `u32`; never change signedness or width. tsrs positions are `u32` (Go's `int`): widen to `i64` where Go computes with `int` (negative intermediates) |
| `float64` | `f64` |
| `[]T` | `Vec<T>` returned/stored, `&[T]` parameter. Nil and empty are both `Vec::new()` unless the difference is observable (JSON `null` vs `[]`, `reflect.DeepEqual` in a test): then `Option<Vec<T>>` |
| `map[K]V` | `FxHashMap<K, V>` (`rustc_hash`). Go map iteration order is random: code with observable order sorts first in Go; port the sort. Where Go relies on insertion order (an ordered slice next to a map), port both |
| `*T` that may be nil | `Option<&T>` / `Option<Box<T>>` / `Option<Rc<T>>` by ownership |
| `*ast.Node`, `*checker.Type`, `*ast.Symbol`, ... (tsrs arena objects) | tsrs's handles (`P<Node>`, `&'static Type`, `P<Symbol>`; see `TSRS_API.md`); `Option<...>` where Go may hold nil |
| `*T` of a package-internal object graph | `Rc<RefCell<T>>` when Go writes through one pointer and reads through another after sharing; `Rc<T>` when complete before sharing; `&mut T` / `&T` parameters for the rest. Decide per type from the Go code and write the decision on the type |
| struct embedding | a named field (`base: Base`) plus explicit forwarding methods for the promoted ones that are used |
| embedding of a tsrs type whose whole API the Go code uses through the embedder (`rewriter.Tracker` embeds `*change.Tracker`) | a named field plus `Deref`/`DerefMut` to it: the embedder's own methods shadow the promoted ones of the same name, as in Go |
| a Go struct holding both `*checker.Checker` and `*typeparser.TypeParser` (`rule.Context`, `fixable.Context`) | hold the `ets_typeparser::TypeParser<'c>` (it borrows the checker mutably) and reach the checker through a `checker()` accessor that forwards to `type_parser.checker()` |
| interface with a closed set of implementations in the module | `enum` |
| interface used as an extension point (rules, fixables, refactors registered by other packages, test fakes) | trait object (`Box<dyn Trait>`, `&'static dyn Trait`) |
| `(T, bool)` | `Option<T>` |
| `(T, error)` | `Result<T, Error>` with Go's error text (see "Errors") |
| `any` / `interface{}` | an `enum` of the cases that occur |
| generics `[T any]` | generics `<T>` with the bounds the body needs |
| a long-lived Go object that stores `*TypeParser` next to its own state while the caller keeps using the parser (`ApiStabilitySession`) | the Rust object owns only its state; each method takes the parser as a `tp: &mut TypeParser<'_>` parameter (the parser borrows the checker mutably, so it cannot be stored) |
| `lazy[T]`-style memoized closures | `std::cell::OnceCell<T>` (or the crate's existing helper), computed on first read with the same inputs Go captures |
| `context.Context` passed to tsrs | tsrs's `Context` (via `ets_shim`) |
| `sync.Mutex` / `Once` | `parking_lot::Mutex`, `std::sync::OnceLock` |
| package-level registries filled by `init()` (rule lists, hook callbacks) | a `static` initialized by the crate's `pub fn init()` (see "Go runtime equivalents") |

Use `rustc_hash::{FxHashMap, FxHashSet}`; the std SipHash maps are banned by `clippy.toml`.

More mappings (worked out with `etscore` and its dependents):

- A Go method that checks for a nil receiver (`func (e *EffectPluginOptions) GetX() ... { if e == nil {...} }`)
  -> an associated function taking `e: Option<&Self>` (`EffectPluginOptions::get_x(opts)`); `&self` otherwise.
- A Go closure that captures the `*TypeParser` (subject and transformation predicates, lazy computations) ->
  a `&dyn Fn(&mut TypeParser<'_>, ...)` (or `new_lazy(|tp| ...)`) that gets the parser as an argument: the parser
  holds `&mut Checker`, so a closure cannot also keep it.
- A Go string type used only through its constants (`type DispatchConditionKind string`) -> an enum with
  `as_str()`; a Go integer flag type -> a newtype with associated consts.
- A Go `*[]T` / `*map[K]V` field ("set or not" in an override) -> `Option<Vec<T>>` / `Option<FxHashMap<K, V>>`:
  a non-nil pointer to a nil slice is not told apart from one to an empty slice (both encode as `[]`).
- Go `append([]T(nil), s...)` of an `Option<Vec<T>>` field (nil when `s` is empty) keeps that nil-ness: write a
  small `append_nil` helper, do not `clone()`.
- Go `int(f)` of a `float64` from config: truncate, and give `i64::MIN` for NaN and out-of-range values as amd64
  does (`ets_etscore`'s `go_int_from_float64`); never a bare `as i64`, which saturates.
- `strings.ToLower` / `strings.EqualFold`: `ets_shim::core::stringutil::{go_strings_to_lower, equal_fold}` (Go's
  tables). `ets_etscore` cannot depend on tsrs (below) and uses `ets_go::strings::{to_lower, trim_space}`.
- `etscore` is a leaf crate, as the Go package is: tsrs's `CompilerOptions` holds its `EffectPluginOptions` (patch
  007), so `ets_etscore` must not depend on `ets_shim`. The `any` that `ParseFromPlugins` type-switches on is the
  trait `ets_etscore::AnyValue` (one method per type assertion); tsrs implements it for `CompilerOptionsValue`
  in its port of patch 013 (the orphan rule rules out an Effect crate), tests implement it for their own value type.
- Go's `regexp` patterns spelled for the `regex` crate: RE2's `\s` is `[\t\n\f\r ]` (Rust's ASCII `\s` adds
  `\v`) and `\w` is `[0-9A-Za-z_]` (Rust's is Unicode); write the classes out (`ets_directives`).
- Go `reflect` over a struct's fields and tags (`reflect.TypeOf(etscore.EffectPluginOptions{})` in the schema
  generator test) -> a Rust-only `pub const TYPE: ets_go::reflect::StructType` in an `impl` right below the struct,
  listing every Go field in order with its Go name, kind and verbatim tag text (`StructTag::get` is Go's parser).
  When the Go tags change, the struct's hash changes and its `TYPE` is updated with it.
- A test that builds and encodes Go `map[string]any` documents with `encoding/json` (`options_schema_test.rs`): a
  test-local `Any` enum with Go's reference semantics for maps (`Rc<RefCell<BTreeMap>>`, keys in Go's sorted
  encoding order) and an encoder with Go's HTML-safe escaping and float format; tsrs's `json::marshal` follows
  encoding/json v2 (no HTML escaping) and does not match.

Mappings of the rule framework (`ets_rule`, `ets_rules`, `ets_rulerunner`):

- Go code that holds or passes both a `*checker.Checker` and a `*typeparser.TypeParser` over it: the Rust
  `TypeParser<'c>` borrows the checker mutably, so only the parser is held or passed and `tp.checker()` gives the
  checker. `rule::Context` has a `type_parser: &mut TypeParser` field and a `checker()` method for Go's `Checker`
  field; a Go function `AnalyzeX(tp, c, sf)` becomes `analyze_x(tp, sf)`.
- A package-level `var X = rule.Rule{...}` -> `pub static X: LazyLock<Rule>` (snake case of the Go name, upper
  case); `Codes` calls `Message::code()`, which is not `const`. `Name`, `Group` and `Description` are
  `&'static str`, `Run` is a `fn(&mut Context<'_, '_>) -> Vec<P<Diagnostic>>` (a closure without captures). A Go
  recursive `walk` closure becomes a nested `fn walk(...)` taking what the closure captured.
- Effect diagnostic messages (`tsdiag.This_..._effect_<rule>`) are `ets_shim::diagnostics::This_..._effect_<rule>`
  with Go's identifiers (tsrs patch 008 generates them from `internal/diagnostics/effectDiagnosticMessages.json`).
- `ctx.NewDiagnostic(sf, loc, message, related, args...)` -> `ctx.new_diagnostic(sf, loc, &message, &related, &[args])`.
- A message identifier too long for rustfmt to lay out the rule's `pub static` (rustfmt then leaves the whole
  initializer unformatted) is bound once to a private Rust-only `static MESSAGE: &Message = &tsdiag::...;` (several
  messages: descriptive names) used for `Codes` and every `NewDiagnostic`, as `async_function.rs` does.
- The shim accessors `checker.Checker_getSymbolOfDeclaration` and `Checker_getSymbolIfSameReference` are
  `ets_typeparser::{get_symbol_of_declaration, get_symbol_if_same_reference}` (Rust-only helpers, see `TSRS_API.md`,
  "Gaps"), taking the checker as `tp.checker()`.
- A Go closure over a loop's local state (`flush`, `finalizeChunk`) -> a nested `fn` taking that state as `&mut`
  parameters; a package-level `map[string]bool` that only holds `true` -> an `FxHashSet` in a `LazyLock`; a slice of
  positional struct literals -> a `static` slice built with a Rust-only `const fn` constructor.
- A Go test helper that returns a `*rule.Context` (and a `done`) -> a value that owns the program, the checker handle
  and the context's options, with `with_context(|ctx| ...)` building the `TypeParser` and the `rule::Context`; every
  rule run inside one call shares the parser, as Go's runs on one context do (`ets_rules`'s
  `api_stability_leak_test.rs`, `ApiStabilityRuleContext`).

### Adding a rule to `ets_rules`

`ets_rules::ALL` (`crates/ets_rules/src/rules.rs`) lists Go's whole `All`, in Go's order; the rules that are not
ported yet are commented-out entries (`// FLOATING_EFFECT,`). To add a rule:

1. Port its Go file to `crates/ets_rules/src/<file>.rs` (the `pub static`, its `Analyze*` function and match type,
   and the file's helpers), add `mod <file>; pub use <file>::*;` to `lib.rs` (alphabetical), and add any Go import
   the file brings in (`ets_importstyle`, `ets_layergraph`, ...) to `crates/ets_rules/Cargo.toml`.
2. Uncomment its entry in `ALL` as `(*FLOATING_EFFECT).clone(),`, in place: never move entries.
3. `cargo test -p ets_rules` checks that `ALL` is exactly Go's `All`, in order.
4. Port the rule's tests (`internal/rules/<rule>_test.go`, `internal/rulerunner/<rule>_test.go`), run
   `cargo test -p ets_effecttest -- <rule>`: every baseline of the rule must match (a mismatch fails).
5. `port-fix-lines`, `port-ack` the rule's declarations (and `internal/rules/rules.go All` if its hash changed), `port-check`.

### Porting a fixable, refactor or completion

The registries follow `ets_rules::ALL`: `ets_fixables::ALL` (`crates/ets_fixables/src/fixables.rs`),
`ets_refactors::ALL` (`crates/ets_refactors/src/refactors.rs`) and `ets_completions::ALL`
(`crates/ets_completions/src/completions.rs`) list Go's whole `All`, in Go's order, with the entries not ported yet
commented out (`// FLAT_MAP_TO_MAP_FIX,`). `ets_etslshooks` (port of `etslshooks`) registers them with tsrs's
language service exactly as Go does: `EFFECT_FIX_PROVIDER` (all `ets_fixables::all_error_codes()`, dispatching
through `by_error_code`), `EFFECT_REFACTOR_PROVIDER` (every refactor in `ets_refactors::ALL` when refactors are
enabled) and `after_completion` (every completion in `ets_completions::ALL`). The frameworks are ported:
`ets_fixable::{Fixable, Context, FixAction}`, `ets_refactor::{Refactor, Context, RefactorAction}`,
`ets_completion::{Completion, Context}` (each context holds the `TypeParser` and reaches the checker through
`checker()`).
A Go `Run` closure that uses `ctx.TypeParser` (or `c`) while editing cannot borrow the parser while
`new_refactor_action` borrows the context: build that action with the Rust-only
`ets_refactor::Context::new_refactor_action_with_type_parser(description, |tracker, tp| ..)`, which lends the
parser to the closure (`structuralTypeToSchema`, `writeTagClassAccessors`). Generators that Go builds over the
tracker and the parser (`schemagen.NewStructuralSchemaGen`) borrow the tracker and take the parser per call.

1. Port the Go file to `crates/ets_fixables/src/<file>.rs` (or `ets_refactors`, `ets_completions`): the `pub static`
   (`pub static FLAT_MAP_TO_MAP_FIX: LazyLock<Fixable>`, `LazyLock<Refactor>`, `LazyLock<Completion>`), its `run`
   function and the file's helpers; `mod <file>; pub use <file>::*;` in `lib.rs`; the file's Go imports in the
   crate's `Cargo.toml`.
2. Uncomment its entry in `ALL` in place, as `(*FLAT_MAP_TO_MAP_FIX).clone(),`.
3. Port the Go tests of the file, if any (`internal/completions/*_test.go`).
4. Validate against the language-service runners of `ets_effecttest`, which drive a real tsrs language server
   through `ets_fourslash` (below) exactly as Go's drive typescript-go's:
   - a fixable: `cargo test -p ets_effecttest -- QuickFixes/<case>` writes `<case>.quickfixes.txt` (the
     diagnostics of every file, each with its quick-fix titles, then every non-"Disable" fix applied and undone);
   - a refactor: `cargo test -p ets_effecttest -- Refactors/<case>` runs the cases of
     `testdata/tests/effect-v{3,4}-refactors` (the selections of the first-line `// refactor:` comment) and writes
     `<case>.refactors.txt`;
   - a completion: there are no completion baselines; port the assertion tests (`internal/effecttest/
     completion_test.go`, `internal/completions/fourslash_test.go`) over `ets_fourslash` (add the fourslash
     helpers they call, ported from typescript-go's `fourslash.go`, as `VerifyQuickInfoAt` was for the hover tests).
   A quick-fix baseline also needs the case's diagnostics (the rule) and every other fixable the case offers, so
   most quick-fix baselines need all of them ported. Every baseline must match (a mismatch fails).
5. `port-fix-lines`, `port-ack` the file's declarations and `All` if its hash changed, `port-check`.

Mappings of the fixable bodies (`ets_fixables`):

- Go's `ctx.NewFixAction(fixable.FixAction{Run: func(tracker) {...}})` closure becomes a `Box::new(move |tracker|
  ...)` that moves the match fields it reads (`P<Node>`s are `Copy`; move the whole match when the closure needs
  several owned fields). A closure that calls `ctx.BytePosToLSPPosition` borrows the context through a shared
  `let ctx_ref: &Context = ctx;` used for both `new_fix_action` and the closure.
- A Go closure that also calls the type parser (`runEffectInsideEffect`'s scan for the `runtime`/`context`
  variables) cannot borrow it inside `new_fix_action`: the parser-only part (reads of the source and the checker,
  no tracker edits) runs just before `new_fix_action`, in the same order, and the closure moves its results.
- `tracker.NewIdentifier(s)` / `NewStringLiteral(s, ...)` with a computed `string`: tsrs's factory takes
  `&'static str`, so the text is `ets_shim::core::alloc_str(&s)` (the AST arena, as tsrs's own code fixes do).
- A long diagnostic-message identifier in `ErrorCodes` is bound to a private `static MESSAGE: &Message` (as for
  rules), so rustfmt formats the `pub static`.
Mappings that come up in these files (worked out with `ets_refactors`):
- A Go helper that takes `*rewriter.Tracker` only to build nodes (`tracker.NewIdentifier`, `DeepCloneNode`, ...)
  takes `&NodeFactory` (callers pass the tracker: it derefs to its factory); one that edits (`ReplaceNode`,
  `InsertNodeBefore`) takes `&mut Tracker`.
- A Go `string` passed to a factory method that takes `&'static str` (`NewIdentifier(name)`,
  `NewStringLiteral(name, 0)`): `ets_shim::core::alloc_str(&name)` (tsrs's arena, as tsrs's own code fixes do).
- A Go callback with a `*rewriter.Tracker` parameter called from a `NodeVisitor` callback (`TransformAwaitExpr`):
  an `Rc<dyn Fn(&NodeFactory, ...)>` (the visitor's closure is `'static`); state the Go closure mutates is shared
  through `Rc<Cell<_>>` / `Rc<RefCell<_>>`.
- A Go `RefactorAction.Run` / `FixAction.Run` closure that also uses the checker (`c.TypeToStringEx`): the
  context is borrowed while it builds the action, so the closure cannot reach the checker; compute those values
  before `new_refactor_action` / `new_fix_action` and move them in (printing does not depend on the tracker), and
  say so on the declaration (`layer_magic.rs`).
- `action.Kind = "refactor.rewrite.effect.x"`: `action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.x")`.
- `typeparser.ModuleReference` (a `[]string` path) is the newtype `ets_typeparser::ModuleReference`, which derefs to
  `[String]`; Go's `ref.Append(names...)` is `ref.append(&[..])` and `ref.String()` is `ref.string()`.
  `tracker.NewPropertyAccessPath(ref...)` / `NewQualifiedNamePath(ref...)` are ported on the node factory as the
  trait `ets_rewriter::NodeFactoryPaths` (so helpers that take `&NodeFactory` call them after `use
  ets_rewriter::NodeFactoryPaths;`), with forwarding methods on `Tracker`; they return the node and panic on an
  empty path instead of Go's nil, since every caller passes a module reference.
Mappings that recur in fixables (`ets_fixables`):
- A Go `fixable.FixAction{Run: func(tracker) {...}}` closure -> `Box::new(move |tracker| ...)` that moves the
  (owned) match in; Go's `return []ls.CodeAction{*action}` inside the match loop stays a loop with `continue`s.
- A Go string passed to a factory `New*` that takes text (`tracker.NewIdentifier(name)`,
  `NewStringLiteral(tag, 0)`) -> `ets_shim::core::alloc_str(&name)`: tsrs's factory takes `&'static str`, and tsrs's
  own code fixes copy dynamic text into the arena the same way. Go's token-flags `0` is `TokenFlags::None`.
- Go's nil checks on match fields that the Rust match types hold as non-optional (`match.Transformation == nil`,
  `match.CalleeNode == nil`) drop, with a `// Rust:` note on the function.
- A fixable's `tsdiag` message too long for rustfmt to lay out the `pub static` is bound to a Rust-only
  `static MESSAGE: &Message` (`MESSAGE_1`, `MESSAGE_2` for several), as for rules.

### Errors

Error text is observable when it reaches a diagnostic, a config diagnostic or the CLI. Keep Go's text byte for byte.
`errors.New` sentinels compared with `errors.Is` become a value compared by identity (an enum variant or a
`static`); `fmt.Errorf("...: %w", err)` keeps the wrapped error reachable. A nil `error` is `None`/`Ok`. `panic` ->
`panic!` only where Go panics; a Go `recover` boundary becomes `std::panic::catch_unwind` at the same place (the rule
runner's per-rule recovery, for instance).

## Go runtime equivalents

Effect code uses a small part of Go's standard library, but some of it shows through in output. Use one mapping
everywhere, not one per package:

- **`fmt`**: `%s`, `%d` on plain values map to `format!`. `%v`/`%q`/`%+v` of non-trivial values, floats and
  quoting must reproduce Go's output: write a Go-shaped helper once (shared crate, `// Rust-only:`), never `{:?}`.
- **`strconv`**: `Quote`, `FormatFloat`, `ParseInt`/`Atoi` syntax differ from Rust's; port the used ones as helpers.
- **`strings`**: `ToLower`/`ToUpper` use Go's simple per-rune mappings (not `str::to_lowercase`), `EqualFold` uses
  simple case folding, `Fields`/`TrimSpace` split on Go's `unicode.IsSpace`. Most Effect code works on ASCII
  identifiers; where non-ASCII input can reach the call, use the Go-faithful helper.
- **`sort` / `slices`**: `sort.Slice` and `slices.SortFunc` are unstable but deterministic (pdqsort) and Rust's
  `sort_unstable_by` breaks ties differently. Where ties can reach output, port Go's algorithm (or prove the keys
  are unique and say so); `sort.SliceStable` / `slices.SortStableFunc` -> `sort_by`.
- **`encoding/json`**: serde with the Go field names (`#[serde(rename = ...)]` from the `json:` tags), `omitempty` ->
  `skip_serializing_if`. Output Go writes with `encoding/json` must match its encoder (HTML escaping, map key order,
  float format). Decoding of plugin options follows `etscore`'s own parser, which is ported as code.
- **`regexp`**: Go's RE2 syntax and ASCII classes; use the `regex` crate behind a Go-faithful front end where the
  pattern is not a fixed literal known to mean the same in both.
- **`reflect`, `testing/fstest`** (tests): replace with explicit code; `fstest.MapFS` -> tsrs's `vfstest` map FS.
- **Package `init`**: a Go `init()` that registers something (checker hooks, LS hooks, rule registries) becomes a
  `pub(crate) fn` with the `init` marker, called from the crate's `pub fn init()`, which runs them once
  (`std::sync::Once`) in Go's order (by file name). The binary calls each hook crate's `init()` in the order the Go
  main imports them. No life-before-main (`ctor`).
- **Build tags**: a Go file selected by a build tag the release uses becomes a cargo feature of the same name.

If several crates need the same stand-in, put it in one Rust-only helper crate (`ets_go`) rather than duplicating
it, Go-shaped, with a test against Go's behavior.

## TypeScript: tsrs

Effect Go code reaches typescript-go only through the generated shim packages (`shim/<pkg>`, module
`github.com/microsoft/TypeScript/tsc/shim/<pkg>`). The Rust port reaches tsrs only through the generated `ets_shim`
crate, which mirrors the shims module for module. [`TSRS_API.md`](TSRS_API.md) maps each shim API to tsrs and lists
the gaps.

- **Never depend on `tsrs_*` crates from an `ets_*` crate.** Use `ets_shim::<pkg>::...`. If something is missing
  from `ets_shim`, change its generator config (the analogue of `_tools/gen_shims/config/<pkg>/extra-shim.json`),
  then `pnpm exec repoctl submodules setup --component tsrs`; never edit the generated crate.
- **Never edit `tsrs/` directly.** The submodule is reset by every `pnpm exec repoctl submodules setup --component tsrs` (and removed by
  `pnpm setup-repo`, which resets to the Go-only setup); changes live only in
  `_patches/tsrs/NNN-*.patch`.
- **Use tsrs's public API first.** Go's exported checker methods (`exports.go`) are tsrs's `*_exported` methods
  where those exist, otherwise the `pub` method of the same name.

### API gaps: a tsrs patch or Rust-only code?

When Go uses something tsrs does not offer publicly, decide in this order:

1. **The Go side relies on a `_patches/typescript` patch** (a hook registry, `EffectLinks` on the checker, a version
   suffix, an LS capability): port that patch to tsrs as `_patches/tsrs/NNN-<same name>.patch`, with the same
   number. The diff is the Go patch's change in tsrs's port of the same code, and nothing else.
2. **The Go shim exposes an unexported member** (a `Checker_getSymbolOfDeclaration`-style accessor from
   `extra-shim.json` or `integration.go`): if tsrs's public API computes the same thing, use it (document the
   mapping in `TSRS_API.md`). Otherwise, a minimal tsrs-only patch (number 100 and up) that makes the tsrs item
   `pub`, and an `ets_shim` entry for it. Visibility-only patches are preferred over reimplementing checker logic.
3. **Small, non-checker helpers** (path utilities, a missing convenience on a public type): Rust-only code in the
   calling crate or `ets_shim` glue, marked `// Rust-only: <shim API> is not public in tsrs`, and a row in
   `TSRS_API.md`, "Gaps".

Never reimplement checker internals in an Effect crate. Every patch:

- has a header (text before the first `---`) naming the Go patch or shim accessor it mirrors and why the port needs
  it;
- is minimal, formatted and warning-free (Cargo builds the patched crates as path dependencies and shows their
  warnings);
- has a regression test in our workspace that fails without it, and a row in `TSRS_API.md`, "tsrs patches".

A tsrs bug (tsrs differs from tsgo at tsrs's own pin) is not ours to fix in Effect code: reduce it, record it in
`DIVERGENCES.md` (kind `tsrs`), and work around it only if blocking, marked `// tsrs workaround: <issue>`, or patch it
in `_patches/tsrs` if the fix is small and the workaround would be worse (say so in the patch header).

tsrs's memory model (arena-allocated `'static` AST and types, regions, checker handles that lock the checker) is
described in `tsrs/docs/RUST.md` and `TSRS_API.md`.

## Tests

- **Go unit tests** (`*_test.go`) are ported with their package: `func TestX(t *testing.T)` -> `#[test] fn test_x()`,
  same cases, same expectations, same fixtures. Table tests with `t.Run` keep one case per entry (a loop with the
  case name in the assertion message). `t.Fatalf` -> `panic!` / `assert!` with Go's message.
- **Go tests that run one subtest per test-data case** (the baseline runners: `t.Run(caseName, ...)` over
  `testdata/tests/<suite>`) are a `[[test]]` target with `harness = false` at `tests/<name>/main.rs`
  (`libtest-mimic`), one trial per case, named like Go's subtest (`TestEffectDiagnostics/<case>`), so
  `cargo test -p <crate> -- <case>` runs one case. The Go test function becomes a function that returns the trials;
  its body runs under `ets_go::testing::run` on a thread with `CHECKER_STACK_SIZE`.
- **Baselines** are the main evidence. `ets_effecttest` (port of `internal/effecttest`) runs every case in
  `testdata/tests/<suite>/` and writes the same baseline files as the Go runner, to `testdata/baselines/local-rs/`
  (git-ignored; Go's is `local/`) when they differ, then diffs them against `testdata/baselines/reference/`. A
  baseline is never regenerated from Rust output: the Go runner owns the reference (a missing reference is a
  mismatch, not created). Any mismatch fails its case, as in Go. `cargo test -p ets_effecttest -- <case>` runs one
  case; the run ends with a summary line (`baselines: N checked, M match, K mismatched ..., J not produced`). A
  reference baseline the ported runners do not write (a runner that stops early) fails its case too.
- **Language-service runners and tests** (quick fixes, refactors, inlay hints, hover; Go: typescript-go's fourslash
  harness, `shim/fourslash`) run over `ets_fourslash`, a Rust-only port of the parts of typescript-go's
  `fourslash.go` / `test_parser.go` they use plus effect-tsgo's `010-fourslash-fourslash.patch` additions. It starts
  a tsrs `tsrs_lsp::Server` per test and talks to it through tsrs's in-process LSP client
  (`ets_shim::lsp::lsptestutil`, JSON-RPC over in-memory pipes), so requests take the editor's path through the
  server, as Go's do. The test file system gets the Effect packages from `ets_etstesthooks` (the
  `PrepareTestFSCallback`). A fourslash helper that is missing is ported from `fourslash.go` into `ets_fourslash`
  (its doc comment names the `fourslash.go` item it ports), never reimplemented. The document-symbol runner uses a
  `tsrs_project::Session` directly, as Go's does.
- **No allowlist**: the transitional `porting/expected-failures.tsv` of the initial port is gone. A baseline
  mismatch is a parity bug to fix in the same pull request (`fix-parity`), never something to tolerate.
- Never change a ported test's expectation unless `DIVERGENCES.md` allows it, with an entry.

## What not to port

List each skipped declaration in `porting/skipped.tsv` with a one-line reason.

- Go plumbing with no Rust counterpart: `String()` methods used only for `%v` debugging, `init()` that only
  registers with a Go mechanism replaced here, build-tag variants for platforms not built.
- Code that exists only for the oxlint/tsgolint integration.
- Run-time checks Rust's types make impossible; say which property the type system guarantees (a `types` entry in
  `DIVERGENCES.md` if a test is affected).
- Benchmarks (`Benchmark*`) are ported only when a performance claim needs them.

## Divergences

[`DIVERGENCES.md`](DIVERGENCES.md) lists every accepted observable difference. An entry needs a kind
(`typescript`, `tsrs`, `library`, `scope`, `types`), the difference, the evidence and the affected tests or
baselines. "Looks like a TypeScript difference" is not evidence. The runners have no way to tolerate a baseline
mismatch: one with a recorded cause still fails, and is resolved in the same pull request (in the port, or by a
shared test case both builds agree on).

## Parity evidence

A port is done when it compiles cleanly **and** its behavior is shown equal to Go's:

1. **The package's Go tests, ported**, passing.
2. **Baselines**: the rules, fixables, refactors and LS features the package implements match their reference
   baselines.
3. **Regression cases** for every parity bug fixed outside a ported test: a new test case in `testdata/tests/` (so
   both runners exercise it, and the Go runner writes the reference) or a Rust test with the Go output as its
   expectation.

No special-casing a test, no papering over a symptom: fix the root cause in the shared helper and grep for the same
slip elsewhere (sibling bugs are common because the same agent ported neighboring code).

## Working rules for agents

- Read the Go declaration, write the Rust one. Do not touch Go code unless the task is a Go change (then the Rust
  follows in the same PR).
- Never leave `todo!()` / `unimplemented!()`. If a callee is not ported yet, port it first; `repoctl tsrs port-ready` and
  `port-deps` say what is unblocked.
- Only edit the crates you were assigned. If you need something from another crate that is missing or wrong, say so
  in your report (and, if tiny and clearly correct, fix it and say so).
- Each crate compiles with zero warnings: `cargo clippy -p <crate> --all-targets -- -D warnings`. For the whole
  workspace, `pnpm exec repoctl tsrs lint|check|test` runs what the Validate tsrs CI workflow runs (the Validate
  workflow has no tsrs checkout and runs no cargo step). If flagged code is
  intended, use `#[expect(clippy::<lint>, reason = "...")]` on the smallest item, never a bare `#[allow]`. With
  several agents in one checkout, use your own target dir: `CARGO_TARGET_DIR=target/<agent-name>`.
- Scratch files go in `target/scratch/<agent-name>/`, never in the source tree.
- Commit only your own paths (`git add crates/<crate> porting/ ...`), never `git add -A`, never rebase/reset/stash
  someone else's work, never force-push.
- When you make a mapping decision this file does not cover, apply it consistently and add it here (or to
  `TSRS_API.md`) in the same commit, so the next agent does not decide differently.
- Report back concisely: what is done (with `repoctl tsrs port-status <prefix>` numbers), what is not, divergences
  recorded, anything other crates need.
