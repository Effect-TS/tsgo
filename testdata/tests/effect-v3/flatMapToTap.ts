import { Effect, pipe } from "effect"
import * as E from "effect/Effect"
import { as as reYield, flatMap as bind, tap as observe } from "effect/Effect"
// @effect-v3
interface User {
  id: string
  readonly name: string
}

declare const source: Effect.Effect<User>
declare const audit: (user: User) => Effect.Effect<number, "audit", "service">
declare const other: User
declare const record: (value: unknown) => void

export const pipeableAs = source.pipe(Effect.flatMap((user) => audit(user).pipe(Effect.as(user))))
export const pipeMap = pipe(source, Effect.flatMap((user) => pipe(audit(user), Effect.map(() => user))))
export const dataFirstMap = Effect.flatMap(source, (user) => Effect.map(audit(user), () => user))
export const dataFirstAs = Effect.andThen(source, (user) => Effect.as(audit(user), user))
export const andThenPipe = source.pipe(Effect.andThen((user) => audit(user).pipe(Effect.map((unused) => user))))
export const curriedUnknown = Effect.flatMap((user) => Effect.as(Effect.log(user), user))(Effect.succeed<unknown>(other))
export const curriedAndThenUnknown = Effect.andThen((user) => Effect.as(Effect.log(user), user))(Effect.succeed<unknown>(other))
export const andThenFunctionPipe = pipe(source, Effect.andThen((user) => pipe(audit(user), Effect.as(user))))
export const directCurriedAs = source.pipe(Effect.flatMap((user) => Effect.as(user)(audit(user))))
export const directCurriedMap = source.pipe(Effect.flatMap((user) => Effect.map(() => user)(audit(user))))
export const singleReturn = source.pipe(Effect.flatMap(function(user) {
  return Effect.map(audit(user), (unused) => { return (user) })
}))
export const statements = source.pipe(Effect.andThen((user) => {
  const id = user.id
  record(id)
  const nested = () => { return id }
  record(nested())
  return audit(user).pipe(Effect.as(user))
}))
export const priorPipeSteps = source.pipe(Effect.flatMap((user) => audit(user).pipe(
  Effect.map((n) => n + 1),
  Effect.as(user)
)))
export const nestedPipeSteps = source.pipe(Effect.flatMap((user) => pipe(
  audit(user).pipe(Effect.map((n) => n + 1)),
  Effect.map((n) => n * 2),
  Effect.map(() => user)
)))
export const nestedDirectCalls = source.pipe(Effect.flatMap((user) => Effect.as(
  Effect.map(audit(user), (n) => n + 1),
  user
)))
export const comments = source.pipe(Effect.flatMap((user) => {
  // This audit records the identifier before publishing.
  record(user.id)
  return Effect.as(audit(/* original user */ user).pipe(Effect.map((n) => n + 1)), user)
}))
export const heapMutation = source.pipe(Effect.flatMap((user) => {
  user.id = "updated"
  return audit(user).pipe(Effect.as(user))
}))
export const namespaceAlias = source.pipe(E.flatMap((user) => E.as(audit(user), user)))
const Module = Effect
export const moduleAlias = source.pipe(Module.andThen((user) => Effect.as(audit(user), user)))
const flatMapAlias = Effect.flatMap
export const bareAlias = source.pipe(flatMapAlias((user) => audit(user).pipe(reYield(user))))
export const importedAlias = source.pipe(bind((user) => Effect.as(audit(user), user)))
export const annotatedDataFirst = Effect.flatMap(source, (user: User) => Effect.as(audit(user), user))
export const annotatedPipe = source.pipe(Effect.flatMap((user: User) => Effect.as(audit(user), user)))

export const reassigned = source.pipe(Effect.flatMap((user) => {
  user = other
  return audit(user).pipe(Effect.as(user))
}))
export const closureAssignment = source.pipe(Effect.flatMap((user) => {
  const update = () => { user = other }
  update()
  return audit(user).pipe(Effect.as(user))
}))
export const redeclared = source.pipe(Effect.flatMap(function(user) {
  var user: User
  return Effect.as(audit(user), user)
}))
export const shadowed = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), (user) => user)))
export const wrongSymbol = source.pipe(Effect.flatMap((user) => Effect.as(audit(user), other)))
export const propertyValue = source.pipe(Effect.flatMap((user) => Effect.as(audit(user), user.id)))
export const computedMap = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), () => ({ ...user }))))
export const impureMap = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), () => (record(user), user))))
export const multiStatementMapper = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), () => {
  record(user)
  return user
})))
export const defaultedMapper = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), (n = (record(user), 0)) => user)))
export const destructuredMapper = source.pipe(Effect.flatMap((user) => Effect.map(Effect.succeed(other), ({ id }) => user)))
export const restMapper = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), (...values) => user)))
export const asyncMapper = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), async () => user)))
export const generatorMapper = source.pipe(Effect.flatMap((user) => Effect.map(audit(user), function*() { return user })))
export const alternateReturns = source.pipe(Effect.flatMap((user) => {
  if (user.id === "skip") return Effect.as(audit(user), user)
  return Effect.as(audit(user), user)
}))
export const statementsAfterReturn = source.pipe(Effect.flatMap((user) => {
  return Effect.as(audit(user), user)
  record(user)
}))
export const destructuredOuter = source.pipe(Effect.flatMap(({ id }) => Effect.as(Effect.log(id), id)))
export const defaultedOuter = source.pipe(Effect.flatMap((user = other) => Effect.as(audit(user), user)))
export const restOuter = source.pipe(Effect.flatMap((...users) => Effect.as(Effect.log(users), users)))
export const annotatedReturn = source.pipe(Effect.flatMap((user): Effect.Effect<User, "audit", "service"> => Effect.as(audit(user), user)))
export const annotatedCurried = Effect.flatMap((user: User) => Effect.as(audit(user), user))(source)
export const explicitGenerics = Effect.flatMap<User, User, "audit", "service">((user) => Effect.as(audit(user), user))(source)
export const widenedSuccess = Effect.flatMap(source, (user: User | undefined) => Effect.as(Effect.log(user), user))
export const laterTransformation = source.pipe(Effect.flatMap((user) => audit(user).pipe(Effect.as(user), Effect.map((u) => u.name))))
export const errorFallback = source.pipe(Effect.flatMap((user) => audit(user).pipe(Effect.orElseSucceed(() => user))))
export const bareAndThenEffect = source.pipe(Effect.andThen(audit(other)))
export const alreadyTap = source.pipe(Effect.tap(audit))
const custom = { flatMap: Effect.flatMap }
export const propertyApiAlias = source.pipe(custom.flatMap((user) => Effect.as(audit(user), user)))
export const customApi = source.pipe(((Effect) => Effect.flatMap((user: User) => E.as(audit(user), user)))({
  flatMap: (callback: (user: User) => E.Effect<User, "audit", "service">) => (self: E.Effect<User>) => E.flatMap(self, callback)
}))
void observe
