// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics timeoutCatchTagToTimeoutOrElse:warning
import { Cause, Effect, Option, pipe } from "effect"
import * as E from "effect/Effect"

declare const task: Effect.Effect<string>
declare const tagged: Effect.Effect<string, { readonly _tag: "OtherError" }>
declare const innerTimeout: Effect.Effect<string, Cause.TimeoutError>
declare const customTimeout: Effect.Effect<string, { readonly _tag: "TimeoutError" }>
declare const broadTag: Effect.Effect<string, { readonly _tag: string }>
declare const unknownError: Effect.Effect<string, unknown>

export const basic = task.pipe(Effect.timeout("5 seconds"), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))
export const option = task.pipe(Effect.timeout(1), Effect.asSome, Effect.catchTag("TimeoutError", () => Effect.succeedNone))
export const optionMap = pipe(task, Effect.timeout(1), Effect.map(Option.some), Effect.catchTag("TimeoutError", () => Effect.succeed(Option.none())))
export const dataFirst = Effect.catchTag(Effect.timeout(task, 1), "TimeoutError", () => Effect.succeed("fallback"))
export const curriedTimeout = Effect.catchTag(Effect.timeout(1)(task), "TimeoutError", () => Effect.succeed("fallback"))
export const nestedOption = Effect.catchTag(Effect.asSome(Effect.timeout(task, 1)), "TimeoutError", () => Effect.succeedNone)
export const mixed = Effect.catchTag(task.pipe(Effect.timeout(1)), "TimeoutError", () => Effect.succeed("fallback"))
export const priorStep = innerTimeout.pipe(Effect.orDie, Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")), Effect.asVoid)
export const otherError = tagged.pipe(E.timeout(1), E.catchTag("TimeoutError", () => E.succeed("fallback")))
export const effectFn = Effect.fn(function*() { return yield* task }, Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))
export const block = task.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", () => { const value = "fallback"; return Effect.succeed(value) }))

const catchTag = Effect.catchTag
export const aliasedCatchTag = task.pipe(Effect.timeout(1), catchTag("TimeoutError", () => Effect.succeed("fallback")))

// Must not change: these handlers also catch failures from the input.
export const collision = innerTimeout.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))
export const structuralCollision = customTimeout.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))
export const introduced = task.pipe(Effect.flatMap(() => innerTimeout), Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))
export const broad = broadTag.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))
// @ts-expect-error unknown error cannot be dispatched with catchTag
export const unknown = unknownError.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))

// Must not change: non-adjacent steps, different handlers, or error inspection.
export const nonAdjacent = task.pipe(Effect.timeout(1), Effect.map(String), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))
export const observes = task.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", (error) => Effect.succeed(error.message)))
export const differentOption = task.pipe(Effect.timeout(1), Effect.asSome, Effect.catchTag("TimeoutError", () => Effect.succeed(Option.some("fallback"))))
export const differentTag = tagged.pipe(Effect.timeout(1), Effect.catchTag("OtherError", () => Effect.succeed("fallback")))
export const catchTags = task.pipe(Effect.timeout(1), Effect.catchTags({ TimeoutError: () => Effect.succeed("fallback") }))
const fake = { timeout: (_duration: number) => <A, Err, R>(self: Effect.Effect<A, Err, R>) => self }
export const lookalike = innerTimeout.pipe(fake.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback")))

export const unusedParameter = task.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", (_error) => Effect.succeed("fallback")))
export const unusedOptionParameter = task.pipe(Effect.timeout(1), Effect.asSome, Effect.catchTag("TimeoutError", (_error) => Effect.succeedNone))
export const nestedReference = task.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", (error) => Effect.sync(() => error.message)))
export const destructured = task.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", ({ message }) => Effect.succeed(message)))
export const functionArguments = task.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", function() { return Effect.succeed(String(arguments[0])) }))
export const shorthand = task.pipe(Effect.timeout(1), Effect.catchTag("TimeoutError", (error) => Effect.succeed({ error })))
