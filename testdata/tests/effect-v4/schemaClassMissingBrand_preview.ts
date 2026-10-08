// @effect-diagnostics *:off
// @effect-diagnostics schemaClassMissingBrand:warning
import { Schema } from "effect"

export class User extends Schema.Class<User>("User")({ name: Schema.String }) {}

export class StructuralUser extends Schema.Class<StructuralUser, {}>("StructuralUser")({ name: Schema.String }) {}
