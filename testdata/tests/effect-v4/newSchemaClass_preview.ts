// @effect-diagnostics *:off
// @effect-diagnostics newSchemaClass:suggestion
import { Schema } from "effect"

class User extends Schema.Class<User>("User")({ name: Schema.String }) {}

export const preview = new User({ name: "John" })
