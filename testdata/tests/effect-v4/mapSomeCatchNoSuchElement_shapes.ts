// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics mapSomeCatchNoSuchElement:warning
import { Cause, Effect, Option, pipe } from "effect"

declare const lookup: Effect.Effect<string, Cause.NoSuchElementError>
declare const lookupOrTimeout: Effect.Effect<string, Cause.NoSuchElementError | Cause.TimeoutError>
declare const withNativeError: Effect.Effect<string, Cause.NoSuchElementError | Error>

// MUST TRIGGER: Effect.fnUntraced pipeline steps
export const fnUntraced = Effect.fnUntraced(
  function*() {
    return yield* lookup
  },
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST TRIGGER: named Effect.fn pipeline steps
export const namedFn = Effect.fn("namedFn")(
  function*() {
    return yield* lookup
  },
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST TRIGGER: nested pipe(...) calls
export const nestedPipe = pipe(
  pipe(lookup, Effect.asSome),
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

const catchTag = Effect.catchTag

// MUST TRIGGER: a constant alias of Effect.catchTag
export const aliasedCatchTag = lookup.pipe(
  Effect.asSome,
  catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST TRIGGER: a pair inside an Effect.gen body
export const insideGen = Effect.gen(function*() {
  return yield* lookup.pipe(
    Effect.asSome,
    Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
  )
})

// MUST TRIGGER (once): the TimeoutError pair is rejected, the later NoSuchElementError pair is reported
export const rejectedThenValid = lookupOrTimeout.pipe(
  Effect.asSome,
  Effect.catchTag("TimeoutError", () => Effect.succeedNone),
  Effect.flatMap(Effect.fromOption),
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST TRIGGER: a parenthesized handler
export const parenthesizedHandler = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", (() => Effect.succeedNone))
)

// MUST NOT TRIGGER: piping flows keep a parenthesized step opaque, as for every other piping rule
export const parenthesizedWrap = lookup.pipe(
  (Effect.asSome),
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)
export const parenthesizedCatch = lookup.pipe(
  Effect.asSome,
  (Effect.catchTag("NoSuchElementError", () => Effect.succeedNone))
)

// MUST TRIGGER: comments around the pair; the fix keeps them on the replacement
export const commented = lookup.pipe(
  // wrap successes
  Effect.asSome,
  // recover missing values
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST TRIGGER: the fix keeps comments on kept steps, inside kept callbacks and
// on the same line as the replaced steps
export const commentsAcrossFlow = lookup.pipe(
  // kept step
  Effect.tap((value) => {
    // inside a kept callback
    return Effect.log(value)
  }),
  /* before wrap */ Effect.asSome, // after wrap
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone) // after catch
)

// MUST TRIGGER: data-first; comments on the kept subject stay with it and
// comments inside the replaced call move to the replacement
export const commentsDataFirst = Effect.catchTag(Effect.asSome(/* subject */ lookup), /* tag */ "NoSuchElementError", () => {
  // handler body
  return Effect.succeedNone
})

// MUST TRIGGER: comments inside empty parameter lists, calls and blocks are kept,
// in kept callbacks and in the replaced handler
export const commentsInEmptyDelimiters = lookup.pipe(
  Effect.tap((/* empty params */) =>
    Effect.sync(() => {
      /* empty block */
    })
  ),
  Effect.map((value) => String(/* empty call */) + value),
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", (/* handler params */) => {
    return Effect.succeedNone
    // end of handler
  })
)

// MUST NOT TRIGGER: a generic error channel may contain lookalike tags
export const generic = <A, E>(self: Effect.Effect<A, E | Cause.NoSuchElementError>) =>
  self.pipe(
    Effect.asSome,
    Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
  )

// MUST NOT TRIGGER: an untagged member makes the channel fail closed
export const nativeError = withNativeError.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST NOT TRIGGER: an annotated mapper parameter can widen the success type
export const annotatedMapper = lookup.pipe(
  Effect.map((value: string) => Option.some(value)),
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST NOT TRIGGER: an instantiation expression on the mapper
export const instantiatedMapper = lookup.pipe(
  Effect.map(Option.some<string>),
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST NOT TRIGGER: an async handler
export const asyncHandler = lookup.pipe(
  Effect.asSome,
  // @ts-expect-error an async handler returns a Promise, not an Effect
  Effect.catchTag("NoSuchElementError", async () => Effect.succeedNone)
)

// MUST TRIGGER: a function expression handler returning None cannot observe the error
export const functionHandler = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", function() {
    return Effect.succeedNone
  })
)

// MUST TRIGGER: catchTag always passes the error, so the default is never evaluated,
// and an unused rest parameter does not read it
export const defaultedParameter = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", (_error = new Cause.NoSuchElementError()) => Effect.succeedNone)
)
export const restParameter = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", (..._args) => Effect.succeedNone)
)

// MUST NOT TRIGGER: destructuring reads the error
export const destructuredParameter = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", ({ message: _message }) => Effect.succeedNone)
)
