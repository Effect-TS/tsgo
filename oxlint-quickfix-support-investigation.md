# Effect quick-fix support for the experimental Oxlint bridge

**Status:** Research only. No implementation was performed.

**Source snapshots inspected:** `tsgo` branch `feat/oxlint-bridge-prototype` at `d6024a473a2fa3d19fe3ad32e1a5c8218715338c`; `typescript-go` at `70c2f5e51856a908b05ac98b5e954b4c685520dd`; `.repos/oxlint` at `a065946a8ce95eb3374e08242cd9086ab050314b`; `.repos/tsgolint` at `16a224c6cc96e4111cc6edfeded8e3028c2b59ce`.

## Executive conclusion

Effect quick fixes can be computed inside the existing persistent `etsjsapi` process without going through the LSP server and without adding a `typescript-go` patch or shim. The temporary snapshot already supplies the exact current-file text, project, program, source file, checker, and snapshot-backed language-service host. Existing shim aliases expose `project.Snapshot`, `project.Project`, `ls.LanguageService`, `ls.CodeFixContext`, `ls.CodeAction`, and `ls.NewLanguageService`; `LanguageService` also has the exported `ProvideCodeActions` method. [`etsjsapi/server.go:184-243`](etsjsapi/server.go#L184-L243), [`shim/project/shim.go:79-84`](shim/project/shim.go#L79-L84), [`shim/ls/shim.go:31-34`](shim/ls/shim.go#L31-L34), [`shim/ls/shim.go:88-109`](shim/ls/shim.go#L88-L109), [`typescript-go/internal/ls/codeactions.go:104-176`](typescript-go/internal/ls/codeactions.go#L104-L176)

The important caveat is product semantics, not technical reachability. Effect actions currently have titles and edits but no safety, preferred-action, or meaningful per-action fix-ID metadata. Oxlint treats a JavaScript rule's `fix` as a safe fix eligible for plain `--fix`; suggestions are separate and require `meta.hasSuggestions`. Therefore the safe initial integration is to return all non-disable Effect actions and expose each as an Oxlint suggestion, not an automatic fix. [`.repos/oxlint/apps/oxlint/src-js/plugins/rule_meta.ts:29-38`](.repos/oxlint/apps/oxlint/src-js/plugins/rule_meta.ts#L29-L38), [`.repos/oxlint/crates/oxc_linter/src/lib.rs:845-890`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L845-L890), [`.repos/oxlint/crates/oxc_linter/src/fixer/fix.rs:15-47`](.repos/oxlint/crates/oxc_linter/src/fixer/fix.rs#L15-L47)

Recommended first release:

1. Bump the private `etsjsapi` protocol from version 1 to version 2.
2. Add `includeFixes?: boolean` to the existing `diagnostics` request and `actions?: EffectCodeAction[]` to each diagnostic.
3. Compute fixes in the same temporary snapshot and checker lifetime as diagnostics.
4. Return only current-file, non-overlapping edits as absolute UTF-16 offsets.
5. Exclude `effectDisable` actions initially and expose every other action as a separate Oxlint suggestion.
6. Let Oxlint continue to own configured severity, `oxlint-disable` filtering, conflict resolution, and application under `--fix-suggestions`.

## 1. Current Effect diagnostic-to-fix mapping

### Verified facts

`fixable.Fixable` is the unit of registration. It has a unique provider `Name`, descriptive text, one or more handled diagnostic `ErrorCodes`, optional provider-level `FixIDs`, and a `Run` function returning zero or more `ls.CodeAction` values. [`internal/fixable/fixable.go:12-37`](internal/fixable/fixable.go#L12-L37)

`fixables.All` is an explicit ordered registry. `ByErrorCode` scans it and returns every provider handling a code; `AllErrorCodes` and `AllFixIDs` derive the language-service registration lists. [`internal/fixables/fixables.go:7-63`](internal/fixables/fixables.go#L7-L63), [`internal/fixables/fixables.go:66-94`](internal/fixables/fixables.go#L66-L94)

The first provider, `EffectDisable`, handles every Effect diagnostic code and offers two actions: disable the rule for the line and disable it for the file. It inserts Effect-specific comments, not Oxlint comments. [`internal/fixables/disable.go:13-44`](internal/fixables/disable.go#L13-L44), [`internal/fixables/disable.go:47-89`](internal/fixables/disable.go#L47-L89)

All other providers are code-specific. A provider commonly reruns the corresponding rule analysis, intersects each match with the requested diagnostic span, then creates one action. `MissingReturnYieldStarFix` is representative. [`internal/fixables/missing_return_yield_star.go:13-53`](internal/fixables/missing_return_yield_star.go#L13-L53)

Some providers return alternatives. This means “one diagnostic code maps to one fix” is not a valid invariant. The existing baseline format inventories fixes by diagnostic and preserves multiple action titles; a real baseline shows two disable actions plus one semantic action for the same diagnostic. [`internal/effecttest/quickfix_baseline.go:64-91`](internal/effecttest/quickfix_baseline.go#L64-L91), [`testdata/baselines/reference/effect-v4/catchTagToCatchReason_preview.quickfixes.txt:1-16`](testdata/baselines/reference/effect-v4/catchTagToCatchReason_preview.quickfixes.txt#L1-L16)

Metadata's `fixable` flag deliberately excludes the universal disable provider and is true when any code belonging to a rule has a non-disable provider. It is therefore suitable for setting wrapper metadata, but not for deciding whether a particular diagnostic instance will produce an applicable action. [`internal/rules/rules_json_test.go:174-195`](internal/rules/rules_json_test.go#L174-L195), [`_packages/tsgo/src/metadata.json:89-101`](_packages/tsgo/src/metadata.json#L89-L101)

The Effect language-service provider registers all handled codes and all declared fix IDs, then uses `ByErrorCode`, resolves per-file Effect options, obtains a checker for the file, creates a `TypeParser` and `fixable.Context`, and runs every applicable provider. [`etslshooks/init.go:84-134`](etslshooks/init.go#L84-L134)

Declared `FixIDs` currently do not identify returned actions. `fixable.Context.NewFixAction` creates an `ls.CodeAction` with only `Description` and `Changes`, leaving `FixID` empty. [`internal/fixable/context.go:69-88`](internal/fixable/context.go#L69-L88), [`typescript-go/internal/ls/codeactions.go:38-45`](typescript-go/internal/ls/codeactions.go#L38-L45)

Consequently, current Effect actions do not participate in TypeScript-Go's fix-all path: `ProvideCodeActions` records a provider for fix-all only when a returned action has a non-empty `FixID`, and the Effect provider has no `GetAllCodeActions` callback. [`typescript-go/internal/ls/codeactions.go:161-183`](typescript-go/internal/ls/codeactions.go#L161-L183), [`typescript-go/internal/ls/codeactions.go:205-253`](typescript-go/internal/ls/codeactions.go#L205-L253), [`etslshooks/init.go:86-90`](etslshooks/init.go#L86-L90)

### Recommendation

Do not expose `FixIDs` in the first wire protocol. They are currently provider declarations rather than actionable IDs, and Oxlint needs an action title plus edits, not TypeScript fix-all identity. Preserve registry/action order only for deterministic presentation; do not describe the first action as “preferred” until preference is represented explicitly in the Effect model.

Exclude `EffectDisable` from the initial Oxlint action list. Oxlint already owns its suppression directives and filters external-rule diagnostics after JavaScript reports them. Returning Effect-specific suppression comments would create a second suppression vocabulary and two extra suggestions on every diagnostic. [`.repos/oxlint/crates/oxc_linter/src/lib.rs:833-843`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L833-L843), [`internal/fixables/disable.go:47-89`](internal/fixables/disable.go#L47-L89)

## 2. State required to compute fixes

### Verified facts

Every fix receives:

- `SourceFile`, internal byte-based `Span`, and diagnostic `ErrorCode`;
- resolved per-file Effect `Options`;
- the exact `Program` and per-file `Checker`;
- a `TypeParser` built from that program/checker pair;
- a request `context.Context`; and
- an `ls.CodeFixContext`, primarily for the language service's format options and position converters. [`internal/fixable/context.go:19-53`](internal/fixable/context.go#L19-L53), [`internal/fixable/context.go:69-88`](internal/fixable/context.go#L69-L88)

The bridge already obtains almost all of this state. It opens the project/file in a persistent base snapshot, overlays `params.Text` in a temporary snapshot, finds the default project, gets its program and source file, resolves request or project Effect options, acquires a checker, and runs diagnostics. [`etsjsapi/server.go:184-255`](etsjsapi/server.go#L184-L255)

The exact request text is authoritative for both diagnostics and fixes because `APIUpdateTemporary` produces the program used by the rest of the request. The temporary snapshot is dereferenced only after the request completes. [`etsjsapi/server.go:220-243`](etsjsapi/server.go#L220-L243)

Per-file Effect options must be resolved from the same normalized options used for diagnostics. `rulerunner.Run` resolves path-scoped options before running rules. [`internal/rulerunner/diagnostics.go:28-50`](internal/rulerunner/diagnostics.go#L28-L50), [`internal/rulerunner/diagnostics.go:64-75`](internal/rulerunner/diagnostics.go#L64-L75)

The existing LSP hook is not sufficient unchanged for settings-only Oxlint configuration: it computes actions only when `fixCtx.Program.Options().Effect` is non-nil and resolves from that project configuration. The diagnostics endpoint, by contrast, permits request-provided `effectOptions` when the project has none. [`etslshooks/init.go:100-134`](etslshooks/init.go#L100-L134), [`etsjsapi/server.go:235-240`](etsjsapi/server.go#L235-L240), [`etsjsapi/server.go:258-282`](etsjsapi/server.go#L258-L282)

`NewFixAction` creates a TypeScript change tracker with compiler options, language-service formatting preferences, and converters. It then intentionally extracts only changes for `c.SourceFile.FileName()`. Thus all currently returned Effect actions are current-file-only even if a tracker were to accumulate other files. [`internal/fixable/context.go:69-88`](internal/fixable/context.go#L69-L88)

### Recommendation

Create one language service over the temporary program and snapshot, resolve one `ResolvedEffectPluginOptions`, and reuse the already acquired checker and one `TypeParser` across all diagnostics/actions in the request. Do not reacquire a checker per diagnostic as the current generic LSP callback does.

Extract the action-collection loop from `etslshooks` into a small internal helper accepting the resolved options explicitly. Both LSP and `etsjsapi` should call that helper. This avoids duplicating provider-selection behavior while allowing request-provided Oxlint settings.

## 3. Direct invocation without LSP

### Verified facts

The LSP server is only a caller of `LanguageService.ProvideCodeActions`; the actual provider dispatch is a normal exported Go method. [`typescript-go/internal/lsp/server.go:1680-1680`](typescript-go/internal/lsp/server.go#L1680), [`typescript-go/internal/ls/codeactions.go:104-176`](typescript-go/internal/ls/codeactions.go#L104-L176)

`ls.NewLanguageService(projectPath, program, host, activeFile)` needs a project path, program, `ls.Host`, and active file. A `project.Snapshot` implements the required host operations, including converters, preferences, reads, and line information. [`typescript-go/internal/ls/languageservice.go:15-37`](typescript-go/internal/ls/languageservice.go#L15-L37), [`typescript-go/internal/ls/host.go:10-25`](typescript-go/internal/ls/host.go#L10-L25), [`typescript-go/internal/project/snapshot.go:104-135`](typescript-go/internal/project/snapshot.go#L104-L135)

The temporary snapshot exposes its default project; `Project.ID()` and `GetProgram()` are exported. [`typescript-go/internal/project/project.go:187-211`](typescript-go/internal/project/project.go#L187-L211)

No new shim is required for the recommended path. Existing generated shims alias the needed project and LS types and expose `NewLanguageService`; the existing converter accessor is also available. [`shim/project/shim.go:79-84`](shim/project/shim.go#L79-L84), [`shim/ls/shim.go:31-34`](shim/ls/shim.go#L31-L34), [`shim/ls/shim.go:88-109`](shim/ls/shim.go#L88-L109)

### Recommendation

Invoke the shared Effect action helper directly rather than round-tripping through an LSP-shaped diagnostic and `ProvideCodeActions`. Direct invocation avoids running built-in TypeScript providers/refactors, avoids reconstructing LSP diagnostic positions, and permits request-resolved Effect options. A language-service object is still needed because current change tracking obtains formatting options and converters from it.

Construct it as conceptually:

```go
languageService := ls.NewLanguageService(project.ID(), program, temporary, params.File)
```

This is an in-process API call, not an LSP request.

## 4. Oxlint JavaScript fix and suggestion contract

### Verified facts

`context.report` requires a message/message ID and a node/location. It accepts an optional `fix` callback and optional suggestion array. The existing bridge's synthetic `{ range: [start, end] }` node is valid because Oxlint reads `node.range` directly and validates non-negative integer diagnostic offsets. [`.repos/oxlint/apps/oxlint/src-js/plugins/report.ts:16-38`](.repos/oxlint/apps/oxlint/src-js/plugins/report.ts#L16-L38), [`.repos/oxlint/apps/oxlint/src-js/plugins/report.ts:161-200`](.repos/oxlint/apps/oxlint/src-js/plugins/report.ts#L161-L200)

A fix callback may return one fix, an array, or an iterator; falsy entries are ignored. The fixer API supports insert-before/after, remove, and replace operations over nodes or explicit ranges. [`.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts:7-17`](.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts#L7-L17), [`.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts:51-86`](.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts#L51-L86), [`.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts:173-213`](.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts#L173-L213)

A rule returning direct fixes must set `meta.fixable` to `"code"` or `"whitespace"`; a rule returning suggestions must set `meta.hasSuggestions: true`. Oxlint throws if produced fixes/suggestions contradict metadata. [`.repos/oxlint/apps/oxlint/src-js/plugins/rule_meta.ts:29-38`](.repos/oxlint/apps/oxlint/src-js/plugins/rule_meta.ts#L29-L38), [`.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts:88-115`](.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts#L88-L115), [`.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts:117-170`](.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts#L117-L170)

Each suggestion needs a description or message ID and its own fix callback. Suggestions with no resulting edits are dropped. [`.repos/oxlint/apps/oxlint/src-js/plugins/report.ts:53-73`](.repos/oxlint/apps/oxlint/src-js/plugins/report.ts#L53-L73), [`.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts:131-170`](.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts#L131-L170)

JavaScript diagnostic and fix offsets are UTF-16 code-unit offsets. Rust converts diagnostic spans and every fix span back to UTF-8 bytes; it rejects reversed, out-of-bounds, non-character-boundary, and invalid multi-edit ranges. [`.repos/oxlint/crates/oxc_linter/src/lib.rs:824-831`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L824-L831), [`.repos/oxlint/crates/oxc_linter/src/external_linter.rs:112-178`](.repos/oxlint/crates/oxc_linter/src/external_linter.rs#L112-L178)

Offsets are relative to source text without a BOM. `-1` has a special before-BOM meaning; Effect edits should never need it. [`.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts:19-49`](.repos/oxlint/apps/oxlint/src-js/plugins/fix.ts#L19-L49), [`.repos/oxlint/crates/oxc_linter/src/external_linter.rs:181-223`](.repos/oxlint/crates/oxc_linter/src/external_linter.rs#L181-L223)

Multiple edits belonging to one action are merged into one composite fix. They are sorted and may be adjacent, but they may not overlap; invalid groups become plugin-error diagnostics. [`.repos/oxlint/crates/oxc_linter/src/fixer/fix.rs:620-678`](.repos/oxlint/crates/oxc_linter/src/fixer/fix.rs#L620-L678), [`.repos/oxlint/crates/oxc_linter/src/lib.rs:845-868`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L845-L868)

Across diagnostics, Oxlint sorts candidate fixes and skips later fixes that overlap an already applied fix. Boundary-adjacent fixes from different diagnostics are deliberately treated as overlapping. Only one alternative from `PossibleFixes::Multiple` is selected, with index zero in normal operation. [`.repos/oxlint/crates/oxc_linter/src/fixer/mod.rs:340-375`](.repos/oxlint/crates/oxc_linter/src/fixer/mod.rs#L340-L375), [`.repos/oxlint/crates/oxc_linter/src/fixer/mod.rs:389-436`](.repos/oxlint/crates/oxc_linter/src/fixer/mod.rs#L389-L436)

Plain `--fix` applies safe fixes; `--fix-suggestions` enables suggestions and may change behavior; `--fix-dangerously` enables dangerous fixes and suggestions. [`.repos/oxlint/apps/oxlint/src/command/lint.rs:219-255`](.repos/oxlint/apps/oxlint/src/command/lint.rs#L219-L255)

The JavaScript bridge has no API to tag a direct plugin fix as dangerous. A direct `diagnostic.fixes` group becomes `FixKind::Fix` (the safe-fix bit), while suggestion groups become `FixKind::Suggestion`. [`.repos/oxlint/crates/oxc_linter/src/lib.rs:870-890`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L870-L890), [`.repos/oxlint/crates/oxc_linter/src/fixer/fix.rs:23-42`](.repos/oxlint/crates/oxc_linter/src/fixer/fix.rs#L23-L42)

Oxlint applies disable directives after receiving JavaScript diagnostics and before attaching fixes or severity. Disabled diagnostics therefore carry no fix/suggestion into later processing. Final severity comes from Oxlint's resolved external-rule configuration, not from the JavaScript report or Effect diagnostic severity. [`.repos/oxlint/crates/oxc_linter/src/lib.rs:833-843`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L833-L843), [`.repos/oxlint/crates/oxc_linter/src/lib.rs:893-899`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L893-L899)

For next-line/line directives Oxlint checks the diagnostic start; for ordinary disable intervals it checks overlap. [`.repos/oxlint/crates/oxc_linter/src/disable_directives.rs:306-364`](.repos/oxlint/crates/oxc_linter/src/disable_directives.rs#L306-L364)

### Recommendation

For each Effect action, produce one Oxlint suggestion whose `fix` callback returns an array of `fixer.replaceTextRange([start, end], text)` edits. Set `meta.hasSuggestions: true` on wrappers whose metadata says `fixable`, or on all wrappers if generation simplicity is more important than metadata precision. Do not set `meta.fixable` until individual actions have been safety-audited.

Validate on the Go side before returning: same file, `0 <= start <= end <= UTF16Length(text)`, edits sorted, and no overlap within an action. This converts a plugin-fatal malformed action into an omitted action plus trace/debug information.

## 5. Tsgolint comparison

### Verified facts

Oxlint already decides whether to request tsgolint fixes and suggestions from its selected `FixKind`: fixes and suggestions are separate booleans and command-line flags. [`.repos/oxlint/crates/oxc_linter/src/tsgolint.rs:35-62`](.repos/oxlint/crates/oxc_linter/src/tsgolint.rs#L35-L62), [`.repos/oxlint/crates/oxc_linter/src/tsgolint.rs:333-351`](.repos/oxlint/crates/oxc_linter/src/tsgolint.rs#L333-L351)

Tsgolint's first-party headless diagnostic shape carries a main range, message, file path, rule, direct `fixes`, grouped `suggestions`, and optional labeled ranges. A suggestion has its own message and edit list. [`.repos/tsgolint/cmd/tsgolint/headless.go:68-145`](.repos/tsgolint/cmd/tsgolint/headless.go#L68-L145)

The Go backend computes direct fixes only under `-fix` and suggestions only under `-fix-suggestions`, then serializes them next to the diagnostic. [`.repos/tsgolint/cmd/tsgolint/headless.go:338-377`](.repos/tsgolint/cmd/tsgolint/headless.go#L338-L377)

Oxlint deserializes omitted fixes/suggestions as empty lists, merges each edit list into one composite action, labels direct fixes `FixKind::Fix`, labels suggestions `FixKind::Suggestion`, and combines the alternatives in direct-fix-first order. [`.repos/oxlint/crates/oxc_linter/src/tsgolint.rs:687-743`](.repos/oxlint/crates/oxc_linter/src/tsgolint.rs#L687-L743), [`.repos/oxlint/crates/oxc_linter/src/tsgolint.rs:809-851`](.repos/oxlint/crates/oxc_linter/src/tsgolint.rs#L809-L851)

Tsgolint uses source overrides for exact requested text and computes programs/checkers within the invocation. [`.repos/tsgolint/cmd/tsgolint/headless.go:242-293`](.repos/tsgolint/cmd/tsgolint/headless.go#L242-L293), [`.repos/tsgolint/cmd/tsgolint/headless.go:413-449`](.repos/tsgolint/cmd/tsgolint/headless.go#L413-L449)

Oxlint applies its own disable directives to tsgolint diagnostics, checking plain and TypeScript-prefixed rule names. [`.repos/oxlint/crates/oxc_linter/src/tsgolint.rs:1192-1218`](.repos/oxlint/crates/oxc_linter/src/tsgolint.rs#L1192-L1218)

### Recommendation

Mirror tsgolint's useful semantic boundary: actions travel with the diagnostic and each alternative owns a complete edit list. Do not copy its process lifecycle or byte-offset encoding. The JS bridge is persistent and its public offsets must remain UTF-16 because they are fed directly to Oxlint's JavaScript report/fixer contract.

## 6. Minimal versioned `etsjsapi` extension

### Current verified protocol

Both sides hard-code protocol version 1. The server rejects any other version and supports only the `diagnostics` method. The request includes file, exact text, optional project, selected rules, and optional Effect options; the response contains diagnostics and options source, but no actions. [`etsjsapi/server.go:30-70`](etsjsapi/server.go#L30-L70), [`etsjsapi/server.go:154-176`](etsjsapi/server.go#L154-L176), [`_packages/tsgo/src/experimental/oxlint/sync-api.ts:4-33`](_packages/tsgo/src/experimental/oxlint/sync-api.ts#L4-L33)

### Recommended version 2 Go types

```go
const protocolVersion = 2

type diagnosticsParams struct {
    File          string          `json:"file"`
    Text          string          `json:"text"`
    Project       string          `json:"project,omitempty"`
    Rules         []string        `json:"rules"`
    EffectOptions json.RawMessage `json:"effectOptions,omitempty"`
    IncludeFixes  bool            `json:"includeFixes,omitempty"`
}

type diagnostic struct {
    File     string       `json:"file"`
    Start    int          `json:"start"`
    End      int          `json:"end"`
    Code     int32        `json:"code"`
    RuleName string       `json:"ruleName"`
    Message  string       `json:"message"`
    Actions  []codeAction `json:"actions,omitempty"`
}

type codeAction struct {
    Title string     `json:"title"`
    Edits []textEdit `json:"edits"`
}

type textEdit struct {
    Start   int    `json:"start"` // absolute UTF-16 offset
    End     int    `json:"end"`   // absolute UTF-16 offset
    NewText string `json:"newText"`
}
```

### Recommended version 2 TypeScript types

```ts
export interface EffectTextEdit {
  readonly start: number
  readonly end: number
  readonly newText: string
}

export interface EffectCodeAction {
  readonly title: string
  readonly edits: ReadonlyArray<EffectTextEdit>
}

export interface EffectDiagnostic {
  readonly file: string
  readonly start: number
  readonly end: number
  readonly code: number
  readonly ruleName: string
  readonly message: string
  readonly actions?: ReadonlyArray<EffectCodeAction>
}

export interface DiagnosticsParams {
  readonly file: string
  readonly text: string
  readonly project?: string
  readonly rules: ReadonlyArray<string>
  readonly effectOptions?: unknown
  readonly includeFixes?: boolean
}
```

### Recommended algorithm

1. Build/update the persistent base snapshot and temporary current-file overlay exactly as version 1 does.
2. Resolve/normalize Effect options and acquire one checker exactly as diagnostics do now.
3. Run `rulerunner.Run` once and retain the original `*ast.Diagnostic` values, not only formatted wire diagnostics.
4. If `includeFixes` is false, format the existing version 1 result with no `actions` keys.
5. If true, create one `LanguageService` from the temporary project's ID/program and temporary snapshot host.
6. Resolve `ResolvedEffectPluginOptions` for the same file from the normalized request options, then create one `TypeParser` using the already-held checker.
7. For each diagnostic, call the shared Effect action helper with its internal byte span and code; skip `effectDisable`.
8. For every `lsproto.TextEdit`, use the language service's converter to turn its line/character endpoint back into an internal byte position, then use the source file position map's `UTF8ToUTF16` conversion to emit absolute UTF-16 offsets. The existing diagnostic formatter uses the same final conversion. [`etsjsapi/server.go:319-333`](etsjsapi/server.go#L319-L333), [`shim/ls/shim.go:99-101`](shim/ls/shim.go#L99-L101)
9. Reject an action if it targets another file, has no edits, has invalid ranges, or has internally overlapping edits. Deduplicate identical `(title, edits)` actions per diagnostic.
10. Return diagnostics and actions in registry order. The TypeScript broker groups diagnostics by rule as it does today. [`_packages/tsgo/src/experimental/oxlint/bridge.ts:44-66`](_packages/tsgo/src/experimental/oxlint/bridge.ts#L44-L66)
11. In each wrapper, report the diagnostic once and map its actions to `suggest` entries. Each suggestion callback returns all edits for that action.

This is intentionally an additive request/response shape but still deserves a version bump because both current endpoints enforce exact protocol equality and are private prototype code. A version bump makes mixed old/new binaries fail explicitly rather than silently omitting fixes. [`etsjsapi/server.go:154-163`](etsjsapi/server.go#L154-L163), [`_packages/tsgo/src/experimental/oxlint/sync-api.ts:43-55`](_packages/tsgo/src/experimental/oxlint/sync-api.ts#L43-L55)

## 7. Behavior decisions

### Recommended decisions

| Question | Initial decision | Reason |
|---|---|---|
| One preferred fix or alternatives? | Return every non-disable action as a separate suggestion; preserve registry/action order but claim no preferred action. | Effect actions carry no `IsPreferred` or equivalent, and some diagnostics have genuine alternatives. |
| Automatic `--fix`? | No. | Oxlint would classify every JS `fix` as safe, while Effect has no safety metadata. |
| Unsafe/dangerous fixes? | Suggestions only until audited. Do not attempt `--fix-dangerously` classification from JS because the current JS plugin contract cannot mark a direct fix dangerous. | Prevents plain `--fix` from applying semantically uncertain transformations. |
| Disable actions? | Exclude initially. | Oxlint owns final suppression; Effect disable actions add a competing directive syntax to every diagnostic. |
| Multiple edits in one action? | Preserve as one suggestion after sorting/validating non-overlap. | Oxlint natively merges a suggestion's edit group. |
| Overlapping actions across diagnostics? | Return them and let Oxlint choose according to its normal fixer ordering; test the outcome. | This matches all other Oxlint fixes and avoids inventing bridge-specific arbitration. |
| Cross-file edits? | Reject/omit the whole action. | Oxlint JS fixes are current-source ranges only; current Effect action construction already emits only current-file changes. |
| Stale current-file text? | Compute and report synchronously from the exact `context.sourceCode.text`; do not add a hash in v2. If fixes become a separate later request, require a text hash/version then. | The current request sends exact text, and the fix closure is registered during the same lint callback. |
| Stale imported files? | Document as an existing bridge limitation, not a quick-fix-specific blocker. | The temporary overlay guarantees only the requested file; the persistent base snapshot has no complete Oxlint editor overlay. |
| Final severity? | Oxlint configuration remains authoritative. | External-rule severity is applied by Oxlint after plugin reporting. |
| Rule metadata? | `hasSuggestions: metadata.fixable`; no `fixable` initially. | Metadata excludes universal disable-only support and reflects non-disable provider availability. |

### Additional verified constraints

The broker makes one synchronous diagnostics call on first `Program:exit`, caches by rule, and retains the exact frame text. Fixes should be included in that same call; a per-diagnostic follow-up would multiply checker/provider work and introduce a stale-text seam. [`_packages/tsgo/src/experimental/oxlint/bridge.ts:16-23`](_packages/tsgo/src/experimental/oxlint/bridge.ts#L16-L23), [`_packages/tsgo/src/experimental/oxlint/bridge.ts:44-66`](_packages/tsgo/src/experimental/oxlint/bridge.ts#L44-L66), [`_packages/tsgo/src/experimental/oxlint/bridge.ts:89-99`](_packages/tsgo/src/experimental/oxlint/bridge.ts#L89-L99)

The current plugin reports only message and range and declares no fix metadata. [`_packages/tsgo/src/experimental/oxlint/plugin.ts:22-49`](_packages/tsgo/src/experimental/oxlint/plugin.ts#L22-L49)

## 8. Implementation phases and likely files

### Phase 1: shared Go action computation

Likely files:

- Add `internal/codefixes/actions.go` (or a narrowly named equivalent) for shared provider selection/action collection.
- Modify `etslshooks/init.go` to delegate to that helper while preserving current LSP behavior.
- Add focused unit tests beside the helper or under `internal/effecttest`.

No `typescript-go/`, `_patches/`, shim generator, or generated shim change should be needed. The existing architecture requires Effect code to import shims rather than `typescript-go/internal` directly; the required shims already exist.

### Phase 2: protocol version 2

Likely files:

- `etsjsapi/server.go`: request/response types, version bump, LS construction, action computation, edit conversion/validation.
- `_packages/tsgo/src/experimental/oxlint/sync-api.ts`: version bump and TypeScript wire types.
- Go protocol tests in a new `etsjsapi/server_test.go` or equivalent.

### Phase 3: Oxlint adapter

Likely files:

- `_packages/tsgo/src/experimental/oxlint/bridge.ts`: request `includeFixes`, retain actions in grouped results.
- `_packages/tsgo/src/experimental/oxlint/plugin.ts`: read `fixable` metadata, set `hasSuggestions`, and map actions to suggestion callbacks.
- `testdata/tests/oxlint/run.ts`: run diagnostic, suggestion, and fix-suggestion scenarios with explicit output assertions rather than only exit status.
- `testdata/tests/oxlint/oxlintrc.json`: enable one or more fixable Effect wrappers.
- New fixtures under `testdata/tests/oxlint/` for single-edit, multi-edit, alternatives, overlap, and UTF-16 behavior.
- `testdata/tests/oxlint/README.md`: document suggestion-only behavior and `--fix-suggestions`.

The prototype package's current test only builds, invokes pinned `oxlint@1.75.0`, and considers lint exit status 1 a success; it does not inspect fixed output. [`testdata/tests/oxlint/run.ts:5-29`](testdata/tests/oxlint/run.ts#L5-L29)

### Phase 4: safety audit before automatic fixes

If automatic `--fix` is desired, extend Effect's action model with explicit applicability/safety and optional preferred metadata. This would likely touch `internal/fixable/fixable.go`, individual providers, metadata generation, the protocol, and plugin metadata. Only audited safe actions should become `context.report({ fix })`; alternatives and behavior-changing actions should remain suggestions.

## 9. Testing strategy

### Go unit/integration tests

1. Preserve all existing Effect quick-fix baselines. They already register `etslshooks`, discover V3/V4 cases, inventory actions, and apply non-disable actions. [`internal/effecttest/quickfix_runner_test.go:11-39`](internal/effecttest/quickfix_runner_test.go#L11-L39), [`internal/effecttest/quickfix_runner_test.go:43-65`](internal/effecttest/quickfix_runner_test.go#L43-L65), [`internal/effecttest/quickfix_runner.go:80-99`](internal/effecttest/quickfix_runner.go#L80-L99)
2. Test shared action computation directly with project-config options and settings-only options; the latter guards against accidentally retaining the current LSP hook's `Program.Options().Effect != nil` gate.
3. Protocol v2 round-trip tests: `includeFixes` absent/false, true with no applicable action, true with one action, multiple alternatives, and malformed protocol version.
4. Assert action titles and exact UTF-16 ranges/text, including an astral character before every edit and CRLF input.
5. Assert multi-edit action ordering and omission on overlap/out-of-range.
6. Assert `effectDisable` is omitted and non-fixable metadata rules return no actions.
7. Assert the current-file overlay, not disk text, drives both diagnostic and edits.
8. Assert settings-only Effect configuration can compute fixes.

### Oxlint end-to-end tests

1. Normal lint: diagnostic and suggestions are present; source is unchanged.
2. Plain `--fix`: source remains unchanged because Effect actions are suggestions.
3. `--fix-suggestions`: the first suggestion is applied and fixed diagnostics are no longer reported where appropriate. Oxlint documents this flag as behavior-changing. [`.repos/oxlint/apps/oxlint/src/command/lint.rs:219-249`](.repos/oxlint/apps/oxlint/src/command/lint.rs#L219-L249)
4. Multiple edits in one Effect action are applied atomically.
5. Two overlapping Effect diagnostics: verify Oxlint applies only one normal candidate and leaves the other diagnostic, matching its fixer contract. [`.repos/oxlint/crates/oxc_linter/src/fixer/mod.rs:389-436`](.repos/oxlint/crates/oxc_linter/src/fixer/mod.rs#L389-L436)
6. `// oxlint-disable-next-line effect/<rule>` suppresses both the diagnostic and its suggestions.
7. Oxlint rule severity `warn` versus `error` remains authoritative and unaffected by Effect options.
8. Non-BMP Unicode before/inside the diagnostic and edit proves the full Go UTF-8 byte -> wire UTF-16 -> Oxlint UTF-8 conversion path.
9. Unsaved/current-text simulation proves the action applies to `context.sourceCode.text` even when disk differs.
10. Run at least one V3 and one V4 fixture because existing fix coverage spans both suites.

### Validation workflow after implementation

Because implementation would change Go and TypeScript files, follow this repository's required sequence: `pnpm setup-repo`, `pnpm lint`, `pnpm check`, then `pnpm test`. The existing Oxlint prototype remains separately runnable through its workspace test command. [`package.json:2-17`](package.json#L2-L17), [`testdata/tests/oxlint/package.json:1-7`](testdata/tests/oxlint/package.json#L1-L7)

## 10. Blockers and unknowns

### Not blockers for suggestion support

- **No general code-fix API:** not needed; the in-process provider model and existing shims are sufficient.
- **No new TypeScript-Go patch:** not needed for current-file Effect actions.
- **Cross-file action transport:** current Effect action construction discards non-current-file changes already.
- **Fix IDs/fix-all:** Oxlint suggestions do not require them, and current Effect fix IDs are not wired to returned actions.

### Real blockers for safe automatic `--fix`

- **No safety classification.** Effect providers do not state whether an action preserves behavior. Names such as “unsafe” describe diagnosed APIs, not action applicability, and cannot be used as a mechanical classification. [`internal/fixable/fixable.go:12-32`](internal/fixable/fixable.go#L12-L32), [`internal/fixables/unsafe_effect_type_assertion.go:11-34`](internal/fixables/unsafe_effect_type_assertion.go#L11-L34)
- **No preferred action.** Multiple actions can be returned and `ls.CodeAction` supports no Effect-populated preference field. [`typescript-go/internal/ls/codeactions.go:38-45`](typescript-go/internal/ls/codeactions.go#L38-L45), [`testdata/baselines/reference/effect-v4/catchTagToCatchReason_preview.quickfixes.txt:1-16`](testdata/baselines/reference/effect-v4/catchTagToCatchReason_preview.quickfixes.txt#L1-L16)
- **No dangerous-fix marker in the JS report path.** A JS direct fix becomes a safe fix, so the adapter cannot honestly route an unaudited action to only `--fix-dangerously`. [`.repos/oxlint/crates/oxc_linter/src/lib.rs:870-890`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L870-L890)

### Unknowns to resolve during implementation

- Whether every current Effect multi-edit action is non-overlapping after TypeScript change-tracker normalization. Oxlint will reject overlapping edits within one action, so this needs a corpus test over all existing quick-fix baselines.
- Whether any provider's output depends materially on editor formatting preferences unavailable in the prototype's snapshot defaults. The language service obtains preferences from the snapshot host, but the bridge does not receive user editor formatting settings. [`typescript-go/internal/ls/languageservice.go:24-53`](typescript-go/internal/ls/languageservice.go#L24-L53)
- Whether Oxlint's editor defaults always request/show JavaScript suggestions for external plugins at the pinned and future versions. Rust only attaches JS suggestions when the active fix kind permits suggestions. [`.repos/oxlint/crates/oxc_linter/src/lib.rs:873-890`](.repos/oxlint/crates/oxc_linter/src/lib.rs#L873-L890)
- Whether wrapper `meta.hasSuggestions` should be true for every rule or generated from `metadata.fixable`. The latter is more precise, but a rule-level flag can be true while a particular diagnostic code/instance produces no action.
- How a future complete multi-file unsaved editor overlay reaches the persistent child. This is an existing diagnostic correctness issue and can make both diagnostics and fixes semantically stale with respect to imported unsaved files; it is not introduced by the quick-fix extension.
- Whether Effect-specific disable actions should later be translated into native `oxlint-disable-next-line effect/<rule>` suggestions. That would be an Oxlint-facing feature, not faithful transport of the existing Effect action.

## Final assessment

Suggestion-grade quick-fix support is a contained extension of the current prototype: shared action computation, one versioned wire-field addition, UTF-16 edit conversion, and plugin suggestion mapping. It requires no LSP transport, TypeScript-Go patch, or new shim. Automatic `--fix` should not ship until Effect actions gain explicit safety and preferred-action semantics.
