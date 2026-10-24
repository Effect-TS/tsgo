// @filename: anonymousDefault.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferSchemaTaggedError:warning
import { Data } from "effect"

declare const dynamicTag: string
export default class extends Data.TaggedError(dynamicTag)<{ message: string }> {}

// @filename: namedDefault.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferSchemaTaggedError:warning
import { Data } from "effect"

export default class NamedDefaultError extends Data.TaggedError("NamedDefaultError")<{ message: string }> {}
