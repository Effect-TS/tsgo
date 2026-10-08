// @effect-diagnostics *:off
// @effect-diagnostics catchTagToCatchReason:suggestion
import { Effect as Fx, Predicate } from "effect"

class ReasonA {
  readonly _tag = "ReasonA"
}
class ReasonB {
  readonly _tag = "ReasonB"
  constructor(readonly detail: string) {}
}
class OuterError {
  readonly _tag = "OuterError"
  constructor(readonly reason: ReasonA | ReasonB) {}
}
class OtherError {
  readonly _tag = "OtherError"
}

declare const program: Fx.Effect<number, OuterError | OtherError>

export const direct = program.pipe(
  Fx.catchTag("OuterError", (error) => Predicate.isTagged(error.reason, "ReasonA")
    ? Fx.succeed(1)
    : Fx.fail(error))
)

export const curriedWithReasonUse = program.pipe(
  Fx.catchTag("OuterError", (error) => Predicate.isTagged("ReasonB")(error.reason)
    ? Fx.succeed(error.reason.detail.length)
    : Fx.fail(error))
)

export const mixed = program.pipe(
  Fx.catchTag("OuterError", (error) => {
    if (Predicate.isTagged(error.reason, "ReasonA")) return Fx.succeed(1)
    if (error.reason._tag === "ReasonB") return Fx.succeed(error.reason.detail.length)
    return Fx.fail(error)
  })
)

const fake = { isTagged: (_value: unknown, _tag: string) => false }
export const lookalike = program.pipe(
  Fx.catchTag("OuterError", (error) => fake.isTagged(error.reason, "ReasonA")
    ? Fx.succeed(1)
    : Fx.fail(error))
)

declare const expectedTag: "ReasonA" | "ReasonB"
export const dynamicTag = program.pipe(
  Fx.catchTag("OuterError", (error) => Predicate.isTagged(expectedTag)(error.reason)
    ? Fx.succeed(1)
    : Fx.fail(error))
)
