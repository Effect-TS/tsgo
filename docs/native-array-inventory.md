# Native Array method inventory

`preferEffectArray` covers 16 native methods in Effect 3.19.19 and 4.0.0-rc.118. The rule is disabled by default, belongs to `effectNative`, and emits diagnostic 377140 without conversion fixes. Each recommendation names `effect/Array.<API>` and states the migration caveat.

The `effect-native` preset enables `preferEffectArray` at warning severity. The `recommended` and `strict` presets leave it off. `Effect.all(xs.map(f))` can also trigger `allOfMapToForEach`.

The bundled compiler libraries declare 39 Array methods, 30 ReadonlyArray methods, and four named Array constructor methods. Overloads count once. The declarations are in `typescript/tsc/internal/bundled/libs/lib.es5.d.ts`, `lib.es2015.core.d.ts`, `lib.es2015.iterable.d.ts`, `lib.es2015.symbol.wellknown.d.ts`, `lib.es2016.array.include.d.ts`, `lib.es2019.array.d.ts`, `lib.es2022.array.d.ts`, `lib.es2023.array.d.ts`, and `lib.es2026.array.d.ts`.

Constructor call and new signatures, length, prototype, Symbol.species, and Symbol.unscopables are outside this method inventory.

## Instance methods

Every ReadonlyArray method also appears on Array. The ReadonlyArray column identifies the 30 shared methods. A blank counterpart means the rule excludes that method.

| Native member | ReadonlyArray | Effect counterpart | Caveat or exclusion reason |
| --- | --- | --- | --- |
| `[Symbol.iterator]` | Yes |  | No Effect Array counterpart with this contract. |
| `at` | Yes |  | Effect get returns Option, floors indices, and does not implement native negative indices. v3 unsafeGet and v4 getUnsafe throw. |
| `concat` | Yes |  | appendAll omits scalar arguments, multiple arguments, and Symbol.isConcatSpreadable behavior. |
| `copyWithin` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `entries` | Yes |  | No Effect Array counterpart with this contract. |
| `every` | Yes | `effect/Array.every` | The callback type accepts (value, index), with no array parameter or thisArg. |
| `fill` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `filter` | Yes | `effect/Array.filter` | The callback type accepts (value, index), with no array parameter or thisArg; sparse-array holes are visited. |
| `find` | Yes | `effect/Array.findFirst` | Returns Option, with None replacing undefined; the callback type accepts (value, index), with no array parameter or thisArg. |
| `findIndex` | Yes | `effect/Array.findFirstIndex` | Returns Option<number>, with None replacing -1; the callback type accepts (value, index), with no array parameter or thisArg. |
| `findLast` | Yes | `effect/Array.findLast` | Returns Option, with None replacing undefined; the callback type accepts (value, index), with no array parameter or thisArg. |
| `findLastIndex` | Yes | `effect/Array.findLastIndex` | Returns Option<number>, with None replacing -1; the callback type accepts (value, index), with no array parameter or thisArg. |
| `flat` | Yes |  | flatten handles one level of arrays. Native flat also handles depth, scalar elements, and holes. |
| `flatMap` | Yes | `effect/Array.flatMap` | Callbacks must return arrays; the callback type accepts (value, index), with no array parameter or thisArg, and sparse-array holes are visited. |
| `forEach` | Yes | `effect/Array.forEach` | The callback type accepts (value, index), with no array parameter or thisArg. |
| `includes` | Yes |  | contains uses Effect equality rather than SameValueZero. containsWith needs an explicit comparator and fromIndex adaptation. |
| `indexOf` | Yes |  | Predicate search requires equality, fromIndex, and missing-result adaptation. |
| `join` | Yes | `effect/Array.join` | Accepts strings and requires an explicit separator. |
| `keys` | Yes |  | No Effect Array counterpart with this contract. |
| `lastIndexOf` | Yes |  | Predicate search requires equality, fromIndex, and missing-result adaptation. |
| `map` | Yes | `effect/Array.map` | The callback type accepts (value, index), with no array parameter or thisArg. |
| `pop` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `push` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `reduce` | Yes | `effect/Array.reduce` | An initial value is required before the callback; the callback type omits the array parameter. |
| `reduceRight` | Yes | `effect/Array.reduceRight` | An initial value is required before the callback; the callback type omits the array parameter. |
| `reverse` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `shift` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `slice` | Yes |  | copy, take, and drop cover subsets of native slicing. |
| `some` | Yes | `effect/Array.some` | The callback type accepts (value, index), with no array parameter or thisArg. |
| `sort` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `splice` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `toLocaleString` | Yes |  | No Effect Array counterpart with this contract. |
| `toReversed` | Yes | `effect/Array.reverse` | Returns a new array, like native toReversed. |
| `toSorted` | Yes | `effect/Array.sort` | Requires an explicit Order instead of native default ordering or a numeric comparator. |
| `toSpliced` | Yes |  | insertAt and remove do not implement the complete contract. |
| `toString` | Yes |  | No Effect Array counterpart with this contract. |
| `unshift` | No |  | Mutates the receiver. Immutable alternatives change mutation and return behavior. |
| `values` | Yes |  | No Effect Array counterpart with this contract. |
| `with` | Yes |  | replace has different indexing and failure behavior. v3 returns an array; v4 returns Option. |

