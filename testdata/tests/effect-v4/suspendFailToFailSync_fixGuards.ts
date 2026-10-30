// @filename: shapes.ts
// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics suspendFailToFailSync:warning
import { Effect, pipe } from "effect"

declare const error: Error
declare const cond: boolean
declare const tag: "A" | "B"

// MUST TRIGGER: the suspended effect is a Function.pipe subject -> pipe(Effect.failSync(() => error), ...)
export const pipeSubject = pipe(
  Effect.suspend(() => Effect.fail(error)),
  Effect.catch(() => Effect.succeed(0))
)

// MUST TRIGGER: an object literal stays an expression body -> Effect.failSync(() => ({ _tag: "Oops" }))
export const objectLiteral = Effect.suspend(() => Effect.fail({ _tag: "Oops" }))

// MUST TRIGGER: a union-typed binding is inferred the same in both forms -> Effect.failSync(() => tag)
export const unionBinding = Effect.suspend(() => Effect.fail(tag))

// MUST TRIGGER (report only): Effect.fail widens "B" to string, a failSync thunk would keep it
export const mixedUnion = Effect.suspend(() => Effect.fail(cond ? error : "B"))

// @filename: typeOnlyEffect.ts
// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics suspendFailToFailSync:warning
import type { Effect } from "effect"
import { fail, suspend } from "effect/Effect"

declare const error: Error

// MUST TRIGGER (report only): Effect is a type-only binding; Effect.failSync would be TS1361
export const lazy: Effect.Effect<never, Error> = suspend(() => fail(error))

// @filename: shadowedEffect.ts
// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics suspendFailToFailSync:warning
import { Effect } from "effect"
import { fail, suspend } from "effect/Effect"

// MUST TRIGGER (report only): the parameter shadows the file's Effect binding where
// the fallback fix would write Effect.failSync
export const shadowed = (Effect: { readonly name: string }) => suspend(() => fail(Effect.name))

export const program = Effect.void
