import { Effect } from "effect"

interface EnvA {
  _tag: "EnvA"
}

declare const effectWithContext: Effect.Effect<number, never, EnvA>

// TS expect-error suppresses TS2322 but NOT Effect TS377004
// @ts-expect-error
export const missingContext: Effect.Effect<number> = effectWithContext

// TS expect-error does NOT suppress floating Effect TS377001
// @ts-expect-error
Effect.succeed("floating")

// TS ignore does NOT suppress floating Effect TS377001
// @ts-ignore
Effect.succeed("ignored")

// TS expect-error combined with effect-diagnostics-next-line suppresses the Effect diagnostic
// @ts-expect-error
// @effect-diagnostics-next-line floatingEffect:off
Effect.succeed("both suppressed")
