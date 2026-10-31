// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics mapSomeCatchNoSuchElement:warning
import { Cause, Effect, Option, pipe } from "effect"
import { succeedNone } from "effect/Effect"
import { none as optionNone } from "effect/Option"

declare const lookup: Effect.Effect<string, Cause.NoSuchElementError>

// MUST TRIGGER: handler returns Effect.succeed(Option.none())
export const succeedOptionNone = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeed(Option.none()))
)

// MUST TRIGGER: handler returns a named import of Effect.succeedNone
export const namedSucceedNone = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => succeedNone)
)

// MUST TRIGGER: handler uses a named import of Option.none
export const namedOptionNone = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeed(optionNone()))
)

// MUST TRIGGER: an unused, plain error parameter does not observe the error
export const unusedParameter = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", (_error) => Effect.succeedNone)
)

// MUST TRIGGER: single-return block body
export const blockBody = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => {
    return Effect.succeedNone
  })
)

// MUST TRIGGER: the complete returned flow constructs a successful None
export const functionPipe = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => pipe(Option.none(), Effect.succeed))
)
export const methodPipe = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Option.none().pipe(Effect.succeed))
)

// MUST NOT TRIGGER: Effect.die turns the error into a defect instead of None
export const die = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", Effect.die)
)

// MUST NOT TRIGGER: the handler recovers with a Some value, not None
export const recoversWithSome = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeed(Option.some("fallback")))
)

// MUST NOT TRIGGER: the handler recovers with void, not None
export const recoversWithVoid = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.void)
)

// MUST NOT TRIGGER: the handler fails with another error
export const refails = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.fail("missing" as const))
)

// MUST NOT TRIGGER: the handler observes the error
export const logsError = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", (error) => Effect.log(error.message).pipe(Effect.as(Option.none())))
)

// MUST NOT TRIGGER: the handler runs an extra effect before returning None
export const extraEffect = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.log("missing").pipe(Effect.andThen(Effect.succeedNone)))
)

// MUST NOT TRIGGER: additional transformations on the None effect could be observable
export const delayedNone = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeedNone.pipe(Effect.delay(1)))
)
export const transformedNoneInput = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () =>
    pipe(Option.none(), Option.orElse(() => Option.some("fallback")), Effect.succeed))
)

// MUST NOT TRIGGER: the handler is a reference, not an inline arrow returning None
const recover = () => Effect.succeedNone
export const handlerReference = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", recover)
)

// MUST NOT TRIGGER: an Option.none() with explicit type arguments
export const typedNone = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => Effect.succeed(Option.none<number>()))
)

const localNone = Effect.sync(() => Option.none<never>())

// MUST NOT TRIGGER: the handler returns a local effect that is not Effect.succeedNone
export const localNoneEffect = lookup.pipe(
  Effect.asSome,
  Effect.catchTag("NoSuchElementError", () => localNone)
)
