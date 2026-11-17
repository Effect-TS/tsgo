import { Effect } from "effect"

// The quick fix rewrites this to Effect.zipWith(getUser, getConfig, (user, config) => `${user.name} (${config.locale})`).
// Notes:
// - The rule stays silent when the inner effect uses the outer result (e.g. `loadProfile(user.id)`),
//   because the second effect depends on the first and zipWith does not apply.
// - When the combiner is exactly `[user, config]`, the fix suggests Effect.zip(getUser, getConfig).
// - The rule stays silent when the mapper returns the outer value unchanged, because flatMapToTap covers that.

declare const getUser: Effect.Effect<{ readonly name: string }>
declare const getConfig: Effect.Effect<{ readonly locale: string }>

export const greeting = Effect.flatMap(getUser, (user) =>
  Effect.map(getConfig, (config) => `${user.name} (${config.locale})`))
