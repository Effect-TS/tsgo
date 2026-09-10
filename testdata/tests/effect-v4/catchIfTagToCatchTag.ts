// @effect-diagnostics *:off
// @effect-diagnostics catchIfTagToCatchTag:suggestion
import { Effect as Fx, pipe } from "effect"

class NotFoundError {
  readonly _tag = "NotFoundError"
  constructor(readonly id: string) {}
}

class TimeoutError {
  readonly _tag = "TimeoutError"
}

declare const program: Fx.Effect<string, NotFoundError | TimeoutError>
declare const handler: (error: NotFoundError) => Fx.Effect<string>
declare const broadHandler: (error: NotFoundError | TimeoutError) => Fx.Effect<string>

// Should trigger in pipe method form.
export const methodPipe = program.pipe(
  Fx.catchIf((error) => error._tag === "NotFoundError", handler)
)

// Should trigger with reversed equality in pipe(...) form.
export const functionPipe = pipe(
  program,
  Fx.catchIf((error) => "TimeoutError" == error._tag, () => Fx.succeed("timeout"))
)

// Should trigger in data-first form and preserve a block-bodied handler.
export const dataFirst = Fx.catchIf(
  program,
  (error) => {
    return error._tag === "NotFoundError"
  },
  (error) => {
    return Fx.succeed(error.id)
  }
)

// Should NOT trigger: an inverted check cannot be represented by catchTag.
export const inverted = program.pipe(
  Fx.catchIf((error) => error._tag !== "NotFoundError", broadHandler)
)

// Should NOT trigger: the predicate includes an additional condition.
export const compound = program.pipe(
  Fx.catchIf((error) => error._tag === "NotFoundError" && error.id === "special", broadHandler)
)

// Should NOT trigger: the compared literal is not in the error union.
export const impossible = program.pipe(
  Fx.catchIf((error) => error._tag === ("OtherError" as string), broadHandler)
)

// Should NOT trigger: a nested tag is not the error's own tag.
declare const nested: Fx.Effect<string, {
  readonly _tag: "OuterError"
  readonly reason: NotFoundError | TimeoutError
}>

export const nestedTag = nested.pipe(
  Fx.catchIf((error) => error.reason._tag === "NotFoundError", () => Fx.succeed("missing"))
)

// Should NOT trigger: lookalike APIs are ignored.
declare const fake: {
  readonly catchIf: (
    predicate: (error: NotFoundError | TimeoutError) => boolean,
    recover: (error: NotFoundError | TimeoutError) => Fx.Effect<string>
  ) => (self: typeof program) => Fx.Effect<string>
}
export const lookalike = program.pipe(
  fake.catchIf((error) => error._tag === "NotFoundError", broadHandler)
)
