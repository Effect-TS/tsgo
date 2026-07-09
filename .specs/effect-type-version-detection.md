# Effect Type Version-Aware Detection

## Goal
Support detecting Effect types for both Effect v3 and Effect v4, so diagnostics that rely on Effect type identification work across both versions.

## Scope
- Applies to Effect type detection only (not Layer, Service, or other Effect types).
- Covers the type parser's ability to identify a type as an Effect and extract its variance parameters (A, E, R).
- Covers version fallback behavior when the Effect version is unknown.
- Covers a normalized supported-version helper for call sites that should not branch on `unknown`.

## Requirements
1. When the detected Effect version is v4, Effect type identification must use the current direct symbol lookup approach (looking up the `~effect/Effect` computed property by name).
2. When the detected Effect version is v3 (or unknown), Effect type identification must iterate the type's properties, filtering for required non-optional properties, and attempt to parse each one as a variance struct until one succeeds.
3. When the detected Effect version is unknown, the type parser must default to v3 behavior.
4. For v3 detection, properties containing "EffectTypeId" in their name should be tried first as an optimization heuristic.
5. The variance struct extraction logic (reading `_A`, `_E`, `_R` as covariant type parameters) is shared between both versions.
6. The variance extraction helpers (covariant, contravariant, and invariant) must reject call signatures that have type parameters. A generic call signature (e.g., `<T>() => T`) is not a valid variance encoding — only non-generic signatures should be accepted. When a signature has type parameters, the extraction must fail (return nil) rather than proceeding with the return type or parameter type.
7. The Effect type parsing function must detect the Effect version internally (by calling the version detection function itself) rather than receiving it as a parameter. This keeps the public API of the type parser unchanged and avoids spreading a version parameter through all call sites.
8. The type parser module must provide a `SupportedEffectVersion` function that internally checks detected version and returns:
   - Effect v4 when detected version is v4.
   - Effect v3 for all other outcomes (including unknown).
9. Diagnostic and type-parsing call sites that only need supported runtime behavior must use `SupportedEffectVersion` rather than branching on `unknown` from raw detection.
10. The `SupportedEffectVersion` contract must be the central extension point for future compiler-option-based version forcing, so new forcing behavior can be added there without refactoring all call sites.

## Non-Goals
- Supporting v3 detection for Layer, Service, or other Effect ecosystem types.
- Adapting individual diagnostic rules for v3 behavioral differences.
- Supporting Effect versions other than v3 and v4.

## Acceptance Criteria
1. Given an Effect v4 project, Effect type detection continues to work as before via direct symbol lookup.
2. Given an Effect v3 project, Effect type detection identifies Effect types via property iteration and successfully extracts A, E, R parameters.
3. Given a project with no detectable Effect version, the type parser defaults to v3 behavior.
4. `SupportedEffectVersion` returns v4 only when v4 is detected; otherwise it returns v3.
5. Call sites that branch for supported behavior do not need to handle `unknown` directly.
6. The covariant, contravariant, and invariant extraction helpers return nil when the call signature has type parameters (is generic).
