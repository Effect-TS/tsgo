// @effect-diagnostics *:off
// @effect-diagnostics errorPayloadShadowsName:warning
import { Data } from "effect"

export class ToolNotFound extends Data.TaggedError("ToolNotFound")<{ readonly name: string }> {}
new ToolNotFound({ name: "search" }).name
