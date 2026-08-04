# Oxlint Fixer Support

Date: 2026-08-04

This note evaluates whether Effect diagnostics produced through `etsoxlintrunner`
can also expose the existing Effect code actions to Oxlint.

The versions currently pinned by `_packages/tsgo/upstream.json` are:

- Oxlint 1.77.0 at `9a423f2f485b79c2353c49442c0c7f60f900261d`
- tsgolint 7.0.2001 at `482dcf70bffce7ea56f63128c74beb67dec658a2`

## Conclusion

Yes, the rule runner can support Oxlint fixes. The pinned tsgolint-to-Oxlint
protocol already carries both automatic fixes and suggestion fixes, including
multiple edits in one action and multiple alternative suggestions. No Oxlint
wire-format change is needed for an initial implementation.

The smallest safe first increment is to export existing Effect code actions as
**suggestions**. Existing actions have not been classified by safety, and some
diagnostics offer several mutually exclusive actions. Treating them all as
automatic fixes would make them eligible for `oxlint --fix`, which is too strong
without a rule-by-rule audit.

After that audit, transformations proven not to change runtime behavior can be
promoted to automatic fixes. Dangerous fixes cannot be represented by the
current tsgolint protocol and would require a protocol extension.

## Terminology

Oxlint has three orthogonal concepts that should not be conflated:

1. A diagnostic's severity controls whether the diagnostic is allowed, warned,
   or denied.
2. A `Fix` is an automatic transformation expected not to change program
   behavior. Safe fixes are selected by `--fix`.
3. A `Suggestion` should remain syntactically valid but may change behavior or
   may not match the author's intent. Suggestions are selected separately by
   `--fix-suggestions`.

