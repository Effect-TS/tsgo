// Parity with .repos/effect-language-service specialChars.ts snapshots verified.
// All four output formats (flat graph, nested graph, outline graph, quickinfo)
// are semantically identical to the .repos reference for the single export (NoComment).
// Special character encoding (<, >, #, ", !) in Mermaid output is correctly handled.
// Divergence #1 (alphabetical sorting): does NOT manifest — single provides type, nothing to sort.
// Divergences that do NOT apply (single-file test, no cross-file references):
//   2. Cross-file location annotations in flat output format (format.go)
//   3. "at in" double preposition fix in quickinfo for cross-file refs (format.go)
import { Layer, Context } from "effect"

const MyTypeId: unique symbol = Symbol.for("x")
type MyTypeId = typeof MyTypeId

export interface IsGeneric<X> {
  readonly [MyTypeId]: MyTypeId
  readonly value: X
}


declare const WithSpecialChars: Context.Service<IsGeneric<"With<Special>Chars#!">, { value: "WithSpecialChars" }>
const withSpecialCharsLayer = Layer.succeed(WithSpecialChars, { value: "WithSpecialChars" } as any)

export const NoComment = withSpecialCharsLayer
