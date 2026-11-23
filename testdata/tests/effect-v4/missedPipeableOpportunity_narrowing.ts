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
import { Option, Result } from "effect"

declare function assert(value: unknown): asserts value

declare const result: Result.Result<number, string>
declare const holder: { readonly result: Result.Result<number, string> }
declare const o: Option.Option<number>

// Should not trigger: the predicate narrows `result` only when called on it directly.
// `result.pipe(Result.isFailure, assert)` would leave `result` un-narrowed.
export function onIdentifier() {
  assert(Result.isFailure(result))
  return result.failure
}

// Should not trigger: same for a property access reference
export function onPropertyAccess() {
  assert(Result.isFailure(holder.result))
  return holder.result.failure
}

// Should not trigger: same for a `this` reference
export class Holder {
  constructor(readonly result: Result.Result<number, string>) {}
  check() {
    assert(Result.isFailure(this.result))
    return this.result.failure
  }
}

// Should trigger: the predicate is applied to a call result, nothing is narrowed
export function onCallResult() {
  assert(Option.isSome(Option.map(o, (n) => n + 1)))
}
