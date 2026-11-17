// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics promiseAllInEffect:warning
import { Effect } from "effect"

declare const ids: ReadonlyArray<string>
declare const fetchUser: (id: string) => Promise<{ readonly name: string }>

export const users = Effect.tryPromise(() => Promise.all(ids.map((id) => fetchUser(id))))
