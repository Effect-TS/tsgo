// @effect-v3
import * as Schema from "effect/Schema"

export const value = Schema.decodeUnknownSync(Schema.String)("hello")
