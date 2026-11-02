import { match } from "effect/Effect"
import type { Effect } from "effect/Effect"

declare const saveUser: Effect<void, Error>
const isSuccess = 1

// MUST TRIGGER: reported, but nothing reaches Effect.isSuccess without shadowing -> no quick fix
export const saved = match(saveUser, { onFailure: () => false, onSuccess: () => true })
export const flag = isSuccess
