// @effect-v3
import { Clock, Context, Effect, Either, Option, pipe } from "effect"
import * as E from "effect/Effect"

interface User {
  readonly id: number
  readonly name: string
}
interface Config {
  readonly locale: string
}

class Database extends Context.Tag("Database")<Database, { readonly load: (id: number) => Effect.Effect<string> }>() {}

declare const getUser: Effect.Effect<User, "UserError", "UserRepo">
declare const getConfig: Effect.Effect<Config, "ConfigError", "ConfigRepo">
declare const loadProfile: (id: number) => Effect.Effect<string>
declare const loadSettings: () => Effect.Effect<Config>

// ---------------------------------------------------------------------------
// MUST TRIGGER (quick fix: Effect.zipWith / Effect.zip, both exist in v3 with the same dual shape)
// ---------------------------------------------------------------------------

// MUST TRIGGER: data-first flatMap with a data-first inner map.
// Fix: Effect.zipWith(getUser, getConfig, (user, config) => `${user.name} (${config.locale})`)
export const dataFirst = Effect.flatMap(getUser, (user) =>
  Effect.map(getConfig, (config) => `${user.name} (${config.locale})`))

// MUST TRIGGER: pipeable flatMap with a pipeable inner map.
// Fix: getUser.pipe(Effect.zipWith(getConfig, (user, config) => user.name + config.locale))
export const pipeablePipeable = getUser.pipe(
  Effect.flatMap((user) => getConfig.pipe(Effect.map((config) => user.name + config.locale)))
)

// MUST TRIGGER: function pipe with a data-first inner map.
export const functionPipe = pipe(
  getUser,
  Effect.flatMap((user) => Effect.map(getConfig, (config) => user.name + config.locale))
)

// MUST TRIGGER: curried flatMap applied to self.
export const curriedFlatMap = Effect.flatMap((user: User) => Effect.map(getConfig, (config) => user.name + config.locale))(
  getUser
)

// MUST TRIGGER: independent clock effect (T3 Code shape), with a later outer step kept.
export const clockSite = getUser.pipe(
  Effect.flatMap((result) => Clock.currentTimeMillis.pipe(Effect.map((at) => ({ result, at })))),
  Effect.withSpan("load")
)

// MUST TRIGGER: namespace alias.
export const namespaceAlias = E.flatMap(getUser, (user) => E.map(getConfig, (config) => user.name + config.locale))

// MUST TRIGGER: the combiner is exactly the tuple of the two parameters. Fix: Effect.zip(getUser, getConfig)
export const tupleDataFirst = Effect.flatMap(getUser, (user) => Effect.map(getConfig, (config) => [user, config]))

// MUST TRIGGER: Fix: getUser.pipe(Effect.zip(getConfig))
export const tuplePipeable = getUser.pipe(Effect.flatMap((user) => getConfig.pipe(Effect.map((config) => [user, config]))))

// MUST TRIGGER, NO FIX: `that` is a call expression; zipWith would evaluate it eagerly, once,
// instead of after `self` succeeds on every run.
export const thatIsCall = Effect.flatMap(getUser, (user) => Effect.map(loadSettings(), (config) => user.name + config.locale))

// MUST NOT TRIGGER: the mapper re-yields the outer result unchanged; that is Effect.tap (flatMapToTap), not a combination.
export const mapperReturnsOuter = getUser.pipe(Effect.flatMap((user) => Effect.map(getConfig, () => user)))
// MUST NOT TRIGGER: the mapper re-yields the outer result unchanged; that is Effect.tap (flatMapToTap), not a combination.
export const mapperReturnsOuterDestructuredParameter = Effect.flatMap(getUser, (user) =>
  Effect.map(getConfig, ({ locale }) => user))

// MUST TRIGGER, NO FIX: the outer parameter is optional, so merging it before the required mapper parameter
// would give `(a?: string, b) => ...`, a required parameter after an optional one (TS1016).
declare const getName: Effect.Effect<string>
declare const getCount: Effect.Effect<number>
export const outerOptionalParameter = Effect.flatMap(getName, (a?: string) => Effect.map(getCount, (b) => a + String(b)))

// MUST TRIGGER: an optional outer parameter followed only by optional ones is valid to merge.
// Fix: Effect.zipWith(getName, getCount, (a?: string, b?: number) => a + String(b))
export const outerOptionalThenOptional = Effect.flatMap(getName, (a?: string) =>
  Effect.map(getCount, (b?: number) => a + String(b)))

