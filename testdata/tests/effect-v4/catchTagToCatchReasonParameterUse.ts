// @effect-diagnostics *:off
// @effect-diagnostics catchTagToCatchReason:suggestion
import { Effect } from "effect"

class ReasonA {
  readonly _tag = "ReasonA"
  constructor(readonly detail: string) {}
}

class ReasonB {
  readonly _tag = "ReasonB"
}

class OuterError {
  readonly _tag = "OuterError"
  constructor(
    readonly reason: ReasonA | ReasonB,
    readonly context: string
  ) {}
}

declare const program: Effect.Effect<number, OuterError>
declare const otherError: OuterError
const _ = "outer"

export const parameterUse = program.pipe(
  Effect.catchTag("OuterError", (error) => {
    switch (error.reason._tag) {
      case "ReasonA":
        return Effect.succeed(`${_}: ${error.context}: ${error.reason.detail}`)
      default:
        return Effect.fail(error)
    }
  })
)

export const reassignedParameter = program.pipe(
  Effect.catchTag("OuterError", (error) => {
    switch (error.reason._tag) {
      case "ReasonA":
        return (error = otherError, Effect.succeed(1))
      default:
        return Effect.fail(error)
    }
  })
)

export const multipleBranches = program.pipe(
  Effect.catchTag("OuterError", (error) => {
    switch (error.reason._tag) {
      case "ReasonA":
        return Effect.succeed(error.reason.detail)
      case "ReasonB":
        return Effect.succeed("reason b")
      default:
        return Effect.fail(error)
    }
  })
)
