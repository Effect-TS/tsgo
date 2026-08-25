// @effect-v3
import { Effect } from "effect"

declare const values: ReadonlyArray<number>
declare const effectful: (value: number, index: number) => Effect.Effect<string>

// Should trigger without options.
export const direct = Effect.all(values.map(effectful))

// Should trigger and carry compatible options through to Effect.forEach.
export const withOptions = Effect.all(
  values.map((value, index) => Effect.succeed(`${index}:${value}`)),
  { concurrency: 2, discard: true }
)

// The suggested replacements must type-check with the same receiver, callback,
// index parameter, and options.
export const expectedDirect = Effect.forEach(values, effectful)
export const expectedWithOptions = Effect.forEach(
  values,
  (value, index) => Effect.succeed(`${index}:${value}`),
  { concurrency: 2, discard: true }
)

// Should trigger, but should not offer an unsafe fix that drops the map generic.
export const explicitMapGeneric = Effect.all(
  values.map<Effect.Effect<string>>((value) => Effect.succeed(String(value)))
)

// Should not trigger when Effect.all mode changes the result semantics.
export const eitherMode = Effect.all(values.map(effectful), { mode: "either" })
const validateOptions = { mode: "validate" as const, concurrency: 2 }
export const validateMode = Effect.all(values.map(effectful), validateOptions)

// Should not trigger for a non-array receiver with a map method.
declare const recordMapper: {
  readonly [index: number]: number
  map<B>(f: (value: number) => B): ReadonlyArray<B>
}
export const record = Effect.all(recordMapper.map((value) => Effect.succeed(value)))

// Should not trigger when the callback does not return an Effect.
// @ts-expect-error Effect.all requires Effect values
export const plainValues = Effect.all(values.map((value) => value + 1))

// Should not trigger for legitimate Effect.all inputs.
export const tuple = Effect.all([Effect.succeed(1), Effect.succeed("two")] as const)
