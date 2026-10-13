import * as Effect from "effect/Effect"
import * as Stream from "effect/Stream"

export const shouldReport: Effect.Effect<void> = Effect.succeed(Effect.succeed(42))

export const shouldReport2 = Effect.suspend((): Effect.Effect<void> => {
  return Stream.empty.pipe(
    Stream.runCollect,
    Effect.interruptible,
    Effect.matchCause({
      onSuccess: () => "success",
      onFailure: () => Effect.fail("error")
    })
  )
})

interface NestedSubtype extends Effect.Effect<Effect.Effect<number>> {}
declare const nestedSubtype: NestedSubtype

export const shouldReportSubtype = Effect.suspend((): Effect.Effect<void> => nestedSubtype)
