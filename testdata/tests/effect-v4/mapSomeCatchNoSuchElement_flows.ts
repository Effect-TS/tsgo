// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics mapSomeCatchNoSuchElement:warning
import { Cause, Effect, Option, pipe } from "effect"

declare const lookup: Effect.Effect<string, Cause.NoSuchElementError>
declare const lookupOrTimeout: Effect.Effect<string, Cause.NoSuchElementError | NotFound | Cause.TimeoutError>
declare const tagUnion: Effect.Effect<string, Cause.NoSuchElementError | { readonly _tag: "A" | "NoSuchElementError" }>

interface NotFound {
  readonly _tag: "NotFound"
}

// MUST TRIGGER: a data-first pair passed as a later argument of another call
export const laterArgument = Effect.andThen(
  Effect.log("start"),
  Effect.catchTag(Effect.asSome(lookup), "NoSuchElementError", () => Effect.succeedNone)
)
export const laterArgumentZip = Effect.zip(
  Effect.succeed(1),
  Effect.catchTag(Effect.asSome(lookup), "NoSuchElementError", () => Effect.succeed(Option.none()))
)

// MUST TRIGGER: a data-first pair whose result is piped into a kept data-first step
export const dataFirstKeptOuter = Effect.map(
  Effect.catchTag(Effect.asSome(/* subject */ lookup), "NoSuchElementError", () => Effect.succeedNone),
  /* kept */ Option.isSome
)

// MUST TRIGGER: several unrelated tagged errors stay in the channel
export const severalOtherErrors = lookupOrTimeout.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST NOT TRIGGER: a member whose tag union also contains "NoSuchElementError"
export const tagUnionChannel = tagUnion.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST TRIGGER: comments in a chained pipe that the fix empties
export const commentsChainedPipe = lookup // subject
  .pipe(/* wrap */ Effect.asSome)
  .pipe(/* catch */ Effect.catchTag("NoSuchElementError", () => Effect.succeedNone))

// MUST TRIGGER: comments in Effect.fn steps and in its body are kept
export const commentsEffectFn = Effect.fn(function*() {
  // body
  return yield* lookup
}, /* wrap */ Effect.asSome, Effect.catchTag("NoSuchElementError", () => Effect.succeedNone) /* catch */)

// MUST TRIGGER: comments on steps after the pair stay with them
export const commentsAfterPair = pipe(
  lookup,
  Effect.asSome,
  /* catch */ Effect.catchTag("NoSuchElementError", () => Effect.succeedNone), // trailing
  /* kept */ Effect.map(Option.isSome)
)

// MUST TRIGGER: comment-like text in literals is not a comment; only the real one moves
export const commentLikeLiterals = lookup.pipe(
  Effect.tap(() => Effect.log("/* text */")),
  Effect.tap(() => Effect.log(`// template ${1} /* tail */`)),
  Effect.tap(() => Effect.log(/\/* regex *\//.source)),
  Effect.asSome,
  /* real */ Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
)

// MUST TRIGGER: comments before closing tokens of kept blocks and of the pipe
export const commentsBeforeClosingTokens = lookup.pipe(
  Effect.tap((value) => {
    return Effect.log(value)
    // end of kept block
  }),
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
  // end of pipe
)
