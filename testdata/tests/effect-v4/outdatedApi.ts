import { Effect } from "effect"

// === Removed APIs — SHOULD trigger warnings ===

// Effect.runtime (removed)
export const p_runtime = Effect.gen(function*() {
    const runtime = yield* Effect.runtime()
})

// Effect.dieMessage (removed)
export const p_dieMessage = Effect.dieMessage("something went wrong")

// Effect.dieSync (removed)
export const p_dieSync = Effect.dieSync(() => new Error("boom"))

// Effect.Tag (removed)
class MyTag extends Effect.Tag("MyTag")<MyTag, string>() {}

// Effect.zipLeft (removed)
export const p_zipLeft = Effect.zipLeft(Effect.succeed(1), Effect.succeed(2))

// Effect.zipRight (removed)
export const p_zipRight = Effect.zipRight(Effect.succeed(1), Effect.succeed(2))

// Effect.orElse (removed)
export const p_orElse = Effect.orElse(Effect.fail("err"), () => Effect.succeed(0))

// Effect.makeLatch (removed — post-fix state from commit 0154667)
export const p_makeLatch = Effect.makeLatch()

// Effect.makeSemaphore (removed — post-fix state from commit 0154667)
export const p_makeSemaphore = Effect.makeSemaphore(1)

// === Renamed APIs (same behaviour) — SHOULD trigger warnings ===

// Effect.catchAll → Effect.catch
export const p_catchAll = Effect.catchAll(Effect.fail("err"), (_e) => Effect.succeed(0))

// Effect.catchAllCause → Effect.catchCause
export const p_catchAllCause = Effect.catchAllCause(Effect.fail("err"), (_cause) => Effect.succeed(0))

// Effect.catchAllDefect → Effect.catchDefect
export const p_catchAllDefect = Effect.catchAllDefect(Effect.die("defect"), (_defect) => Effect.succeed(0))

// Effect.fork → Effect.forkChild
export const p_fork = Effect.fork(Effect.succeed(1))

// Effect.forkDaemon → Effect.forkDetach
export const p_forkDaemon = Effect.forkDaemon(Effect.succeed(1))

// === Renamed APIs (needs options) — SHOULD trigger warnings ===

// Effect.async → Effect.callback
export const p_async = Effect.async<number>((resume) => {
    resume(Effect.succeed(42))
})

// Effect.either → Effect.result
export const p_either = Effect.either(Effect.succeed(1))

// === Unchanged APIs — should NOT trigger warnings ===

// Effect.succeed
export const u_succeed = Effect.succeed(1)

// Effect.map
export const u_map = Effect.map(Effect.succeed(1), (n) => n + 1)

// Effect.gen
export const u_gen = Effect.gen(function*() {
    return yield* Effect.succeed(1)
})

// Effect.flatMap
export const u_flatMap = Effect.flatMap(Effect.succeed(1), (n) => Effect.succeed(n + 1))

// Effect.Do (unchanged — post-fix state from commit 0154667)
export const u_Do = Effect.Do

// Effect.bind (unchanged — post-fix state from commit 0154667)
export const u_bind = Effect.bind(Effect.Do, "a", () => Effect.succeed(1))

// Effect.partition (unchanged — post-fix state from commit 0154667)
export const u_partition = Effect.partition([Effect.succeed(1), Effect.fail("err")])

// Effect.filterMap (unchanged in v4 — still exists on the module)
export const u_filterMap = Effect.filterMap([Effect.succeed(1), Effect.succeed(2)], (n) => n > 1 ? Effect.succeed(n) : Effect.succeed(null))
