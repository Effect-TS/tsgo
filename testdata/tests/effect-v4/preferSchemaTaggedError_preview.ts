// @effect-diagnostics *:off
// @effect-diagnostics preferSchemaTaggedError:warning
import { Data } from "effect"

export class NotFound extends Data.TaggedError("NotFound")<{
  id: string
}> {}
