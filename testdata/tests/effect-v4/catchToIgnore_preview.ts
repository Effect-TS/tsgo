import { Effect } from "effect"

declare const program: Effect.Effect<void, string, never>

export const recovered = program.pipe(
  Effect.catch(() => Effect.void)
)
