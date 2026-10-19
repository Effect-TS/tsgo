// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics schemaClassObjectLiteral:warning
import { Ref, Schema } from "effect"

export class User extends Schema.Class<User>("User")({ name: Schema.String }) {}
export const p = new User({ name: "a" })
export const u: User = { name: "a" }
export const a: User = { ...p }

export function rename(): User {
  return { ...p, name: "b" }
}

declare function acceptUser(value: User): void
acceptUser({ ...p })

declare const ref: Ref.Ref<User>
export const update = Ref.update(ref, (u) => ({ ...u, name: "c" }))

export class TaggedUser extends Schema.TaggedClass<TaggedUser>()("TaggedUser", { name: Schema.String }) {}
export const tagged = new TaggedUser({ name: "a" })
export const taggedCopy: TaggedUser = { ...tagged }

export class Failure extends Schema.TaggedError<Failure>()("Failure", { message: Schema.String }) {}
export const failure = new Failure({ message: "a" })
export const failureCopy: Failure = { ...failure }

export class Admin extends User {}
export const admin = new Admin({ name: "a" })
export const adminCopy: Admin = { ...admin }

export const nullable: User | null | undefined = { ...p }
export const classUnion: User | TaggedUser = { ...p }

export const made = User.make({ ...p, name: "b" })
export const constructed = new User({ ...p })
export const dto = { ...p }
export const partialUnion: User | Partial<User> = { ...p }

export const list: Array<User> = [p]
export const plainList = list.map((u) => ({ ...u, name: "x" }))

export class Regular {
  name = "a"
}
export const regular: Regular = { name: "a" }

export class A extends Schema.Class<A>("A")({ a: Schema.String }) {}
export const nestedA = new A({ a: "a" })
export class B extends Schema.Class<B>("B")({ inner: A }) {}
export const nestedConstructed = new B({ inner: { ...nestedA } })
