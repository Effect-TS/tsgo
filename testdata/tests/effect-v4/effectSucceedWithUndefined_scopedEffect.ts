// @effect-diagnostics *:off
// @effect-diagnostics effectSucceedWithUndefined:suggestion
import { Effect } from "effect"
import { succeed } from "effect/Effect"

// MUST TRIGGER: the Effect import is in scope -> Effect.undefined
export const topLevel = succeed(undefined)

// MUST TRIGGER, diagnostic only: NO replacement quickfix, the parameter `Effect` shadows the Effect import
export function shadowedByParameter(Effect: number) {
  return [Effect, succeed(undefined)] as const
}

// MUST TRIGGER, diagnostic only: NO replacement quickfix, the local `Effect` shadows the Effect import
export const shadowedByLocal = () => {
  const Effect = { undefined: 1 }
  return [Effect.undefined, succeed(undefined)] as const
}

// MUST TRIGGER: the shadowing local is in a sibling scope, Effect.undefined resolves to the import here
export const siblingScope = () => succeed(undefined)

void Effect
