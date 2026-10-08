import { Context, Schema } from "effect"
import { Model } from "effect/schema"
import * as S from "effect/Schema"
import { Class as MakeClass, TaggedClass as MakeTaggedClass, Error as MakeError,
  TaggedError as MakeTaggedError, Opaque as MakeOpaque } from "effect/Schema"

type BrandAlias = { readonly brand: unique symbol }

export class ClassNamespace extends Schema.Class<ClassNamespace>("ClassNamespace")({ value: Schema.String }) {}
export class ClassModule extends S.Class<ClassModule>("ClassModule")({ value: Schema.String }) {}
export class ClassRenamed extends MakeClass<ClassRenamed>("ClassRenamed")({ value: Schema.String }) {}

export class TaggedClassNamespace extends Schema.TaggedClass<TaggedClassNamespace>()("TaggedClassNamespace", { value: Schema.String }) {}
export class TaggedClassModule extends S.TaggedClass<TaggedClassModule>()("TaggedClassModule", { value: Schema.String }) {}
export class TaggedClassRenamed extends MakeTaggedClass<TaggedClassRenamed>()("TaggedClassRenamed", { value: Schema.String }) {}

export class ErrorNamespace extends Schema.Error<ErrorNamespace>("ErrorNamespace")({ value: Schema.String }) {}
export class ErrorModule extends S.Error<ErrorModule>("ErrorModule")({ value: Schema.String }) {}
export class ErrorRenamed extends MakeError<ErrorRenamed>("ErrorRenamed")({ value: Schema.String }) {}

export class TaggedErrorNamespace extends Schema.TaggedError<TaggedErrorNamespace>()("TaggedErrorNamespace", { value: Schema.String }) {}
export class TaggedErrorModule extends S.TaggedError<TaggedErrorModule>()("TaggedErrorModule", { value: Schema.String }) {}
export class TaggedErrorRenamed extends MakeTaggedError<TaggedErrorRenamed>()("TaggedErrorRenamed", { value: Schema.String }) {}

export class OpaqueNamespace extends Schema.Opaque<OpaqueNamespace>()(Schema.Struct({ value: Schema.String })) {}
export class OpaqueModule extends S.Opaque<OpaqueModule>()(Schema.Struct({ value: Schema.String })) {}
export class OpaqueRenamed extends MakeOpaque<OpaqueRenamed>()(Schema.Struct({ value: Schema.String })) {}

export class ClassBranded extends Schema.Class<ClassBranded, { readonly brand: unique symbol }>("ClassBranded")({ value: Schema.String }) {}
export class TaggedClassBranded extends Schema.TaggedClass<TaggedClassBranded, { readonly brand: unique symbol }>()("TaggedClassBranded", { value: Schema.String }) {}
export class ErrorBranded extends Schema.Error<ErrorBranded, { readonly brand: unique symbol }>("ErrorBranded")({ value: Schema.String }) {}
export class TaggedErrorBranded extends Schema.TaggedError<TaggedErrorBranded, { readonly brand: unique symbol }>()("TaggedErrorBranded", { value: Schema.String }) {}
export class OpaqueBranded extends Schema.Opaque<OpaqueBranded, { readonly brand: unique symbol }>()(Schema.Struct({ value: Schema.String })) {}

export class ClassAlias extends Schema.Class<ClassAlias, BrandAlias>("ClassAlias")({ value: Schema.String }) {}
export class TaggedClassAlias extends Schema.TaggedClass<TaggedClassAlias, BrandAlias>()("TaggedClassAlias", { value: Schema.String }) {}
export class ErrorAlias extends Schema.Error<ErrorAlias, BrandAlias>("ErrorAlias")({ value: Schema.String }) {}
export class TaggedErrorAlias extends Schema.TaggedError<TaggedErrorAlias, BrandAlias>()("TaggedErrorAlias", { value: Schema.String }) {}
export class OpaqueAlias extends Schema.Opaque<OpaqueAlias, BrandAlias>()(Schema.Struct({ value: Schema.String })) {}

