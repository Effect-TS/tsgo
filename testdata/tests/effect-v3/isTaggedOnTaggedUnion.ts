// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics isTaggedOnTaggedUnion:warning
import { Array, Data, pipe, Predicate } from "effect"
import { isTagged } from "effect/Predicate"

type Shape = Data.TaggedEnum<{
  Circle: { readonly radius: number }
  Square: { readonly side: number }
}>
declare const shape: Shape
declare const shapes: ReadonlyArray<Shape>

type Handwritten = { readonly _tag: "On" } | { readonly _tag: "Off"; readonly since: number }
declare const handwritten: Handwritten

// reported
export const dataFirst = Predicate.isTagged(shape, "Circle")
export const curried = Predicate.isTagged("Circle")(shape)
export const piped = pipe(shape, Predicate.isTagged("Square"))
export const namedImport = isTagged(handwritten, "Off")
export const negated = !Predicate.isTagged(shape, "Circle")
export const nativeCallback = shapes.filter(Predicate.isTagged("Square"))
export const pipedCallback = pipe(shapes, Array.filter(Predicate.isTagged("Square")))
export const dataFirstCallback = Array.filter(shapes, Predicate.isTagged("Square"))

// reported without a fix: the tag is not one of the union's
export const misspelled = Predicate.isTagged(shape, "Cirle")

// not reported
declare const unknownValue: unknown
declare const optional: Shape | undefined
declare const unknownValues: ReadonlyArray<unknown>
export const onUnknown = Predicate.isTagged(unknownValue, "Circle")
export const onOptional = Predicate.isTagged(optional, "Circle")
export const unbound = Predicate.isTagged("Circle")
export const unknownElements = Array.filter(unknownValues, Predicate.isTagged("Circle"))
