// @effect-diagnostics *:off
// @effect-diagnostics promiseInEffectSuccess:warning

import { Effect } from "effect"

declare const save: (value: number) => Promise<void>

export const preview = Effect.succeed(1).pipe(Effect.map(save))
