import { Effect, Exit, Option, Result, UndefinedOr } from "effect"

declare const option: Option.Option<number>
declare const result: Result.Result<number, string>
declare const maybe: number | undefined
declare const exit: Exit.Exit<number, string>

export const some = Effect.succeed(((Option.some(1))))
export const none = Effect.succeed(((Option.none())))

export const optionGetOrElse = Option.getOrElse(option, () => "none")
export const resultMatch = Result.match(result, {
  onFailure: (error) => error.length,
  onSuccess: (value) => value
})
export const undefinedOrMap = UndefinedOr.map(maybe, (value) => String(value))
export const exitMatch = Exit.match(exit, {
  onFailure: (cause) => String(cause),
  onSuccess: (value) => value
})
