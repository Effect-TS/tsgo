import { Effect } from "effect"

declare const getUser: Effect.Effect<{ readonly id: string }>
declare const getOrders: Effect.Effect<ReadonlyArray<string>>

export const program = Effect.flatMap(getUser, (user) =>
  Effect.map(getOrders, (orders) => [user, orders] as const))
