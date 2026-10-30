// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics suspendFailToFailSync:warning
import * as Effect from "effect/Effect"

declare const error: Error
declare const defect: unknown
declare const args: [Error]

// MUST TRIGGER: arrow thunk returning Effect.fail
export const failCall = Effect.suspend(() => Effect.fail(error))

// MUST NOT TRIGGER: v4 has no Effect.dieSync to suggest
export const dieCall = Effect.suspend(() => Effect.die(defect))

// MUST TRIGGER: function expression thunk
export const functionThunk = Effect.suspend(function() {
  return Effect.fail(error)
})

// MUST NOT TRIGGER: async thunk
export const asyncThunk = Effect.suspend((async () => Effect.fail(error)) as any)

// MUST NOT TRIGGER: generator thunk
export const generatorThunk = Effect.suspend(function*() {
  return Effect.fail(error)
} as any)

// MUST NOT TRIGGER: generic thunk
export const typeParameterThunk = Effect.suspend(<T>() => Effect.fail(error))

// MUST NOT TRIGGER: spread argument to Effect.fail
export const spreadArgument = Effect.suspend(() => Effect.fail(...args))

// MUST NOT TRIGGER: empty return
export const emptyReturn = Effect.suspend((() => {
  return
}) as any)

// MUST NOT TRIGGER: disabled by a next-line directive
// @effect-diagnostics-next-line suspendFailToFailSync:off
export const disabledNextLine = Effect.suspend(() => Effect.fail(error))
