# Native String method inventory

`preferEffectString` suggests Effect String APIs at native String method invocations in Effect 3.19.19 and 4.0.2. The rule belongs to `effectNative`, supports v3 and v4, and defaults to off. The `effect-native` preset enables it as a warning. Diagnostic 377150 includes the migration caveat. Only suppression quick fixes are available.

This inventory covers native String instance and constructor methods. 32 of the 51 instance methods have an `effect/String` counterpart. None of the three constructor methods has one.

The bundled compiler libraries declare 51 String instance methods, one `length` property, and three named String constructor methods. Overloads count once. The declarations are in `typescript/tsc/internal/bundled/libs/lib.es5.d.ts`, `lib.es2015.core.d.ts`, `lib.es2015.iterable.d.ts`, `lib.es2015.symbol.wellknown.d.ts`, `lib.es2017.string.d.ts`, `lib.es2019.string.d.ts`, `lib.es2020.string.d.ts`, `lib.es2021.string.d.ts`, `lib.es2022.string.d.ts`, and `lib.es2024.string.d.ts`. The libraries mark 16 of the instance methods `@deprecated`: the 13 HTML methods, `substr`, `trimLeft`, and `trimRight`.

Constructor call and new signatures, `length`, and `prototype` are outside this method inventory.

## Reported invocations

The rule reports native String method invocations. The diagnostic range is the member name of the callee:

- `trim` in `value.trim()`.
- `"trim"` in `value["trim"]()`.
- `trim` in `(value.trim)()`, `value.trim?.()`, and `maybe?.trim()`.

A member reference is an invocation when, after parentheses are skipped, it is the callee of a call expression. `preferEffectArray` anchors its diagnostic on the same name node. That rule also reports member references that are not called, and this rule does not. A reference such as `const f = value.trim` is not reported. `String.prototype.trim.call(value)` is not reported either, because its callee resolves to `CallableFunction.call`.

The rule reports a receiver when the referenced property's root symbols all resolve to one default-library String member. These receivers resolve to String members: `string`, string literal types, template literal types, branded intersections such as `string & { _brand: "UserId" }`, `String` objects, `T extends string` type parameters, and `string | undefined` through optional chaining. These receivers do not resolve to one String member:

- A `string | Array<string>` receiver for `includes` or `slice` resolves to both String and Array roots. Neither rule reports it.
- An `any` receiver has no symbol.
- A member declared by a user augmentation of the `String` interface is not a default-library declaration.

The rule also skips `super` receivers, assignment targets, and `delete` operands, as `preferEffectArray` does.

## Instance methods

The form column gives the call shapes that the Effect API exports. "Unary" APIs take the string as their only argument. "Data-last" APIs only accept `String.f(args)(self)`, so a rewrite outside `pipe` needs the curried call. "Dual" APIs also accept `String.f(self, args)`. A blank counterpart means the rule excludes that method.

