// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics globalJson:warning
import { Schema } from "effect"

const User = Schema.Struct({ name: Schema.String })

export const parseUser = (text: string) => Schema.decodeUnknownSync(User)(JSON.parse(text))