export class ClassStructural extends Schema.Class<ClassStructural, {}>("ClassStructural")({ value: Schema.String }) {}
export class TaggedClassStructural extends Schema.TaggedClass<TaggedClassStructural, {}>()("TaggedClassStructural", { value: Schema.String }) {}
export class ErrorStructural extends Schema.Error<ErrorStructural, {}>("ErrorStructural")({ value: Schema.String }) {}
export class TaggedErrorStructural extends Schema.TaggedError<TaggedErrorStructural, {}>()("TaggedErrorStructural", { value: Schema.String }) {}
export class OpaqueStructural extends Schema.Opaque<OpaqueStructural, {}>()(Schema.Struct({ value: Schema.String })) {}

export const ClassValue = class ClassExpression extends Schema.Class<ClassExpression>("ClassExpression")({ value: Schema.String }) {}
export const TaggedClassValue = class TaggedClassExpression extends Schema.TaggedClass<TaggedClassExpression>()("TaggedClassExpression", { value: Schema.String }) {}
export const ErrorValue = class ErrorExpression extends Schema.Error<ErrorExpression>("ErrorExpression")({ value: Schema.String }) {}
export const TaggedErrorValue = class TaggedErrorExpression extends Schema.TaggedError<TaggedErrorExpression>()("TaggedErrorExpression", { value: Schema.String }) {}
export const OpaqueValue = class OpaqueExpression extends Schema.Opaque<OpaqueExpression>()(Schema.Struct({ value: Schema.String })) {}

export const AnonymousOpaque = class extends Schema.Opaque<OpaqueNamespace>()(Schema.Struct({ value: Schema.String })) {}

export const AnonymousClass = class extends Schema.Class<ClassNamespace>("AnonymousClass")({ value: Schema.String }) {}

export class UnbrandedParent extends Schema.Class<UnbrandedParent>("UnbrandedParent")({}) {
  static nested() {
    return class NestedInUnbranded extends Schema.TaggedClass<NestedInUnbranded>()("NestedInUnbranded", {}) {}
  }
}

export class BrandedParent extends Schema.Class<BrandedParent, BrandAlias>("BrandedParent")({}) {
  static nested() {
    return class NestedInBranded extends Schema.Error<NestedInBranded>("NestedInBranded")({}) {}
  }
}

export class OrdinaryBase {}
export class OrdinarySubclass extends OrdinaryBase {}
export class IndirectSchemaSubclass extends ClassStructural {}
export class ExtendedSchemaClass extends ClassStructural.extend<ExtendedSchemaClass>("ExtendedSchemaClass")({ extra: Schema.String }) {}
export class Service extends Context.Service<Service, { readonly value: string }>()("Service") {}
export class ModelClass extends Model.Class<ModelClass>("ModelClass")({ value: Schema.String }) {}

namespace Local {
  export declare function Class<Self>(...args: unknown[]): (...fields: unknown[]) => typeof OrdinaryBase
  export declare function TaggedClass<Self>(...args: unknown[]): (...fields: unknown[]) => typeof OrdinaryBase
  export declare function Error<Self>(...args: unknown[]): (...fields: unknown[]) => typeof OrdinaryBase
  export declare function TaggedError<Self>(...args: unknown[]): (...fields: unknown[]) => typeof OrdinaryBase
  export declare function Opaque<Self>(...args: unknown[]): (...fields: unknown[]) => typeof OrdinaryBase
}

export class ClassLookalike extends Local.Class<ClassLookalike>("ClassLookalike")({ value: Schema.String }) {}
export class TaggedClassLookalike extends Local.TaggedClass<TaggedClassLookalike>()("TaggedClassLookalike", { value: Schema.String }) {}
export class ErrorLookalike extends Local.Error<ErrorLookalike>("ErrorLookalike")({ value: Schema.String }) {}
export class TaggedErrorLookalike extends Local.TaggedError<TaggedErrorLookalike>()("TaggedErrorLookalike", { value: Schema.String }) {}
export class OpaqueLookalike extends Local.Opaque<OpaqueLookalike>()(Schema.Struct({ value: Schema.String })) {}
