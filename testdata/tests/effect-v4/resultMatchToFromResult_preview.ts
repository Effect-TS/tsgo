// @effect-diagnostics *:off
import { Effect, Result } from "effect"

declare const parsePort: (raw: string) => Result.Result<number, Error>

export const port = Result.match(parsePort("8080"), {
  onFailure: (error) => Effect.fail(error),
  onSuccess: (value) => Effect.succeed(value)
})
