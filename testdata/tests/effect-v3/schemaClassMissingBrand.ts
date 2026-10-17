// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics schemaClassMissingBrand:warning
import { Context, Schema } from "effect"

export class User extends Schema.Class<User>("User")({ name: Schema.String }) {}
export class TaggedUser extends Schema.TaggedClass<TaggedUser>()("TaggedUser", { name: Schema.String }) {}
export class Failure extends Schema.TaggedError<Failure>()("Failure", { message: Schema.String }) {}
export class Request extends Schema.TaggedRequest<Request>()("Request", {
  payload: { name: Schema.String },
  success: Schema.String,
  failure: Schema.Never
}) {}
export class Service extends Context.Tag("Service")<Service, { readonly value: string }>() {}
