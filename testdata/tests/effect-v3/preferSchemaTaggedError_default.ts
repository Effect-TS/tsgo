// @effect-v3
import { Data } from "effect"

export class DefaultOffError extends Data.TaggedError("DefaultOffError")<{
  message: string
}> {}
