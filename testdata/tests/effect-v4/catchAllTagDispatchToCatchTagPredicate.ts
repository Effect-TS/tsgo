// @effect-diagnostics *:off
// @effect-diagnostics catchAllTagDispatchToCatchTag:suggestion
// @effect-diagnostics catchConditionalRefailToCatchIf:suggestion
import { Effect as Fx, Predicate } from "effect"

class NotFoundError {
  readonly _tag = "NotFoundError"
  constructor(readonly id: string) {}
}
class TimeoutError {
  readonly _tag = "TimeoutError"
  constructor(readonly ms: number) {}
}
class AuthError {
  readonly _tag = "AuthError"
}

declare const program: Fx.Effect<string, NotFoundError | TimeoutError | AuthError>

export const direct = program.pipe(
  Fx.catch((error) => Predicate.isTagged(error, "NotFoundError")
    ? Fx.succeed(error.id)
    : Fx.fail(error))
)

export const curried = Fx.catch(program, (error) =>
  Predicate.isTagged("TimeoutError")(error)
    ? Fx.succeed(String(error.ms))
    : Fx.fail(error)
)

export const mixed = program.pipe(
  Fx.catch((error) => {
    if (error._tag === "NotFoundError") return Fx.succeed(error.id)
    if (Predicate.isTagged("TimeoutError")(error)) return Fx.succeed(String(error.ms))
    return Fx.fail(error)
  })
)
