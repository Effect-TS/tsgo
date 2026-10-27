// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Data, Effect, Either, Exit, pipe, STM, Stream } from "effect"
import * as E from "effect/Effect"
import { map as mapSuccess, mapError as mapFailure } from "effect/Effect"

class TaskError extends Data.TaggedError("TaskError")<{
  readonly message: string
}> {}

declare const fetchCount: Effect.Effect<number, string>
declare const fetchName: Effect.Effect<string, Error, "Service">
declare const record: (value: unknown) => void

// MUST TRIGGER: pipeable, Effect.map directly followed by Effect.mapError
export const pipeableMapThenMapError = fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.mapError((message) => new TaskError({ message }))
)

// MUST TRIGGER: pipeable, reverse order (Effect.mapError then Effect.map)
export const pipeableMapErrorThenMap = fetchCount.pipe(
  Effect.mapError((message) => new TaskError({ message })),
  Effect.map((n) => n > 0)
)

// MUST TRIGGER: function pipe style
export const functionPipe = pipe(
  fetchCount,
  Effect.map((n) => n + 1),
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER: function pipe style, reverse order
export const functionPipeReverse = pipe(
  fetchCount,
  Effect.mapError((message) => message.length),
  Effect.map((n) => n + 1)
)

// MUST TRIGGER: nested data-first, Effect.map(Effect.mapError(eff, g), f)
export const nestedDataFirst = Effect.map(
  Effect.mapError(fetchCount, (message) => new TaskError({ message })),
  (n) => n > 0
)

// MUST TRIGGER: nested data-first, Effect.mapError(Effect.map(eff, f), g)
export const nestedDataFirstReverse = Effect.mapError(
  Effect.map(fetchCount, (n) => n > 0),
  (message) => new TaskError({ message })
)

// MUST TRIGGER: the pair sits in the middle of a longer flow (other steps preserved)
export const middleOfLongerFlow = fetchName.pipe(
  Effect.tap((name) => Effect.log(name)),
  Effect.map((name) => name.length),
  Effect.mapError((error) => new TaskError({ message: error.message })),
  Effect.flatMap((length) => Effect.succeed(length * 2))
)

// MUST TRIGGER: requirements channel is untouched, still a safe merge
export const withRequirements = fetchName.pipe(
  Effect.mapError((error) => error.message),
  Effect.map((name) => name.toUpperCase())
)

// MUST TRIGGER: point-free / function-reference callbacks
const toBoolean = (n: number) => n > 0
const toTaskError = (message: string) => new TaskError({ message })
export const functionReferences = fetchCount.pipe(Effect.map(toBoolean), Effect.mapError(toTaskError))

// MUST TRIGGER: block-bodied callbacks are still independent channel transformations
export const blockBodies = fetchCount.pipe(
  Effect.map((n) => {
    record(n)
    return n * 2
  }),
  Effect.mapError((message) => {
    record(message)
    return new TaskError({ message })
  })
)

// MUST TRIGGER: explicit type arguments on the combinators
export const explicitTypeArguments = fetchCount.pipe(
  Effect.map<number, string>((n) => String(n)),
  Effect.mapError<string, TaskError>((message) => new TaskError({ message }))
)

// MUST TRIGGER: namespace import alias (resolved by symbol, not by name)
export const namespaceAlias = fetchCount.pipe(
  E.map((n) => n + 1),
  E.mapError((message) => message.length)
)

// MUST TRIGGER: renamed named imports from "effect/Effect"
export const renamedImports = fetchCount.pipe(
  mapSuccess((n) => n + 1),
  mapFailure((message) => message.length)
)

// MUST TRIGGER: local constant aliases of the Effect APIs
const mapAlias = Effect.map
const mapErrorAlias = Effect.mapError
export const constantAliases = fetchCount.pipe(
  mapAlias((n: number) => n + 1),
  mapErrorAlias((message: string) => message.length)
)

// MUST TRIGGER: inside a generator, on a yielded flow
export const insideGen = Effect.gen(function*() {
  const value = yield* fetchCount.pipe(
    Effect.map((n) => n + 1),
    Effect.mapError((message) => new TaskError({ message }))
  )
  return value
})

// MUST TRIGGER: adjacent steps in the pipeline arguments of Effect.fn
export const effectFnPipeline = Effect.fn("effectFnPipeline")(
  function*(n: number) {
    if (n < 0) return yield* Effect.fail("negative")
    return n
  },
  Effect.map((n) => n * 2),
  Effect.mapError((message) => new TaskError({ message }))
)

// MUST TRIGGER (once): map, mapError, map -> pairs are taken left to right without
// overlap, so only the first map + mapError pair is reported
export const tripleMapMapErrorMap = fetchCount.pipe(
  Effect.map((n) => n + 1),
  Effect.mapError((message) => message.length),
  Effect.map((n) => n * 2)
)

// MUST TRIGGER (once): map, map, mapError -> only the second map is adjacent to mapError
export const twoMapsThenMapError = fetchCount.pipe(
  Effect.map((n) => n + 1),
  Effect.map((n) => n * 2),
  Effect.mapError((message) => message.length)
)

// MUST TRIGGER (twice): two disjoint adjacent pairs in the same flow
export const twoDisjointPairs = fetchCount.pipe(
  Effect.map((n) => n + 1),
  Effect.mapError((message) => message.length),
  Effect.mapError((length) => new TaskError({ message: String(length) })),
  Effect.map((n) => n > 0)
)

// MUST NOT TRIGGER: already using Effect.mapBoth
export const alreadyMapBoth = fetchCount.pipe(
  Effect.mapBoth({
    onSuccess: (n) => n > 0,
    onFailure: (message) => new TaskError({ message })
  })
)

// MUST NOT TRIGGER: only Effect.map
export const onlyMap = fetchCount.pipe(Effect.map((n) => n > 0))

// MUST NOT TRIGGER: only Effect.mapError
export const onlyMapError = fetchCount.pipe(Effect.mapError((message) => new TaskError({ message })))

// MUST NOT TRIGGER: two Effect.map steps (same channel, not a mapBoth candidate)
export const twoMaps = fetchCount.pipe(
  Effect.map((n) => n + 1),
  Effect.map((n) => n > 0)
)

// MUST NOT TRIGGER: two Effect.mapError steps (same channel)
export const twoMapErrors = fetchCount.pipe(
  Effect.mapError((message) => message.length),
  Effect.mapError((length) => new TaskError({ message: String(length) }))
)

// MUST NOT TRIGGER: an intervening step separates map and mapError
export const separatedByTap = fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.tap((positive) => Effect.log(positive)),
  Effect.mapError((message) => new TaskError({ message }))
)

