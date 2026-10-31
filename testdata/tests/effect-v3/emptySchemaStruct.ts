// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics emptySchemaStruct:warning
import { Data, Schema as S } from "effect"
import * as Schema from "effect/Schema"
import { Struct, Struct as SchemaStruct } from "effect/Schema"

// MUST TRIGGER (no fix): Schema.Struct({}) models the type {}, so any non-null value decodes
export const Empty = Schema.Struct({})

// MUST TRIGGER (no fix): the reproduction from the issue, decoding 42 succeeds and returns 42
export const decoded = Schema.decodeUnknownSync(Schema.Struct({}))(42)

// MUST TRIGGER (no fix): the right replacement depends on intent, and replacing the call
// with Schema.Object would change the type to object and break Struct-only uses
// such as .fields
export const emptyFields = Schema.Struct({}).fields

// MUST TRIGGER (no fix): a body holding only a comment still has zero properties
export const Placeholder = Schema.Struct({
  // TODO: add the fields
})

// MUST TRIGGER (no fix): a parenthesised empty literal
export const Parenthesised = Schema.Struct(({}))

// MUST TRIGGER (no fix): barrel import under another name
export const BarrelAlias = S.Struct({})

// MUST TRIGGER (no fix): named import from effect/Schema
export const NamedImport = Struct({})

// MUST TRIGGER (no fix): renamed named import from effect/Schema
export const RenamedImport = SchemaStruct({})

// MUST TRIGGER (no fix): const alias of the API
const makeStruct = Schema.Struct
export const ConstAlias = makeStruct({})

// MUST TRIGGER (no fix): the empty struct is the subject of a method chain, the call itself is reported
export const Annotated = Schema.Struct({}).annotations({ identifier: "Annotated" })

// MUST TRIGGER (inner call only, no fix): the outer struct has fields, the nested one is empty
export const Nested = Schema.Struct({
  name: Schema.String,
  metadata: Schema.Struct({})
})

// MUST TRIGGER (no fix): member of a union
export const InUnion = Schema.Union(Schema.String, Schema.Struct({}))

// MUST TRIGGER (no fix): inside a Schema.Class field bag
export class Settings extends Schema.Class<Settings>("Settings")({
  extra: Schema.Struct({})
}) {}

// MUST NOT TRIGGER: a non-empty field literal
export const User = Schema.Struct({
  name: Schema.String
})

// MUST NOT TRIGGER: the fields are passed by reference, only a literal {} is reported
const noFields = {}
export const FromIdentifier = Schema.Struct(noFields)

// MUST NOT TRIGGER: a spread is a property element, so the literal is not empty
const baseFields = { id: Schema.String }
export const Spread = Schema.Struct({ ...baseFields })

// MUST NOT TRIGGER: an index-signature record argument makes the schema meaningful
// (decoding 42 fails here)
export const WithRecord = Schema.Struct({}, Schema.Record({ key: Schema.String, value: Schema.Number }))

// MUST NOT TRIGGER: index-signature records passed as a rest argument
const records = [Schema.Record({ key: Schema.String, value: Schema.Number })] as const
export const WithRestRecords = Schema.Struct({}, ...records)

// MUST NOT TRIGGER: the suggested replacements
export const AnyObject = Schema.Object
export const OpenMap = Schema.Record({ key: Schema.String, value: Schema.Number })

// MUST NOT TRIGGER: TaggedStruct always has the _tag field
export const Tagged = Schema.TaggedStruct("Tagged", {})

// MUST NOT TRIGGER: other Schema APIs taking an empty field bag are out of scope
export class EmptyClass extends Schema.Class<EmptyClass>("EmptyClass")({}) {}

// MUST NOT TRIGGER: Data.struct is a different module
export const dataStruct = Data.struct({})

// MUST NOT TRIGGER: Schema.Struct is referenced, not called
export const reference = Schema.Struct

// MUST NOT TRIGGER: a local object whose method is named like the Effect API
const Local = {
  Struct: <A extends object>(fields: A) => fields
}
export const localStruct = Local.Struct({})

// MUST NOT TRIGGER: a next-line directive disables the rule for one line
// @effect-diagnostics-next-line emptySchemaStruct:off
export const nextLineOff = Schema.Struct({})

// MUST TRIGGER, as an error (no fix): a next-line directive overrides the severity
// @effect-diagnostics-next-line emptySchemaStruct:error
export const nextLineError = Schema.Struct({})