// MUST TRIGGER: an optional outer parameter with a parameterless mapper stays the only parameter.
// Fix: Effect.zipWith(getName, getCount, (a?: string) => a + "!")
export const outerOptionalAlone = Effect.flatMap(getName, (a?: string) => Effect.map(getCount, () => a + "!"))

// MUST TRIGGER: optional parameters make the pair more than `[a, b]`, so the fix is zipWith, not zip.
// Fix: Effect.zipWith(getName, getCount, (a?, b?) => [a, b])
export const tupleOptionalParameters = Effect.flatMap(getName, (a?) => Effect.map(getCount, (b?) => [a, b]))

// MUST TRIGGER, NO FIX: the mapper declares a `this` parameter, which must stay first, so the outer
// parameter cannot be prepended to its parameter list (TS2680).
export const mapperThisParameter = Effect.flatMap(getUser, (user) =>
  Effect.map(getConfig, function(this: void, config: Config) {
    return user.name + config.locale
  }))

// MUST TRIGGER, NO FIX (adversarial regression): readonly does not make the backing object immutable.
// The original returns 11; moving destructuring after `aliasSecond` runs would return 12.
const aliasMutable = { value: 1 }
const aliasView: { readonly value: number } = aliasMutable
const aliasFirst = Effect.succeed(aliasView)
const aliasSecond = Effect.sync(() => {
  aliasMutable.value = 2
  return 10
})
export const readonlyAlias = Effect.flatMap(aliasFirst, ({ value }) =>
  Effect.map(aliasSecond, (n) => value + n))

// MUST TRIGGER, NO FIX (adversarial regression): a `this` parameter must stay first;
// prepending `value` would emit `function(value, this: void)` (TS2680).
const thisFirst = Effect.succeed(1)
const thisSecond = Effect.succeed(2)
export const explicitThis = Effect.flatMap(thisFirst, (value) =>
  Effect.map(thisSecond, function(this: void) { return value + 1 }))

// MUST TRIGGER (adversarial regression): only the inner mapper branches; the outer callback only returns map.
// The combiner keeps the mapper body as written.
// Fix: Effect.zipWith(branchFirst, branchSecond, (a, b) => { if (b > 0) return a + b; return a - b })
declare const branchFirst: Effect.Effect<number>
declare const branchSecond: Effect.Effect<number>
export const innerEarlyReturn = Effect.flatMap(branchFirst, (a) =>
  Effect.map(branchSecond, (b) => {
    if (b > 0) return a + b
    return a - b
  }))

// MUST TRIGGER (adversarial regression): a function mapper can return from both branches, with no final return.
// Fix: pipe(branchFirst, Effect.zipWith(branchSecond, function (a, b) { if (b > 0) { return a + b } else { return a - b } }))
export const innerBranchReturns = pipe(branchFirst, Effect.flatMap((a) =>
  branchSecond.pipe(Effect.map(function (b) {
    if (b > 0) {
      return a + b
    } else {
      return a - b
    }
  }))))

// MUST TRIGGER: a mapper with no return statement at all (its result is void) is still a combiner.
// Fix: Effect.zipWith(branchFirst, branchSecond, (a, b) => { console.log(a + b) })
export const innerNoReturn = Effect.flatMap(branchFirst, (a) =>
  Effect.map(branchSecond, (b) => {
    console.log(a + b)
  }))

// MUST TRIGGER, NO FIX: an arrow mapper that may call `eval` can read the outer function's `this` or
// `arguments` by name at run time; the merged arrow would see different ones.
declare const evalFirst: Effect.Effect<number>
declare const evalSecond: Effect.Effect<number>
export const evalInArrowMapper = Effect.flatMap(evalFirst, function(a) {
  return Effect.map(evalSecond, (b) => a + b + Number(eval("arguments.length")))
})

// MUST TRIGGER, NO FIX: both callbacks are generic; the fix does not merge type parameter lists.
export const genericBoth = Effect.flatMap(evalFirst, <A extends number>(a: A) =>
  Effect.map(evalSecond, <B extends number>(b: B) => a + b))

// MUST TRIGGER: the pair has an explicit return type, so the fix is zipWith (keeping it), not zip.
export const tupleAnnotated = Effect.flatMap(getUser, (user) => Effect.map(getConfig, (config): unknown => [user, config]))

// MUST TRIGGER, NO FIX: the default `bump()` runs before `that` in the nested form, after it in the combiner.
declare const bump: () => number
declare const getPartialCount: Effect.Effect<{ readonly n?: number }>
export const outerDefaultEffectful = Effect.flatMap(getPartialCount, ({ n = bump() }) =>
  Effect.map(getConfig, (config) => n + config.locale))

