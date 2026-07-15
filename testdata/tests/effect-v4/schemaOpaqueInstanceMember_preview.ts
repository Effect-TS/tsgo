// @effect-diagnostics *:off
// @effect-diagnostics schemaOpaqueInstanceMember:warning
import { Schema } from "effect"

class User extends Schema.Opaque<User>()(Schema.Struct({ name: Schema.String })) {
  displayName = "User"
  greet() {
    return `Hello, ${this.displayName}`
  }
}