// MUST NOT TRIGGER: nested data-first separated by an intervening call
export const nestedSeparated = Effect.map(
  Effect.tap(
    Effect.mapError(fetchCount, (message) => new TaskError({ message })),
    (n) => Effect.log(n)
  ),
  (n) => n > 0
)

// MUST TRIGGER: the two steps are adjacent in one flow across chained .pipe(...) calls
export const differentPipeCalls = fetchCount
  .pipe(Effect.map((n) => n > 0))
  .pipe(Effect.mapError((message) => new TaskError({ message })))

// MUST TRIGGER: data-first call wrapping a pipeable flow (mixed styles, adjacent in one flow)
export const mixedDataFirstAndPipeable = Effect.map(
  fetchCount.pipe(Effect.mapError((message) => new TaskError({ message }))),
  (n) => n > 0
)

// MUST NOT TRIGGER: Effect.as is a constant success mapping, not Effect.map
export const asThenMapError = fetchCount.pipe(
  Effect.as(true),
  Effect.mapError((message) => new TaskError({ message }))
)

// MUST NOT TRIGGER: Effect.flatMap is not a pure success transformation
export const flatMapThenMapError = fetchCount.pipe(
  Effect.flatMap((n) => Effect.succeed(n > 0)),
  Effect.mapError((message) => new TaskError({ message }))
)

// MUST NOT TRIGGER: Effect.catchAll recovers from the error, it does not just transform it
export const mapThenCatchAll = fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.catchAll((message) => Effect.succeed(message.length > 0))
)

// MUST NOT TRIGGER: Effect.mapErrorCause works on the whole Cause, not the typed error
export const mapThenMapErrorCause = fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.mapErrorCause((cause) => cause)
)

// MUST NOT TRIGGER: Effect.tapError observes the error channel, it does not transform it
export const mapThenTapError = fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.tapError((message) => Effect.log(message))
)

// MUST NOT TRIGGER: map and mapError belong to different (nested) flows
export const differentNestedFlows = fetchCount.pipe(
  Effect.map((n) => Effect.succeed(n).pipe(Effect.mapError(() => new TaskError({ message: "inner" })))),
  Effect.flatten
)

// MUST NOT TRIGGER: Stream.map + Stream.mapError (not the Effect module)
export const streamPair = Stream.make(1, 2, 3).pipe(
  Stream.map((n) => n + 1),
  Stream.mapError(() => new TaskError({ message: "stream" }))
)

// MUST NOT TRIGGER: Exit.map + Exit.mapError (not the Effect module)
declare const exit: Exit.Exit<number, string>
export const exitPair = exit.pipe(
  Exit.map((n) => n + 1),
  Exit.mapError((message) => new TaskError({ message }))
)

// MUST NOT TRIGGER: Either.map + Either.mapLeft (not the Effect module)
declare const either: Either.Either<number, string>
export const eitherPair = either.pipe(
  Either.map((n) => n + 1),
  Either.mapLeft((message) => new TaskError({ message }))
)

// MUST NOT TRIGGER: STM.map + STM.mapError (STM is an Effect subtype in v3, but these are not Effect module APIs)
declare const stm: STM.STM<number, string>
export const stmPair = stm.pipe(
  STM.map((n) => n + 1),
  STM.mapError((message) => new TaskError({ message }))
)

// MUST NOT TRIGGER: user-defined object whose methods are merely named map / mapError
const custom = {
  map: <A, B>(f: (a: A) => B) => (self: ReadonlyArray<A>) => self.map(f),
  mapError: <A>(_f: (a: A) => A) => (self: ReadonlyArray<A>) => self
}
export const customApi = pipe(
  [1, 2, 3],
  custom.map((n: number) => n + 1),
  custom.mapError((n: number) => n)
)