## Constructor methods

| Native member | Effect counterpart | Caveat or exclusion reason |
| --- | --- | --- |
| `from` |  | fromIterable requires an iterable, omits mapper and thisArg overloads, and can return the original array. |
| `fromAsync` |  | No Effect Array counterpart. |
| `isArray` | `effect/Array.isArray` | Narrows elements to unknown rather than any. |
| `of` |  | Effect of is unary; make requires at least one argument. Neither implements the complete contract. |

## Callback and sparse-array contracts

The callback types in both Effect versions accept value and index, without the native array parameter or thisArg overload. Runtime delegation can still pass the array parameter. Both versions delegate map, every, and some directly to the native receiver method.

For array inputs, fromIterable returns the input unchanged. forEach, reduce, and reduceRight use native methods on that input and preserve native hole skipping. reduce and reduceRight wrap the callback and require an initial value before it.

filter and flatMap use index loops. Their callbacks visit holes as undefined; flatMap also copies holes in returned arrays as undefined. Native filter and flatMap skip those holes. Effect reverse and sort copy with Array.from, which converts holes to undefined, as native toReversed and toSorted do. sort requires an explicit Order.

findFirst, findLast, findFirstIndex, and findLastIndex return Option. None replaces native undefined or -1. Their searches visit holes as undefined, as the corresponding native searches do.

These facts come from `testdata/tests/effect-v3/node_modules/effect/src/Array.ts` and `testdata/tests/effect-v4/node_modules/effect/src/Array.ts`.

## Matching coverage and limits

`internal/typeparser` owns native Array recognition. Its first member-reference query builds one registry per checker on `EffectLinks`, shared by later parser instances and source files. The registry enumerates the global Array, ReadonlyArray, and Array constructor's own members. All canonical member declarations must belong to compiler default-library files. Recognition normalizes canonical and referenced symbols through `GetRootSymbols` and requires every referenced root to match the same namespace and native member. `EffectLinks` caches both successful and unsuccessful reference classifications. The rule keeps the recommendations and migration caveats.

The rule reports value member accesses, including calls, references, literal bracket access, optional access, native constructor aliases, inherited native methods, constrained generics, readonly tuples, native array unions, and Array.prototype.map.call. Diagnostics cover the member name token.

Mixed custom and native unions, declared overrides, user-augmented members, typed arrays, shadowed globals, custom methods, Effect APIs, type positions, and writes are excluded. A native method alias initializer reports once. Later identifier uses and destructuring aliases are outside traversal. Runtime reassignment can retain the native symbol and is outside matching.

Literal bracket access falls back to the property symbol of the nonnullable receiver type when direct token lookup returns no symbol. This includes constrained type parameters. The fallback still requires every symbol root to match a canonical native member. Dynamic computed member names are outside traversal.
