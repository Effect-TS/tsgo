# missedPipeableOpportunity Diagnostic Rule

## Goal

Detect nested function call chains that could be rewritten in pipeable style (using `.pipe()`), and offer a quick fix to perform the conversion automatically. This improves code readability by making data flow explicit and linear.

## Scope

- A new diagnostic rule `missedPipeableOpportunity` in `internal/rules/`.
- A pipeability checker function in `internal/typeparser/`.
- A `ReconstructPipingFlow` function in `internal/typeparser/`.
- A corresponding quick fix (fixable) in `internal/fixables/`.
- The rule and fix must work for both **Effect v3** and **Effect v4**. The rule logic is version-agnostic — it relies on type information (`.pipe()` presence, call signatures) rather than API-specific naming. Test baselines must cover both v3 and v4 harnesses.

## Requirements

### Rule Behavior

1. The rule must be **off by default** (`SeverityOff`). Users opt in via directives or plugin options.

2. The rule must accept a configurable **minimum transformation count** (`pipeableMinArgCount`, default: 2). Only piping flows with at least this many contiguous pipeable transformations trigger the diagnostic.

3. The rule must use the existing `PipingFlows` infrastructure to collect all piping flows in the source file, excluding Effect.fn flows (pass `includeEffectFn: false`).

4. For each piping flow, the rule must find the longest contiguous segment of transformations where:
   - Every intermediate type in the segment is **pipeable** (has a callable `.pipe()` method).
   - Every callee in the segment is **safely pipeable** (would not lose `this` context when extracted into a pipe argument).

5. If a contiguous pipeable segment meets the minimum transformation count threshold, the rule emits a diagnostic on the outermost expression of that segment.

6. The rule must **skip** flows where the final output type is callable (returns a function), since these are typically partial application patterns where pipe style is not idiomatic.

### Pipeability Checker

7. A type is pipeable if it has a property named `"pipe"` and that property's type has at least one call signature. The check must use `GetPropertyOfType` + `GetSignaturesOfType` on the pipe property.

### Safe Callee Check

8. A callee is safely pipeable if it does not depend on a `this` context that would be lost in pipe style. Safely pipeable callees include:
   - Call expressions (already evaluated, no `this` issue).
   - Arrow functions (lexical `this`).
   - Function expressions.
   - Identifiers pointing to standalone functions, modules, or imports.
   - Property access on modules/namespaces (e.g., `Effect.map`).

   Unsafe callees include:
   - Property access on instances (method calls that depend on `this`).

### Piping Flow Reconstruction

9. A `ReconstructPipingFlow` function must be implemented that takes a piping flow (subject + transformations) and produces an AST expression by applying transformations sequentially:
   - For `"call"` transformations: `callee(accumulator)`.
   - For `"pipe"` or `"pipeable"` transformations with args: `callee(args...)(accumulator)` (curried).
   - For `"pipe"` or `"pipeable"` transformations without args: `callee(accumulator)`.
   - `"effectFn"` and `"effectFnUntraced"` transformations cannot be reconstructed; if all transformations are of these kinds, return the original subject node.

### Quick Fix

10. The quick fix must convert the detected pipeable segment to `.pipe()` style:
    - The subject of the pipeable segment becomes the receiver of `.pipe()`.
    - The pipeable transformations become arguments to `.pipe()`.
    - Transformations before the pipeable segment (the "prefix") must be reconstructed around the subject using `ReconstructPipingFlow`.
    - Transformations after the pipeable segment (the "suffix") must be reconstructed around the `.pipe()` call using `ReconstructPipingFlow`.

11. The quick fix must follow the established Analyze + Match pattern: the rule exposes an `AnalyzeMissedPipeableOpportunity` function returning match structs, and the fixable consumes those matches.

## Non-Goals

- Detecting opportunities to convert `.pipe()` style back to nested calls.
- Handling `Effect.fn` / `Effect.fnUntraced` transformations in reconstruction (they are structurally part of the fn call itself).
- Piping flow extraction — this already exists in `internal/typeparser/piping_flow.go`.

## Acceptance Criteria

1. The rule does not fire when severity is off (default) and no directive enables it.
2. The rule fires on nested call chains with sufficient contiguous pipeable transformations.
3. The rule does not fire on chains where intermediate types lack `.pipe()`.
4. The rule does not fire on chains involving unsafe callees (instance methods).
5. The rule does not fire when the final output type is callable.
6. The quick fix correctly converts nested calls to `.pipe()` style.
7. The `pipeableMinArgCount` threshold is respected.
8. Baseline tests cover basic cases, edge cases with unsafe callees, and threshold behavior for both Effect v3 and v4 harnesses.
