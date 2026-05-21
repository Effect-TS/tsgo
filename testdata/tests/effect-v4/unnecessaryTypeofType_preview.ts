// @effect-diagnostics *:off
// @effect-diagnostics unnecessaryTypeofType:warning
import { Schema } from "effect"

export namespace UsersRepo {
  export const User = Schema.Struct({ id: Schema.Number })
  export type User = typeof User.Type
}

export const preview: typeof UsersRepo.User.Type = { id: 1 }
