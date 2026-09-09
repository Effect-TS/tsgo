// @effect-v4
import * as Schema from "effect/Schema"

export const value = Schema.decodeUnknownSync(Schema.String)("hello")
