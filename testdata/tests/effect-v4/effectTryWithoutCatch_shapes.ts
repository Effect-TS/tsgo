// @effect-diagnostics *:off
// @effect-diagnostics effectTryWithoutCatch:warning
import { type Cause, Data, Effect } from "effect"

class ParseError extends Data.TaggedError("ParseError")<{ readonly cause: unknown }> {}

declare const url: string

// MUST TRIGGER: subject of a data-first call
export const dataFirst = Effect.map(Effect.try(() => 1), (n) => n + 1)

// MUST TRIGGER: subject of a curried pipeable call
export const curried = Effect.map((n: number) => n + 1)(Effect.try(() => 1))

// MUST TRIGGER: inside an Effect.fnUntraced body
export const inFnUntraced = Effect.fnUntraced(function*() {
  return yield* Effect.try(() => 1)
})

// MUST TRIGGER: inside a non-generator Effect.fn body
export const inPlainFn = Effect.fn("inPlainFn")(() => Effect.try(() => 1))

// MUST TRIGGER: parenthesised thunk
export const parenthesisedThunk = Effect.try((() => 1))

// MUST TRIGGER: the fix keeps the comment before the thunk
export const leadingComment = Effect.try(/* thunk */ () => 1)

// MUST TRIGGER: one explicit type argument on tryPromise
export const tryPromiseTypeArgument = Effect.tryPromise<Response>(() => fetch(url))

// MUST TRIGGER, without a fix: the annotation narrows the alias to the short form,
// so the object form would not type-check
const narrowedTry: <A>(thunk: () => A) => Effect.Effect<A, Cause.UnknownError> = Effect.try
export const narrowedAlias = narrowedTry(() => 1)

// MUST TRIGGER, without a fix: a const chain through the narrowed alias
const narrowedAgain = narrowedTry
export const narrowedChain = narrowedAgain(() => 1)

// MUST TRIGGER: an alias annotated with the API's own type keeps the object form
const typedTry: typeof Effect.try = Effect.try
export const typedAlias = typedTry(() => 1)

// MUST NOT TRIGGER: parenthesised object form
export const parenthesisedObject = Effect.try(({ try: () => 1, catch: (cause) => new ParseError({ cause }) }))

// MUST NOT TRIGGER: a spread argument has no single thunk to rewrite
declare const args: [() => number]
export const spread = Effect.try(...args)

// MUST TRIGGER: a data-first value argument
export const dataFirstArgument = Effect.andThen(Effect.void, Effect.try(() => 1))

// MUST TRIGGER: the fix keeps a multi-line async thunk and its comments
export const multiline = Effect.tryPromise(async (signal) => {
  // keep this comment
  const response = await fetch(url, { signal })
  return response.status
})

// MUST TRIGGER, as an error: a next-line directive overrides the severity
// @effect-diagnostics-next-line effectTryWithoutCatch:error
export const nextLineError = Effect.try(() => 1)

// MUST NOT TRIGGER: a next-line directive disables the rule for one line
// @effect-diagnostics-next-line effectTryWithoutCatch:off
export const nextLineOff = Effect.try(() => 1)
