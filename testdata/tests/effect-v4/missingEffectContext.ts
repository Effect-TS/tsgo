import { Effect } from "effect"

interface EnvA {
  _tag: "EnvA"
}

interface EnvB {
  _tag: "EnvB"
}

declare const effectWithContext: Effect.Effect<number, never, EnvA | EnvB>

// Should trigger missingEffectContext - missing EnvB
export const missingContextB: Effect.Effect<number, never, EnvA> = effectWithContext

// Should trigger missingEffectContext - assigning Effect with context to Effect<number> (no context)
export const missingAllContext: Effect.Effect<number> = effectWithContext

// Should NOT trigger - context matches
export const contextMatch: Effect.Effect<number, never, EnvA | EnvB> = effectWithContext

// Should NOT trigger - no context in source
declare const effectNoContext: Effect.Effect<number>
export const noSourceContext: Effect.Effect<number, never, EnvA> = effectNoContext
