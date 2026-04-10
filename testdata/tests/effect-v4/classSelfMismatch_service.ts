import { Context, Effect } from "effect"

// @effect-expect-leaking FileSystem Cache
export class CorrectName extends Context.Service<CorrectName, {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>()("CorrectName") {}

// @effect-expect-leaking FileSystem Cache
export class WrongName extends Context.Service<CorrectName, {
  writeCache: () => Effect.Effect<void, never, FileSystem | Cache>
  readCache: Effect.Effect<void, never, FileSystem | Cache>
}>()("WrongName") {}
