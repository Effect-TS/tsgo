// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics effectTryWithoutCatch:warning
// @effect-diagnostics unknownInEffectCatch:warning
import { Effect } from "effect"

declare const text: string

// MUST TRIGGER effectTryWithoutCatch: the fix rewrites it to the form below
export const before = Effect.try(() => JSON.parse(text))

// MUST NOT TRIGGER effectTryWithoutCatch, MUST TRIGGER unknownInEffectCatch: the
// quick fix's output; its placeholder catch returns unknown, so unknownInEffectCatch
// takes over and asks for a domain error
export const after = Effect.try({ try: () => JSON.parse(text), catch: (cause) => cause })