| Native member | Effect counterpart | Form | Caveat or exclusion reason |
| --- | --- | --- | --- |
| `[Symbol.iterator]` |  |  | No Effect String counterpart. `linesIterator` iterates lines, not code points. |
| `anchor` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `at` | `effect/String.at` | Dual | Returns Option, with None replacing undefined. |
| `big` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `blink` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `bold` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `charAt` | `effect/String.charAt` | Dual | Returns Option, with None replacing the empty string for an out-of-range index. |
| `charCodeAt` | `effect/String.charCodeAt` | Dual | Returns Option<number>, with None replacing NaN. |
| `codePointAt` | `effect/String.codePointAt` | Dual | Returns Option<number>, with None replacing undefined. |
| `concat` | `effect/String.concat` | Dual | Accepts exactly one string. Native concat accepts any number of arguments and converts non-strings. |
| `endsWith` | `effect/String.endsWith` | Data-last | Same contract. |
| `fixed` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `fontcolor` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `fontsize` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `includes` | `effect/String.includes` | Data-last | Same contract. |
| `indexOf` | `effect/String.indexOf` | Data-last | Returns Option<number>, with None replacing -1. Has no position parameter. |
| `isWellFormed` |  |  | No Effect String counterpart. |
| `italics` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `lastIndexOf` | `effect/String.lastIndexOf` | Data-last | Returns Option<number>, with None replacing -1. Has no position parameter. |
| `link` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `localeCompare` | `effect/String.localeCompare` | Data-last | Returns an Ordering of -1, 0, or 1. v4 types `locales` as `Array<string>` and rejects a single locale string. |
| `match` | `effect/String.match` | Data-last | Returns Option, with None replacing null. |
| `matchAll` | `effect/String.matchAll` | Data-last | Requires a RegExp. Native matchAll also accepts a string. A non-global RegExp throws TypeError in both. |
| `normalize` | `effect/String.normalize` | Data-last | Same contract. |
| `padEnd` | `effect/String.padEnd` | Data-last | Same contract. |
| `padStart` | `effect/String.padStart` | Data-last | Same contract. |
| `repeat` | `effect/String.repeat` | Data-last | v4 returns the empty string for a negative count, where native repeat throws RangeError. |
| `replace` | `effect/String.replace` | Data-last | Requires a string replacement. Replacer callbacks have no counterpart. |
| `replaceAll` | `effect/String.replaceAll` | Data-last | Requires a string replacement. Replacer callbacks have no counterpart. |
| `search` | `effect/String.search` | Dual | Returns Option<number>, with None replacing -1. |
| `slice` | `effect/String.slice` | Data-last | Same contract. |
| `small` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `split` | `effect/String.split` | Dual | Returns NonEmptyArray. Splitting the empty string by `""` returns `[""]`, where native split returns `[]`. Has no limit parameter. |
| `startsWith` | `effect/String.startsWith` | Data-last | Same contract. |
| `strike` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `sub` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `substr` |  |  | Deprecated. slice and substring handle negative and length arguments differently. |
| `substring` | `effect/String.substring` | Data-last | Same contract. |
| `sup` |  |  | Deprecated HTML method. No Effect String counterpart. |
| `toLocaleLowerCase` | `effect/String.toLocaleLowerCase` | Data-last | v4 types `locale` as `string \| Array<string>`, which excludes `Intl.Locale`. |
| `toLocaleUpperCase` | `effect/String.toLocaleUpperCase` | Data-last | v4 types `locale` as `string \| Array<string>`, which excludes `Intl.Locale`. |
| `toLowerCase` | `effect/String.toLowerCase` | Unary | Same contract. Literal inputs return `Lowercase<T>`. |
| `toString` |  |  | No Effect String counterpart with this conversion contract. |
| `toUpperCase` | `effect/String.toUpperCase` | Unary | Same contract. Literal inputs return `Uppercase<T>`. |
| `toWellFormed` |  |  | No Effect String counterpart. |
| `trim` | `effect/String.trim` | Unary | Same runtime contract. The literal result type removes only space, `\n`, `\t`, and `\r`. |
| `trimEnd` | `effect/String.trimEnd` | Unary | Same runtime contract. The literal result type removes only space, `\n`, `\t`, and `\r`. |
| `trimLeft` | `effect/String.trimStart` | Unary | Deprecated alias of trimStart. The literal result type removes only space, `\n`, `\t`, and `\r`. |
| `trimRight` | `effect/String.trimEnd` | Unary | Deprecated alias of trimEnd. The literal result type removes only space, `\n`, `\t`, and `\r`. |
| `trimStart` | `effect/String.trimStart` | Unary | Same runtime contract. The literal result type removes only space, `\n`, `\t`, and `\r`. |
| `valueOf` |  |  | No Effect String counterpart with this conversion contract. |

## Constructor methods

| Native member | Effect counterpart | Caveat or exclusion reason |
| --- | --- | --- |
| `fromCharCode` |  | No Effect String counterpart. |
| `fromCodePoint` |  | No Effect String counterpart. |
| `raw` |  | No Effect String counterpart. |

