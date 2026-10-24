// @effect-diagnostics *:off
// @effect-diagnostics errorPayloadShadowsName:warning
import { Data, Schema } from "effect"

export class ToolNotFound extends Data.TaggedError("ToolNotFound")<{ readonly name: string }> {}
export class PlainError extends Data.Error<{ name: string }> {}
export class ParenthesizedDataError extends (Data).Error<{ name: string }> {}
export class NonNullDataError extends Data!.Error<{ name: string }> {}
export class TaggedSchemaError extends Schema.TaggedError<TaggedSchemaError>()("TaggedSchemaError", { name: Schema.String }) {}
export class PlainSchemaError extends Schema.Error<PlainSchemaError>("PlainSchemaError")({ name: Schema.String }) {}
export class QuotedSchemaError extends Schema.TaggedError<QuotedSchemaError>()("QuotedSchemaError", { "name": Schema.String }) {}
const name = Schema.String
export class ShorthandSchemaError extends Schema.TaggedError<ShorthandSchemaError>()("ShorthandSchemaError", { name }) {}
export class DifferentDataName extends Data.TaggedError("A")<{ name: "B" }> {}
export class DifferentSchemaName extends Schema.TaggedError<DifferentSchemaName>()("DifferentSchemaName", { name: Schema.Literal("Other") }) {}
export const E = class E extends Data.TaggedError("E")<{ name: string }> {}
export class QuotedDataError extends Data.Error<{ "name": string }> {}
export const AnonymousError = class extends Data.Error<{ name: string }> {}
export class ComputedDataError extends Data.TaggedError("ComputedDataError")<{ ["name"]: string }> {}
export class ComputedSchemaError extends Schema.TaggedError<ComputedSchemaError>()("ComputedSchemaError", { ["name"]: Schema.String }) {}
export class TemplateSchemaError extends Schema.TaggedError<TemplateSchemaError>()("TemplateSchemaError", { [`name`]: Schema.String }) {}
export class A extends Schema.TaggedError<A>("app/A")("A", { name: Schema.Literal("A") }) {}
const Fake = { Literal: (_: string) => Schema.String }
export class F extends Schema.TaggedError<F>()("F", { name: Fake.Literal("F") }) {}

export class SafeDataError extends Data.TaggedError("SafeDataError")<{ message: string; cause: unknown; stack: string; toolName: string }> {}
export class SafeSchemaError extends Schema.TaggedError<SafeSchemaError>()("SafeSchemaError", { message: Schema.String, cause: Schema.Unknown, stack: Schema.String, toolName: Schema.String }) {}
export class WorktreeError extends Data.TaggedError("WorktreeError")<{ name: "WorktreeError" }> {}
export class WorktreeSchemaError extends Schema.TaggedError<WorktreeSchemaError>()("WorktreeError", { name: Schema.Literal("WorktreeError") }) {}
export class WorktreeTagSchemaError extends Schema.TaggedError<WorktreeTagSchemaError>()("WorktreeError", { name: Schema.tag("WorktreeError") }) {}
export class WorktreeTagDefaultOmitSchemaError extends Schema.TaggedError<WorktreeTagDefaultOmitSchemaError>()("WorktreeError", { name: Schema.tagDefaultOmit("WorktreeError") }) {}
export class B extends Schema.TaggedError<B>("app/B")("B", { name: Schema.Literal("app/B") }) {}
export class C extends Schema.Error<C>("C")({ name: Schema.Literal("C") }) {}
export class NamedTaggedClass extends Schema.TaggedClass<NamedTaggedClass>()("NamedTaggedClass", { name: Schema.String }) {}
export class NamedClass extends Schema.Class<NamedClass>("NamedClass")({ name: Schema.String }) {}
export class NestedSchemaError extends Schema.TaggedError<NestedSchemaError>()("NestedSchemaError", { meta: Schema.Struct({ name: Schema.String }) }) {}
export class NestedDataError extends Data.TaggedError("NestedDataError")<{ meta: { name: string } }> {}

// Payload references are outside this AST-only rule's scope.
interface P { name: string }
export class ReferencedDataError extends Data.TaggedError("ReferencedDataError")<P> {}
export class ReferencedPlainError extends Data.Error<P> {}
const fields = { name: Schema.String }
export class ReferencedSchemaError extends Schema.TaggedError<ReferencedSchemaError>()("ReferencedSchemaError", fields) {}
export class StructSchemaError extends Schema.TaggedError<StructSchemaError>()("StructSchemaError", Schema.Struct({ name: Schema.String })) {}
export class SpreadSchemaError extends Schema.TaggedError<SpreadSchemaError>()("SpreadSchemaError", { ...fields }) {}
const k = "name"
export class ReferencedKeySchemaError extends Schema.TaggedError<ReferencedKeySchemaError>()("ReferencedKeySchemaError", { [k]: Schema.String }) {}
export class NoFields extends Data.Error {}

const UnrelatedData = {
  TaggedError: (tag: string) => class<Fields> { readonly _tag = tag; declare readonly fields: Fields },
  Error: class<Fields> { declare readonly fields: Fields }
}
export class UnrelatedError extends UnrelatedData.TaggedError("UnrelatedError")<{ name: string }> {}
export function shadowedData() {
  const Data = UnrelatedData
  class ShadowedTaggedError extends Data.TaggedError("ShadowedTaggedError")<{ name: string }> {}
  class ShadowedPlainError extends Data.Error<{ name: string }> {}
  return { ShadowedTaggedError, ShadowedPlainError }
}

// @effect-diagnostics-next-line errorPayloadShadowsName:off
export class SuppressedError extends Data.TaggedError("SuppressedError")<{ name: string }> {}
