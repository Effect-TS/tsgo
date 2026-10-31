// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics mapSomeCatchNoSuchElement:warning
import { Effect, Option } from "effect"

const users = new Map<string, { readonly name: string }>()

const findUser = (id: string) => Effect.fromNullishOr(users.get(id))

export const lookupUser = (id: string) =>
  findUser(id).pipe(
    Effect.map(Option.some),
    Effect.catchTag("NoSuchElementError", () => Effect.succeed(Option.none()))
  )
