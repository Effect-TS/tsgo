// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "brandedSchemaClasses": ["Class", "Error", "Opaque"]
      }
    ]
  }
}

// @filename: schemaClassMissingBrand_brandedSchemaClassesReduced.ts
// Only Class, Error and Opaque need a brand: tagged classes and tagged errors are not reported.
import { Schema } from "effect"

export class ClassPlain extends Schema.Class<ClassPlain>("ClassPlain")({ value: Schema.String }) {}
export class TaggedClassPlain extends Schema.TaggedClass<TaggedClassPlain>()("TaggedClassPlain", { value: Schema.String }) {}
export class ErrorPlain extends Schema.Error<ErrorPlain>("ErrorPlain")({ value: Schema.String }) {}
export class TaggedErrorPlain extends Schema.TaggedError<TaggedErrorPlain>()("TaggedErrorPlain", { value: Schema.String }) {}
export class OpaquePlain extends Schema.Opaque<OpaquePlain>()(Schema.Struct({ value: Schema.String })) {}

export class ClassBranded extends Schema.Class<ClassBranded, { readonly brand: unique symbol }>("ClassBranded")({ value: Schema.String }) {}
export class TaggedClassBranded extends Schema.TaggedClass<TaggedClassBranded, { readonly brand: unique symbol }>()("TaggedClassBranded", { value: Schema.String }) {}
