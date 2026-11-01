// @effect-v3
import { isFailure, match } from "effect/Effect"
import type { Effect } from "effect/Effect"

declare const saveUser: Effect<void, Error>

export const already = isFailure(saveUser)

// MUST TRIGGER: named import of match, isSuccess not imported yet -> isSuccess added to the same import
export const saved = match(saveUser, { onFailure: () => false, onSuccess: () => true })

// MUST TRIGGER: named import of match, isFailure already imported -> reuses isFailure
export const failed = match(saveUser, { onFailure: () => true, onSuccess: () => false })
