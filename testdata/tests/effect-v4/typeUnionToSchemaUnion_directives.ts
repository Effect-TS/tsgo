// @effect-diagnostics typeUnionToSchemaUnion:error
import { Schema } from "effect"

export const A = Schema.String
export const B = Schema.Boolean

// @effect-diagnostics-next-line typeUnionToSchemaUnion:off
export type AB = typeof A.Type | typeof B.Type

// MUST TRIGGER as an error - enabled by the file-level directive
export type BA = typeof B.Type | typeof A.Type
