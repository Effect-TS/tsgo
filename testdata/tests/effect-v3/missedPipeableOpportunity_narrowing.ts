// @effect-v3
// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}

// @filename: missedPipeableOpportunity_narrowing.ts
// @effect-diagnostics missedPipeableOpportunity:warning
import { Either, Option } from "effect"

declare function assert(value: unknown): asserts value

declare const result: Either.Either<number, string>
declare const holder: { readonly result: Either.Either<number, string> }
declare const o: Option.Option<number>
declare const results: ReadonlyArray<Either.Either<number, string>>
declare const maybe: Either.Either<number, string> | undefined
declare function compute(): Either.Either<number, string>

// Should not trigger: the predicate narrows `result` only when called on it directly.
// `result.pipe(Either.isLeft, assert)` would leave `result` un-narrowed.
export function onIdentifier() {
  assert(Either.isLeft(result))
  return result.left
}

// Should not trigger: same for a property access reference
export function onPropertyAccess() {
  assert(Either.isLeft(holder.result))
  return holder.result.left
}

// Should not trigger: same for a `this` reference
export class Holder {
  constructor(readonly result: Either.Either<number, string>) {}
  check() {
    assert(Either.isLeft(this.result))
    return this.result.left
  }
}

// Should not trigger: same for an element access reference
export function onElementAccess() {
  assert(Either.isLeft(results[0]))
  return results[0].left
}

// Should not trigger: same for a non-null assertion on a reference
export function onNonNull() {
  assert(Either.isLeft(maybe!))
  return maybe.left
}

// Should not trigger: same for an assignment
export function onAssignment() {
  let assigned: Either.Either<number, string>
  assert(Either.isLeft((assigned = compute())))
  return assigned.left
}

// Should trigger: the predicate is applied to a call result, nothing is narrowed
export function onCallResult() {
  assert(Option.isSome(Option.map(o, (n) => n + 1)))
}