`Dangerous` is an additional bit that can qualify either a fix or a suggestion.
It covers aggressive or experimental transformations that may break code.
Oxlint defines these as `FixKind::Fix`, `FixKind::Suggestion`, and
`FixKind::Dangerous` bitflags [in its fixer model](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/fixer/fix.rs#L12-L48).
The rule-facing contracts are documented by the
[`LintContext` diagnostic methods](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/context/mod.rs#L307-L396).

An Effect rule's `DefaultSeverity: suggestion` is unrelated to an Oxlint
suggestion fix. Severity cannot be used to infer fixer safety.

## User-Visible Behavior

Oxlint's CLI exposes the categories separately:

- `--fix` applies safe automatic fixes.
- `--fix-suggestions` applies suggestions, which may change behavior.
- `--fix-dangerously` permits dangerous fixes and suggestions as well as safe
  ones.

See the official [automatic fixes guide](https://oxc.rs/docs/guide/usage/linter/automatic-fixes.html)
and [CLI reference](https://oxc.rs/docs/guide/usage/linter/cli.html#fix-problems).

In the Oxlint language server, each alternative is exposed as an individual
quick fix and the first alternative is preferred. `source.fixAll.oxc` applies
safe fixes only. Suggestions do not participate in editor fix-all, even though
the CLI can apply them when `--fix-suggestions` is explicitly requested. See
[`apply_fix_code_actions`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/apps/oxlint/src/lsp/code_actions.rs#L21-L60)
and the [fix-all implementations](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/apps/oxlint/src/lsp/code_actions.rs#L152-L194).

## Protocol Capabilities

The pinned tsgolint rule API already has:

- `RuleFix`, containing replacement text and a source range
- `RuleSuggestion`, containing a message and one or more `RuleFix` edits
- `ReportDiagnosticWithFixes`
- `ReportDiagnosticWithSuggestions`

See [`internal/rule/rule.go`](https://github.com/oxc-project/tsgolint/blob/482dcf70bffce7ea56f63128c74beb67dec658a2/internal/rule/rule.go#L47-L129).
The headless payload serializes automatic edits in `fixes` and alternatives in
`suggestions`; generation is gated by the `fix` and `fix-suggestions` request
flags in [`cmd/tsgolint/headless.go`](https://github.com/oxc-project/tsgolint/blob/482dcf70bffce7ea56f63128c74beb67dec658a2/cmd/tsgolint/headless.go#L97-L145).

On the Oxlint side, tsgolint `fixes` become a single composite
`FixKind::Fix`. Each tsgolint suggestion becomes a separate composite
`FixKind::Suggestion`. See
[`Message::from_tsgo_lint_diagnostic`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/tsgolint.rs#L809-L852).

This produces two important constraints:

- One automatic fix may contain multiple non-overlapping edits, but the
  protocol does not represent several alternative automatic fixes.
- Suggestions can represent several alternatives, and each alternative may
  contain multiple edits.

Oxlint normalizes multi-edit actions into an atomic composite replacement. Its
native model is described by [`PossibleFixes` and `CompositeFix`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/fixer/fix.rs#L316-L480).

The tsgolint payload has no dangerous bit. It maps every `fixes` entry to a safe
fix and every `suggestions` entry to a non-dangerous suggestion. Supporting
dangerous Effect actions would require coordinated tsgolint and Oxlint protocol
changes.

## Effect Architecture

Before suggestion support, the integration stopped at diagnostics:

- `internal/rulerunner/diagnostics.go` runs Effect rules and returns
  `*ast.Diagnostic` values.
- `etsoxlintrunner/runner.go` converted those to `ReportedDiagnostic` values
  containing only ranges and messages.
- `_tools/repoctl/src/codegen.ts` generates tsgolint adapters that call only
  `ctx.ReportDiagnostic`.

Effect fixes already exist, but on a separate language-service path:

- `internal/fixables/fixables.go` registers the providers by diagnostic code.
- `etslshooks/init.go` invokes those providers for a TypeScript language-service
  `CodeFixContext`.
- `internal/fixable/context.go` creates a TypeScript-Go change tracker and
  returns `ls.CodeAction` values containing LSP text edits.

This separation is useful: fix providers can be reused, but the runner should
not expose language-service types as its public contract.

The initial suggestion implementation bridges these gaps as described below.

### 1. Standalone Fix Context

`fixable.Context` currently requires `*ls.CodeFixContext` and uses its language
service for formatting options and position converters. tsgolint has a
`*compiler.Program`, checker, source file, and diagnostic span, but no language
service.

A standalone context can use TypeScript-Go's default format settings and an
`lsconv.Converters` instance for the source file, following the existing pattern
in `etsgoapi/structural_schema.go`. No direct import from `typescript-go/internal`
is necessary; the existing shim packages expose the required APIs.

The resulting LSP edit ranges must then be converted back to TypeScript-Go byte
offset ranges for `RuleFix`. This conversion must explicitly handle UTF-16
characters and non-ASCII source text rather than assuming an LSP character is a
byte offset.

### 2. Neutral Runner Model

`etsoxlintrunner` needs integration-neutral action types, for example:

```go
type ReportedEdit struct {
    Range core.TextRange
    Text  string
}

type ReportedAction struct {
    Description string
    Edits       []ReportedEdit
}
```

Action generation should remain lazy. tsgolint only invokes a fix or suggestion
closure when the corresponding request flag is enabled. Eager generation would
rerun Effect analyzers and build change trackers on every normal lint invocation.

### 3. Explicit Safety Metadata

`fixable.Fixable` currently records names, diagnostic codes, fix-all IDs, and a
runner, but no safety category. `ls.CodeAction` likewise has no Oxlint fix kind.
Classification must be explicit; it cannot be inferred from diagnostic severity,
the presence of a `FixID`, or the action title.

Several providers return mutually exclusive alternatives. For example:

- `effectFnOpportunity` can offer several `Effect.fn` variants.
- `overriddenSchemaConstructor` can either rewrite or remove the constructor.
- `missingEffectError` can offer catch-all and tagged-error variants.
- the general Effect disable provider offers line and file suppression actions.

These naturally map to suggestions. An automatic-fix provider must designate at
most one action as the automatic composite fix; any other actions must remain
suggestions.

## Recommended Delivery

### Phase 1: Suggestions

1. Add a standalone fix context using default formatting and explicit UTF-16
   range conversion.
2. Add lazy, integration-neutral action generation to `etsoxlintrunner`.
3. Generate adapters using `ctx.ReportDiagnosticWithSuggestions`.
4. Convert every returned `ls.CodeAction` into one `RuleSuggestion`, preserving
   its description and all of its edits.
5. Decide whether to omit `EffectDisable` actions. Oxlint already has lint-disable
   workflows, and exporting Effect's own line/file directives may be redundant.

This phase requires changes in this repository and its Effect-owned tsgolint
patch/code generation, but no Oxlint protocol change.

### Phase 2: Audited Automatic Fixes

1. Add an explicit fix kind to fixable metadata or to each produced action.
2. Audit transformations against the Oxlint invariant that automatic fixes do
   not change behavior.
3. Use `ReportDiagnosticWithFixes` only for providers guaranteed to produce one
   unambiguous automatic action.
4. Keep alternative or intent-dependent actions as suggestions.

If a diagnostic needs both one automatic fix and additional suggestions, the
current tsgolint reporting callbacks cannot lazily attach both categories in one
call. Either add a combined reporting callback upstream/in the Effect-owned
tsgolint patch, or generate the complete diagnostic only when either fix mode is
requested. The former preserves lazy generation cleanly.

### Phase 3: Dangerous Actions, If Needed

Extend the tsgolint payload with fix-kind information and update Oxlint's
tsgolint conversion. This is unnecessary unless an Effect transformation should
be available only through `--fix-dangerously`.

## Validation Plan

- Unit-test conversion of single edits, multi-edits, insertion/deletion, and
  non-ASCII UTF-16 positions.
- Unit-test preservation of multiple alternative action descriptions.
- Verify no action analyzer runs when neither tsgolint fix flag is enabled.
- Test generated adapters with `fix-suggestions` disabled and enabled.
- Add an end-to-end Oxlint fixture proving an Effect suggestion is reported but
  not applied by `--fix`, then applied by `--fix-suggestions`.
- For each future automatic fix, prove `--fix` applies it and that repeated
  lint/fix passes are stable.
- Compare edits produced through the language service and Oxlint for the same
  fixture to catch formatting or range-conversion drift.

## Examples From Oxlint

- Safe fix: [`oxc/no-const-enum`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/rules/oxc/no_const_enum.rs#L39-L74)
- Suggestion: [`oxc/approx-constant`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/rules/oxc/approx_constant.rs#L46-L76)
- Dangerous fix: [`eslint/for-direction`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/rules/eslint/for_direction.rs#L84-L137)
- Multiple alternatives: [`typescript/prefer-enum-initializers`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/rules/typescript/prefer_enum_initializers.rs#L57-L96)
- Multiple edits per alternative: [`typescript/consistent-type-assertions`](https://github.com/oxc-project/oxc/blob/9a423f2f485b79c2353c49442c0c7f60f900261d/crates/oxc_linter/src/rules/typescript/consistent_type_assertions.rs#L528-L560)
