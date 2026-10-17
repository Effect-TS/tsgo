// @effect-diagnostics *:off
// @effect-diagnostics catchIfTagToCatchTag:suggestion
import { Effect as Fx, Predicate } from "effect"
import { isTagged as tagged } from "effect/Predicate"

class NotFoundError {
  readonly _tag = "NotFoundError"
  constructor(readonly id: string) {}
}
class TimeoutError {
  readonly _tag = "TimeoutError"
}

declare const program: Fx.Effect<string, NotFoundError | TimeoutError>

export const direct = program.pipe(
  Fx.catchIf((error) => Predicate.isTagged(error, "NotFoundError"),
    (error) => Fx.succeed(error.id))
)

export const curried = Fx.catchIf(
  program,
  (error) => tagged("TimeoutError")(error),
  () => Fx.succeed("timeout")
)

const tagAlias = tagged
export const aliased = program.pipe(
  Fx.catchIf((error) => tagAlias(error, "NotFoundError"), () => Fx.succeed("missing"))
)

const fake = { isTagged: (_value: unknown, _tag: string) => false }
export const lookalike = program.pipe(
  Fx.catchIf((error) => fake.isTagged(error, "NotFoundError"), () => Fx.succeed("fake"))
)

declare const expectedTag: "NotFoundError" | "TimeoutError"
export const dynamicTag = program.pipe(
  Fx.catchIf((error) => Predicate.isTagged(error, expectedTag), () => Fx.succeed("dynamic"))
)

declare const nested: Fx.Effect<string, {
  readonly _tag: "OuterError"
  readonly reason: NotFoundError | TimeoutError
}>
export const nestedSubject = nested.pipe(
  Fx.catchIf((error) => tagged("NotFoundError")(error.reason), () => Fx.succeed("nested"))
)
