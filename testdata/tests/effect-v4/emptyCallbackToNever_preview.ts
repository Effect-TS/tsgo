import { Effect } from "effect"

export const daemon = Effect.log("worker started").pipe(
  Effect.andThen(Effect.callback<never>(() => {}))
)
