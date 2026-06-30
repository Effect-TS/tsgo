import { Effect, pipe } from "effect"

declare const voidEffect: Effect.Effect<void, string, never>
declare const undefinedEffect: Effect.Effect<undefined, string, never>
declare const numberEffect: Effect.Effect<number, string, never>

// Should trigger: catch returning Effect.void on a void success channel
export const shouldTriggerCatch = voidEffect.pipe(
  Effect.catch(() => Effect.void)
)

// Should trigger: catchCause returning Effect.void on a void success channel
export const shouldTriggerCatchCause = voidEffect.pipe(
  Effect.catchCause((_cause) => Effect.void)
)

// Should trigger: pipe style
export const shouldTriggerPipe = pipe(
  Effect.fail("error"),
  Effect.catch(() => Effect.void)
)

// Should trigger: block body
export const shouldTriggerBlock = undefinedEffect.pipe(
  Effect.catch(() => {
    return Effect.void
  })
)

// Should trigger: data-first style
export const shouldTriggerDataFirst = Effect.catch(voidEffect, () => Effect.void)

// Should NOT trigger: Effect.ignore would discard the number success value
export const shouldNotTriggerValueSuccess = numberEffect.pipe(
  Effect.catch(() => Effect.void)
)

// Should NOT trigger: fallback is not exactly Effect.void
export const shouldNotTriggerOtherFallback = voidEffect.pipe(
  Effect.catch(() => Effect.sync(() => undefined))
)
