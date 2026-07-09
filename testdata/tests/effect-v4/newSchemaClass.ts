// @effect-diagnostics newSchemaClass:suggestion
import { Effect, Schema } from "effect"

class User extends Schema.Class<User>("User")({
  name: Schema.String
}) {}

const PlainSchema = Schema.Struct({
  name: Schema.String
})

export const newUser = new User({ name: "John" })

export const newInsideEffect = Effect.gen(function*() {
  return new User({ name: "Jane" })
})

export const nestedFunction = Effect.gen(function*() {
  const makeUser = () => new User({ name: "Nested" })
  return makeUser
})

export const plainSchemaInGen = Effect.gen(function*() {
  const user = PlainSchema.make({ name: "Struct" })
  return user
})

class RegularUser {
  constructor(readonly name: string) {}
}

export const regular = new RegularUser("Regular")
