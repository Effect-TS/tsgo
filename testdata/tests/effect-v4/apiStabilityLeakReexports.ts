// @filename: dep.ts
/** @stability experimental */
export interface DepExperimental {
  value: string
}

export interface DepLeaky {
  value: DepExperimental
}

/** @stability unstable */
export interface DepUnstable {
  value: string
}

// @filename: named.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// Re-exporting a stable API whose surface exposes an experimental type reports
// at the re-export specifier in this file.
export { DepLeaky } from "./dep"
// Re-exporting the unstable type directly inherits its stability, so the export
// is treated as unstable and does not itself leak.
export { DepUnstable } from "./dep"

// @filename: star.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// `export *` forwards DepLeaky, whose surface leaks an experimental type. The
// diagnostic is anchored at the `export *` declaration in this file.
export * from "./dep"

// @filename: tagged.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// The tag on the re-export declaration governs its ceiling: an experimental
// re-export imposes no restriction on the forwarded surface.
/** @stability experimental */
export { DepLeaky } from "./dep"

// @filename: barrel.ts
export { DepLeaky } from "./dep"

// @filename: aliased.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A namespace re-export of a barrel resolves the forwarded alias and reports
// the offending member at the namespace export in this file.
export * as api from "./barrel"

// @filename: taggedNamespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A tagged namespace re-export keeps its own ceiling and reports nothing.
/** @stability experimental */
export * as api from "./dep"
