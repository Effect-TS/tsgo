// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics effectTryWithoutCatch:warning
import { Data, Effect, pipe } from "effect"
import * as Eff from "effect/Effect"
import { try as attempt, tryPromise as attemptPromise } from "effect/Effect"
import * as Either from "effect/Either"

class ParseError extends Data.TaggedError("ParseError")<{ readonly cause: unknown }> {}
class FetchError extends Data.TaggedError("FetchError")<{ readonly cause: unknown }> {}

declare const text: string
declare const url: string
declare function parseDocument(input: string): { readonly title: string }

// MUST TRIGGER: short-form Effect.try fails with Cause.UnknownException
export const tryArrow = Effect.try(() => JSON.parse(text))

// MUST TRIGGER: short-form Effect.tryPromise fails with Cause.UnknownException
export const tryPromiseArrow = Effect.tryPromise(() => fetch(url))

// MUST TRIGGER: the short-form tryPromise thunk may use the AbortSignal parameter
export const tryPromiseSignal = Effect.tryPromise((signal) => fetch(url, { signal }))

// MUST TRIGGER: async arrow thunk
export const tryPromiseAsync = Effect.tryPromise(async () => {
  const response = await fetch(url)
  return response.status
})

// MUST TRIGGER: function expression thunk
export const tryFunctionExpression = Effect.try(function() {
  return parseDocument(text)
})

// MUST TRIGGER: the thunk is passed by reference instead of inline
const parseText = () => parseDocument(text)
export const tryReference = Effect.try(parseText)

// MUST TRIGGER: the thunk is the result of a call
declare function makeThunk(): () => number
export const tryCallResult = Effect.try(makeThunk())

// MUST TRIGGER: explicit type arguments do not change the form
export const tryExplicitTypeArguments = Effect.try<number>(() => 1)

// MUST TRIGGER: inside Effect.gen
export const inGen = Effect.gen(function*() {
  const document = yield* Effect.try(() => parseDocument(text))
  return document.title
})

// MUST TRIGGER: inside Effect.fn
export const inFn = Effect.fn("inFn")(function*(input: string) {
  return yield* Effect.tryPromise(() => fetch(input))
})

// MUST TRIGGER: first argument of pipe(...)
export const inPipe = pipe(
  Effect.try(() => parseDocument(text)),
  Effect.map((document) => document.title)
)

// MUST TRIGGER: the rule checks the call only; a later mapError does not suppress it
export const followedByMapError = Effect.try(() => parseDocument(text)).pipe(
  Effect.mapError((cause) => new ParseError({ cause }))
)

// MUST TRIGGER: namespace import of effect/Effect under another name
export const namespaceAlias = Eff.tryPromise(() => fetch(url))

// MUST TRIGGER: renamed named imports (`try` is a reserved word, so it must be renamed)
export const namedTry = attempt(() => parseDocument(text))
export const namedTryPromise = attemptPromise(() => fetch(url))

// MUST TRIGGER: const alias of the API
const effectTry = Effect.try
export const constAlias = effectTry(() => parseDocument(text))

// MUST NOT TRIGGER: object form with a domain error in catch
export const tryObject = Effect.try({
  try: () => JSON.parse(text),
  catch: (cause) => new ParseError({ cause })
})

// MUST NOT TRIGGER: tryPromise object form with a domain error in catch
export const tryPromiseObject = Effect.tryPromise({
  try: (signal) => fetch(url, { signal }),
  catch: (cause) => new FetchError({ cause })
})

// MUST NOT TRIGGER: object form with an untyped catch (that is unknownInEffectCatch's job)
export const tryObjectUnknownCatch = Effect.try({
  try: () => JSON.parse(text),
  catch: (cause) => cause
})

// MUST NOT TRIGGER: object form with shorthand properties
const catchParse = (cause: unknown) => new ParseError({ cause })
const tryParse = () => parseDocument(text)
export const tryObjectShorthand = Effect.try({ try: tryParse, catch: catchParse })

// MUST NOT TRIGGER: the options object is passed by reference
const fetchOptions = {
  try: () => fetch(url),
  catch: (cause: unknown) => new FetchError({ cause })
}
export const tryPromiseOptionsReference = Effect.tryPromise(fetchOptions)

// MUST NOT TRIGGER: the argument is `any`, so the form cannot be determined
// (v4 resolves the short-form overload here, v3 the object form)
declare const untyped: any
export const tryAny = Effect.try(untyped)

// MUST NOT TRIGGER: tryMap and tryMapPromise always require catch
export const tryMap = Effect.succeed(text).pipe(
  Effect.tryMap({ try: parseDocument, catch: (cause) => new ParseError({ cause }) })
)
export const tryMapPromise = Effect.succeed(url).pipe(
  Effect.tryMapPromise({ try: (input) => fetch(input), catch: (cause) => new FetchError({ cause }) })
)

// MUST NOT TRIGGER: Effect.sync and Effect.promise have no error channel to type
export const sync = Effect.sync(() => parseDocument(text))
export const promise = Effect.promise(() => fetch(url))

// MUST NOT TRIGGER: Either.try is a different module (out of scope)
export const eitherTry = Either.try(() => parseDocument(text))

// MUST NOT TRIGGER: Effect.try is referenced, not called
export const reference = Effect.try

// MUST TRIGGER: Effect.try is a pipe step applied to the thunk; the piping flow
// sees the same short form, and the fix moves the thunk into the object form
// (v3 lists the short form last, so this compiles)
export const asCallback = pipe(() => parseDocument(text), Effect.try)

// MUST NOT TRIGGER: a local object whose methods are named like the Effect APIs
const Local = {
  try: <A>(thunk: () => A) => thunk(),
  tryPromise: <A>(thunk: () => Promise<A>) => thunk()
}
export const localTry = Local.try(() => parseDocument(text))
export const localTryPromise = Local.tryPromise(() => fetch(url))
