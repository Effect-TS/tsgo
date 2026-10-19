// @effect-diagnostics *:off
// @effect-diagnostics schemaClassObjectLiteral:warning
import { Ref, Schema } from "effect"

export class Plain extends Schema.Class<Plain, { readonly brand: unique symbol }>("Plain")({ name: Schema.String }) {}
export const p = Plain.make({ name: "a" })
export const a: Plain = { ...p }

export function rename(): Plain {
  return { ...p, name: "b" }
}

declare function acceptPlain(value: Plain): void
acceptPlain({ ...p })

declare const ref: Ref.Ref<Plain>
export const update = Ref.update(ref, (u) => ({ ...u, name: "c" }))

export class Tagged extends Schema.TaggedClass<Tagged, { readonly brand: unique symbol }>()("Tagged", { name: Schema.String }) {}
export const tagged = Tagged.make({ name: "a" })
export const taggedCopy: Tagged = { ...tagged }

export class Failure extends Schema.Error<Failure, { readonly brand: unique symbol }>("Failure")({ message: Schema.String }) {}
export const failure = Failure.make({ message: "a" })
export const failureCopy: Failure = { ...failure }

export class TaggedFailure extends Schema.TaggedError<TaggedFailure, { readonly brand: unique symbol }>()("TaggedFailure", { message: Schema.String }) {}
export const taggedFailure = TaggedFailure.make({ message: "a" })
export const taggedFailureCopy: TaggedFailure = { ...taggedFailure }

export class Admin extends Plain {}
export const admin = new Admin({ name: "a" })
export const adminCopy: Admin = { ...admin }

export class SuperAdmin extends Admin {}
export const superAdminCopy: SuperAdmin = { ...admin }

export class Unbranded extends Schema.Class<Unbranded>("Unbranded")({ name: Schema.String }) {}
export const literal: Unbranded = { name: "a" }

export const nullable: Plain | null | undefined = { ...p }
export const classUnion: Plain | Tagged = { ...p }
export type PlainAlias = Plain
export const alias: PlainAlias = { ...p }

export const made = Plain.make({ ...p, name: "b" })
export const constructed = new Plain({ ...p })
export const dto = { ...p }
export const partialUnion: Plain | Partial<Plain> = { ...p }
export const nullablePartialUnion: Plain | Partial<Plain> | null | undefined = { ...p }

export class Opaque extends Schema.Opaque<Opaque, { readonly brand: unique symbol }>()(Schema.Struct({ name: Schema.String })) {}
export const opaque = Opaque.make({ name: "a" })
export const opaqueCopy: Opaque = { ...opaque }

export const list: Array<Plain> = [p]
export const plainList = list.map((u) => ({ ...u, name: "x" }))

export class Regular {
  name = "a"
}
export const regular: Regular = { name: "a" }

export class A extends Schema.Class<A, { readonly brand: unique symbol }>("A")({ a: Schema.String }) {}
export const nestedA = A.make({ a: "a" })
export class B extends Schema.Class<B>("B")({ inner: A }) {}
export const S = Schema.Struct({ inner: A })

export const nestedMade = B.make({ inner: { ...nestedA } })
export const nestedConstructed = new B({ inner: { ...nestedA } })
export const nestedStructMade = S.make({ inner: { ...nestedA } })
export const nestedMadeOption = B.makeOption({ inner: { ...nestedA } })
export const nestedMadeEffect = B.makeEffect({ inner: { ...nestedA } })
export const nestedParenthesized = B.make({ inner: ({ ...nestedA }) })

export class X extends Schema.Class<X>("X")({ items: Schema.Array(A) }) {}
export const arrayMade = X.make({ items: [{ ...nestedA }] })

declare function pick(f: () => A): A
export const nestedCallback = B.make({ inner: pick(() => ({ ...nestedA })) })

declare function passThrough(value: A): A
export const nestedCall = B.make({ inner: passThrough({ ...nestedA }) })

export const encodedNested = Schema.encodeSync(B)({ inner: { ...nestedA } })

declare const factory: { make(value: A): A }
export const nonSchemaMade = factory.make({ ...nestedA })
