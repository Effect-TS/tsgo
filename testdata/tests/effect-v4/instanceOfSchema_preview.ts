// @effect-diagnostics *:off
// @effect-diagnostics instanceOfSchema:warning
import { Schema as S } from "effect"

class User extends S.Class<User>("User")({ name: S.String }) {}
declare const value: unknown
export const preview = value instanceof User
