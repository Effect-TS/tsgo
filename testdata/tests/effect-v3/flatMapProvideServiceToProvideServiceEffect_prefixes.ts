// @effect-v3
import { Context, Effect, pipe } from "effect"

class CurrentMeta extends Context.Reference<CurrentMeta>()("CurrentMeta", {
  defaultValue: (): { readonly attempt: number } => ({ attempt: 0 })
}) {}

declare const makeMeta: Effect.Effect<{ readonly attempt: number }>

const metaProgram = Effect.gen(function*() {
  const meta = yield* CurrentMeta
  return meta.attempt
})
const bump = (meta: { readonly attempt: number }) => ({ attempt: meta.attempt + 1 })

// MUST TRIGGER: the acquisition is a function-pipe prefix.
// Fix: Effect.provideServiceEffect(metaProgram, CurrentMeta, pipe(makeMeta, Effect.map(bump)))
export const functionPipePrefix = pipe(
  makeMeta,
  Effect.map(bump),
  Effect.flatMap((meta) => Effect.provideService(metaProgram, CurrentMeta, meta))
)

// MUST TRIGGER: a function-pipe prefix with a later step, which stays in the pipe.
export const functionPipePrefixLaterStep = pipe(
  makeMeta,
  Effect.map(bump),
  Effect.map(bump),
  Effect.flatMap((meta) => Effect.provideService(metaProgram, CurrentMeta, meta)),
  Effect.map((attempt) => attempt * 2)
)

// MUST TRIGGER: chained pipes; the acquisition is the receiver of the second pipe.
export const chainedPipes = makeMeta.pipe(Effect.map(bump)).pipe(
  Effect.flatMap((meta) => Effect.provideService(metaProgram, CurrentMeta, meta))
)

// MUST TRIGGER: `self` has function-pipe steps before provideService.
// Fix: Effect.provideServiceEffect(pipe(metaProgram, Effect.map(...)), CurrentMeta, makeMeta)
export const selfFunctionPipeSteps = makeMeta.pipe(
  Effect.flatMap((meta) => pipe(metaProgram, Effect.map((n) => n + 1), Effect.provideService(CurrentMeta, meta)))
)

// MUST TRIGGER: `self` has two pipeable steps before provideService.
export const selfTwoSteps = makeMeta.pipe(
  Effect.flatMap((meta) =>
    metaProgram.pipe(Effect.map((n) => n + 1), Effect.map((n) => n * 2), Effect.provideService(CurrentMeta, meta))
  )
)

// MUST NOT TRIGGER: a function-expression callback whose `self` uses its own `this`,
// which would change meaning once `self` leaves the callback.
export const usesThis = makeMeta.pipe(
  Effect.flatMap(function(this: unknown, meta) {
    return Effect.provideService(Effect.as(metaProgram, this), CurrentMeta, meta)
  })
)
