// @effect-diagnostics *:off
// @effect-diagnostics isTaggedOnTaggedUnion:warning
import { Predicate, Schema } from "effect"

const Size = Schema.TaggedUnion({
  Auto: {},
  Px: { value: Schema.Finite }
})

export const width = (size: typeof Size.Type): number =>
  Predicate.isTagged(size, "Auto") ? 0 : size.value
