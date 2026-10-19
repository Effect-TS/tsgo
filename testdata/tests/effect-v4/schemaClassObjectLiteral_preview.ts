// @effect-diagnostics *:off
// @effect-diagnostics schemaClassObjectLiteral:warning
import { Schema } from "effect"

export class User extends Schema.Class<User, { readonly brand: unique symbol }>("User")({ name: Schema.String }) {}
export const user = User.make({ name: "a" })
export const copy: User = { ...user }
export const renamed = User.make({ ...user, name: "b" })
