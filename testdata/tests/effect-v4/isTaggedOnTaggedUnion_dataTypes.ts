// @effect-diagnostics *:off
// @effect-diagnostics isTaggedOnTaggedUnion:warning
import { Array, Exit, Option, pipe, Predicate, Result } from "effect"

declare const option: Option.Option<number>
declare const options: ReadonlyArray<Option.Option<number>>
declare const exit: Exit.Exit<number, string>
declare const result: Result.Result<number, string>

export const some = Predicate.isTagged(option, "Some")
export const none = !Predicate.isTagged(option, "None")
export const somes = options.filter(Predicate.isTagged("Some"))
export const effectSomes = Array.filter(options, Predicate.isTagged("Some"))
export const failure = Predicate.isTagged(exit, "Failure")
export const success = Predicate.isTagged(result, "Success")
export const pipeable = option.pipe(Predicate.isTagged("Some"))
export const pipedAfterStep = pipe(option, (current) => current, Predicate.isTagged("Some"))
