// @effect-diagnostics *:off
// @effect-diagnostics preferEffectRecord:warning
import * as Record from "effect/Record"

const scores = { alice: 1, bob: 2 }
export const native = Object.entries(scores)
export const preferred = Record.toEntries(scores)
