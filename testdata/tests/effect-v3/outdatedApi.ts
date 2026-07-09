// @effect-v3
import * as Effect from "effect/Effect"

// These are all v3 APIs — no diagnostics should be emitted for a v3 project

// Removed in v4, but valid in v3
export const p_runtime = Effect.gen(function*() {
    const runtime = yield* Effect.runtime()
})

export const p_dieMessage = Effect.dieMessage("something went wrong")

export const p_catchAll = Effect.catchAll(Effect.fail("err"), (_e) => Effect.succeed(0))

export const p_fork = Effect.fork(Effect.succeed(1))

// Unchanged — valid in both v3 and v4
export const u_succeed = Effect.succeed(1)

export const u_gen = Effect.gen(function*() {
    return yield* Effect.succeed(1)
})
