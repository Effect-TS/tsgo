// @effect-diagnostics *:off
// @effect-diagnostics preferSchemaTaggedError:warning
import { Data, Data as AliasedData, Schema } from "effect"
import * as EffectNamespace from "effect"
import * as NamespacedData from "effect/Data"
import { TaggedError } from "effect/Data"

class WithoutFields extends Data.TaggedError("WithoutFields") {}
export class WithFields extends Data.TaggedError("WithFields")<{ message: string }> {}
export class AliasedDataError extends AliasedData.TaggedError("AliasedDataError") {}
export class NamespaceDataError extends NamespacedData.TaggedError("NamespaceDataError") {}
export class NamedFactoryError extends TaggedError("NamedFactoryError") {}
export class EffectNamespaceError extends EffectNamespace.Data.TaggedError("EffectNamespaceError") {}

const ConstantFactory = Data.TaggedError
export class ConstantFactoryError extends ConstantFactory("ConstantFactoryError") {}

const ChainedFactory = (ConstantFactory)
export class ChainedFactoryError extends ChainedFactory("ChainedFactoryError") {}

export class ExportedError extends Data.TaggedError("ExportedError") {
  static nested() {
    class NestedInMatchedClass extends Data.TaggedError("NestedInMatchedClass") {}
    return NestedInMatchedClass
  }
}

export function nestedDeclaration() {
  class NestedInFunction extends Data.TaggedError("NestedInFunction") {}
  return NestedInFunction
}

export namespace NestedDeclarations {
  export class NestedInNamespace extends Data.TaggedError("NestedInNamespace") {}
}

declare const dynamicTag: string
export class DynamicTagError extends Data.TaggedError(dynamicTag) {}
export class CallbackPayloadError extends Data.TaggedError("CallbackPayloadError")<{
  callback: () => void
}> {}

// @effect-diagnostics-next-line preferSchemaTaggedError:off
export class SuppressedError extends Data.TaggedError("SuppressedError") {}

const UnrelatedData = {
  TaggedError: (tag: string) => class { readonly _tag = tag }
}
export class UnrelatedError extends UnrelatedData.TaggedError("UnrelatedError") {}

export function shadowedData() {
  const Data = UnrelatedData
  class ShadowedDataError extends Data.TaggedError("ShadowedDataError") {}
  return ShadowedDataError
}

export function shadowedFactory() {
  const TaggedError = UnrelatedData.TaggedError
  class ShadowedFactoryError extends TaggedError("ShadowedFactoryError") {}
  return ShadowedFactoryError
}

export class SchemaError extends Schema.TaggedError<SchemaError>()("SchemaError", {
  message: Schema.String
}) {}
export class UntaggedDataError extends Data.Error<{ message: string }> {}
export class IndirectError extends WithoutFields {}

const SavedBase = Data.TaggedError("SavedBase")
export class SavedFactoryResultError extends SavedBase<{ message: string }> {}

export const NamedExpression = class NamedClassExpression extends Data.TaggedError("NamedExpression") {}
export const AnonymousExpression = class extends Data.TaggedError("AnonymousExpression") {}

let MutableFactory = Data.TaggedError
export class MutableFactoryError extends MutableFactory("MutableFactoryError") {}

export default class extends Data.TaggedError("AnonymousDefaultDeclaration") {}
