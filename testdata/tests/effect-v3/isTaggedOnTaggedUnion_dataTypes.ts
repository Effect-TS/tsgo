// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics isTaggedOnTaggedUnion:warning
import { Either, Exit, Option, Predicate } from "effect"

declare const option: Option.Option<number>
declare const options: ReadonlyArray<Option.Option<number>>
declare const exit: Exit.Exit<number, string>
declare const either: Either.Either<number, string>

export const some = Predicate.isTagged(option, "Some")
export const somes = options.filter(Predicate.isTagged("Some"))
export const failure = Predicate.isTagged(exit, "Failure")
export const right = Predicate.isTagged(either, "Right")
