// @effect-diagnostics *:off
// @effect-diagnostics isTaggedOnTaggedUnion:warning
import { Array, Data, pipe, Predicate, Schema } from "effect"
import { isTagged } from "effect/Predicate"

const Size = Schema.TaggedUnion({
  Auto: {},
  Px: { value: Schema.Finite },
  Percent: { value: Schema.Finite }
})
type Size = typeof Size.Type
declare const size: Size
declare const sizes: ReadonlyArray<Size>
declare const expected: boolean

type Shape = Data.TaggedEnum<{
  Circle: { readonly radius: number }
  Square: { readonly side: number }
}>
declare const shape: Shape

type Handwritten = { readonly _tag: "On" } | { readonly _tag: "Off"; readonly since: number }
declare const handwritten: Handwritten

declare const optional: Size | undefined

// reported
export const dataFirst = Predicate.isTagged(size, "Auto")
export const curried = Predicate.isTagged("Auto")(size)
export const piped = pipe(size, Predicate.isTagged("Px"))
export const pipedAfterStep = pipe(size, (current) => current, Predicate.isTagged("Px"))
export const namedImport = isTagged(shape, "Circle")
export const handwrittenUnion = Predicate.isTagged(handwritten, "Off")
export const negated = !Predicate.isTagged(size, "Auto")
export const inCondition = (current: Size): number => Predicate.isTagged(current, "Auto") ? 0 : 1
export const inBinary = Predicate.isTagged(size, "Auto") === expected
export const asserted = Predicate.isTagged(size, "Auto") as boolean
export const nestedInput = String(Predicate.isTagged(size, "Px"))
export const parenthesizedInput = Predicate.isTagged(optional ?? size, "Px")
export const generic = <S extends Size>(value: S) => Predicate.isTagged(value, "Px")
export const nativeCallback = sizes.filter(Predicate.isTagged("Px"))
export const pipedCallback = pipe(sizes, Array.filter(Predicate.isTagged("Px")))
export const conditionalCallback: (current: Size) => boolean = expected ? Predicate.isTagged("Px") : () => false

// reported without a fix: the tag is not one of the union's
export const misspelled = Predicate.isTagged(size, "Atuo")
export const misspelledCallback = sizes.filter(Predicate.isTagged("Atuo"))

// not reported
declare const unknownValue: unknown
declare const anyValue: any
declare const mixed: Size | string
declare const dynamicTag: "Auto" | "Px"
export const onUnknown = Predicate.isTagged(unknownValue, "Px")
export const onAny = Predicate.isTagged(anyValue, "Px")
export const onOptional = Predicate.isTagged(optional, "Px")
export const onMixed = Predicate.isTagged(mixed, "Px")
export const onDynamicTag = Predicate.isTagged(size, dynamicTag)
export const unbound = Predicate.isTagged("Px")
const lookalike = { isTagged: (_value: unknown, _tag: string) => false }
export const notPredicate = lookalike.isTagged(size, "Px")
// isTagged's own unknown parameter makes Array.filter infer unknown elements,
// so the callback's contextual parameter is not the union.
export const dataFirstCallback = Array.filter(sizes, Predicate.isTagged("Px"))
