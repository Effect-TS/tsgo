// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics catchAllToMapError:warning
// @effect-diagnostics catchToOrElseSucceed:warning
import { pipe } from "effect/Function"
import { catchAll, catchAll as recover, fail, succeed } from "effect/Effect"
import * as Effect from "effect/Effect"

declare const program: Effect.Effect<number, string>

// The message names Effect.catchAll whichever way the callee is written

// Named import of catchAll
export const namedMapError = pipe(program, catchAll((error) => fail(error.length)))
export const namedOrElseSucceed = pipe(program, catchAll(() => succeed(0)))

// Renamed import of catchAll
export const renamedMapError = pipe(program, recover((error) => fail(error.length)))
export const renamedOrElseSucceed = pipe(program, recover(() => succeed(0)))

// Local alias of Effect.catchAll
const catchAlias = Effect.catchAll
export const aliasMapError = pipe(program, catchAlias((error) => Effect.fail(error.length)))

// Property access, as before
export const propertyMapError = program.pipe(Effect.catchAll((error) => Effect.fail(error.length)))
