import { Effect } from "effect"

interface User {
  readonly id: string
  readonly name: string
}

declare const findUser: (id: string) => Effect.Effect<User>
declare const auditUser: (user: User) => Effect.Effect<void>

export const loadUser = (id: string) => findUser(id).pipe(
  Effect.flatMap((user) => auditUser(user).pipe(Effect.as(user)))
)