// MUST TRIGGER, NO FIX: `count` is mutable, so reading it after `that` runs may see a different value.
interface MutableCounter {
  count: number
}
declare const getCounter: Effect.Effect<MutableCounter>
declare const increment: Effect.Effect<void>
export const outerMutableProperty = Effect.flatMap(getCounter, ({ count }) => Effect.map(increment, () => count + 1))

// MUST TRIGGER, NO FIX: `value` is a get-only accessor (readonly to the checker); its read runs code and may
// see state that `that` changed, so it cannot move after `that`.
let getterCount = 0
class GetterCounter {
  get value(): number {
    return getterCount
  }
}
declare const getGetterCounter: Effect.Effect<GetterCounter>
declare const bumpGetter: Effect.Effect<number>
export const outerGetter = Effect.flatMap(getGetterCounter, ({ value }) => Effect.map(bumpGetter, (b) => value + b))

// MUST TRIGGER, NO FIX: a getter reached through a nested pattern.
declare const getGetterHolder: Effect.Effect<{ readonly inner: GetterCounter }>
export const outerNestedGetter = Effect.flatMap(getGetterHolder, ({ inner: { value } }) =>
  Effect.map(bumpGetter, (b) => value + b))

// MUST TRIGGER, NO FIX: destructuring a readonly property.
// The combiner would destructure after `that` runs instead of before; `readonly` does not make the object
// immutable (another reference can change it while `that` runs), so the binding cannot move.
export const destructuredReadonly = Effect.flatMap(getUser, ({ name }) => Effect.map(getConfig, (config) => name + config.locale))

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: already uses zipWith / zip.
export const alreadyZipWith = Effect.zipWith(getUser, getConfig, (user, config) => user.name + config.locale)
export const alreadyZip = getUser.pipe(Effect.zip(getConfig))

// MUST NOT TRIGGER (independence guard): `that` uses the outer parameter.
export const dependentCall = Effect.flatMap(getUser, (user) => Effect.map(loadProfile(user.id), (profile) => user.name + profile))

// MUST NOT TRIGGER (independence guard): `that` uses a binding destructured from the outer parameter.
export const dependentDestructured = Effect.flatMap(getUser, ({ id }) => Effect.map(loadProfile(id), (profile) => profile + id))

// MUST NOT TRIGGER (independence guard): `that` is a method of the acquired service (HttpBody.ts shape).
export const dependentService = Effect.flatMap(Database, (db) => Effect.map(db.load(1), (row) => row.length))

// MUST NOT TRIGGER: the outer callback has statements besides returning the inner map.
export const extraStatement = getUser.pipe(Effect.flatMap((user) => {
  console.log(user.name)
  return Effect.map(getConfig, (config) => user.name + config.locale)
}))

// MUST NOT TRIGGER: the outer callback returns conditionally.
export const conditionalReturn = getUser.pipe(
  Effect.flatMap((user) =>
    user.id > 0
      ? Effect.map(getConfig, (config) => user.name + config.locale)
      : Effect.succeed("anonymous")
  )
)

// MUST NOT TRIGGER: in v3 Option and Either are Effect subtypes, so these type-check, but `that`
// (or `self`) is not an Effect; the rule only fires when both are Effects.
export const thatIsOption = Effect.flatMap(getUser, (user) => Effect.map(Option.some(1), (n) => user.name + n))
export const thatIsEither = Effect.flatMap(getUser, (user) => Effect.map(Either.right(1), (n) => user.name + n))
export const selfIsOption = Effect.flatMap(Option.some(2), (m) => Effect.map(getConfig, (config) => config.locale + m))

// MUST NOT TRIGGER: service keys are out of scope; the rule only fires when both `self` and `that` are strict Effects.
class Counter extends Context.Tag("Counter")<Counter, { readonly q: Effect.Effect<number> }>() {}
// MUST NOT TRIGGER: self is a service key, not a strict Effect.
export const selfIsServiceKey = Effect.flatMap(Counter, (db) => Effect.map(Clock.currentTimeMillis, (t) => [db, t]))
// MUST NOT TRIGGER: that is a service key, not a strict Effect.
export const thatIsServiceKey = Effect.flatMap(getUser, (user) => Effect.map(Counter, (db) => [db, user]))

// MUST NOT TRIGGER: Option combinators with the same names.
export const optionNested = Option.flatMap(Option.some(1), (a) => Option.map(Option.some(2), (b) => a + b))

// MUST NOT TRIGGER: out-of-scope collapse (andThen outer).
export const outerAndThen = Effect.andThen(getUser, (user) => Effect.map(getConfig, (config) => user.name + config.locale))
