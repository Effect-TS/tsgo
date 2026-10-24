// @filename: nsdep.ts
/** @stability experimental */
export interface NsExperimental {
  value: string
}

export interface NsLeaky {
  value: NsExperimental
}

// @filename: namespace.ts
// @effect-v4
// @effect-diagnostics apiStabilityLeak:warning
// A namespace re-export must enumerate the dependency's exported symbols and
// report the offending member at the namespace export in this file.
export * as api from "./nsdep"
