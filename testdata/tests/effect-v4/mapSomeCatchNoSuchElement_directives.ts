// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics mapSomeCatchNoSuchElement:warning
import { Cause, Effect } from "effect"

declare const lookup: Effect.Effect<string, Cause.NoSuchElementError>

// MUST NOT TRIGGER: a next-line directive turns the rule off for one pair
// @effect-diagnostics-next-line mapSomeCatchNoSuchElement:off
export const disabled = lookup.pipe(Effect.asSome, Effect.catchTag("NoSuchElementError", () => Effect.succeedNone))

// MUST TRIGGER: the next pair is still reported
export const reported = lookup.pipe(Effect.asSome, Effect.catchTag("NoSuchElementError", () => Effect.succeedNone))

// MUST TRIGGER (as error): a next-line directive overrides the severity
// @effect-diagnostics-next-line mapSomeCatchNoSuchElement:error
export const asError = lookup.pipe(Effect.asSome, Effect.catchTag("NoSuchElementError", () => Effect.succeedNone))

// MUST TRIGGER (as suggestion): a next-line directive overrides the severity
// @effect-diagnostics-next-line mapSomeCatchNoSuchElement:suggestion
export const asSuggestion = lookup.pipe(Effect.asSome, Effect.catchTag("NoSuchElementError", () => Effect.succeedNone))

// @effect-diagnostics mapSomeCatchNoSuchElement:off

// MUST NOT TRIGGER: a file directive turns the rule off for the rest of the file
export const afterFileDirective = lookup.pipe(Effect.asSome, Effect.catchTag("NoSuchElementError", () => Effect.succeedNone))
