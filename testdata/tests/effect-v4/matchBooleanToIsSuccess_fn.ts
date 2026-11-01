import { Effect } from "effect"

declare const saveUser: Effect.Effect<void, Error>

// MUST TRIGGER: Effect.fn trailing pipe step -> Effect.isSuccess
export const trySave = Effect.fn(
  function*() {
    return yield* saveUser
  },
  Effect.match({
    onFailure: () => false,
    onSuccess: () => true
  })
)

// MUST TRIGGER: named Effect.fn trailing pipe step -> Effect.isSuccess
export const trySaveNamed = Effect.fn("trySaveNamed")(
  function*() {
    return yield* saveUser
  },
  Effect.match({
    onFailure: () => false,
    onSuccess: () => true
  })
)

// MUST TRIGGER: Effect.fnUntraced trailing pipe step with swapped literals -> Effect.isFailure
export const saveFailed = Effect.fnUntraced(
  function*() {
    return yield* saveUser
  },
  Effect.match({
    onFailure: () => true,
    onSuccess: () => false
  })
)

// MUST TRIGGER: data-first Effect.match inside an Effect.fn body -> Effect.isSuccess
export const checkSave = Effect.fn(function*() {
  return yield* Effect.match(saveUser, {
    onFailure: () => false,
    onSuccess: () => true
  })
})

// MUST NOT TRIGGER: Effect.matchCause trailing step converts defects too
export const trySaveCause = Effect.fn(
  function*() {
    return yield* saveUser
  },
  Effect.matchCause({
    onFailure: () => false,
    onSuccess: () => true
  })
)

// MUST NOT TRIGGER: trailing step handler reads its parameter
export const trySaveMessage = Effect.fn(
  function*() {
    return yield* saveUser
  },
  Effect.match({
    onFailure: (error) => error.message.length === 0,
    onSuccess: () => true
  })
)


// MUST TRIGGER: Effect.matchEager in a pipeable flow -> Effect.isSuccess
export const eagerPipeable = saveUser.pipe(Effect.matchEager({
  onFailure: () => false,
  onSuccess: () => true
}))

// MUST TRIGGER: Effect.matchEager data-last form applied directly -> Effect.isFailure
export const eagerDataLastApplied = Effect.matchEager({
  onFailure: () => true,
  onSuccess: () => false
})(saveUser)

// MUST TRIGGER: Effect.matchEager as an Effect.fn trailing pipe step -> Effect.isSuccess
export const trySaveEager = Effect.fn(
  function*() {
    return yield* saveUser
  },
  Effect.matchEager({
    onFailure: () => false,
    onSuccess: () => true
  })
)

// MUST NOT TRIGGER: Effect.matchCauseEager trailing step converts defects too
export const trySaveCauseEager = Effect.fn(
  function*() {
    return yield* saveUser
  },
  Effect.matchCauseEager({
    onFailure: () => false,
    onSuccess: () => true
  })
)