## Effect String APIs without a native method

These exports replace expressions or idioms rather than one native method. A rule that matches native method invocations cannot suggest them.

| Effect API | Native idiom | Versions |
| --- | --- | --- |
| `isString` | `typeof value === "string"` | v3, v4 |
| `empty` | `""` | v3, v4 |
| `isEmpty`, `isNonEmpty` | `value.length === 0`, `value.length > 0` | v3, v4 |
| `length` | the `length` property | v3, v4 |
| `takeLeft`, `takeRight` | `value.slice(0, n)`, `value.slice(-n)` with clamped `n` | v3, v4 |
| `capitalize`, `uncapitalize` | `value[0].toUpperCase() + value.slice(1)` | v3, v4 |
| `Equivalence`, `Order` | `===` and `<` comparisons | v3, v4 |
| `linesIterator`, `linesWithSeparators`, `stripMargin`, `stripMarginWith` | No single native method | v3, v4 |
| `snakeToCamel`, `snakeToPascal`, `snakeToKebab`, `camelToSnake`, `pascalToSnake`, `kebabToSnake` | `replace` with case-conversion patterns | v3, v4 |
| `noCase`, `camelCase`, `pascalCase`, `kebabCase`, `snakeCase`, `constantCase`, `configCase` | `replace` with case-conversion patterns | v4 |
| `ReducerConcat` | `reduce((a, b) => a + b, "")` | v4 |

v4 also exports `String`, an alias of `globalThis.String`.

## Runtime and typing evidence

The implementations are in `testdata/tests/effect-v3/node_modules/effect/src/String.ts` and `testdata/tests/effect-v4/node_modules/effect/src/String.ts`. Every counterpart except `concat` calls the native method on its input. `concat` uses `+`. The Option-returning APIs wrap the native result and map the missing-value sentinel to None. Apart from the new case-conversion exports, the v3 and v4 sources differ in five places:

- v4 `repeat` normalizes its count with `Count.normalize`. Negative counts become 0, so the result is the empty string instead of a RangeError.
- v4 `takeLeft` and `takeRight` normalize their counts the same way.
- v4 `snakeToCamel` and `snakeToPascal` return the empty string for empty input.
- v4 `localeCompare` types `locales` as `Array<string>`. v3 uses `Intl.LocalesArgument`.
- v4 `toLocaleLowerCase` and `toLocaleUpperCase` type `locale` as `string | Array<string>`. v3 uses `Intl.LocalesArgument`.

Node 24.14.0 comparisons ran 688 cases for the 32 counterparts against both installed versions. The inputs covered empty, ASCII, non-breaking space, vertical tab, line separator, astral, lone surrogate, and Turkish dotted-I strings, with negative, out-of-range, fractional, and NaN indices. Native sentinels were mapped to None before comparison. Two behaviors differ from native results:

- `"".split("")` returns `[]` natively and `[""]` in both Effect versions.
- `value.repeat(-1)` throws RangeError natively. v4 `repeat(-1)` returns `""`. v3 also throws.

TypeScript 5.9.3 typing checks against both versions found these differences:

- `replace` and `replaceAll` reject replacer callbacks.
- `matchAll` rejects string patterns.
- `concat` rejects more than one argument.
- `indexOf` and `lastIndexOf` reject a position argument.
- `split` rejects a limit argument.
- v4 `localeCompare("b", "en")` fails with TS2345, because `locales` is `Array<string>`. v3 accepts it.
- `trim` on the literal `" a\v"` is typed `"a\v"`, but the runtime result is `"a"`. `trim` on `" a"` keeps the non-breaking space in its type, but the runtime result is `"a"`.

The receiver and invocation shapes in [Reported invocations](#reported-invocations) come from the TypeScript 5.9.3 checker. That checker resolved each probe's `GetSymbolAtLocation` and `GetRootSymbols`, which are the same queries `internal/typeparser/native_array.go` runs through the tsgo checker.
