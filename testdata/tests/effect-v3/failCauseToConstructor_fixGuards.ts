// @filename: literalUnions.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { Cause, Effect } from "effect"
import { failCause } from "effect/Effect"

declare const cond: boolean
declare const error: Error

// MUST TRIGGER: eager literal union; Cause.fail and Effect.fail both infer string
// -> Effect.fail(cond ? "A" : "B")
export const eagerLiteralUnion = Effect.failCause(Cause.fail(cond ? "A" : "B"))

// MUST TRIGGER (report only): Cause.fail widens the literal union to string, but a
// failSync thunk would keep "A" | "B", so the rewrite would change the error type
export const lazyLiteralUnion = Effect.failCauseSync(() => Cause.fail(cond ? "A" : "B"))
export const lazyMixedUnion = Effect.failCauseSync(() => Cause.fail(cond ? error : "B"))

// MUST TRIGGER: the value's own type is the inferred error type -> Effect.failSync(() => error)
export const lazySameType = Effect.failCauseSync(() => Cause.fail(error))

// MUST TRIGGER (report only): the parameter shadows the file's Effect binding where
// the fallback fix would write Effect.fail
export const shadowedEffect = (Effect: { readonly name: string }) => failCause(Cause.fail(Effect.name))

// @filename: typeOnlyImport.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import type { Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"

declare const error: Error

// MUST TRIGGER (report only): Effect is a type-only binding; Effect.fail would be TS1361
export const eager: Effect.Effect<never, Error> = failCause(causeFail(error))
export const lazy: Effect.Effect<never, Error> = failCauseSync(() => causeFail(error))

// @filename: typeOnlySpecifier.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { type Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCause } from "effect/Effect"

declare const error: Error

// MUST TRIGGER (report only): an inline type-only specifier is not a value either
export const eager: Effect.Effect<never, Error> = failCause(causeFail(error))

// @filename: typeOnlyNamespace.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import type * as Fx from "effect/Effect"
import { fail as causeFail } from "effect/Cause"
import { failCause } from "effect/Effect"

declare const error: Error

// MUST TRIGGER (report only): a type-only namespace import is not a value
export const eager: Fx.Effect<never, Error> = failCause(causeFail(error))

// @filename: reexport.ts
export type { Effect } from "effect"

// @filename: typeOnlyReexport.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { Effect } from "./reexport"
import { fail as causeFail } from "effect/Cause"
import { failCause } from "effect/Effect"

declare const error: Error

// MUST TRIGGER (report only): Effect reaches the module through a type-only re-export
export const eager: Effect.Effect<never, Error> = failCause(causeFail(error))
