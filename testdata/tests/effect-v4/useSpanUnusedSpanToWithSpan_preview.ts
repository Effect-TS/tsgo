// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics useSpanUnusedSpanToWithSpan:warning
import { Effect } from "effect"

declare const fetchUser: (id: string) => Effect.Effect<{ name: string }>

export const getUser = (id: string) => Effect.useSpan("fetch-user", (_span) => fetchUser(id))
