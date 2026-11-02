// @effect-diagnostics *:off
// @effect-diagnostics yieldInCallArgument:warning
import { Effect } from "effect"

declare const load: Effect.Effect<number>
declare function transform(n: number): string

export const enabledByDirective = Effect.gen(function*() {
  // MUST TRIGGER: the file directive enables the rule as a warning
  return transform(yield* load)
})

export const nextLineOff = Effect.gen(function*() {
  // MUST NOT TRIGGER: the next-line directive turns the rule off for this line only
  // @effect-diagnostics-next-line yieldInCallArgument:off
  const a = transform(yield* load)
  // MUST TRIGGER: the next-line directive covered only the previous line
  const b = transform(yield* load)
  return a + b
})

// @effect-diagnostics yieldInCallArgument:error
export const errorSeverity = Effect.gen(function*() {
  // MUST TRIGGER (error): the section directive raises the severity to error
  return transform(yield* load)
})

// @effect-diagnostics yieldInCallArgument:off
export const sectionOff = Effect.gen(function*() {
  // MUST NOT TRIGGER: the section directive turns the rule off for the rest of the file
  return transform(yield* load)
})
