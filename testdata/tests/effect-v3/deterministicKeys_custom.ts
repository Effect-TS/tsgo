// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "extendedKeyDetection": true
      }
    ]
  }
}

// @filename: utils.ts
// @effect-v3
import * as Context from "effect/Context"

export function MyConstructor(/** @effect-identifier */ identifier: string) {
  return Context.Tag("hey/" + identifier)
}

export const Class = <
  Config extends {
    payload: Record<string, unknown>
  } = { payload: {} }
>() =>
<const Tag extends string>(/** @effect-identifier */ tag: Tag, _options: {
  readonly primaryKey: (payload: Config["payload"]) => string
}): new(
  args: Config["payload"]
) =>
  & { readonly _tag: Tag }
  & { readonly [K in keyof Config["payload"]]: Config["payload"][K] } =>
{
  function Persistable(this: any, props: any) {
    this._tag = tag
    if (props) {
      Object.assign(this, props)
    }
  }
  return Persistable as any
}

// @filename: test.ts
// @effect-v3
// @effect-diagnostics deterministicKeys:error
import * as Persistable from "./utils"
import * as Context from "effect/Context"

// simple case inside same file
export function LocalConstructor(/** @effect-identifier */ identifier: string) {
  return Context.Tag("hey/" + identifier)
}

export class MyClass extends LocalConstructor("Hello")<MyClass, {}>() {
}

// referenced on another file
export class TTLRequest extends Persistable.Class<{
  payload: { id: number }
}>()("TTLRequest", {
  primaryKey: (req) => `TTLRequest:${req.id}`
}) {}
