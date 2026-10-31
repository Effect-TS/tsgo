// @effect-diagnostics *:off
// @effect-diagnostics emptySchemaStruct:warning
import { Schema } from "effect"

export const Empty = Schema.Struct({})
export const decoded = Schema.decodeUnknownSync(Empty)(42)
