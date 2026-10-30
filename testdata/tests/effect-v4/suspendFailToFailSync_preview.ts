// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics suspendFailToFailSync:warning
import { Data, Effect } from "effect"

class ValidationError extends Data.TaggedError("ValidationError")<{
  readonly input: string
}> {}

export const rejectInput = (input: string) => Effect.suspend(() => Effect.fail(new ValidationError({ input })))
