# Native Object methods and Effect Record

`preferEffectRecord` suggests six Effect Record counterparts in Effect 3.19.19 and 4.0.2. The rule belongs to `effectNative`, supports v3 and v4, and defaults to off. The `effect-native` preset enables it as a warning. Diagnostic 377143 includes the migration caveat. Only suppression quick fixes are available.

The bundled compiler libraries declare 23 named Object constructor methods and six Object instance methods. Overloads count once. The declarations are in `typescript/tsc/internal/bundled/libs/lib.es5.d.ts`, `lib.es2015.core.d.ts`, `lib.es2017.object.d.ts`, `lib.es2019.object.d.ts`, `lib.es2022.object.d.ts`, and `lib.es2024.object.d.ts`. Constructor call and new signatures and the `prototype` property are outside this method inventory.

## Constructor methods

| Native member | Effect counterpart | Caveat or exclusion reason |
| --- | --- | --- |
| `assign` | | Mutates the target and copies symbol keys. Record merging changes that contract. |
| `create` | | Creates an object with a chosen prototype and optional descriptors. No Record counterpart. |
| `defineProperties` | | Mutates property descriptors. No Record counterpart. |
| `defineProperty` | | Mutates a property descriptor. No Record counterpart. |
| `entries` | `effect/Record.toEntries` | Requires a record rather than arrays or primitives. Types entries with inferred keys. Snapshots keys before reads, so getter or proxy mutations can change the result. |
| `freeze` | | Changes object integrity. No Record counterpart. |
| `fromEntries` | `effect/Record.fromEntries` | Requires typed string or symbol key tuples. Native numeric keys and loose entry arrays need adaptation. |
| `getOwnPropertyDescriptor` | | Reflects a descriptor. No Record counterpart. |
| `getOwnPropertyDescriptors` | | Reflects descriptors. No Record counterpart. |
| `getOwnPropertyNames` | | Includes nonenumerable keys. Record keys omits them. |
| `getOwnPropertySymbols` | | Includes symbol keys. Record keys omits them. |
| `getPrototypeOf` | | Reflects the prototype. No Record counterpart. |
| `groupBy` | | Its candidate counterpart belongs to Effect Array, outside this Record comparison. |
| `hasOwn` | `effect/Record.has` | Requires a record rather than arrays or primitives and a string or symbol key of that record's key type. Arbitrary strings may need narrowing. |
| `is` | | SameValue comparison has no Record counterpart. |
| `isExtensible` | | Tests object integrity. No Record counterpart. |
| `isFrozen` | | Tests object integrity. No Record counterpart. |
| `isSealed` | | Tests object integrity. No Record counterpart. |
| `keys` | `effect/Record.keys` | Requires a record rather than arrays or primitives. Types keys as the inferred key union, which does not guarantee exact runtime keys. |
| `preventExtensions` | | Changes object integrity. No Record counterpart. |
| `seal` | | Changes object integrity. No Record counterpart. |
| `setPrototypeOf` | | Mutates the prototype. No Record counterpart. |
| `values` | `effect/Record.values` | Requires a record rather than arrays or primitives. Snapshots keys before reads, so getter or proxy mutations can change the result. |

## Instance methods

| Native member | Effect counterpart | Caveat or exclusion reason |
| --- | --- | --- |
| `hasOwnProperty` | `effect/Record.has` | Pass the receiver explicitly. Requires a record rather than arrays or primitives and a string or symbol key of that record's key type. Avoids instance binding and overrides. |
| `isPrototypeOf` | | Tests the prototype chain. No Record counterpart. |
| `propertyIsEnumerable` | | Tests enumerability. Record has also accepts nonenumerable properties. |
| `toLocaleString` | | No Record counterpart with this conversion contract. |
| `toString` | | No Record counterpart with this conversion contract. |
| `valueOf` | | No Record counterpart with this conversion contract. |

## Runtime and typing evidence

The implementations are in `testdata/tests/effect-v3/node_modules/effect/src/Record.ts` and `testdata/tests/effect-v4/node_modules/effect/src/Record.ts`. Both versions implement `keys` with `Object.keys` and directly alias `Object.fromEntries`. Both implement `values` and `toEntries` through `collect`, which gets the keys before reading their values. v3 `has` uses `Object.prototype.hasOwnProperty.call`. v4 uses `Object.hasOwn`.

Node comparisons passed in both installed versions for ordinary own enumerable string keys, integer-key order, inherited and hidden properties, symbols, null-prototype records, duplicate entries, and an own `__proto__` data property. `fromEntries` keeps symbol keys, retains an own `__proto__` key, and uses the last duplicate value. `has` includes hidden and symbol properties and excludes inherited properties.

The comparisons also established a runtime difference. If a getter deletes a later property, native `Object.values` returns `[1]`, while Effect `Record.values` returns `[1, undefined]`. `toEntries` has the same key-snapshot difference. Proxy descriptor and read ordering can also differ. These recommendations do not promise equivalent behavior for getters or proxies.

Calling an instance's `hasOwnProperty` binds the receiver and can call an override. A null-prototype object has no inherited method. `Record.has(record, key)` passes the receiver explicitly and avoids those instance dependencies. Borrowed `Object.prototype.hasOwnProperty.call(record, key)` also avoids an instance override.

Typing checks with TypeScript 5.9.3 found these differences in both Effect versions:

- `keys`, `values`, `toEntries`, and `has` reject arrays accepted by native Object methods. Their record types can also reject primitive inputs accepted through native coercion.
- For `{ a: number; b: string }`, `keys` returns `Array<"a" | "b">` and `toEntries` returns `Array<["a" | "b", number | string]>`. Structural types can hide extra runtime keys. The inferred union does not establish an exact key set.
- `has` rejects an arbitrary `string` or the key `"c"` against that literal-key record. It also rejects numeric keys on a string-key record.
- `fromEntries` rejects `Array<[number, string]>`. Its typed tuples require string or symbol keys, while native overloads accept numeric PropertyKey values and loose entry arrays.

## Matching coverage and limits

`internal/typeparser/native_object.go` owns recognition. Its lazy registry and symbol classifications belong to the checker's `EffectLinks`, shared across parser instances and source files. Every declaration of a canonical member must belong to a compiler default library. Every referenced root must match the same canonical namespace and method. Both successful and unsuccessful classifications are cached. The rule's recommendation table owns the spelling prefilter and migration messages.

The rule reports calls and value references, including literal brackets and templates, optional accesses, constructor and import aliases, inherited native methods, constrained generics, `globalThis.Object`, and borrowed `.call` references. The diagnostic range covers the member name token. Native identity alone determines eligibility. Receiver and argument shapes do not gate the advice.

Custom members, shadowed globals, declared overrides, user augmentation, mixed roots, missing libraries, dynamic computed names, type positions, writes, declaration names, deletes, and `super` accesses are excluded. An alias initializer reports once. Later identifier uses and destructuring aliases are outside traversal. Runtime reassignment that retains a native symbol is outside recognition.
